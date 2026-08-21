//! The media engine: yt-dlp, as a process, in its own process group.
//!
//! # What it downloads, and why the container is MKV
//!
//! "Best quality" on the sites that matter is a video track and an audio track
//! served separately, which have to be joined. MP4 cannot hold every pair —
//! VP9 or AV1 with Opus is exactly what YouTube serves at the top, and forcing
//! MP4 makes yt-dlp step DOWN to a pair that fits. Matroska holds all of them
//! with no re-encoding, so the file that lands is bit-for-bit what the site
//! served. What it costs is a few television sets that will not play it.
//!
//! # Pausing is real
//!
//! A subprocess cannot be asked to pause, so it is stopped: `SIGSTOP` to the
//! process GROUP — yt-dlp spawns ffmpeg, and stopping only the parent leaves
//! the child muxing. The socket stalls, the server times nothing out inside a
//! reasonable hold, and `SIGCONT` picks it up mid-stream. That is a truer pause
//! than killing and re-running, which would throw away the extraction and hit
//! the site again.
//!
//! Cancelling kills the group and removes the parts, which is the same promise
//! a cancelled copy makes: nothing half-written is left behind.

use crate::auth::{self, Cookies};
use crate::engine::{POLL, Progress, media_afoot, yt_dlp};
use crate::job::{Identity, Stage, Want, Work};
use crate::ledger::Shared;
use anyhow::{Context as _, Result, bail};
use ronoctua_files::watch::{Control, Flow};
use std::io::{BufRead as _, BufReader};
use std::path::Path;
use std::sync::Arc;

/// The marker that tells our progress lines apart from everything else yt-dlp
/// prints. A word and not a symbol: it survives a terminal, a log and a pipe.
const MARK: &str = "RONOCTUA";

/// One reading off yt-dlp's progress template.
#[derive(Debug, PartialEq, Eq)]
struct Beat {
    done: u64,
    total: u64,
    name: String,
}

/// yt-dlp writes `NA` for a field it does not have yet.
fn count(field: &str) -> u64 {
    field.trim().parse().unwrap_or(0)
}

/// Reads one progress line, or decides it is not one.
///
/// A pure function on purpose: the only way to test this against the real
/// output is to have kept some, and a parser that needs the network to be
/// exercised is a parser nobody exercises.
fn beat_of(line: &str) -> Option<Beat> {
    let rest = line.trim().strip_prefix(MARK)?.strip_prefix('\t')?;
    let mut fields = rest.split('\t');
    let done = count(fields.next()?);
    let total = count(fields.next()?);
    // `total_bytes` is exact and `total_bytes_estimate` is what a fragmented
    // stream has instead. Taking the estimate only when the exact one is
    // missing keeps a bar from jumping backwards when the exact one arrives.
    let estimate = count(fields.next().unwrap_or(""));
    let name = fields.next().unwrap_or("").trim();
    let name = Path::new(name)
        .file_name()
        .map_or_else(|| name.to_owned(), |n| n.to_string_lossy().into_owned());
    Some(Beat {
        done,
        total: if total > 0 { total } else { estimate },
        name,
    })
}

/// The command line, built from the request.
///
/// A function of its own, and tested: what these flags select is the whole
/// difference between "the best quality on offer" and "whatever was easiest to
/// serve", and it is not something to discover by watching a file arrive.
fn arguments(
    url: &str,
    into: &Path,
    want: Want,
    identity: Identity,
    cookies: &Cookies,
) -> Vec<String> {
    let mut out: Vec<String> = vec![
        // Machine-readable progress on its own lines. `--newline` is what stops
        // yt-dlp rewriting one line with carriage returns, which no line reader
        // can split.
        "--newline".to_owned(),
        "--no-colors".to_owned(),
        "--progress".to_owned(),
        "--progress-template".to_owned(),
        format!(
            "download:{MARK}\t%(progress.downloaded_bytes)s\t%(progress.total_bytes)s\t\
             %(progress.total_bytes_estimate)s\t%(progress.filename)s"
        ),
        // THE PAGE ONE IS LOOKING AT, and not the playlist it belongs to. A
        // menu item that says "download this video" and starts two hundred is
        // the surprise this flag exists to avoid.
        "--no-playlist".to_owned(),
        "-P".to_owned(),
        into.to_string_lossy().into_owned(),
    ];
    match want {
        Want::Video => out.extend([
            "-f".to_owned(),
            // Best video plus best audio, falling back to the best single
            // stream when a site serves only those.
            "bv*+ba/b".to_owned(),
            "--merge-output-format".to_owned(),
            "mkv".to_owned(),
        ]),
        Want::Audio => out.extend([
            "-f".to_owned(),
            "ba/b".to_owned(),
            "-x".to_owned(),
            "--audio-format".to_owned(),
            "opus".to_owned(),
        ]),
        Want::Subtitles => out.extend([
            // No media at all: the subtitles are the whole request.
            "--skip-download".to_owned(),
            "--write-subs".to_owned(),
            // The automatic ones too — for most videos they are the only ones
            // there are, and an empty download would be the alternative.
            "--write-auto-subs".to_owned(),
            "--sub-langs".to_owned(),
            "pt.*,en.*".to_owned(),
            "--convert-subs".to_owned(),
            "srt".to_owned(),
        ]),
    }
    // WHO IS ASKING, and only when the request said so. Empty for the default,
    // which is anonymous — see `crate::auth`.
    out.extend(auth::yt_dlp_flags(identity, cookies));

    // END OF OPTIONS, and then the address. A URL cannot begin with a dash,
    // but this function is handed whatever the browser sent, and one argument
    // that got read as a flag is a class of bug worth one token.
    out.push("--".to_owned());
    out.push(url.to_owned());
    out
}

/// One media request, whole.
///
/// A struct and not five more parameters: three of them are small enums and
/// two are borrowed, which is a call whose arguments can be reordered without
/// the compiler noticing — and one of those arguments decides whether the
/// request carries who you are.
struct Asked<'a> {
    url: &'a str,
    into: &'a Path,
    want: Want,
    identity: Identity,
    cookies: &'a Cookies,
}

/// Signals the whole group, and says nothing when there is no group left.
fn signal(pid: u32, sig: rustix::process::Signal) {
    let Ok(raw) = i32::try_from(pid) else {
        return;
    };
    let Some(pid) = rustix::process::Pid::from_raw(raw) else {
        return;
    };
    // The GROUP, not the process: yt-dlp spawns ffmpeg to join the two tracks,
    // and stopping only the parent leaves the child running.
    let _ = rustix::process::kill_process_group(pid, sig);
}

/// Runs one media request to the end, or to whatever the switch says.
pub fn run(shared: &Arc<Shared>, id: u64, switch: &Arc<Control>, work: Work) {
    let Work::Media {
        url,
        into,
        want,
        identity,
    } = &work
    else {
        tracing::error!("o motor de midia recebeu trabalho que nao e midia");
        shared.touch(id, |j| j.stage = Stage::Failed);
        return;
    };
    // THE REDUCED PROFILE HAS NO yt-dlp, and a row that was accepted there
    // would sit at nothing per cent for ever. Refusing at the door with a
    // sentence is the honest answer.
    if !media_afoot() {
        shared.touch(id, |j| {
            j.stage = Stage::Failed;
            j.trouble = Some("esta instalacao nao tem o yt-dlp".to_owned());
        });
        return;
    }
    // WHOEVER ASKED TO BE THEMSELVES AND CANNOT BE gets a sentence, not a
    // quiet anonymous download: that one fetches the trailer instead of the
    // video, or nothing at all, and blames the site.
    let cookies = Cookies::here();
    if let Some(why) = auth::refusal(*identity, &cookies) {
        shared.touch(id, |j| {
            j.stage = Stage::Failed;
            j.trouble = Some(why.to_owned());
        });
        return;
    }
    let asked = Asked {
        url,
        into,
        want: *want,
        identity: *identity,
        cookies: &cookies,
    };
    match carry(shared, id, switch, &asked) {
        Ok(()) => {}
        Err(e) => {
            let cancelled = switch.flow() == Flow::Stop;
            let text = format!("{e:#}");
            shared.touch(id, |j| {
                j.stage = if cancelled {
                    Stage::Cancelled
                } else {
                    Stage::Failed
                };
                if !cancelled {
                    j.trouble = Some(text);
                }
            });
        }
    }
}

fn carry(shared: &Arc<Shared>, id: u64, switch: &Arc<Control>, asked: &Asked<'_>) -> Result<()> {
    let into = asked.into;
    let program = yt_dlp();
    let mut command = std::process::Command::new(&program);
    command
        .args(arguments(
            asked.url,
            into,
            asked.want,
            asked.identity,
            asked.cookies,
        ))
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    // ITS OWN GROUP, which is what makes one signal reach yt-dlp and ffmpeg
    // together — and what stops a signal meant for it reaching this daemon.
    std::os::unix::process::CommandExt::process_group(&mut command, 0);

    // BEFORE THE SPAWN: what is already half-written here belongs to somebody
    // else, and a cancel must not touch it.
    let before = parts_now(into);

    let mut child = command
        .spawn()
        .with_context(|| format!("nao consegui rodar {program}"))?;
    let pid = child.id();
    shared.touch(id, |j| j.stage = Stage::Running);

    // TWO READER THREADS AND A CHANNEL, because reading a line BLOCKS and the
    // switch has to stay answerable. The alternative was a non-blocking read
    // and a hand-rolled line buffer, which is the same thing with a bug in it.
    let (lines, arriving) = std::sync::mpsc::channel::<Line>();
    reader(child.stdout.take(), lines.clone(), false);
    reader(child.stderr.take(), lines, true);

    let mut progress = Progress::new(shared, id);
    let mut held = false;
    // The LAST thing stderr said, which is what yt-dlp puts a failure in.
    let mut last_word = String::new();

    loop {
        match switch.flow() {
            Flow::Stop => {
                signal(pid, rustix::process::Signal::CONT);
                signal(pid, rustix::process::Signal::KILL);
                let _ = child.wait();
                progress.flush();
                sweep_parts(into, &before);
                bail!("desistido");
            }
            Flow::Pause if !held => {
                signal(pid, rustix::process::Signal::STOP);
                held = true;
                progress.stage(Stage::Paused);
            }
            Flow::Run if held => {
                signal(pid, rustix::process::Signal::CONT);
                held = false;
                progress.stage(Stage::Running);
            }
            Flow::Pause | Flow::Run => {}
        }

        while let Ok(line) = arriving.try_recv() {
            if line.stderr {
                if !line.text.trim().is_empty() {
                    last_word = line.text.trim().to_owned();
                }
            } else if let Some(beat) = beat_of(&line.text) {
                progress.set(beat.done, beat.total, &beat.name);
            }
        }

        // A HELD PROCESS NEVER EXITS, so it must not be waited on: `try_wait`
        // is right either way, but the sleep below is what keeps a paused job
        // from spinning this thread at full speed.
        if !held && let Some(status) = child.try_wait()? {
            // Drain whatever the readers still had.
            while let Ok(line) = arriving.try_recv() {
                if line.stderr {
                    if !line.text.trim().is_empty() {
                        last_word = line.text.trim().to_owned();
                    }
                } else if let Some(beat) = beat_of(&line.text) {
                    progress.set(beat.done, beat.total, &beat.name);
                }
            }
            progress.flush();
            if status.success() {
                shared.touch(id, |j| {
                    j.stage = Stage::Done;
                    // A finished download is a hundred per cent even when the
                    // last beat said otherwise: the estimate was an estimate.
                    if j.total == 0 || j.done > j.total {
                        j.total = j.done;
                    } else {
                        j.done = j.total;
                    }
                });
                return Ok(());
            }
            if last_word.is_empty() {
                bail!("o yt-dlp saiu com {status}");
            }
            bail!("{last_word}");
        }

        std::thread::sleep(POLL);
    }
}

/// The half-written files that are in the landing directory right now.
///
/// Taken BEFORE the download starts, so that what a cancel removes is the
/// difference and never the whole set. Two media jobs into one directory is
/// ordinary — a playlist, or simply two videos — and a sweep by suffix alone
/// would make cancelling one of them delete the other's work. The bug is
/// silent: the surviving job carries on and fails later, at a hash check, with
/// a message about the file it no longer has.
fn parts_now(into: &Path) -> std::collections::HashSet<std::path::PathBuf> {
    let mut found = std::collections::HashSet::new();
    let Ok(entries) = std::fs::read_dir(into) else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| matches!(e, "part" | "ytdl"))
        {
            found.insert(path);
        }
    }
    found
}

/// Removes what THIS download left half-written, and nothing else.
///
/// Only the two suffixes yt-dlp uses, only in the directory the job was told to
/// land in, and only what was not already there when it started.
fn sweep_parts(into: &Path, before: &std::collections::HashSet<std::path::PathBuf>) {
    for path in parts_now(into).difference(before) {
        let _ = std::fs::remove_file(path);
    }
}

/// One line, and which stream it came from.
struct Line {
    text: String,
    stderr: bool,
}

/// Pumps a stream into the channel until it ends.
fn reader<R: std::io::Read + Send + 'static>(
    from: Option<R>,
    into: std::sync::mpsc::Sender<Line>,
    stderr: bool,
) {
    let Some(from) = from else {
        return;
    };
    let built = std::thread::Builder::new()
        .name(if stderr { "yt-dlp-err" } else { "yt-dlp-out" }.into())
        .spawn(move || {
            for line in BufReader::new(from).lines().map_while(Result::ok) {
                // The receiver going away means the job is over; there is
                // nothing useful to log about it.
                if into.send(Line { text: line, stderr }).is_err() {
                    return;
                }
            }
        });
    if let Err(e) = built {
        tracing::warn!(error = %e, "nao consegui subir a thread de leitura do yt-dlp");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_progress_line_is_read_and_anything_else_is_ignored() {
        let line = "RONOCTUA\t1048576\t4194304\t4194304\t/home/x/Vídeos/coisa.mkv.part";
        assert_eq!(
            beat_of(line),
            Some(Beat {
                done: 1_048_576,
                total: 4_194_304,
                name: "coisa.mkv.part".to_owned(),
            })
        );
        // Everything else yt-dlp prints — and it prints a lot — must not be
        // mistaken for progress.
        assert_eq!(beat_of("[youtube] Extracting URL: https://…"), None);
        assert_eq!(beat_of("[Merger] Merging formats into \"a.mkv\""), None);
        assert_eq!(beat_of(""), None);
    }

    #[test]
    fn na_is_not_a_number_and_reads_as_not_known() {
        // yt-dlp writes `NA` until it knows, and a fragmented stream never
        // learns the exact total. Zero is what the row means by "not known".
        let line = "RONOCTUA\t512\tNA\t2048\tvideo.mkv";
        let beat = beat_of(line).expect("é uma linha de progresso");
        assert_eq!(beat.done, 512);
        // The estimate stands in for the exact one, and only then.
        assert_eq!(beat.total, 2048);

        let neither = beat_of("RONOCTUA\t512\tNA\tNA\tvideo.mkv").expect("ainda é uma");
        assert_eq!(neither.total, 0);
    }

    #[test]
    fn the_exact_total_beats_the_estimate() {
        // Otherwise the bar jumps the moment the exact figure arrives.
        let beat = beat_of("RONOCTUA\t10\t100\t97\tv.mkv").expect("linha");
        assert_eq!(beat.total, 100);
    }

    #[test]
    fn the_three_menu_items_ask_for_three_different_things() {
        let into = Path::new("/onde/vai");

        let video = arguments(
            "https://exemplo/v",
            into,
            Want::Video,
            Identity::Anonymous,
            &Cookies::Nowhere,
        );
        assert!(video.windows(2).any(|w| w == ["-f", "bv*+ba/b"]));
        assert!(
            video
                .windows(2)
                .any(|w| w == ["--merge-output-format", "mkv"]),
            "o contêiner é MKV, senão o yt-dlp desce de qualidade para caber"
        );

        let audio = arguments(
            "https://exemplo/v",
            into,
            Want::Audio,
            Identity::Anonymous,
            &Cookies::Nowhere,
        );
        assert!(audio.iter().any(|a| a == "-x"));
        assert!(audio.windows(2).any(|w| w == ["--audio-format", "opus"]));
        assert!(!audio.iter().any(|a| a == "--merge-output-format"));

        let subs = arguments(
            "https://exemplo/v",
            into,
            Want::Subtitles,
            Identity::Anonymous,
            &Cookies::Nowhere,
        );
        assert!(
            subs.iter().any(|a| a == "--skip-download"),
            "legenda não baixa mídia nenhuma"
        );
        assert!(subs.iter().any(|a| a == "--write-auto-subs"));

        // And all three: the page one is looking at, never its playlist.
        for set in [&video, &audio, &subs] {
            assert!(set.iter().any(|a| a == "--no-playlist"));
            assert_eq!(set.last().map(String::as_str), Some("https://exemplo/v"));
            assert_eq!(set.get(set.len() - 2).map(String::as_str), Some("--"));
        }
    }

    #[test]
    fn a_cancel_removes_only_what_this_download_left() {
        // Two media jobs into one directory is ordinary — a playlist, or two
        // videos. Sweeping by suffix alone made cancelling one delete the
        // other's work, and the survivor failed later at a hash check with a
        // message about a file it no longer had.
        // `CARGO_TARGET_DIR` at RUN time and not `env!` at compile time, and
        // never `temp_dir`: on this machine `/tmp` is a RAM filesystem, and
        // this crate's `transfers-socket-is-private` refuses the call outright.
        // The build directory is where cargo already writes, so it is a real
        // disk by construction — and `.` is the build directory anyway.
        let dir = std::env::var_os("CARGO_TARGET_DIR")
            .map_or_else(
                || std::path::PathBuf::from("target"),
                std::path::PathBuf::from,
            )
            .join("sweep-parts");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("criar o diretorio");
        let theirs = dir.join("de-outro.mkv.part");
        std::fs::write(&theirs, b"x").expect("escrever");

        let before = parts_now(&dir);
        let mine = dir.join("meu.mkv.part");
        std::fs::write(&mine, b"y").expect("escrever");
        std::fs::write(dir.join("meu.mkv.ytdl"), b"z").expect("escrever");
        // And something that is not a partial file at all.
        let done = dir.join("pronto.mkv");
        std::fs::write(&done, b"w").expect("escrever");

        sweep_parts(&dir, &before);

        assert!(theirs.exists(), "o pedaco do outro trabalho foi apagado");
        assert!(done.exists(), "um arquivo pronto foi apagado");
        assert!(!mine.exists(), "o proprio pedaco ficou para tras");
        assert!(!dir.join("meu.mkv.ytdl").exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_url_is_the_last_argument_and_never_looks_like_a_flag() {
        // A URL cannot start with a dash, but a caller could hand one over. The
        // separator is what keeps it an argument.
        let args = arguments(
            "-nao-sou-flag",
            Path::new("/onde/vai"),
            Want::Video,
            Identity::Anonymous,
            &Cookies::Nowhere,
        );
        assert_eq!(args.last().map(String::as_str), Some("-nao-sou-flag"));
        assert_eq!(
            args.get(args.len() - 2).map(String::as_str),
            Some("--"),
            "sem o fim-de-opcoes, um endereco com traco vira flag"
        );
    }
}
