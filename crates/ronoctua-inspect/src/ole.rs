//! The compound file: a filesystem inside a file.
//!
//! # Why it is written here
//!
//! Because `.doc`, `.xls` and `.ppt` are not documents in the way the six
//! office formats next door are. Those are a ZIP OF XML and this crate has
//! read one since the day it learnt to draw a 3MF. These three are OLE
//! compound files — a FAT, a directory of named streams, and the payload
//! inside one of them — which is a small filesystem and nothing like a zip.
//!
//! It is here rather than taken from a crate for the reason [`crate::rtf`]
//! gives: what a preview needs is one named stream out of a file, and the
//! whole of that is a header, a sector table and a walk. A crate that did this
//! properly would bring writing, transactions and the whole directory API with
//! it.
//!
//! # And the reason it was written at all is a MEASUREMENT
//!
//! `catppt`, the packaged reader for a `.ppt`, answered **zero bytes with exit
//! status zero** on the owner's own `sample.ppt` — a file `file` calls a real
//! OLE compound document and which carries its text in plain sight. The
//! refusal it earned was honest and the owner asked for the other thing:
//! *"o preview também não foi capaz de abrir o arquivo sample.ppt"*. So the
//! presentation is read here instead, and the program is gone.
//!
//! # What it does NOT do
//!
//! Write anything, understand a transaction, or walk the red-black tree the
//! directory is sorted as. The directory is read as a flat array of entries,
//! which is what it physically is; the tree is an index for finding a name
//! quickly among thousands, and these files have eight.

use crate::Refusal;

/// The eight bytes every compound file begins with.
const MAGIC: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

/// The end of a chain.
const END: u32 = 0xFFFF_FFFE;

/// One directory entry, in bytes. Fixed by the format.
const ENTRY: usize = 128;

/// How many sectors one chain may be followed for.
///
/// A malformed FAT can point at itself, and a walk with no bound is a file
/// manager that stops answering. This is a quarter of a million sectors —
/// 128 MiB at the 512-byte size, which is past anything these three formats
/// hold — and reaching it is a refusal rather than a wrong answer.
const CHAIN_CEILING: usize = 256 * 1024;

/// The most one stream may come to.
///
/// The document module's own `PART_CEILING` argument, for the same reason:
/// this is read on the inspector's thread to fill a panel of four lines.
const STREAM_CEILING: usize = 32 * 1024 * 1024;

/// A compound file, with its tables read.
struct Vault<'a> {
    raw: &'a [u8],
    /// How big a sector is. 512 in version 3, 4096 in version 4.
    sector: usize,
    /// The whole file allocation table, sector by sector.
    fat: Vec<u32>,
    /// The mini file allocation table, for streams below the cutoff.
    mini_fat: Vec<u32>,
    /// The mini stream itself — the root entry's own chain.
    mini: Vec<u8>,
    /// Below this size a stream lives in the mini stream.
    cutoff: u32,
    /// The directory, as a flat run of 128-byte entries.
    directory: Vec<u8>,
}

/// One named stream out of a compound file.
///
/// # Errors
///
/// [`Refusal::Failed`] when the bytes are not a compound file, when a table
/// does not add up, or when there is no stream of that name;
/// [`Refusal::TooBig`] past [`STREAM_CEILING`].
pub fn stream(raw: &[u8], want: &str) -> Result<Vec<u8>, Refusal> {
    let vault = Vault::of(raw)?;
    vault.named(want)
}

impl<'a> Vault<'a> {
    fn of(raw: &'a [u8]) -> Result<Self, Refusal> {
        if raw.len() < 512 || raw[..8] != MAGIC {
            return Err(Refusal::Failed);
        }
        let shift = u16(raw, 0x1E)?;
        // NINE OR TWELVE, and nothing else: version 3 files are 512-byte
        // sectors and version 4 are 4096. A shift this reader invented a
        // meaning for would be reading a file it has never seen.
        if shift != 9 && shift != 12 {
            return Err(Refusal::Failed);
        }
        // AND THE MINI SECTOR IS ALWAYS SIXTY-FOUR BYTES. The field exists;
        // the format fixes its value.
        if u16(raw, 0x20)? != 6 {
            return Err(Refusal::Failed);
        }
        let sector = 1usize << shift;
        let fat_count = u32(raw, 0x2C)? as usize;
        let dir_first = u32(raw, 0x30)?;
        let cutoff = u32(raw, 0x38)?;
        let mini_first = u32(raw, 0x3C)?;
        let difat_first = u32(raw, 0x44)?;

        // THE DIFAT: where the FAT's own sectors are.
        //
        // A hundred and nine of them fit in the header, and past that they are
        // a chain of sectors each ending in the number of the next. It is the
        // one place in this format where a table has a table.
        let mut difat: Vec<u32> = Vec::new();
        for n in 0..109usize {
            difat.push(u32(raw, 0x4C + n * 4)?);
        }
        let per = sector / 4 - 1;
        let mut next = difat_first;
        let mut walked = 0usize;
        while next < END && walked < CHAIN_CEILING {
            let at = Self::offset(sector, next);
            let body = raw.get(at..at + sector).ok_or(Refusal::Failed)?;
            for n in 0..per {
                difat.push(u32(body, n * 4)?);
            }
            next = u32(body, sector - 4)?;
            walked += 1;
        }

        let mut fat: Vec<u32> = Vec::with_capacity(fat_count * (sector / 4));
        for &s in difat.iter().take(fat_count) {
            let at = Self::offset(sector, s);
            let body = raw.get(at..at + sector).ok_or(Refusal::Failed)?;
            for n in 0..sector / 4 {
                fat.push(u32(body, n * 4)?);
            }
        }
        if fat.is_empty() {
            return Err(Refusal::Failed);
        }

        let mut vault = Self {
            raw,
            sector,
            fat,
            mini_fat: Vec::new(),
            mini: Vec::new(),
            cutoff,
            directory: Vec::new(),
        };
        // THE MINI FAT AND THE MINI STREAM, which cannot be read until the FAT
        // is: both of them live in ordinary sectors, and the mini stream is
        // the ROOT ENTRY's own chain.
        vault.mini_fat = vault
            .chain(mini_first, None)?
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        let directory = vault.chain(dir_first, None)?;
        let root = directory.get(..ENTRY).ok_or(Refusal::Failed)?;
        let (start, size) = Self::where_it_is(root, shift)?;
        vault.mini = vault.chain(start, Some(size))?;
        vault.directory = directory;
        Ok(vault)
    }

    /// Where sector `n` begins in the file.
    const fn offset(sector: usize, n: u32) -> usize {
        (n as usize + 1) * sector
    }

    /// The bytes of one chain, cut to `size` when the entry declared one.
    fn chain(&self, start: u32, size: Option<usize>) -> Result<Vec<u8>, Refusal> {
        let mut out: Vec<u8> = Vec::new();
        let mut at = start;
        let mut walked = 0usize;
        while at < END {
            if walked >= CHAIN_CEILING || out.len() > STREAM_CEILING {
                return Err(Refusal::TooBig(out.len() as u64));
            }
            let from = Self::offset(self.sector, at);
            let body = self
                .raw
                .get(from..from + self.sector)
                .ok_or(Refusal::Failed)?;
            out.extend_from_slice(body);
            at = *self.fat.get(at as usize).ok_or(Refusal::Failed)?;
            walked += 1;
        }
        if let Some(n) = size {
            if n > out.len() {
                return Err(Refusal::Failed);
            }
            out.truncate(n);
        }
        Ok(out)
    }

    /// The bytes of one chain of the MINI stream.
    fn mini_chain(&self, start: u32, size: usize) -> Result<Vec<u8>, Refusal> {
        const MINI: usize = 64;
        let mut out: Vec<u8> = Vec::new();
        let mut at = start;
        let mut walked = 0usize;
        while at < END && out.len() < size {
            if walked >= CHAIN_CEILING {
                return Err(Refusal::Failed);
            }
            let from = at as usize * MINI;
            let body = self.mini.get(from..from + MINI).ok_or(Refusal::Failed)?;
            out.extend_from_slice(body);
            at = *self.mini_fat.get(at as usize).ok_or(Refusal::Failed)?;
            walked += 1;
        }
        if size > out.len() {
            return Err(Refusal::Failed);
        }
        out.truncate(size);
        Ok(out)
    }

    /// Where one directory entry's bytes are, and how many there are.
    fn where_it_is(entry: &[u8], shift: u16) -> Result<(u32, usize), Refusal> {
        let start = u32(entry, 0x74)?;
        let low = u32(entry, 0x78)? as usize;
        // IN VERSION 3 ONLY THE LOW HALF IS A SIZE. The format says the top
        // four bytes must be zero there and writers have put other things in
        // them; a length read from those is a length nobody wrote.
        let size = if shift == 9 {
            low
        } else {
            let high = u32(entry, 0x7C)? as usize;
            low | (high << 32)
        };
        if size > STREAM_CEILING {
            return Err(Refusal::TooBig(size as u64));
        }
        Ok((start, size))
    }

    /// The stream of that name, wherever the format put it.
    fn named(&self, want: &str) -> Result<Vec<u8>, Refusal> {
        for entry in self.directory.chunks_exact(ENTRY) {
            // TWO IS A STREAM. One is a storage — a folder — and five is the
            // root, which is the mini stream itself and never content.
            if entry[0x42] != 2 {
                continue;
            }
            let length = u16(entry, 0x40)? as usize;
            // THE LENGTH COUNTS THE TERMINATING NUL, which is two bytes of
            // UTF-16 and not one.
            let Some(name) = length.checked_sub(2).and_then(|n| entry.get(..n)) else {
                continue;
            };
            let name: String = char::decode_utf16(
                name.chunks_exact(2)
                    .map(|c| u16::from_le_bytes([c[0], c[1]])),
            )
            .map(|c| c.unwrap_or(char::REPLACEMENT_CHARACTER))
            .collect();
            if name != want {
                continue;
            }
            let shift = if self.sector == 512 { 9 } else { 12 };
            let (start, size) = Self::where_it_is(entry, shift)?;
            return if (size as u64) < u64::from(self.cutoff) {
                self.mini_chain(start, size)
            } else {
                self.chain(start, Some(size))
            };
        }
        Err(Refusal::Failed)
    }
}

/// A little-endian `u16` at `at`, or a refusal when the bytes are not there.
fn u16(b: &[u8], at: usize) -> Result<u16, Refusal> {
    let s = b.get(at..at + 2).ok_or(Refusal::Failed)?;
    Ok(u16::from_le_bytes([s[0], s[1]]))
}

/// A little-endian `u32` at `at`, or a refusal when the bytes are not there.
fn u32(b: &[u8], at: usize) -> Result<u32, Refusal> {
    let s = b.get(at..at + 4).ok_or(Refusal::Failed)?;
    Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// Compound files built byte by byte, for the readers that walk them.
///
/// # Why the builder is here and not beside one of them
///
/// Because two readers walk this format — `crate::ppt` and `crate::doc` — and
/// a fixture builder copied into both is the second implementation this crate
/// refuses everywhere else. It lives next to the reader it is the inverse of,
/// which is also the only way a failure can say which of the two is wrong.
///
/// The repository is PUBLIC (invariant 2) and nothing of the owner's goes into
/// it (invariant 13), so his own `sample.doc` cannot be the fixture. A vault
/// written here is one whose every byte the test chose.
#[cfg(test)]
pub(crate) mod fixture {
    /// One sector of a version 3 compound file.
    const SECTOR: usize = 512;

    /// One sector of the mini stream.
    const MINI: usize = 64;

    /// Below this size a stream lives in the mini stream.
    const CUTOFF: u32 = 4096;

    const FREE: u32 = 0xFFFF_FFFF;
    const END: u32 = 0xFFFF_FFFE;
    const FATSECT: u32 = 0xFFFF_FFFD;

    /// One 128-byte directory entry.
    fn entry(name: &str, kind: u8, start: u32, size: u64) -> Vec<u8> {
        let mut out = vec![0u8; 128];
        let utf16: Vec<u8> = name
            .encode_utf16()
            .chain(std::iter::once(0))
            .flat_map(u16::to_le_bytes)
            .collect();
        out[..utf16.len()].copy_from_slice(&utf16);
        // THE LENGTH COUNTS THE TERMINATING NUL, which is two bytes and not one.
        let n = u16::try_from(utf16.len()).expect("a short name");
        out[0x40..0x42].copy_from_slice(&n.to_le_bytes());
        out[0x42] = kind;
        out[0x44..0x48].copy_from_slice(&FREE.to_le_bytes());
        out[0x48..0x4C].copy_from_slice(&FREE.to_le_bytes());
        out[0x4C..0x50].copy_from_slice(&FREE.to_le_bytes());
        out[0x74..0x78].copy_from_slice(&start.to_le_bytes());
        out[0x78..0x80].copy_from_slice(&size.to_le_bytes());
        out
    }

    /// A sector of 128 little-endian `u32`s, padded with `FREE`.
    fn table(entries: &[u32]) -> Vec<u8> {
        let mut out: Vec<u8> = entries.iter().flat_map(|e| e.to_le_bytes()).collect();
        out.resize(SECTOR, 0);
        for n in entries.len()..SECTOR / 4 {
            out[n * 4..n * 4 + 4].copy_from_slice(&FREE.to_le_bytes());
        }
        out
    }

    /// A 512-byte header for a version 3 compound file.
    fn header(fat_sectors: u32) -> Vec<u8> {
        let mut out = vec![0u8; SECTOR];
        out[..8].copy_from_slice(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]);
        out[0x1A..0x1C].copy_from_slice(&3u16.to_le_bytes());
        out[0x1C..0x1E].copy_from_slice(&0xFFFEu16.to_le_bytes());
        out[0x1E..0x20].copy_from_slice(&9u16.to_le_bytes());
        out[0x20..0x22].copy_from_slice(&6u16.to_le_bytes());
        out[0x2C..0x30].copy_from_slice(&fat_sectors.to_le_bytes());
        // 1 IS THE DIRECTORY AND 2 IS THE MINI FAT, by this builder's layout.
        out[0x30..0x34].copy_from_slice(&1u32.to_le_bytes());
        out[0x38..0x3C].copy_from_slice(&CUTOFF.to_le_bytes());
        out[0x3C..0x40].copy_from_slice(&2u32.to_le_bytes());
        out[0x40..0x44].copy_from_slice(&1u32.to_le_bytes());
        out[0x44..0x48].copy_from_slice(&END.to_le_bytes());
        // THE DIFAT: sector 0 holds the one FAT sector these fixtures need.
        out[0x4C..0x50].copy_from_slice(&0u32.to_le_bytes());
        for n in 1..109usize {
            out[0x4C + n * 4..0x50 + n * 4].copy_from_slice(&FREE.to_le_bytes());
        }
        out
    }

    /// A compound file holding these named streams.
    ///
    /// WHICH TABLE A STREAM LANDS IN IS ITS SIZE, exactly as the format says:
    /// under four kilobytes it goes in the mini stream, at or over it in the
    /// ordinary sectors. That is not a detail a fixture may choose — a reader
    /// that only walked one of the two would pass on half the files it claims,
    /// and a real `.doc` uses both at once.
    ///
    /// # Panics
    ///
    /// When the fixture is larger than one FAT sector describes, which is 64
    /// KiB — far past anything a test should be building.
    pub fn vault(parts: &[(&str, &[u8])]) -> Vec<u8> {
        // 0: the FAT. 1: the directory. 2: the mini FAT. 3..: the mini stream,
        // then one chain per big stream.
        //
        // THE SMALL ONES ARE LAID OUT FIRST, all of them, because a big
        // stream's sector number is counted from the end of the mini stream —
        // and a builder that numbered one before the other was finished would
        // write a chain pointing into the middle of somebody else.
        let mut mini_fat: Vec<u32> = Vec::new();
        let mut mini: Vec<u8> = Vec::new();
        let mut directory: Vec<u8> = Vec::new();
        for (name, body) in parts.iter().filter(|(_, b)| b.len() < CUTOFF as usize) {
            let start = u32::try_from(mini.len() / MINI).expect("a small fixture");
            let sectors = body.len().div_ceil(MINI).max(1);
            for n in 0..sectors {
                mini_fat.push(if n + 1 == sectors {
                    END
                } else {
                    start + u32::try_from(n + 1).expect("a small fixture")
                });
            }
            let mut padded = (*body).to_vec();
            padded.resize(sectors * MINI, 0);
            mini.extend_from_slice(&padded);
            directory.extend_from_slice(&entry(
                name,
                2,
                start,
                u64::try_from(body.len()).expect("a small fixture"),
            ));
        }

        let mini_sectors = mini.len().div_ceil(SECTOR);
        let mut fat = vec![FATSECT, END, END];
        for n in 0..mini_sectors {
            fat.push(if n + 1 == mini_sectors {
                END
            } else {
                u32::try_from(3 + n + 1).expect("a small fixture")
            });
        }

        let mut big: Vec<u8> = Vec::new();
        for (name, body) in parts.iter().filter(|(_, b)| b.len() >= CUTOFF as usize) {
            let first = 3 + mini_sectors + big.len() / SECTOR;
            let sectors = body.len().div_ceil(SECTOR);
            for n in 0..sectors {
                fat.push(if n + 1 == sectors {
                    END
                } else {
                    u32::try_from(first + n + 1).expect("a small fixture")
                });
            }
            let mut padded = (*body).to_vec();
            padded.resize(sectors * SECTOR, 0);
            big.extend_from_slice(&padded);
            directory.extend_from_slice(&entry(
                name,
                2,
                u32::try_from(first).expect("a small fixture"),
                u64::try_from(body.len()).expect("a small fixture"),
            ));
        }
        assert!(
            fat.len() <= SECTOR / 4,
            "this fixture needs a second FAT sector"
        );

        let mut out = header(1);
        out.extend_from_slice(&table(&fat));
        // THE MINI STREAM IS THE ROOT ENTRY'S OWN CHAIN, which is the one thing
        // about this format that reads backwards: the root is a directory entry
        // and it holds bytes.
        let mut dir = entry(
            "Root Entry",
            5,
            if mini.is_empty() { END } else { 3 },
            u64::try_from(mini.len()).expect("a small fixture"),
        );
        dir.extend_from_slice(&directory);
        assert!(
            dir.len() <= SECTOR,
            "this fixture needs a second directory sector"
        );
        dir.resize(SECTOR, 0);
        out.extend_from_slice(&dir);
        out.extend_from_slice(&table(&mini_fat));
        let mut padded = mini;
        padded.resize(mini_sectors * SECTOR, 0);
        out.extend_from_slice(&padded);
        out.extend_from_slice(&big);
        out
    }
}
