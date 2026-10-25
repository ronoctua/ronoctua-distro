//! Whether a part is here — the one question every reader of a part asks.
//!
//! A part's path ([`crate::names::profile`]) is stable and may not exist yet:
//! the part is arriving, waits for a network, or was declined. A reader that
//! used the path without asking would fail far from the cause, so it asks
//! here, on EVERY use and never once at start — a part arrives while the
//! session runs, and a reader that asked at start would keep saying it is
//! missing (`part-paths-are-read-as-arriving`).

use std::path::{Path, PathBuf};

/// A part that is not here, and which: the reader's sentence names it and
/// opens the parts' page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Absent {
    /// The part's id, the segment after the parts' root.
    pub part: Option<String>,
    /// The path that was asked after.
    pub path: PathBuf,
}

/// `path` when it exists, else what is absent.
///
/// `path` is a part's profile or anything under it (`…/v1/bin/yt-dlp`): the
/// part is named from the segment after [`crate::names::PARTS_ROOT`].
///
/// # Errors
///
/// [`Absent`] when nothing is at `path` — a dangling profile link included,
/// since `Path::exists` follows it.
pub fn here(path: &Path) -> Result<PathBuf, Absent> {
    if path.exists() {
        return Ok(path.to_path_buf());
    }
    let part = path
        .strip_prefix(crate::names::PARTS_ROOT)
        .ok()
        .and_then(|rest| rest.components().next())
        .and_then(|c| c.as_os_str().to_str())
        .map(str::to_owned);
    Err(Absent {
        part,
        path: path.to_path_buf(),
    })
}

/// A file of the part `id` — `bin/unrar` — at the interface the running
/// system's plan wants ([`crate::names::PLAN`]), when the part is here: what
/// a program asks on EVERY use of a part's program.
///
/// # Errors
///
/// [`Absent`] when the plan does not want the part, or it is not here yet.
pub fn file_of(id: &str, relative: &str) -> Result<PathBuf, Absent> {
    file_of_in(Path::new(crate::names::PLAN), id, relative)
}

/// [`file_of`], over the plan at `plan` — a test's.
///
/// # Errors
///
/// [`Absent`].
pub fn file_of_in(plan: &Path, id: &str, relative: &str) -> Result<PathBuf, Absent> {
    let interface = crate::plan::read(plan)
        .and_then(|p| p.parts.get(id).map(|w| w.interface))
        .ok_or_else(|| Absent {
            part: Some(id.to_owned()),
            path: crate::names::profile(id, 1).join(relative),
        })?;
    here(&crate::names::profile(id, interface).join(relative))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: a part the plan does not want is offered, a part is looked
    /// for at another interface than the plan's, or an absent one is not
    /// named.
    #[test]
    fn a_part_s_file_is_where_the_plan_s_interface_puts_it() {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-parts-file-of-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let plan = dir.join("plan.json");
        let out = "/nix/store/0123456789abcdfghijklmnpqrsvwxyz-unrar-7";
        std::fs::write(
            &plan,
            format!(
                r#"{{"format":1,"parts":{{"unrar":{{"drv":"{out}.drv","out":"{out}","kind":"program","interface":3,"heavy":false,"unfree":true,"name":"unrar"}}}}}}"#
            ),
        )
        .unwrap();
        let absent = file_of_in(&plan, "unrar", "bin/unrar").expect_err("not here in a test");
        assert_eq!(absent.part.as_deref(), Some("unrar"));
        assert_eq!(
            absent.path,
            crate::names::profile("unrar", 3).join("bin/unrar")
        );
        let unwanted = file_of_in(&plan, "uade", "bin/uade").expect_err("not in the plan");
        assert_eq!(unwanted.part.as_deref(), Some("uade"));
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: a missing part is not named, so the screen could not say
    /// WHICH part is arriving — or a path that exists is taken for absent.
    #[test]
    fn a_missing_part_is_named() {
        let missing = Path::new("/nix/var/nix/profiles/ronoctua/parts/yt-dlp/v1/bin/yt-dlp");
        let absent = here(missing).expect_err("nothing is there in a test");
        assert_eq!(absent.part.as_deref(), Some("yt-dlp"));
        assert_eq!(absent.path, missing);

        let elsewhere = here(Path::new("/nonexistent/x")).expect_err("absent");
        assert_eq!(elsewhere.part, None);

        let there = std::env::temp_dir();
        assert_eq!(here(&there), Ok(there.clone()));
    }
}
