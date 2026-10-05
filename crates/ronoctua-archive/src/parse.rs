//! Reading what the programs print — pure, so every shape is tested without
//! them.
//!
//! Three listings, one per program, chosen for being the ones meant for a
//! machine: `7zz l -slt -ba` (one `Key = value` block per member), `unrar lt`
//! (one `Key: value` block per member) and, for the tar family, bsdtar
//! writing the archive AS AN MTREE (`--format=mtree @archive`): one line per
//! member with every awkward byte of a name escaped as `\ooo`, so a name with
//! a space, an `=` or a line end in it is still one field. The times are read
//! as UTC — [`crate::run`] sets `TZ=UTC0` for exactly this.

use crate::{Member, Progress, Tool};

/// Days since 1970-01-01 of a civil date (Howard Hinnant's algorithm). Pure.
#[must_use]
pub(crate) fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Seconds since the epoch of `YYYY-MM-DD HH:MM:SS` (a fraction after `.`
/// or `,` is ignored), read as UTC; `None` for anything else. Pure.
#[must_use]
pub(crate) fn stamp(text: &str) -> Option<i64> {
    let text = text.trim();
    let (date, time) = text.split_once(' ')?;
    let mut d = date.split('-').map(|p| p.parse::<i64>().ok());
    let (year, month, day) = (d.next()??, d.next()??, d.next()??);
    let time = time.split(['.', ',']).next()?;
    let mut t = time.split(':').map(|p| p.parse::<i64>().ok());
    let (hour, minute, second) = (t.next()??, t.next()??, t.next()??);
    let valid = (1..=12).contains(&month)
        && (1..=31).contains(&day)
        && (0..24).contains(&hour)
        && (0..60).contains(&minute)
        && (0..=60).contains(&second);
    valid.then(|| days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 + second)
}

fn member(path: String) -> Member {
    Member {
        path,
        size_bytes: None,
        packed_bytes: None,
        modified_s: None,
        is_dir: false,
        encrypted: false,
    }
}

/// `7zz l -slt -ba`, a piece of output at a time.
#[derive(Debug, Default)]
pub(crate) struct SevenZzListing {
    current: Option<Member>,
    /// The members read so far.
    pub members: Vec<Member>,
}

impl SevenZzListing {
    /// One piece of output.
    pub(crate) fn line(&mut self, line: &str) {
        let Some((key, value)) = line.split_once(" = ").or_else(|| {
            // An empty value is printed as `Key = ` and the line end cut the
            // space off.
            line.strip_suffix(" =").map(|k| (k, ""))
        }) else {
            return;
        };
        if key == "Path" {
            self.finish_one();
            self.current = Some(member(value.to_owned()));
            return;
        }
        let Some(m) = self.current.as_mut() else {
            return;
        };
        match key {
            "Size" => m.size_bytes = value.trim().parse().ok(),
            "Packed Size" => m.packed_bytes = value.trim().parse().ok(),
            "Modified" => m.modified_s = stamp(value),
            "Folder" => m.is_dir |= value.trim() == "+",
            "Attributes" => m.is_dir |= value.starts_with('D'),
            "Encrypted" => m.encrypted = value.trim() == "+",
            _ => {}
        }
    }

    fn finish_one(&mut self) {
        if let Some(m) = self.current.take() {
            self.members.push(m);
        }
    }

    /// Every member.
    pub(crate) fn finish(mut self) -> Vec<Member> {
        self.finish_one();
        self.members
    }
}

/// `unrar lt`, a piece of output at a time.
#[derive(Debug, Default)]
pub(crate) struct UnrarListing {
    current: Option<Member>,
    /// The members read so far.
    pub members: Vec<Member>,
}

impl UnrarListing {
    /// One piece of output.
    pub(crate) fn line(&mut self, line: &str) {
        let Some((key, value)) = line.split_once(": ") else {
            return;
        };
        let key = key.trim_start();
        if key == "Name" {
            self.finish_one();
            self.current = Some(member(value.to_owned()));
            return;
        }
        let Some(m) = self.current.as_mut() else {
            return;
        };
        match key {
            "Type" => m.is_dir = value.trim() == "Directory",
            "Size" => m.size_bytes = value.trim().parse().ok(),
            "Packed size" => m.packed_bytes = value.trim().parse().ok(),
            "mtime" => m.modified_s = stamp(value),
            "Flags" => m.encrypted |= value.split_whitespace().any(|f| f == "encrypted"),
            _ => {}
        }
    }

    fn finish_one(&mut self) {
        if let Some(m) = self.current.take() {
            self.members.push(m);
        }
    }

    /// Every member.
    pub(crate) fn finish(mut self) -> Vec<Member> {
        self.finish_one();
        self.members
    }
}

/// A name as mtree escapes it: `\ooo` is one byte. Pure.
#[must_use]
pub(crate) fn unvis(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let octal = bytes.get(i + 1..i + 4).filter(|o| {
            bytes[i] == b'\\' && o.iter().all(|b| (b'0'..=b'7').contains(b)) && o[0] <= b'3'
        });
        if let Some(o) = octal {
            out.push((o[0] - b'0') * 64 + (o[1] - b'0') * 8 + (o[2] - b'0'));
            i += 4;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// One line of bsdtar's mtree of an archive; `None` for a comment, a `/set`
/// or anything else that is not a member. Pure.
#[must_use]
pub(crate) fn mtree_member(line: &str) -> Option<Member> {
    if line.starts_with('#') || line.starts_with('/') || line.trim().is_empty() {
        return None;
    }
    let mut fields = line.split(' ').filter(|f| !f.is_empty());
    let name = unvis(fields.next()?);
    let name = name.strip_prefix("./").unwrap_or(&name);
    if name.is_empty() || name == "." {
        return None;
    }
    let mut m = member(name.to_owned());
    for field in fields {
        let Some((key, value)) = field.split_once('=') else {
            continue;
        };
        match key {
            "type" => m.is_dir = value == "dir",
            "size" => m.size_bytes = value.parse().ok(),
            "time" => m.modified_s = value.split('.').next().and_then(|s| s.parse().ok()),
            _ => {}
        }
    }
    Some(m)
}

/// Where a program is, from one piece of what it prints: a percentage, the
/// member it is on, or both. Pure.
#[must_use]
pub(crate) fn progress(tool: Tool, piece: &str) -> Option<Progress> {
    let trimmed = piece.trim_start();
    let percent = |s: &str| -> Option<u8> {
        let (digits, _) = s.split_once('%')?;
        let n: u8 = digits.trim().parse().ok()?;
        (n <= 100).then_some(n)
    };
    match tool {
        // ` 29% 1 + big.bin`, ` 45% 3 - folder/a.txt`, `  0%`.
        Tool::SevenZz => {
            let p = percent(trimmed)?;
            let rest = trimmed.split_once('%').map_or("", |(_, r)| r);
            let member = [" + ", " - ", " U ", " = "]
                .iter()
                .find_map(|m| rest.split_once(m).map(|(_, name)| name.to_owned()))
                .filter(|n| !n.is_empty());
            Some(Progress {
                percent: Some(p),
                member,
            })
        }
        // `Extracting  folder/note.txt` and, redrawn, ` 45%`.
        Tool::Unrar => {
            if let Some(name) = trimmed
                .strip_prefix("Extracting from ")
                .map(|_| None)
                .or_else(|| trimmed.strip_prefix("Extracting ").map(Some))
                .flatten()
            {
                let name = name.trim().trim_end_matches("OK").trim_end();
                return (!name.is_empty()).then(|| Progress {
                    percent: None,
                    member: Some(name.to_owned()),
                });
            }
            let p = percent(trimmed)?;
            (trimmed.trim_end().ends_with('%')).then_some(Progress {
                percent: Some(p),
                member: None,
            })
        }
        // `-v`: `x folder/a.txt` extracting, `a folder/a.txt` creating.
        Tool::Bsdtar => {
            let name = piece
                .strip_prefix("x ")
                .or_else(|| piece.strip_prefix("a "))?;
            Some(Progress {
                percent: None,
                member: Some(name.to_owned()),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stamp_is_utc_seconds() {
        assert_eq!(stamp("1970-01-01 00:00:00"), Some(0));
        assert_eq!(stamp("2026-01-01 00:00:00,000000000"), Some(1_767_225_600));
        assert_eq!(stamp("2026-09-30 04:29:49.3921726"), Some(1_790_742_589));
        assert_eq!(stamp("2000-02-29 12:00:00"), Some(951_825_600));
        assert_eq!(stamp(""), None);
        assert_eq!(stamp("2026-13-01 00:00:00"), None);
        assert_eq!(stamp("yesterday"), None);
    }

    #[test]
    fn seven_zz_blocks_are_members() {
        let text = "Enter password:\nPath = sp ace\nSize = 0\nPacked Size = 0\n\
                    Modified = 2026-09-30 04:35:54.1083922\nAttributes = D drwxr-xr-x\n\
                    CRC =\nEncrypted = -\n\nPath = a = b.txt\nSize = 13\nPacked Size =\n\
                    Modified = 2026-09-30 04:29:49.3921726\nAttributes = A -rw-r--r--\n\
                    Encrypted = +\n\nPath = z/\nFolder = +\nSize = 0\n";
        let mut listing = SevenZzListing::default();
        for line in text.lines() {
            listing.line(line);
        }
        let members = listing.finish();
        assert_eq!(members.len(), 3);
        assert!(members[0].is_dir && !members[0].encrypted);
        assert_eq!(members[1].path, "a = b.txt", "only the first ` = ` splits");
        assert_eq!(members[1].size_bytes, Some(13));
        assert_eq!(members[1].packed_bytes, None);
        assert_eq!(members[1].modified_s, Some(1_790_742_589));
        assert!(members[1].encrypted && !members[1].is_dir);
        assert!(members[2].is_dir, "a zip's folder says `Folder = +`");
    }

    #[test]
    fn unrar_blocks_are_members() {
        let text = "Archive: data.rar\nDetails: RAR 5\n\n        Name: folder/note.txt\n\
                    \x20       Type: File\n        Size: 16\n Packed size: 16\n       Ratio: 100%\n\
                    \x20      mtime: 2026-01-01 00:00:00,000000000\n       Flags: encrypted \n\n\
                    \x20       Name: folder\n        Type: Directory\n";
        let mut listing = UnrarListing::default();
        for line in text.lines() {
            listing.line(line);
        }
        let members = listing.finish();
        assert_eq!(members.len(), 2, "{members:?}");
        assert_eq!(members[0].path, "folder/note.txt");
        assert_eq!(members[0].size_bytes, Some(16));
        assert_eq!(members[0].modified_s, Some(1_767_225_600));
        assert!(members[0].encrypted);
        assert!(members[1].is_dir && !members[1].encrypted);
    }

    #[test]
    fn an_mtree_line_is_a_member_with_its_bytes_back() {
        let m = mtree_member(
            "./src/sp\\040ace/we\\075ird\\043name\\134x.txt time=1790742954.0 type=file size=2",
        )
        .unwrap();
        assert_eq!(m.path, "src/sp ace/we=ird#name\\x.txt");
        assert_eq!(m.size_bytes, Some(2));
        assert_eq!(m.modified_s, Some(1_790_742_954));
        let m = mtree_member("./src/new\\012line time=1.5 type=file size=1").unwrap();
        assert_eq!(m.path, "src/new\nline");
        assert!(mtree_member("./d time=0.0 type=dir").unwrap().is_dir);
        assert_eq!(mtree_member("#mtree"), None);
        assert_eq!(mtree_member("/set type=file"), None);
        assert_eq!(mtree_member(". type=dir"), None);
        assert_eq!(unvis("\\1234"), "S4", "three digits, then a plain byte");
        assert_eq!(unvis("a\\9b"), "a\\9b", "not octal: kept");
    }

    #[test]
    fn a_progress_line_says_how_far_and_where() {
        let seven = |s| progress(Tool::SevenZz, s);
        assert_eq!(
            seven(" 29% 1 + big.bin"),
            Some(Progress {
                percent: Some(29),
                member: Some("big.bin".to_owned())
            })
        );
        assert_eq!(seven("  0%").unwrap().member, None);
        assert_eq!(
            seven(" 45% 3 - a - b.txt").unwrap().member.as_deref(),
            Some("a - b.txt")
        );
        assert_eq!(seven("Everything is Ok"), None);
        assert_eq!(seven(" 0M Scan"), None);
        assert_eq!(
            progress(Tool::Unrar, "Extracting  folder/note.txt")
                .unwrap()
                .member
                .as_deref(),
            Some("folder/note.txt")
        );
        assert_eq!(progress(Tool::Unrar, "Extracting from a.rar"), None);
        assert_eq!(progress(Tool::Unrar, " 45%").unwrap().percent, Some(45));
        assert_eq!(
            progress(Tool::Bsdtar, "x sp ace/a.txt")
                .unwrap()
                .member
                .as_deref(),
            Some("sp ace/a.txt")
        );
        assert_eq!(progress(Tool::Bsdtar, "bsdtar: warning"), None);
    }
}
