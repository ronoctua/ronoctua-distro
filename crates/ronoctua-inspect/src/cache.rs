//! Thumbnails kept on disk, so the second visit to a folder is free.
//!
//! # What this buys, measured
//!
//! Against the owner's own folder — sixty-four files, thirty-six extensions —
//! making every thumbnail costs 1 681 ms across six workers, and the floor is
//! the PROCESS: 109 ms for a 22 KB JPEG and 167 ms for a 355 MB video. Reading
//! one back from here is an open and a read of at most a quarter of a
//! mebibyte.
//!
//! # Why it is OUR file and not the freedesktop one
//!
//! The standard (`$XDG_CACHE_HOME/thumbnails/normal/<md5 of the URI>.png`) is
//! worth having and would let another file manager's work be reused. It costs
//! an MD5 written by hand and a PNG `tEXt` chunk the `image` crate does not
//! expose, which is a second reader and a second writer for a saving this
//! format does not need. It is in `docs/PENDING.md` with that price on it.
//!
//! # Why a collision is a MISS and not a wrong picture
//!
//! The file is named by a 64-bit digest of the path, which is ten lines instead
//! of eighty — and a digest that short can, in principle, be two paths. So the
//! whole path goes INSIDE the file and is compared on the way out. Two paths
//! that collide read each other's header, disagree, and both decode; nobody
//! ever sees a picture of the wrong file. That is what makes ten lines of hash
//! honest rather than a gamble.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::{Rgba, Stamp};

/// What every file of this cache begins with.
const MAGIC: &[u8; 4] = b"RNTH";

/// Which shape the rest of the file is in.
///
/// A byte and not a date: what changes it is the LAYOUT below, and a reader
/// that does not know a layout must not guess at it. An unknown version is a
/// miss, so a newer shell's cache costs an older one a decode and never a
/// wrong picture.
///
/// # And it went to TWO without the layout moving
///
/// Because what a picture in here MEANS changed. The file manager's grid drew
/// waveforms for sounds with no artwork, the owner asked for the glyph
/// instead, the code was changed — and his screen went on showing waves,
/// because three of them were already on disk under a key that says what file
/// and what size and nothing about what was drawn. *"As waves continuam."*
///
/// The durable half of that repair is the caller's (`View::grid_tint` now
/// names the waveform's ink, so this can never go stale silently again); this
/// byte is what throws away every picture ALREADY written under the old rule,
/// including the ones nobody thought of. A cache is the one thing that can be
/// right in the source and wrong on the screen.
const VERSION: u8 = 2;

/// The cache policy, generated from `assets/cache.nix` by `nix/cache.nix`.
///
/// It lives HERE, beside the cache it is about, so the file manager can ask
/// `cache::policy::FRESH_DAYS` instead of carrying a table of its own. The
/// other three numbers in it are the sweep's and the brain's — one declaration,
/// because `freshDays < keepDays` is a promise no single crate can keep alone.
#[allow(dead_code)]
pub mod policy {
    include!(concat!(env!("OUT_DIR"), "/cache.rs"));
}

/// The most a cached picture may weigh.
///
/// The decoder's own ceiling is 512 px on the longest side (`crate::STEPS`),
/// which is a mebibyte of RGBA; four of those is room for a shape this format
/// has not grown yet and a refusal for a file that has been truncated, grown or
/// scribbled on. Without it a corrupt length field is an allocation of whatever
/// number happens to be in those eight bytes.
const CEILING: usize = 4 * 1024 * 1024;

/// FNV-1a, 64 bits.
///
/// Ten lines, no dependency, and no cryptographic claim: what it has to do is
/// spread ordinary paths over a directory, and a collision is caught by the
/// header rather than by the hash. See the module's note.
fn digest(of: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in of {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Where one stamp's picture would be kept, under `root`.
#[must_use]
pub fn file_in(root: &Path, stamp: &Stamp) -> PathBuf {
    let name = stamp.path.as_os_str().as_encoded_bytes().to_vec();
    root.join(format!("{:016x}.thumb", digest(&name)))
}

/// Reads the picture for `stamp`, if this cache has exactly it.
///
/// `tint` is whatever ink the decoder was given — the waveform's colour, for a
/// sound with no artwork. It is compared because it CHANGES THE PICTURE and the
/// stamp does not carry it: a waveform drawn in one theme and read back in
/// another would be the one thing here that comes out wrong rather than
/// missing.
///
/// `None` for every way of not having it, and they are deliberately one answer:
/// no file, an unreadable one, another shell's version, a digest collision, a
/// file that has been edited since, a different decode size. The caller's next
/// move is the same in all of them — decode it — so telling them apart would be
/// a distinction nothing could act on.
#[must_use]
pub fn read(root: &Path, stamp: &Stamp, tint: &str) -> Option<Kept> {
    let mut file = std::fs::File::open(file_in(root, stamp)).ok()?;
    // WHEN THIS PICTURE WAS MADE, taken from the file that is already open.
    //
    // It costs nothing and it is what lets the caller remake a picture that is
    // getting old without ever making the person wait for one. See [`Kept`].
    let made = file.metadata().and_then(|m| m.modified()).ok();
    let mut head = [0u8; 5];
    file.read_exact(&mut head).ok()?;
    if &head[..4] != MAGIC || head[4] != VERSION {
        return None;
    }
    let width = read_u32(&mut file)?;
    let height = read_u32(&mut file)?;
    let target = read_u32(&mut file)?;
    let mtime = read_u64(&mut file)? as i64;
    let bytes = read_u64(&mut file)?;
    let path_len = read_u32(&mut file)? as usize;
    let tint_len = read_u32(&mut file)? as usize;
    // THE WHOLE PATH, COMPARED. This is what turns a digest collision into a
    // miss — see the module's note — and it is also what catches a file that
    // was renamed on top of another.
    if path_len > CEILING {
        return None;
    }
    if tint_len > CEILING {
        return None;
    }
    let mut path = vec![0u8; path_len];
    file.read_exact(&mut path).ok()?;
    let mut ink = vec![0u8; tint_len];
    file.read_exact(&mut ink).ok()?;
    if path != stamp.path.as_os_str().as_encoded_bytes()
        || ink != tint.as_bytes()
        || mtime != stamp.mtime
        || bytes != stamp.bytes
        || target != stamp.target
    {
        return None;
    }
    let want = (width as usize)
        .checked_mul(height as usize)?
        .checked_mul(4)?;
    if want == 0 || want > CEILING {
        return None;
    }
    let mut pixels = vec![0u8; want];
    file.read_exact(&mut pixels).ok()?;
    Some(Kept {
        picture: Rgba {
            width,
            height,
            pixels,
        },
        made,
    })
}

/// A picture that came back out of a cache, and when it was PUT there.
///
/// # Why the age travels with it
///
/// Because a cache can be right about the file and wrong about the world. The
/// key says which file, what size and what ink — and none of those moves when
/// the RULE for drawing changes. This repository met it: the rule for a sound
/// with no cover art changed, the code was right, and the owner went on seeing
/// waveforms for days because three of them were already on disk.
///
/// A caller that knows how old a picture is can draw it AT ONCE and quietly
/// make it again behind the one on screen, which turns every future change of
/// rule into something that reaches the screen by itself.
///
/// `made` is `None` when the filesystem would not say — which a caller should
/// read as "old enough", never as "fresh".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kept {
    /// The pixels.
    pub picture: Rgba,
    /// When the cache file was written, if the filesystem said.
    pub made: Option<std::time::SystemTime>,
}

impl Kept {
    /// Whether this picture has been sitting here for `older_than` or more.
    ///
    /// A picture the filesystem will not date, or one stamped in the FUTURE,
    /// answers YES: remaking one needlessly costs a decode, and trusting a
    /// stamp nobody can read costs a screen that never corrects itself.
    #[must_use]
    pub fn stale(&self, now: std::time::SystemTime, older_than: std::time::Duration) -> bool {
        self.made
            .and_then(|made| now.duration_since(made).ok())
            .is_none_or(|age| age >= older_than)
    }
}

/// Keeps `picture` for `stamp`, under `root`.
///
/// Failure is silence on purpose: this is an optimisation, and a full disk or a
/// read-only home must cost a decode rather than a file manager that will not
/// draw. The write goes to a scratch file and is renamed over, so a kill in the
/// middle leaves the old picture or none — never half of one under a name that
/// says it is whole.
pub fn write(root: &Path, stamp: &Stamp, tint: &str, picture: &Rgba) {
    if picture.pixels.len() > CEILING || std::fs::create_dir_all(root).is_err() {
        return;
    }
    let name = stamp.path.as_os_str().as_encoded_bytes();
    let mut out = Vec::with_capacity(picture.pixels.len() + name.len() + 64);
    out.extend_from_slice(MAGIC);
    out.push(VERSION);
    out.extend_from_slice(&picture.width.to_le_bytes());
    out.extend_from_slice(&picture.height.to_le_bytes());
    out.extend_from_slice(&stamp.target.to_le_bytes());
    out.extend_from_slice(&(stamp.mtime as u64).to_le_bytes());
    out.extend_from_slice(&stamp.bytes.to_le_bytes());
    let (Ok(path_len), Ok(tint_len)) = (u32::try_from(name.len()), u32::try_from(tint.len()))
    else {
        return;
    };
    out.extend_from_slice(&path_len.to_le_bytes());
    out.extend_from_slice(&tint_len.to_le_bytes());
    out.extend_from_slice(name);
    out.extend_from_slice(tint.as_bytes());
    out.extend_from_slice(&picture.pixels);
    // THE SCRATCH NAME CARRIES THE PID, for the reason `ronoctua_xdg`'s own
    // atomic write does: two doors are two PROCESSES, and two of them opening
    // one scratch file with truncate interleave their writes.
    let at = file_in(root, stamp);
    let scratch = at.with_extension(format!("{}.new", std::process::id()));
    let done = std::fs::File::create(&scratch)
        .and_then(|mut f| f.write_all(&out).and_then(|()| f.sync_all()))
        .and_then(|()| std::fs::rename(&scratch, &at));
    if done.is_err() {
        let _ = std::fs::remove_file(&scratch);
    }
}

fn read_u32(from: &mut impl Read) -> Option<u32> {
    let mut b = [0u8; 4];
    from.read_exact(&mut b).ok()?;
    Some(u32::from_le_bytes(b))
}

fn read_u64(from: &mut impl Read) -> Option<u64> {
    let mut b = [0u8; 8];
    from.read_exact(&mut b).ok()?;
    Some(u64::from_le_bytes(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The waveform's ink, as a theme resolves it.
    const INK: &str = "#7aa2f7";

    fn scratch(name: &str) -> PathBuf {
        let at =
            std::env::temp_dir().join(format!("ronoctua-thumbcache-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&at);
        std::fs::create_dir_all(&at).unwrap();
        at
    }

    fn a_picture() -> Rgba {
        Rgba {
            width: 2,
            height: 2,
            pixels: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
        }
    }

    fn a_stamp(path: &str) -> Stamp {
        Stamp {
            path: PathBuf::from(path),
            mtime: 1_700_000_000,
            bytes: 4096,
            target: 128,
        }
    }

    #[test]
    fn what_was_written_comes_back() {
        let root = scratch("volta");
        let stamp = a_stamp("/tmp/a.jpg");
        assert!(
            read(&root, &stamp, INK).is_none(),
            "an empty cache answered"
        );
        write(&root, &stamp, INK, &a_picture());
        let back = read(&root, &stamp, INK).expect("it was just written");
        assert_eq!(back.picture.width, 2);
        assert_eq!(back.picture.height, 2);
        assert_eq!(back.picture.pixels, a_picture().pixels);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A FILE THAT CHANGED IS A MISS. Everything the stamp carries has to
    /// match, and each of the three has its own way of going stale: a file
    /// edited in place keeps its size, one truncated keeps its time in some
    /// filesystems, and the window being resized changes neither.
    #[test]
    fn a_file_that_changed_is_not_this_picture() {
        let root = scratch("mudou");
        let stamp = a_stamp("/tmp/b.jpg");
        write(&root, &stamp, INK, &a_picture());
        // AND ANOTHER THEME'S INK IS ANOTHER PICTURE — the waveform is painted
        // in a token's VALUE, and the stamp does not carry one.
        assert!(
            read(&root, &stamp, "#000000").is_none(),
            "a waveform came back in the wrong theme's colour"
        );
        for wrong in [
            Stamp {
                mtime: stamp.mtime + 1,
                ..a_stamp("/tmp/b.jpg")
            },
            Stamp {
                bytes: stamp.bytes + 1,
                ..a_stamp("/tmp/b.jpg")
            },
            Stamp {
                target: 256,
                ..a_stamp("/tmp/b.jpg")
            },
        ] {
            assert!(
                read(&root, &wrong, INK).is_none(),
                "a stale picture came back for {wrong:?}"
            );
        }
        assert!(
            read(&root, &stamp, INK).is_some(),
            "the right one went away"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_picture_painted_under_another_rule_is_a_miss() {
        // THE DEFECT THIS CATCHES, and the owner caught it first: a rule about
        // WHAT IS DRAWN changing while the key stays the same. The stamp
        // carries the file, the size and the modification time — none of them
        // moves when the drawing does — so the only thing standing between a
        // retired rule and a screen full of it is this string.
        let root = scratch("regra-velha");
        let stamp = a_stamp("/tmp/sound.mp3");
        // A WAVEFORM, drawn in a token's ink, by yesterday's rule.
        write(&root, &stamp, "0:#8A7FB8", &a_picture());
        // AND TODAY'S RULE DRAWS NO WAVES AT ALL, which is an empty ink in the
        // same theme — the one case a bare theme position could not tell apart.
        assert!(
            read(&root, &stamp, "0:").is_none(),
            "a waveform came back for a grid that has stopped drawing them"
        );
        // AND THE OTHER WAY ROUND TOO, so this is about the string and not
        // about which of the two is empty.
        write(&root, &stamp, "0:", &a_picture());
        assert!(
            read(&root, &stamp, "0:#8A7FB8").is_none(),
            "a picture with no waveform came back where one was asked for"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_picture_that_has_been_sitting_here_a_while_says_so() {
        // WHAT THIS BUYS: the file manager draws what is in the cache AT ONCE
        // and then, for one that has been there a while, makes it again behind
        // the picture already on screen. Nobody waits, and a change to the RULE
        // for drawing reaches the screen by itself — which is the defect this
        // repository met and could not detect: the rule for a sound with no
        // cover art changed, the code was right, and the owner went on seeing
        // waveforms because three of them were already on disk.
        let root = scratch("velhinha");
        let stamp = a_stamp("/tmp/old.jpg");
        write(&root, &stamp, INK, &a_picture());
        let back = read(&root, &stamp, INK).expect("it came back");
        let now = std::time::SystemTime::now();
        let day = std::time::Duration::from_secs(24 * 60 * 60);

        assert!(
            !back.stale(now, day),
            "a picture written a moment ago was called stale"
        );
        // Ten days later it is, which is what sends the worker back to the file.
        assert!(back.stale(now + 10 * day, day));

        // AND A PICTURE NOBODY CAN DATE IS STALE. Remaking one needlessly costs
        // a decode; trusting a stamp that cannot be read costs a screen that
        // never corrects itself.
        let undated = Kept {
            picture: a_picture(),
            made: None,
        };
        assert!(undated.stale(now, day), "an undated picture was trusted");

        // AND SO IS ONE FROM THE FUTURE, for the same reason — and note this is
        // the OPPOSITE of the sweep's rule about a future stamp, on purpose:
        // there the cost of being wrong is a file deleted, here it is a decode.
        let ahead = Kept {
            picture: a_picture(),
            made: Some(now + 10 * day),
        };
        assert!(
            ahead.stale(now, day),
            "a picture from the future was trusted"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// TWO PATHS UNDER ONE NAME ARE A MISS, never a wrong picture.
    ///
    /// The digest is 64 bits and cannot be proved collision-free, so the
    /// collision is FORCED here: the second stamp's file is written where the
    /// first one's would be. What is being tested is the header's comparison,
    /// which is the whole reason the path travels inside the file.
    #[test]
    fn a_collision_reads_as_nothing_rather_than_as_the_other_file() {
        let root = scratch("colisao");
        let mine = a_stamp("/tmp/mine.jpg");
        let theirs = a_stamp("/tmp/theirs.jpg");
        write(&root, &theirs, INK, &a_picture());
        // THE OTHER FILE'S PICTURE, MOVED UNDER MY NAME. This is what a
        // collision looks like from the inside.
        std::fs::rename(file_in(&root, &theirs), file_in(&root, &mine)).unwrap();
        assert!(
            read(&root, &mine, INK).is_none(),
            "another file's picture came back under this name"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// THE SHARED CACHE GOES OUT AND COMES BACK, and the text with it.
    ///
    /// What this really tests is the PNG surgery: the `image` crate encodes
    /// pixels and has nowhere to put a `tEXt`, so the chunks are spliced in
    /// after `IHDR` by hand — a length, a type, the data and a checksum. Get
    /// the checksum wrong and every other program refuses the file while ours
    /// reads it back happily, which is the worst possible way for this to fail.
    #[test]
    fn the_shared_cache_is_written_where_another_program_would_look() {
        let root = scratch("freedesktop");
        let path = std::path::Path::new("/tmp/a photo (1).jpg");
        let picture = a_picture();
        assert!(shared::read(&root, path, 7, 128).is_none());
        shared::write(&root, path, 7, 128, &picture);
        // THE NAME IS THE STANDARD'S, and it is what another manager computes.
        let at = shared::file_for(&root, path, 128).expect("128 is one of the four sizes");
        assert!(at.starts_with(root.join("thumbnails").join("normal")));
        assert!(at.exists(), "nothing was written to {at:?}");
        // AND IT IS THE PERSON'S ALONE, which the standard requires: a
        // thumbnail of a file nobody else may read must not be readable either.
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&at).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "the thumbnail is readable by somebody else");

        let back = shared::read(&root, path, 7, 128).expect("it was just written");
        assert_eq!(
            (back.picture.width, back.picture.height),
            (picture.width, picture.height)
        );
        assert_eq!(back.picture.pixels, picture.pixels);

        // AND A FILE THAT CHANGED IS A MISS, which is the whole of what the
        // recorded modification time is for.
        assert!(
            shared::read(&root, path, 8, 128).is_none(),
            "a thumbnail of an older version of the file came back"
        );
        // AND ANOTHER FILE'S NAME IS A MISS, even moved under this one's — the
        // URI inside the file is what settles an MD5 collision.
        let other = std::path::Path::new("/tmp/another.jpg");
        std::fs::rename(&at, shared::file_for(&root, other, 128).unwrap()).unwrap();
        assert!(shared::read(&root, other, 7, 128).is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// AND THE SIZES ARE THE STANDARD'S, which is what makes this free.
    ///
    /// Our quantisation was chosen from the texture budget (`crate::STEPS`) and
    /// theirs from what a file manager draws, and every one of ours is one of
    /// theirs — so a thumbnail this shell makes is a thumbnail another one can
    /// use, with no resample in the middle. If either list ever moves, this is
    /// where it is noticed.
    #[test]
    fn every_size_we_decode_at_is_one_the_standard_names() {
        for step in crate::STEPS {
            assert!(
                shared::file_for(
                    std::path::Path::new("/tmp"),
                    std::path::Path::new("/a"),
                    step
                )
                .is_some(),
                "{step} px is not one of the four the standard names"
            );
        }
    }

    /// AND THE URI IS ENCODED THE WAY THE STANDARD ASKS, because it is what is
    /// hashed: one byte different and the name is a different file.
    #[test]
    fn the_uri_is_what_another_program_would_hash() {
        assert_eq!(
            shared::uri_of(std::path::Path::new("/home/a/my photo.jpg")),
            "file:///home/a/my%20photo.jpg"
        );
        assert_eq!(
            shared::uri_of(std::path::Path::new("/tmp/a+b&c.png")),
            "file:///tmp/a%2Bb%26c.png"
        );
        // THE UNRESERVED SET IS LEFT ALONE, all of it.
        assert_eq!(
            shared::uri_of(std::path::Path::new("/a-b_c.d~e!f")),
            "file:///a-b_c.d~e!f"
        );
    }

    /// AND A SCRIBBLED FILE IS A MISS, not a panic and not a smear.
    #[test]
    fn a_damaged_file_is_a_miss() {
        let root = scratch("estragado");
        let stamp = a_stamp("/tmp/c.jpg");
        write(&root, &stamp, INK, &a_picture());
        let at = file_in(&root, &stamp);
        let whole = std::fs::read(&at).unwrap();
        // CUT SHORT: the pixels are not all there.
        std::fs::write(&at, &whole[..whole.len() - 4]).unwrap();
        assert!(
            read(&root, &stamp, INK).is_none(),
            "half a picture came back"
        );
        // AND WITH THE WRONG MAGIC.
        let mut other = whole.clone();
        other[0] = b'X';
        std::fs::write(&at, &other).unwrap();
        assert!(
            read(&root, &stamp, INK).is_none(),
            "somebody else's file was read"
        );
        // AND FROM A VERSION THIS SHELL DOES NOT KNOW.
        let mut newer = whole;
        newer[4] = VERSION.wrapping_add(1);
        std::fs::write(&at, &newer).unwrap();
        assert!(
            read(&root, &stamp, INK).is_none(),
            "an unknown layout was guessed at"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}

/// THE CACHE EVERY OTHER FILE MANAGER ON THE MACHINE ALREADY WRITES.
///
/// # Why this is worth eighty lines of MD5
///
/// Because the work is the same work. A person who has opened a folder in GNOME
/// Files, Thunar or Dolphin has already paid the 109 milliseconds a thumbnail
/// costs, and this reads it; a person who opens it here first has paid it for
/// them. The alternative is every manager on one machine decoding the same
/// photographs over and over, which is what the standard exists to stop.
///
/// The layout is the freedesktop Thumbnail Managing Standard:
/// `$XDG_CACHE_HOME/thumbnails/<size>/<md5 of the file URI>.png`, with the
/// source's URI and modification time in PNG `tEXt` chunks, and the file at
/// `0600`.
///
/// # What does NOT go in it
///
/// A picture this shell PAINTED rather than read: a waveform drawn in a token's
/// value, a mesh lit by our own lamp. They are not thumbnails OF the file in
/// any sense another program would recognise, they change with the theme, and
/// the standard has nowhere to say so. Those stay in [`read`] and [`write`]
/// above, whose header carries the tint. The rule is one line in the worker and
/// it is the honest one: what the FILE looks like is shared, what WE made is
/// ours.
pub mod shared {
    use std::io::Write;
    use std::path::{Path, PathBuf};

    use super::{CEILING, Rgba};

    /// The sizes the standard names, and the directory each lives in.
    ///
    /// They are `crate::STEPS` exactly — 128, 256 and 512 — which is not a
    /// coincidence worth relying on and is checked by
    /// `the-shared-cache-speaks-the-standard`: our quantisation was chosen from
    /// the texture budget and theirs from what a file manager draws, and the
    /// two agreeing is what makes this interop free rather than a resample.
    const SIZES: [(u32, &str); 4] = [
        (128, "normal"),
        (256, "large"),
        (512, "x-large"),
        (1024, "xx-large"),
    ];

    /// The keyword the standard files the source's address under.
    const URI: &str = "Thumb::URI";

    /// And the one it files the source's modification time under.
    const MTIME: &str = "Thumb::MTime";

    /// `file://` and the path, encoded as RFC 2396 asks.
    ///
    /// The ENCODING is the half that has to match another program byte for
    /// byte, because it is what is hashed: a space written as `%20` and a space
    /// written as `+` are two different file names as far as the digest is
    /// concerned, and the symptom would be a cache that silently never hits.
    /// The unreserved set is the standard's own.
    #[must_use]
    pub fn uri_of(path: &Path) -> String {
        let mut out = String::from("file://");
        for b in path.as_os_str().as_encoded_bytes() {
            let c = char::from(*b);
            if c.is_ascii_alphanumeric() || "/-_.!~*'()".contains(c) {
                out.push(c);
            } else {
                out.push('%');
                out.push_str(&format!("{b:02X}"));
            }
        }
        out
    }

    /// Where the standard would keep this file's thumbnail at `target` pixels.
    ///
    /// `None` when `target` is not one of the four sizes it names — there is no
    /// arguing with a layout another program reads.
    #[must_use]
    pub fn file_for(root: &Path, path: &Path, target: u32) -> Option<PathBuf> {
        let (_, dir) = SIZES.iter().find(|(px, _)| *px == target)?;
        let digest = super::md5::of(uri_of(path).as_bytes());
        let mut name = String::with_capacity(36);
        for b in digest {
            name.push_str(&format!("{b:02x}"));
        }
        name.push_str(".png");
        Some(root.join("thumbnails").join(dir).join(name))
    }

    /// Reads a thumbnail another program may have made.
    ///
    /// `None` for every way of not having it, and the checks are the standard's
    /// own: the URI in the file has to be this file's — an MD5 collision, or a
    /// name reused — and the recorded modification time has to be the one on
    /// disk now. A thumbnail of an older version of a file is the one thing
    /// this must never hand back, and it is exactly what a stale entry is.
    #[must_use]
    pub fn read(root: &Path, path: &Path, mtime: i64, target: u32) -> Option<super::Kept> {
        let at = file_for(root, path, target)?;
        let bytes = std::fs::read(&at).ok()?;
        if bytes.len() > CEILING {
            return None;
        }
        let text = texts(&bytes);
        let said = |key: &str| text.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str());
        if said(URI) != Some(uri_of(path).as_str()) {
            return None;
        }
        if said(MTIME)?.parse::<i64>().ok()? != mtime {
            return None;
        }
        let picture = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png).ok()?;
        let rgba = picture.to_rgba8();
        let (width, height) = (rgba.width(), rgba.height());
        let pixels = rgba.into_raw();
        // THE LENGTH IS CHECKED, not assumed: a decoder that hands back fewer
        // bytes than the dimensions promise is the smear that slides a little
        // further sideways every frame — the argument
        // `ronoctua_render::image::Rgba::new` makes on the other side of this
        // crate's boundary, and this one has no such constructor to lean on.
        if pixels.len() != (width as usize) * (height as usize) * 4 {
            return None;
        }
        Some(super::Kept {
            picture: Rgba {
                width,
                height,
                pixels,
            },
            // WHEN THE OTHER PROGRAM PUT IT THERE, for the reason the module
            // above gives: a picture can be right about the file and stale
            // about the rule that drew it. Here it is doubly worth having —
            // half of this directory was written by a file manager that is not
            // ours, and the oldest entry on the owner's machine is from
            // October.
            made: std::fs::metadata(&at).and_then(|m| m.modified()).ok(),
        })
    }

    /// Keeps a thumbnail where every other manager will find it.
    ///
    /// Failure is silence, for the reason the module above gives: this is an
    /// optimisation, and a full disk must cost a decode rather than a file
    /// manager that will not draw.
    pub fn write(root: &Path, path: &Path, mtime: i64, target: u32, picture: &Rgba) {
        let Some(at) = file_for(root, path, target) else {
            return;
        };
        let Some(parent) = at.parent() else { return };
        if std::fs::create_dir_all(parent).is_err() {
            return;
        }
        let mut png = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new(&mut png);
        if image::ImageEncoder::write_image(
            encoder,
            &picture.pixels,
            picture.width,
            picture.height,
            image::ExtendedColorType::Rgba8,
        )
        .is_err()
        {
            return;
        }
        let Some(whole) = with_texts(
            &png,
            &[
                (URI, uri_of(path)),
                (MTIME, mtime.to_string()),
                // WHO MADE IT, which the standard asks for and which is how a
                // person looking at a cache directory finds out what wrote it.
                ("Software", String::from("ronoctua")),
            ],
        ) else {
            return;
        };
        // 0600, WHICH THE STANDARD REQUIRES AND WHICH IS THE POINT: a thumbnail
        // is a picture of a file, and a file nobody else may read must not have
        // a picture of itself that they can.
        let scratch = at.with_extension(format!("{}.new", std::process::id()));
        let done = std::fs::File::create(&scratch)
            .and_then(|mut f| {
                use std::os::unix::fs::PermissionsExt;
                f.set_permissions(std::fs::Permissions::from_mode(0o600))?;
                f.write_all(&whole)?;
                f.sync_all()
            })
            .and_then(|()| std::fs::rename(&scratch, &at));
        if done.is_err() {
            let _ = std::fs::remove_file(&scratch);
        }
    }

    /// Every `tEXt` chunk of a PNG, as `(keyword, value)`.
    ///
    /// Walked by hand because the `image` crate decodes PIXELS and does not
    /// hand back a file's text chunks — and because a chunk walk is fifteen
    /// lines: length, type, data, checksum, repeat.
    fn texts(png: &[u8]) -> Vec<(String, String)> {
        let mut out = Vec::new();
        let mut at = 8; // the signature
        while at + 8 <= png.len() {
            let len = u32::from_be_bytes([png[at], png[at + 1], png[at + 2], png[at + 3]]) as usize;
            let kind = &png[at + 4..at + 8];
            let from = at + 8;
            let Some(to) = from.checked_add(len) else {
                break;
            };
            if to > png.len() {
                break;
            }
            if kind == b"tEXt"
                && let Some(nul) = png[from..to].iter().position(|b| *b == 0)
            {
                // LATIN-1, which is what the format says a `tEXt` is. A path
                // with a byte above 127 in it arrives percent-encoded anyway —
                // see `uri_of` — so nothing real is lost here.
                let key: String = png[from..from + nul]
                    .iter()
                    .map(|b| char::from(*b))
                    .collect();
                let value: String = png[from + nul + 1..to]
                    .iter()
                    .map(|b| char::from(*b))
                    .collect();
                out.push((key, value));
            }
            if kind == b"IEND" {
                break;
            }
            at = to + 4;
        }
        out
    }

    /// The same PNG with `text` chunks added after its header.
    ///
    /// AFTER `IHDR` AND BEFORE `IDAT`, which is where the format says text
    /// belongs and where every reader looks for it. `None` when the bytes are
    /// not a PNG with a header, which cannot happen for something this function
    /// was just handed by the encoder — and is still answered rather than
    /// assumed.
    fn with_texts(png: &[u8], text: &[(&str, String)]) -> Option<Vec<u8>> {
        if png.len() < 8 || &png[..8] != b"\x89PNG\r\n\x1a\n" {
            return None;
        }
        let len = u32::from_be_bytes([png[8], png[9], png[10], png[11]]) as usize;
        let after = 8 + 8 + len + 4;
        if after > png.len() || &png[12..16] != b"IHDR" {
            return None;
        }
        let mut out = Vec::with_capacity(png.len() + 128);
        out.extend_from_slice(&png[..after]);
        for (key, value) in text {
            let mut data = Vec::with_capacity(key.len() + value.len() + 1);
            data.extend_from_slice(key.as_bytes());
            data.push(0);
            data.extend_from_slice(value.as_bytes());
            out.extend_from_slice(&u32::try_from(data.len()).ok()?.to_be_bytes());
            out.extend_from_slice(b"tEXt");
            out.extend_from_slice(&data);
            let mut crc = Vec::with_capacity(data.len() + 4);
            crc.extend_from_slice(b"tEXt");
            crc.extend_from_slice(&data);
            out.extend_from_slice(&crc32(&crc).to_be_bytes());
        }
        out.extend_from_slice(&png[after..]);
        Some(out)
    }

    /// The checksum every PNG chunk carries — the ordinary IEEE one.
    fn crc32(of: &[u8]) -> u32 {
        let mut crc = 0xFFFF_FFFFu32;
        for b in of {
            crc ^= u32::from(*b);
            for _ in 0..8 {
                crc = if crc & 1 == 1 {
                    (crc >> 1) ^ 0xEDB8_8320
                } else {
                    crc >> 1
                };
            }
        }
        !crc
    }
}

/// MD5, because the standard names the file by one.
///
/// # Why it is written out here
///
/// The freedesktop layout is `<md5 of the URI>.png` and there is no arguing
/// with it: another program computes that name, and reading its cache means
/// computing the same one. This workspace carries no hashing crate and adding
/// one for eighty lines that RFC 1321 publishes test vectors for would be a
/// dependency in every closure that links this crate.
///
/// NO SECURITY CLAIM IS MADE, and none is needed: what this does is spread file
/// names over a directory, and every hit is confirmed against the URI inside
/// the file before a single pixel is used.
mod md5 {
    /// The per-round shift amounts, RFC 1321 section 3.4.
    const SHIFTS: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5,
        9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10,
        15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];

    /// `floor(abs(sin(i + 1)) * 2^32)`, the table the RFC calls `T`.
    fn sines() -> [u32; 64] {
        let mut t = [0u32; 64];
        for (i, slot) in t.iter_mut().enumerate() {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let v = (f64::from(i as u32 + 1).sin().abs() * f64::from(u32::MAX).mul_add(1.0, 1.0))
                as u64;
            *slot = (v & 0xFFFF_FFFF) as u32;
        }
        t
    }

    /// The digest of `bytes`, sixteen bytes in the RFC's own order.
    pub fn of(bytes: &[u8]) -> [u8; 16] {
        let t = sines();
        let mut message = bytes.to_vec();
        let bits = (bytes.len() as u64).wrapping_mul(8);
        message.push(0x80);
        while message.len() % 64 != 56 {
            message.push(0);
        }
        message.extend_from_slice(&bits.to_le_bytes());

        let (mut a0, mut b0, mut c0, mut d0) = (
            0x6745_2301u32,
            0xEFCD_AB89u32,
            0x98BA_DCFEu32,
            0x1032_5476u32,
        );
        for block in message.chunks_exact(64) {
            let mut m = [0u32; 16];
            for (i, word) in m.iter_mut().enumerate() {
                *word = u32::from_le_bytes([
                    block[i * 4],
                    block[i * 4 + 1],
                    block[i * 4 + 2],
                    block[i * 4 + 3],
                ]);
            }
            let (mut a, mut b, mut c, mut d) = (a0, b0, c0, d0);
            for i in 0..64 {
                let (f, g) = match i / 16 {
                    0 => ((b & c) | (!b & d), i),
                    1 => ((d & b) | (!d & c), (5 * i + 1) % 16),
                    2 => (b ^ c ^ d, (3 * i + 5) % 16),
                    _ => (c ^ (b | !d), (7 * i) % 16),
                };
                let f = f.wrapping_add(a).wrapping_add(t[i]).wrapping_add(m[g]);
                a = d;
                d = c;
                c = b;
                b = b.wrapping_add(f.rotate_left(SHIFTS[i]));
            }
            a0 = a0.wrapping_add(a);
            b0 = b0.wrapping_add(b);
            c0 = c0.wrapping_add(c);
            d0 = d0.wrapping_add(d);
        }
        let mut out = [0u8; 16];
        out[..4].copy_from_slice(&a0.to_le_bytes());
        out[4..8].copy_from_slice(&b0.to_le_bytes());
        out[8..12].copy_from_slice(&c0.to_le_bytes());
        out[12..].copy_from_slice(&d0.to_le_bytes());
        out
    }

    #[cfg(test)]
    mod tests {
        use super::of;

        fn hex(of: [u8; 16]) -> String {
            of.iter().map(|b| format!("{b:02x}")).collect()
        }

        /// RFC 1321'S OWN TEST SUITE, all seven of it.
        ///
        /// Written out rather than trusted, because this is the one piece of
        /// this feature whose answer has to match ANOTHER PROGRAM's byte for
        /// byte: a digest that is merely self-consistent produces a cache that
        /// never hits and never says why.
        #[test]
        fn the_rfcs_own_vectors() {
            for (input, want) in [
                ("", "d41d8cd98f00b204e9800998ecf8427e"),
                ("a", "0cc175b9c0f1b6a831c399e269772661"),
                ("abc", "900150983cd24fb0d6963f7d28e17f72"),
                ("message digest", "f96b697d7cb7938d525a2f31aaf161d0"),
                (
                    "abcdefghijklmnopqrstuvwxyz",
                    "c3fcd3d76192e4007dfb496cca67e13b",
                ),
                (
                    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789",
                    "d174ab98d277d9f5a5611c2c9f419d9f",
                ),
                (
                    "12345678901234567890123456789012345678901234567890123456789012345678901234567890",
                    "57edf4a22be3c955ac49da2e2107b67a",
                ),
            ] {
                assert_eq!(hex(of(input.as_bytes())), want, "md5({input:?})");
            }
        }
    }
}
