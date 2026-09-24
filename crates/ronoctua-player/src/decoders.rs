//! The formats mpv's ffmpeg does not play, and who plays them instead.
//!
//! ffmpeg-full already reads trackers (libopenmpt, modplug), the game-music
//! emulators (gme: NSF, SPC, VGM, GBS, HES, KSS, AY), APE, WavPack, Musepack,
//! TTA, DSD and the rest. What is left of DeaDBeeF's list is here: each a
//! program that turns the file into sound on a pipe, which the engine hands
//! to mpv as a stream of its own (`ronoctua-decode://`).
//!
//! THE PROGRAM'S PATH COMES FROM THE ENVIRONMENT (`env`), set by
//! `nix/modules/player.nix` — a unit's PATH is minimal, and a name would be
//! found or not by accident.

use std::path::Path;

/// What a decoder writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    /// A WAV, header included, on stdout.
    WavOnStdout,
    /// Raw signed 16-bit stereo at 44.1 kHz on stdout; the engine puts the
    /// WAV header in front.
    RawOnStdout,
    /// A WAV written to a FILE — the engine gives it a FIFO.
    WavToFile,
    /// A WAV written to a REGULAR file, named `{out}` plus `.wav` by the
    /// decoder itself, which the engine reads as it grows: zxtune refuses to
    /// write into a FIFO.
    WavToGrowingFile,
}

/// One decoder.
#[derive(Debug, Clone, Copy)]
pub struct Decoder {
    /// Its id, and the tail of its text key.
    pub id: &'static str,
    /// The extensions it claims, lower case.
    pub extensions: &'static [&'static str],
    /// The environment variable holding its program's path.
    pub env: &'static str,
    /// What it writes.
    pub output: Output,
    /// Bytes the file must carry, at an offset, for the claim to hold — for an
    /// extension another program owns too. `None`: the extension is enough.
    pub mark: Option<(u64, &'static [u8])>,
    /// Whether the claim also holds for a name that puts the format FIRST —
    /// `mdat.Turrican`, `cust.Lotus` — which is how the Amiga's music files
    /// were named, and how they are still passed around.
    pub prefixed: bool,
    /// Its arguments; `{in}` is the file, `{out}` the FIFO when it writes to one,
    /// `{soundfont}` the MIDI soundfont.
    pub args: &'static [&'static str],
}

/// Every decoder, in the order they are asked.
pub const DECODERS: &[Decoder] = &[
    // THE COMMODORE 64. Three minutes when the tune does not say how long it
    // is, which is DeaDBeeF's own default: a SID loops for ever otherwise.
    Decoder {
        id: "sid",
        extensions: &["sid", "psid", "rsid", "mus"],
        env: "RONOCTUA_SIDPLAYFP",
        output: Output::WavToFile,
        mark: None,
        prefixed: false,
        args: &["-q", "-t3:00", "-w{out}", "{in}"],
    },
    // MIDI, through a General MIDI soundfont.
    Decoder {
        id: "midi",
        extensions: &["mid", "midi", "kar", "rmi", "smf"],
        env: "RONOCTUA_FLUIDSYNTH",
        // RAW, to stdout: libsndfile will not open a FIFO to write a WAV
        // ("Failed to open audio file"), and `-F -` is fluidsynth's stdout.
        output: Output::RawOnStdout,
        mark: None,
        prefixed: false,
        args: &[
            "-ni",
            "-q",
            "-r",
            "44100",
            "-T",
            "raw",
            "-O",
            "s16",
            "-F",
            "-",
            "{soundfont}",
            "{in}",
        ],
    },
    // GAME STREAMS: the formats consoles and engines stream music in.
    Decoder {
        id: "vgmstream",
        extensions: &[
            "adx", "ahx", "hca", "brstm", "bcstm", "bfstm", "bwav", "dsp", "idsp", "fsb", "xa",
            "str", "ss2", "vag", "genh", "txth", "awb", "acb", "bnk", "wem", "xwb", "xwav",
            "nus3bank", "lopus", "at3", "at9", "msf", "sgd", "rsd", "mib", "psw",
        ],
        env: "RONOCTUA_VGMSTREAM",
        output: Output::WavOnStdout,
        mark: None,
        prefixed: false,
        args: &["-p", "-l", "2", "{in}"],
    },
    // THE ATARI ST and the Amiga's sc68 files.
    Decoder {
        id: "sc68",
        // NOT `.snd`: that is also the Sun/NeXT audio ffmpeg plays, and a
        // claim here would take every one of them away from it.
        extensions: &["sc68", "sndh"],
        env: "RONOCTUA_SC68",
        output: Output::RawOnStdout,
        mark: None,
        prefixed: false,
        args: &["-c", "--rate=44100", "{in}"],
    },
    // THE SUPER AUDIO CD, from its disc image. An `.iso` is also a DVD, so
    // the claim holds only for an image whose sector 510 says `SACDMTOC`; a
    // DVD's goes on to mpv. The stereo area when there is one, as a WAV of
    // 88.2 kHz on stdout.
    Decoder {
        id: "sacd",
        extensions: &["iso"],
        env: "RONOCTUA_SACD",
        output: Output::WavOnStdout,
        mark: Some(SACD_MARK),
        prefixed: false,
        args: &["-i", "{in}", "-c", "-s"],
    },
    // THE AMIGA's own players: the formats every game and demo wrote its music
    // in, which only their original replay code plays right — uade runs that
    // code on an emulated Amiga. NOT `.mod`, `.med` or `.okt`: libopenmpt
    // plays those, and without an emulator.
    Decoder {
        id: "uade",
        extensions: &[
            "cust", "custom", "mdat", "tfx", "tfmx", "bp", "bp3", "sndmon", "fc", "fc13", "fc14",
            "smod", "hip", "hipc", "hip7", "dw", "dwold", "dm", "dm1", "dm2", "rh", "jam", "jc",
            "mc", "mcr", "ma", "mm4", "mm8", "ems", "ea", "aam", "dl", "sa", "sb", "pvp", "gmc",
            "sjs", "bss", "dh", "jpo", "jpn", "mug", "mug2", "tcb", "thx",
        ],
        env: "RONOCTUA_UADE",
        output: Output::WavOnStdout,
        mark: None,
        prefixed: true,
        args: &["-1", "-f", "-", "-e", "wav", "{in}"],
    },
    // THE CONSOLES' OWN RIPS (PlayStation 1 and 2, the N64, Game Boy
    // Advance, DS, Saturn) and the ZX Spectrum's chip music: zxtune. Not
    // `.dsf`, which is DSD audio ffmpeg plays; the Dreamcast's rips keep only
    // their `minidsf` spelling here. Not `.str` either, vgmstream's above.
    Decoder {
        id: "zxtune",
        extensions: &[
            "psf", "minipsf", "psf2", "minipsf2", "usf", "miniusf", "gsf", "minigsf", "2sf",
            "mini2sf", "ncsf", "minincsf", "ssf", "minissf", "minidsf", "psg", "vtx", "ym", "pt3",
            "pt2", "stc", "stp", "asc", "psc", "sqt", "ay", "ftc", "gtr", "chi", "dmm", "sqd",
            "tfc", "tfd", "tfe",
        ],
        env: "RONOCTUA_ZXTUNE",
        output: Output::WavToGrowingFile,
        mark: None,
        prefixed: false,
        args: &[
            "--silent",
            "--frequency",
            "44100",
            "--wav",
            "filename={out},overwrite=1",
            "{in}",
        ],
    },
    // ADLIB — the PC's FM chip, the music of its games in the eighties.
    Decoder {
        id: "adlib",
        extensions: &[
            "a2m", "a2t", "adl", "agd", "amd", "bam", "bmf", "cff", "cmf", "d00", "dfm", "dmo",
            "dro", "dtm", "got", "ha2", "hsc", "hsp", "hsq", "ims", "jbm", "ksm", "laa", "lds",
            "mad", "mdi", "mkj", "msc", "mtr", "pis", "plx", "rad", "rix", "rol", "sa2", "sat",
            "sci", "sdb", "sop", "sqx", "xad", "xms", "xsm",
        ],
        env: "RONOCTUA_ADPLAY",
        output: Output::WavOnStdout,
        mark: None,
        prefixed: false,
        // `-o` plays the tune ONCE: AdLib music loops for ever otherwise.
        args: &[
            "-O", "disk", "-d", "-", "-f", "44100", "--16bit", "--stereo", "-o", "-q", "{in}",
        ],
    },
];

/// The decoder a file needs, by its extension; `None` is mpv's own.
#[must_use]
pub fn for_path(path: &Path) -> Option<&'static Decoder> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    let first = path
        .file_name()
        .and_then(|n| n.to_str())
        .filter(|n| n.contains('.'))
        .and_then(|n| n.split('.').next())
        .map(str::to_ascii_lowercase);
    DECODERS
        .iter()
        .filter(|d| {
            d.extensions.contains(&ext.as_str())
                || (d.prefixed && first.as_deref().is_some_and(|f| d.extensions.contains(&f)))
        })
        .find(|d| d.mark.is_none_or(|(at, bytes)| carries(path, at, bytes)))
}

/// Whether `path` has `bytes` at offset `at`.
fn carries(path: &Path, at: u64, bytes: &[u8]) -> bool {
    use std::io::{Read as _, Seek as _};
    let Ok(mut f) = std::fs::File::open(path) else {
        return false;
    };
    let mut got = vec![0u8; bytes.len()];
    f.seek(std::io::SeekFrom::Start(at)).is_ok() && f.read_exact(&mut got).is_ok() && got == bytes
}

/// Where a Super Audio CD image says it is one: the master table of contents
/// is sector 510, and it opens with `SACDMTOC`.
pub const SACD_MARK: (u64, &[u8]) = (510 * 2048, b"SACDMTOC");

/// The protocol the engine registers with mpv for these.
pub const PROTOCOL: &str = "ronoctua-decode";

/// The URI mpv is given for `path`.
#[must_use]
pub fn uri_of(path: &Path) -> String {
    format!("{PROTOCOL}://{}", path.display())
}

/// The path a URI names, back.
#[must_use]
pub fn path_of(uri: &str) -> Option<&str> {
    uri.strip_prefix(PROTOCOL)?.strip_prefix("://")
}

/// Makes a streamed WAV's header say "until the stream ends".
///
/// A decoder writing a WAV to a pipe does not know the length and cannot seek
/// back to write it, so it leaves ZERO in both sizes — measured on
/// `sidplayfp -w<fifo>` and `adplay -O disk -d -`. The reader takes zero at its
/// word and calls the first packet corrupt. Only the two sizes change: the
/// format is the decoder's own (sidplayfp's is MONO), which is why this patches
/// their header rather than putting `wav_header` in its place.
pub fn endless(header: &mut [u8; 44]) {
    if &header[0..4] != b"RIFF" || &header[36..40] != b"data" {
        return;
    }
    if header[40..44] == [0, 0, 0, 0] {
        header[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        header[40..44].copy_from_slice(&(u32::MAX - 36).to_le_bytes());
    }
}

/// A WAV header for raw signed 16-bit stereo at 44.1 kHz of unknown length —
/// the sizes are the largest a WAV can say, which every reader takes as
/// "until the stream ends".
#[must_use]
pub fn wav_header() -> [u8; 44] {
    let rate: u32 = 44_100;
    let channels: u16 = 2;
    let bits: u16 = 16;
    let block = channels * bits / 8;
    let mut h = [0u8; 44];
    h[0..4].copy_from_slice(b"RIFF");
    h[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
    h[8..12].copy_from_slice(b"WAVE");
    h[12..16].copy_from_slice(b"fmt ");
    h[16..20].copy_from_slice(&16u32.to_le_bytes());
    h[20..22].copy_from_slice(&1u16.to_le_bytes());
    h[22..24].copy_from_slice(&channels.to_le_bytes());
    h[24..28].copy_from_slice(&rate.to_le_bytes());
    h[28..32].copy_from_slice(&(rate * u32::from(block)).to_le_bytes());
    h[32..34].copy_from_slice(&block.to_le_bytes());
    h[34..36].copy_from_slice(&bits.to_le_bytes());
    h[36..40].copy_from_slice(b"data");
    h[40..44].copy_from_slice(&(u32::MAX - 36).to_le_bytes());
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_format_goes_to_its_decoder_and_the_rest_to_mpv() {
        assert_eq!(
            for_path(Path::new("/m/Commando.SID")).map(|d| d.id),
            Some("sid")
        );
        assert_eq!(for_path(Path::new("song.mid")).map(|d| d.id), Some("midi"));
        assert_eq!(
            for_path(Path::new("x.brstm")).map(|d| d.id),
            Some("vgmstream")
        );
        assert!(for_path(Path::new("a.flac")).is_none());
        assert!(
            for_path(Path::new("a.mod")).is_none(),
            "ffmpeg's libopenmpt"
        );
    }

    #[test]
    fn no_extension_is_claimed_twice() {
        let mut seen = std::collections::BTreeSet::new();
        for d in DECODERS {
            for e in d.extensions {
                assert!(seen.insert(*e), "`{e}` is claimed twice");
            }
        }
    }

    #[test]
    fn a_streamed_header_is_made_endless_and_a_sized_one_is_left_alone() {
        // sidplayfp's, as it came out of a FIFO: mono, 44.1 kHz, data size 0.
        let mut theirs = wav_header();
        theirs[22..24].copy_from_slice(&1u16.to_le_bytes());
        theirs[4..8].copy_from_slice(&36u32.to_le_bytes());
        theirs[40..44].copy_from_slice(&0u32.to_le_bytes());
        endless(&mut theirs);
        assert_eq!(
            u32::from_le_bytes(theirs[40..44].try_into().unwrap()),
            u32::MAX - 36
        );
        assert_eq!(
            u16::from_le_bytes(theirs[22..24].try_into().unwrap()),
            1,
            "still mono"
        );

        let mut sized = wav_header();
        sized[40..44].copy_from_slice(&1000u32.to_le_bytes());
        let before = sized;
        endless(&mut sized);
        assert_eq!(sized, before);
    }

    #[test]
    fn an_iso_is_a_sacd_only_when_it_says_so() {
        let dir = std::env::temp_dir().join(format!("sacd-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a directory");
        let dvd = dir.join("film.iso");
        std::fs::write(&dvd, vec![0u8; 4096]).expect("written");
        assert!(for_path(&dvd).is_none(), "a DVD image goes to mpv");
        let disc = dir.join("album.iso");
        let mut bytes = vec![0u8; usize::try_from(SACD_MARK.0).expect("fits") + 16];
        let at = usize::try_from(SACD_MARK.0).expect("fits");
        bytes[at..at + 8].copy_from_slice(SACD_MARK.1);
        std::fs::write(&disc, bytes).expect("written");
        assert_eq!(for_path(&disc).map(|d| d.id), Some("sacd"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn an_amiga_name_puts_the_format_first_or_last() {
        assert_eq!(
            for_path(Path::new("/m/mdat.Turrican")).map(|d| d.id),
            Some("uade")
        );
        assert_eq!(
            for_path(Path::new("/m/Lotus.cust")).map(|d| d.id),
            Some("uade")
        );
        assert!(for_path(Path::new("/m/song.mod")).is_none(), "libopenmpt's");
        assert!(
            for_path(Path::new("/m/sid.notes.txt")).is_none(),
            "only the Amiga's decoder reads a name from the front"
        );
    }

    #[test]
    fn a_uri_gives_its_path_back() {
        let p = Path::new("/music/a b.sid");
        assert_eq!(path_of(&uri_of(p)), Some("/music/a b.sid"));
    }

    #[test]
    fn the_header_says_wav_and_forty_four_kilohertz_stereo() {
        let h = wav_header();
        assert_eq!(&h[0..4], b"RIFF");
        assert_eq!(u32::from_le_bytes(h[24..28].try_into().unwrap()), 44_100);
        assert_eq!(u16::from_le_bytes(h[22..24].try_into().unwrap()), 2);
    }
}
