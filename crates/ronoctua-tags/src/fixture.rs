//! Media files for the tests, made here, byte by byte.
//!
//! No binary fixture lives in the tree, and no test needs a program to have
//! one: each builder writes the smallest file lofty reads as its format —
//! real headers, a few frames of nothing. They are not music; they are
//! containers, which is what a tag is written into. The same tests run
//! again on files ffmpeg makes when `RONOCTUA_FFMPEG` is set.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// A folder of its own for one test, removed when dropped.
pub(crate) struct Dir {
    pub path: PathBuf,
}

impl Dir {
    pub(crate) fn new(name: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "ronoctua-tags-test-{}-{name}-{n}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("a temp folder");
        Self { path }
    }

    pub(crate) fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let p = self.path.join(name);
        std::fs::write(&p, bytes).expect("a fixture is written");
        p
    }

    /// Every name in it, sorted.
    pub(crate) fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(&self.path)
            .expect("the folder is there")
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// MPEG-1 Layer III, 128 kb/s, 44.1 kHz, joint stereo: ten frames of 417
/// bytes whose side information is all zero — silence to a decoder, a
/// frame sync to lofty.
pub(crate) fn mp3() -> Vec<u8> {
    let mut out = Vec::new();
    for _ in 0..10 {
        out.extend_from_slice(&[0xFF, 0xFB, 0x90, 0x64]);
        out.extend(std::iter::repeat_n(0_u8, 413));
    }
    out
}

/// `fLaC`, a STREAMINFO block (44.1 kHz, two channels, 16 bits), and a
/// frame's worth of bytes after it — a FLAC always has its sound after its
/// metadata, and a writer that moves the one must carry the other.
pub(crate) fn flac() -> Vec<u8> {
    let mut out = flac_without_frames();
    // A frame sync code and a few zero bytes: not decodable, but there.
    out.extend_from_slice(&[0xFF, 0xF8, 0x69, 0x08, 0x00, 0x00, 0x00, 0x00]);
    out
}

/// `fLaC` and a STREAMINFO block, and nothing after: the metadata ends the
/// file, which is where lofty's shrinking write goes wrong.
pub(crate) fn flac_without_frames() -> Vec<u8> {
    let mut out = b"fLaC".to_vec();
    // Last block, type 0, 34 bytes.
    out.extend_from_slice(&[0x80, 0x00, 0x00, 34]);
    out.extend_from_slice(&4096_u16.to_be_bytes());
    out.extend_from_slice(&4096_u16.to_be_bytes());
    out.extend_from_slice(&[0, 0, 0, 0, 0, 0]);
    // 20 bits of rate, 3 of channels - 1, 5 of bits - 1, 36 of samples.
    let packed: u64 = (44_100_u64 << 44) | (1 << 41) | (15 << 36);
    out.extend_from_slice(&packed.to_be_bytes());
    out.extend_from_slice(&[0_u8; 16]);
    out
}

/// A FLAC file whose second block claims more bytes than the file has.
pub(crate) fn broken_flac() -> Vec<u8> {
    let mut out = flac_without_frames();
    // STREAMINFO is no longer the last block…
    out[4] = 0x00;
    // …and a Vorbis comment block says it is a megabyte long.
    out.extend_from_slice(&[0x84, 0x10, 0x00, 0x00]);
    out.extend_from_slice(b"short");
    out
}

/// CRC-32 as Ogg computes it: polynomial 0x04C11DB7, no reflection, no
/// final xor.
fn ogg_crc(data: &[u8]) -> u32 {
    let mut crc: u32 = 0;
    for &b in data {
        crc ^= u32::from(b) << 24;
        for _ in 0..8 {
            crc = if crc & 0x8000_0000 != 0 {
                (crc << 1) ^ 0x04C1_1DB7
            } else {
                crc << 1
            };
        }
    }
    crc
}

/// One Ogg page holding whole packets.
fn ogg_page(kind: u8, granule: u64, sequence: u32, packets: &[&[u8]]) -> Vec<u8> {
    let mut lacing = Vec::new();
    let mut body = Vec::new();
    for p in packets {
        let mut left = p.len();
        loop {
            let seg = left.min(255);
            lacing.push(u8::try_from(seg).expect("a segment is a byte"));
            left -= seg;
            if seg < 255 {
                break;
            }
        }
        body.extend_from_slice(p);
    }
    let mut page = b"OggS".to_vec();
    page.push(0);
    page.push(kind);
    page.extend_from_slice(&granule.to_le_bytes());
    page.extend_from_slice(&0x5249_4F4E_u32.to_le_bytes());
    page.extend_from_slice(&sequence.to_le_bytes());
    page.extend_from_slice(&[0, 0, 0, 0]);
    page.push(u8::try_from(lacing.len()).expect("a few segments"));
    page.extend_from_slice(&lacing);
    page.extend_from_slice(&body);
    let crc = ogg_crc(&page);
    page[22..26].copy_from_slice(&crc.to_le_bytes());
    page
}

/// Ogg Opus: the identification header, an empty comment header, and one
/// page of a single silent packet.
pub(crate) fn opus() -> Vec<u8> {
    let mut head = b"OpusHead".to_vec();
    head.push(1);
    head.push(2);
    head.extend_from_slice(&312_u16.to_le_bytes());
    head.extend_from_slice(&48_000_u32.to_le_bytes());
    head.extend_from_slice(&0_i16.to_le_bytes());
    head.push(0);
    let mut tags = b"OpusTags".to_vec();
    tags.extend_from_slice(&7_u32.to_le_bytes());
    tags.extend_from_slice(b"fixture");
    tags.extend_from_slice(&0_u32.to_le_bytes());
    let mut out = ogg_page(0x02, 0, 0, &[&head]);
    out.extend(ogg_page(0x00, 0, 1, &[&tags]));
    out.extend(ogg_page(0x04, 960, 2, &[&[0xF8, 0xFF, 0xFE]]));
    out
}

/// Ogg Vorbis: the three headers (the setup header's body is not read by
/// lofty) and one page of a packet.
pub(crate) fn vorbis() -> Vec<u8> {
    let mut id = vec![0x01];
    id.extend_from_slice(b"vorbis");
    id.extend_from_slice(&0_u32.to_le_bytes());
    id.push(2);
    id.extend_from_slice(&44_100_u32.to_le_bytes());
    id.extend_from_slice(&0_i32.to_le_bytes());
    id.extend_from_slice(&128_000_i32.to_le_bytes());
    id.extend_from_slice(&0_i32.to_le_bytes());
    id.push(0xB8);
    id.push(1);
    let mut comment = vec![0x03];
    comment.extend_from_slice(b"vorbis");
    comment.extend_from_slice(&7_u32.to_le_bytes());
    comment.extend_from_slice(b"fixture");
    comment.extend_from_slice(&0_u32.to_le_bytes());
    comment.push(1);
    let mut setup = vec![0x05];
    setup.extend_from_slice(b"vorbis");
    setup.extend_from_slice(&[0_u8; 16]);
    let mut out = ogg_page(0x02, 0, 0, &[&id]);
    out.extend(ogg_page(0x00, 0, 1, &[&comment, &setup]));
    out.extend(ogg_page(0x04, 1024, 2, &[&[0_u8; 4]]));
    out
}

/// RIFF WAVE: 16-bit stereo at 44.1 kHz, a tenth of a second of silence.
pub(crate) fn wav() -> Vec<u8> {
    let data = vec![0_u8; 17_640];
    let mut fmt = Vec::new();
    fmt.extend_from_slice(&1_u16.to_le_bytes());
    fmt.extend_from_slice(&2_u16.to_le_bytes());
    fmt.extend_from_slice(&44_100_u32.to_le_bytes());
    fmt.extend_from_slice(&176_400_u32.to_le_bytes());
    fmt.extend_from_slice(&4_u16.to_le_bytes());
    fmt.extend_from_slice(&16_u16.to_le_bytes());
    let mut body = b"WAVE".to_vec();
    body.extend_from_slice(b"fmt ");
    body.extend_from_slice(&u32::try_from(fmt.len()).expect("small").to_le_bytes());
    body.extend_from_slice(&fmt);
    body.extend_from_slice(b"data");
    body.extend_from_slice(&u32::try_from(data.len()).expect("small").to_le_bytes());
    body.extend_from_slice(&data);
    let mut out = b"RIFF".to_vec();
    out.extend_from_slice(&u32::try_from(body.len()).expect("small").to_le_bytes());
    out.extend_from_slice(&body);
    out
}

/// What the M4A's one sample holds, so a test can find it after the
/// `moov` moved it.
pub(crate) const SAMPLE_MARK: &[u8] = b"RONOCTUA-SAMPLE-0001";

fn mp4_box(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut out = u32::try_from(body.len() + 8)
        .expect("small")
        .to_be_bytes()
        .to_vec();
    out.extend_from_slice(kind);
    out.extend_from_slice(body);
    out
}

fn mp4_full(kind: &[u8; 4], flags: u32, body: &[u8]) -> Vec<u8> {
    let mut full = flags.to_be_bytes().to_vec();
    full.extend_from_slice(body);
    mp4_box(kind, &full)
}

/// The identity matrix every `mvhd` and `tkhd` carries.
fn matrix() -> Vec<u8> {
    let mut m = Vec::new();
    for v in [0x0001_0000_u32, 0, 0, 0, 0x0001_0000, 0, 0, 0, 0x4000_0000] {
        m.extend_from_slice(&v.to_be_bytes());
    }
    m
}

/// An M4A laid out the way a "fast start" file is: `moov` BEFORE `mdat`, so
/// a growing `moov` pushes the sound along and every chunk offset has to
/// move with it. One track, one sample, one chunk, whose offset points at
/// [`SAMPLE_MARK`].
pub(crate) fn m4a() -> Vec<u8> {
    let mut ftyp = b"M4A ".to_vec();
    ftyp.extend_from_slice(&0_u32.to_be_bytes());
    for b in [b"M4A ", b"mp42", b"isom"] {
        ftyp.extend_from_slice(b);
    }
    let ftyp = mp4_box(b"ftyp", &ftyp);

    let moov_with = |offset: u32| -> Vec<u8> {
        let mut mvhd = Vec::new();
        for v in [0_u32, 0, 1000, 1000, 0x0001_0000] {
            mvhd.extend_from_slice(&v.to_be_bytes());
        }
        mvhd.extend_from_slice(&0x0100_u16.to_be_bytes());
        mvhd.extend_from_slice(&[0_u8; 10]);
        mvhd.extend(matrix());
        mvhd.extend_from_slice(&[0_u8; 24]);
        mvhd.extend_from_slice(&2_u32.to_be_bytes());
        let mvhd = mp4_full(b"mvhd", 0, &mvhd);

        let mut tkhd = Vec::new();
        for v in [0_u32, 0, 1, 0, 1000] {
            tkhd.extend_from_slice(&v.to_be_bytes());
        }
        tkhd.extend_from_slice(&[0_u8; 8]);
        tkhd.extend_from_slice(&[0, 0, 0, 0]);
        tkhd.extend_from_slice(&0x0100_u16.to_be_bytes());
        tkhd.extend_from_slice(&[0, 0]);
        tkhd.extend(matrix());
        tkhd.extend_from_slice(&[0_u8; 8]);
        let tkhd = mp4_full(b"tkhd", 7, &tkhd);

        let mut mdhd = Vec::new();
        for v in [0_u32, 0, 44_100, 1024] {
            mdhd.extend_from_slice(&v.to_be_bytes());
        }
        mdhd.extend_from_slice(&0x55C4_u16.to_be_bytes());
        mdhd.extend_from_slice(&[0, 0]);
        let mdhd = mp4_full(b"mdhd", 0, &mdhd);
        let mut hdlr = 0_u32.to_be_bytes().to_vec();
        hdlr.extend_from_slice(b"soun");
        hdlr.extend_from_slice(&[0_u8; 12]);
        hdlr.push(0);
        let hdlr = mp4_full(b"hdlr", 0, &hdlr);

        let smhd = mp4_full(b"smhd", 0, &[0, 0, 0, 0]);
        let mut dref = 1_u32.to_be_bytes().to_vec();
        dref.extend(mp4_full(b"url ", 1, &[]));
        let dinf = mp4_box(b"dinf", &mp4_full(b"dref", 0, &dref));

        let mut entry = vec![0_u8; 6];
        entry.extend_from_slice(&1_u16.to_be_bytes());
        entry.extend_from_slice(&[0_u8; 8]);
        entry.extend_from_slice(&2_u16.to_be_bytes());
        entry.extend_from_slice(&16_u16.to_be_bytes());
        entry.extend_from_slice(&[0_u8; 4]);
        entry.extend_from_slice(&(44_100_u32 << 16).to_be_bytes());
        let mut stsd = 1_u32.to_be_bytes().to_vec();
        stsd.extend(mp4_box(b"mp4a", &entry));
        let stsd = mp4_full(b"stsd", 0, &stsd);
        let words = |ws: &[u32]| ws.iter().flat_map(|w| w.to_be_bytes()).collect::<Vec<u8>>();
        let stts = mp4_full(b"stts", 0, &words(&[1, 1, 1024]));
        let stsc = mp4_full(b"stsc", 0, &words(&[1, 1, 1, 1]));
        let sample = u32::try_from(SAMPLE_MARK.len()).expect("small");
        let stsz = mp4_full(b"stsz", 0, &words(&[0, 1, sample]));
        let stco = mp4_full(b"stco", 0, &words(&[1, offset]));
        let stbl = mp4_box(b"stbl", &[stsd, stts, stsc, stsz, stco].concat());
        let minf = mp4_box(b"minf", &[smhd, dinf, stbl].concat());
        let mdia = mp4_box(b"mdia", &[mdhd, hdlr, minf].concat());
        let trak = mp4_box(b"trak", &[tkhd, mdia].concat());
        mp4_box(b"moov", &[mvhd, trak].concat())
    };
    let moov_len = moov_with(0).len();
    let offset = u32::try_from(ftyp.len() + moov_len + 8).expect("small");
    let mdat = mp4_box(b"mdat", SAMPLE_MARK);
    [ftyp, moov_with(offset), mdat].concat()
}

/// The first chunk offset of an MP4's first `stco`, found by walking boxes.
pub(crate) fn first_chunk_offset(bytes: &[u8]) -> Option<u64> {
    fn walk(bytes: &[u8]) -> Option<u64> {
        let mut at = 0;
        while at + 8 <= bytes.len() {
            let size = u32::from_be_bytes(bytes[at..at + 4].try_into().ok()?) as usize;
            let kind = &bytes[at + 4..at + 8];
            if size < 8 || at + size > bytes.len() {
                return None;
            }
            let body = &bytes[at + 8..at + size];
            match kind {
                b"moov" | b"trak" | b"mdia" | b"minf" | b"stbl" => {
                    if let Some(found) = walk(body) {
                        return Some(found);
                    }
                }
                b"stco" if body.len() >= 12 => {
                    return Some(u64::from(u32::from_be_bytes(body[8..12].try_into().ok()?)));
                }
                b"co64" if body.len() >= 16 => {
                    return Some(u64::from_be_bytes(body[8..16].try_into().ok()?));
                }
                _ => {}
            }
            at += size;
        }
        None
    }
    walk(bytes)
}

/// ffmpeg, when the environment names it — the tests that need a real
/// encoder say they were skipped otherwise.
pub(crate) fn ffmpeg() -> Option<PathBuf> {
    std::env::var_os("RONOCTUA_FFMPEG")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// Runs ffmpeg on a list of arguments and says whether it was happy.
pub(crate) fn ffmpeg_run(ffmpeg: &Path, args: &[&str]) -> (bool, String) {
    let out = std::process::Command::new(ffmpeg)
        .args(["-nostdin", "-v", "error", "-y"])
        .args(args)
        .output()
        .expect("ffmpeg starts");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// A short file made by ffmpeg: a sine for sound, a test pattern for a
/// picture.
pub(crate) fn made_by_ffmpeg(ffmpeg: &Path, dir: &Dir, name: &str, codec_args: &[&str]) -> PathBuf {
    let out = dir.path.join(name);
    let mut args = vec!["-f", "lavfi", "-i", "sine=frequency=440:duration=0.5"];
    args.extend_from_slice(codec_args);
    let target = out.to_string_lossy().into_owned();
    args.push(&target);
    let (ok, said) = ffmpeg_run(ffmpeg, &args);
    assert!(ok, "ffmpeg could not make {name}: {said}");
    out
}
