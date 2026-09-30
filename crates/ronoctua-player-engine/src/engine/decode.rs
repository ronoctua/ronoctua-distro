//! The decoders' output as a stream mpv reads: what plays the formats only
//! our decoders open (`ronoctua_player::decoders`) — a WAV, mended, from a
//! pipe, a FIFO or a file the decoder is still writing, and a seek that
//! decodes again.

use std::{
    io::Read,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use anyhow::{Context as _, Result};

use ronoctua_player::decoders;

use crate::mpv;

/// Starts `decoder` on `path` and answers what it writes, WAV-shaped.
pub fn decode(decoder: &'static decoders::Decoder, path: &Path) -> Result<Box<dyn mpv::Stream>> {
    let (reader, child) = start(decoder, path)?;
    Ok(Box::new(Decoded {
        decoder,
        path: path.to_path_buf(),
        reader,
        child,
        at: 0,
        beyond: false,
    }))
}

/// Starts `decoder` on `path`: the WAV it writes, header mended, and the
/// process when the reader does not own it already.
fn start(
    decoder: &decoders::Decoder,
    path: &Path,
) -> Result<(Box<dyn Read + Send>, Option<std::process::Child>)> {
    let program =
        std::env::var(decoder.env).with_context(|| format!("{} is not set", decoder.env))?;
    let soundfont = std::env::var("RONOCTUA_SOUNDFONT").unwrap_or_default();
    // A FIFO for the decoders that only write to a file. It lives in the
    // runtime directory, named after this process and a counter.
    let fifo = match decoder.output {
        decoders::Output::WavToFile => {
            static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let at = ronoctua_xdg::runtime_file(&format!("player-{}-{n}.wav", std::process::id()))
                .context("no runtime directory")?;
            let _ = std::fs::remove_file(&at);
            rustix::fs::mknodat(
                rustix::fs::CWD,
                &at,
                rustix::fs::FileType::Fifo,
                rustix::fs::Mode::from_raw_mode(0o600),
                0,
            )?;
            Some(at)
        }
        // A REGULAR FILE for the decoders that will not write into a FIFO —
        // zxtune checks what it creates and refuses one. In the CACHE and not
        // the runtime directory: a song is tens of megabytes as WAV, and the
        // runtime directory is memory. The decoder is given the name without
        // `.wav`, which it adds itself.
        decoders::Output::WavToGrowingFile => {
            static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let dir = ronoctua_xdg::cache_dir().context("no cache directory")?;
            std::fs::create_dir_all(&dir)?;
            let stem = dir.join(format!("player-{}-{n}", std::process::id()));
            let _ = std::fs::remove_file(stem.with_extension("wav"));
            Some(stem)
        }
        _ => None,
    };
    let args: Vec<String> = decoder
        .args
        .iter()
        .map(|a| {
            a.replace("{in}", &path.to_string_lossy())
                .replace(
                    "{out}",
                    &fifo
                        .as_ref()
                        .map(|f| f.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                )
                .replace("{soundfont}", &soundfont)
        })
        .collect();
    let child = std::process::Command::new(&program)
        .args(&args)
        .stdin(std::process::Stdio::null())
        .stdout(if fifo.is_some() {
            std::process::Stdio::null()
        } else {
            std::process::Stdio::piped()
        })
        .stderr(std::process::Stdio::null())
        .spawn()
        .with_context(|| format!("running {program}"))?;
    let mut child = Some(child);
    let mut wav: Box<dyn Read + Send> = match (decoder.output, fifo) {
        (decoders::Output::WavToGrowingFile, Some(stem)) => Box::new(Growing::open(
            stem.with_extension("wav"),
            child.take().context("no decoder")?,
        )?),
        (decoders::Output::WavToFile, Some(at)) => {
            let file = std::fs::File::open(&at)?;
            let _ = std::fs::remove_file(&at);
            Box::new(file)
        }
        (decoders::Output::RawOnStdout, _) => {
            let out = child
                .as_mut()
                .and_then(|c| c.stdout.take())
                .context("no stdout")?;
            Box::new(std::io::Cursor::new(decoders::wav_header().to_vec()).chain(out))
        }
        _ => Box::new(
            child
                .as_mut()
                .and_then(|c| c.stdout.take())
                .context("no stdout")?,
        ),
    };
    // Every one is a WAV by now, and a streamed one says its data is empty.
    let mut header = [0u8; 44];
    wav.read_exact(&mut header)
        .context("the decoder wrote no header")?;
    decoders::endless(&mut header);
    let reader: Box<dyn Read + Send> = Box::new(std::io::Cursor::new(header.to_vec()).chain(wav));
    Ok((reader, child))
}

/// Removes what an earlier daemon's decoders left in the cache. A daemon that
/// is killed never runs the `Drop` that removes its file, and a song as WAV is
/// tens of megabytes; this runs once, before anything plays, when no file of
/// this process exists yet.
pub fn forget_old_decodes() {
    let Some(dir) = ronoctua_xdg::cache_dir() else {
        return;
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let name = e.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("player-") {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

/// A file a decoder is still writing, read as it grows: at its end the reader
/// waits for more while the decoder lives, and answers the end only once it
/// has exited and nothing more came. It owns the decoder and the file, and
/// both go with it.
struct Growing {
    file: std::fs::File,
    path: PathBuf,
    child: std::process::Child,
}

/// How long a reader waits at the end of a growing file before looking again.
const GROWING_WAIT: Duration = Duration::from_millis(20);

/// How long the decoder is given to create its file at all.
const GROWING_BIRTH: Duration = Duration::from_secs(10);

impl Growing {
    fn open(path: PathBuf, mut child: std::process::Child) -> Result<Self> {
        let born = Instant::now();
        loop {
            if let Ok(file) = std::fs::File::open(&path) {
                return Ok(Self { file, path, child });
            }
            if child.try_wait().ok().flatten().is_some() || born.elapsed() > GROWING_BIRTH {
                let _ = child.kill();
                let _ = child.wait();
                anyhow::bail!("the decoder wrote no file");
            }
            std::thread::sleep(GROWING_WAIT);
        }
    }
}

impl Read for Growing {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        loop {
            let n = self.file.read(buf)?;
            if n > 0 {
                return Ok(n);
            }
            if self.child.try_wait()?.is_some() {
                // EXITED: whatever it wrote after the last read is still owed.
                return self.file.read(buf);
            }
            std::thread::sleep(GROWING_WAIT);
        }
    }
}

impl Drop for Growing {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(&self.path);
    }
}

/// A decoder's output, which reaps the decoder when mpv closes it.
///
/// # It SEEKS, and that is what makes a position row and a follower work
///
/// A pipe cannot go back, and a stream with no seek is one mpv can neither
/// jump in nor START at: a machine following another played a SID six
/// seconds behind for ever. Forward, the bytes up to the place are decoded
/// and thrown away — every decoder here is many times faster than playing;
/// backward, the decoder starts again from the beginning and does the same.
/// Its output is the same bytes every time, so a byte offset means one moment.
struct Decoded {
    decoder: &'static decoders::Decoder,
    path: PathBuf,
    reader: Box<dyn Read + Send>,
    child: Option<std::process::Child>,
    /// The byte the next read gives.
    at: u64,
    /// Sent past the claimed end by a probe: reads answer nothing.
    beyond: bool,
}

impl Read for Decoded {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.beyond {
            return Ok(0);
        }
        let n = self.reader.read(buf)?;
        self.at += n as u64;
        Ok(n)
    }
}

/// Where the endless WAV header's claimed data ends (`decoders::endless`).
const PAST_THE_CLAIM: u64 = u32::MAX as u64 - 36;

impl mpv::Stream for Decoded {
    fn seek_to(&mut self, to: u64) -> std::io::Result<u64> {
        // PAST THE END THE HEADER CLAIMS is a probe, not a place: the WAV
        // says "until the stream ends" with the largest size it can hold, and
        // the demuxer looks for a chunk after it — measured, a seek to byte
        // 4 294 967 304 on open. Decoding up to there played nothing for the
        // length of the whole song; REFUSING it made mpv take the stream for
        // ended. So the stream goes there without decoding anything, answers
        // nothing from there, and the seek back that follows starts the
        // decoder again — one restart per file, at open.
        if to >= PAST_THE_CLAIM {
            self.beyond = true;
            self.at = to;
            return Ok(to);
        }
        self.beyond = false;
        if to < self.at {
            let (reader, child) = start(self.decoder, &self.path).map_err(std::io::Error::other)?;
            if let Some(mut old) = std::mem::replace(&mut self.child, child) {
                let _ = old.kill();
                let _ = old.wait();
            }
            self.reader = reader;
            self.at = 0;
        }
        let skipped = std::io::copy(
            &mut (&mut self.reader).take(to - self.at),
            &mut std::io::sink(),
        )?;
        self.at += skipped;
        Ok(self.at)
    }
}

impl Drop for Decoded {
    fn drop(&mut self) {
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}
