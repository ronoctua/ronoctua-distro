//! The system's application entries — the base of the `LEADER a` launcher.
//!
//! # Why a crate, and not a module of the shell
//!
//! Because this is specification logic over `&str` and `Path`: it knows neither
//! Wayland, nor GPU, nor compositor, and that is why `cargo test` proves it in
//! milliseconds. It is the same reason as `ronoctua-clipboard` (queue and policy,
//! without whatever watches the selection) and as `ronoctua-term` (the VT machine, without the PTY).
//!
//! And because `surface.rs` already has more than four thousand lines.
//!
//! # What it does NOT do
//!
//! **It does not resolve icons.** It returns the raw `Icon=` and the `Categories=`; what
//! calls the resolution chain is the host, which already depends on `ronoctua-notify`.
//! The reverse would invert the crate graph and spread invariant 10 across
//! two places.
//!
//! **It launches nothing.** It returns `(program, arguments)`, which is exactly
//! what `open_foreign` receives.

pub mod desktop;
pub mod exec;
pub mod fold;

use std::path::{Path, PathBuf};

/// An application the system offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopEntry {
    /// The freedesktop id: the path relative to `applications/` with `/` →
    /// `-`, without the extension. `kde4/foo.desktop` → `kde4-foo`.
    ///
    /// It is what deduplicates, and it is what the search matches along with the name — the id
    /// is usually the binary's name, and typing `nvim` to find "Neovim" is
    /// the gesture of whoever knows the system.
    pub id: String,
    /// The name, already in the session's language.
    pub name: String,
    /// The description, already in the session's language. Empty when there is none.
    pub description: String,
    /// The raw `Icon=` — a theme name **or** an absolute path; the spec allows
    /// both, and the resolution chain already handles both.
    pub icon: String,
    /// The `Categories=`, for the icon-by-category step.
    pub categories: Vec<String>,
    /// The program and the arguments, ready for launching.
    ///
    /// The program is the **resolved path** when it exists in the `PATH` of
    /// whoever scanned — see [`resolve_program`]. It only stays as a raw name when
    /// it does not resolve.
    pub command: (String, Vec<String>),
    /// Does it ask for a terminal? These go through OUR terminal.
    pub terminal: bool,
    /// The `.desktop`'s `Path=`: the directory the application expects to start
    /// in. `None` when the entry does not say.
    ///
    /// It used to be parsed and dropped. What suffers is the application that
    /// reads a file next to its own binary — a Wine wrapper, a game with data
    /// alongside — and the symptom is the worst kind: it opens and dies, with
    /// nothing on screen.
    pub working_directory: Option<String>,
    /// The folded form of `name` — the key the search matches on.
    ///
    /// Computed ONCE during the scan. Folding per keystroke would redo the
    /// work of hundreds of entries on every letter typed.
    pub key: String,
}

/// The `applications` directories, in priority order.
///
/// # `$XDG_DATA_HOME` FIRST
///
/// Without it in front, the user's override is invisible — and the override is the
/// whole freedesktop customisation mechanism. Whoever puts a
/// `.desktop` in `~/.local/share/applications` is saying precisely "mine
/// beats the system's".
#[must_use]
pub fn directories() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let casa = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")));
    if let Some(c) = casa {
        out.push(c.join("applications"));
    }
    // The spec's default when the variable does not exist. Without it, a session that
    // does not export it would see no application at all.
    let dirs =
        std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".to_owned());
    for d in dirs.split(':').filter(|d| !d.is_empty()) {
        out.push(Path::new(d).join("applications"));
    }
    out
}

/// How many subdirectory levels to descend.
///
/// Distribution themes put files in `applications/kde4/`; three levels
/// cover what exists. The ceiling is here so the loop terminates, not to
/// limit anyone.
const DEPTH: usize = 3;

/// Scans the given directories and returns the entries, sorted.
///
/// # The order is alphabetical, and not `read_dir`'s
///
/// `read_dir` returns in the filesystem's order: the list would change between
/// machines and on every installation, and the item under the cursor would swap on its own. It is the
/// same reason `icon::look_up` sorts the size directories.
///
/// The sort key is the **folded** one, otherwise `Ágil` would land after
/// `Zebra`.
#[must_use]
pub fn scan_in(dirs: &[PathBuf], language: &str) -> Vec<DesktopEntry> {
    let mut by_id: std::collections::BTreeMap<String, DesktopEntry> =
        std::collections::BTreeMap::new();
    for dir in dirs {
        let mut found = Vec::new();
        collect_from(dir, 0, &mut found);
        // Within a directory, a stable order before deduplicating: two
        // files with the same id in the same place do not exist, but the reading
        // order must not decide anything.
        found.sort();
        for path in found {
            let Some((id, input_of)) = read(&path, dir, language) else {
                continue;
            };
            // The FIRST wins: the directories already come in priority order.
            by_id.entry(id).or_insert(input_of);
        }
    }
    let mut out: Vec<DesktopEntry> = by_id.into_values().collect();
    out.sort_by(|a, b| a.key.cmp(&b.key).then_with(|| a.id.cmp(&b.id)));
    out
}

/// Scans the environment's directories.
#[must_use]
pub fn scan(language: &str) -> Vec<DesktopEntry> {
    scan_in(&directories(), language)
}

/// Gathers `dir`'s `.desktop` files, descending to [`DEPTH`].
fn collect_from(dir: &Path, nivel: usize, out: &mut Vec<PathBuf>) {
    let Ok(items) = std::fs::read_dir(dir) else {
        // A directory that does not exist is the common case, not an error: `XDG_DATA_DIRS`
        // lists places not every system has.
        return;
    };
    for item in items.filter_map(Result::ok) {
        let p = item.path();
        if p.is_dir() {
            if nivel + 1 < DEPTH {
                collect_from(&p, nivel + 1, out);
            }
        } else if p.extension().is_some_and(|e| e == "desktop") {
            out.push(p);
        }
    }
}

/// A `.desktop` becoming `(id, DesktopEntry)`. `None` when it should not appear.
fn read(path: &Path, root: &Path, language: &str) -> Option<(String, DesktopEntry)> {
    let text = std::fs::read_to_string(path).ok()?;
    let fields = desktop::fields(&text);

    // The FILTERS, in order of cost. Measured on a actual host: without them, **41 of
    // 108** files get in, and 38% of the list becomes a MIME handler — the first
    // page is useless.
    if desktop::raw_bytes(&fields, "Type") != Some("Application") {
        return None;
    }
    if desktop::boolean(&fields, "NoDisplay") || desktop::boolean(&fields, "Hidden") {
        return None;
    }
    // `TryExec` is the spec's answer to "a `.desktop` orphaned by a removed package".
    if let Some(t) = desktop::raw_bytes(&fields, "TryExec")
        && !exists_on_path(t)
    {
        return None;
    }

    let name = desktop::localized(&fields, "Name", language)?
        .trim()
        .to_owned();
    if name.is_empty() {
        return None;
    }
    let icon = desktop::raw_bytes(&fields, "Icon")
        .unwrap_or_default()
        .to_owned();
    let raw = desktop::raw_bytes(&fields, "Exec")?;
    let (program, args) = exec::assemble(
        raw,
        exec::Context {
            icon: &icon,
            name: &name,
            file: &path.to_string_lossy(),
        },
    )?;
    // THE PROGRAM BECOMES A PATH HERE, during the scan, and not at launch time:
    // here we are still in the session's `PATH`, and over there inside the systemd scope
    // it is minimal. See [`resolve_program`].
    //
    // If it does not resolve, the raw name stays: the `.desktop` may point at something
    // that only exists under another `PATH`, and making the application disappear over
    // that would be worse than letting it try and fail saying why.
    let program = resolve_program(&program).map_or(program, |p| p.to_string_lossy().into_owned());
    let command = (program, args);

    let id = id_of(path, root)?;
    let description = desktop::localized(&fields, "Comment", language)
        // `GenericName` is the second-best description: "Web Browser" when
        // there is no `Comment`. Measured: 87 have `Comment`, 51 have `GenericName`, and
        // the cascade covers nearly everything.
        .or_else(|| desktop::localized(&fields, "GenericName", language))
        .unwrap_or_default()
        .trim()
        .to_owned();

    Some((
        id.clone(),
        DesktopEntry {
            key: fold::fold(&name),
            id,
            name,
            description,
            icon,
            categories: desktop::list(&fields, "Categories")
                .into_iter()
                .map(str::to_owned)
                .collect(),
            command,
            terminal: desktop::boolean(&fields, "Terminal"),
            // `Path=` is a plain value, never localized — the spec says so, and
            // `raw` is what reads the last occurrence like every other key here.
            working_directory: desktop::raw_bytes(&fields, "Path")
                .map(str::trim)
                .filter(|p| !p.is_empty())
                .map(str::to_owned),
        },
    ))
}

/// The freedesktop id: the path relative to the root, `/` → `-`, without the extension.
fn id_of(path: &Path, root: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    let s = rel.to_str()?;
    Some(s.trim_end_matches(".desktop").replace('/', "-"))
}

/// Where `program` actually is.
///
/// An absolute path is checked directly; a bare name is looked up in `PATH`, which is what
/// the spec demands for `TryExec`.
///
/// # Why the PATH, and not a boolean
///
/// Because whoever launches needs it. The `.desktop` writes `Exec=firefox`, and
/// that name only resolves against the `PATH` of **whoever scanned** — the application starts
/// inside a `systemd-run --scope`, where `PATH` is minimal. It is the trap
/// `CLAUDE.md` records in so many words: *spawning in a systemd unit, use a PATH,
/// never a name* — and the failure shows up far from the cause.
///
/// As a bonus it is what makes the message line always say the same thing: without
/// this, half the applications showed `/nix/store/…/bin/x` and the other
/// half only `x`, depending on what the vendor wrote in the file.
fn resolve_program(program: &str) -> Option<PathBuf> {
    let p = Path::new(program);
    if p.is_absolute() {
        return p.exists().then(|| p.to_path_buf());
    }
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|d| d.join(program))
        .find(|c| c.exists())
}

/// Does the `TryExec` resolve?
fn exists_on_path(program: &str) -> bool {
    resolve_program(program).is_some()
}

#[cfg(test)]
mod tests {
    use super::{DesktopEntry, scan_in};
    use std::path::PathBuf;

    /// Writes `.desktop` files into a temporary directory and scans.
    fn with_it(arquivos: &[(&str, &str)]) -> Vec<DesktopEntry> {
        let root = std::env::temp_dir().join(format!(
            "ronoctua-apps-teste-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ));
        for (name, content) in arquivos {
            let target = root.join(name);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).expect("mkdir");
            }
            std::fs::write(&target, content).expect("write");
        }
        let r = scan_in(std::slice::from_ref(&root), "pt_BR");
        std::fs::remove_dir_all(&root).ok();
        r
    }

    /// THE DEFECT: without the filters, 38% of the list becomes a MIME handler.
    #[test]
    fn what_must_not_show_up_does_not_show_up() {
        let r = with_it(&[
            (
                "bom.desktop",
                "[Desktop Entry]\nType=Application\nName=Bom\nExec=bom\n",
            ),
            (
                "escondido.desktop",
                "[Desktop Entry]\nType=Application\nName=X\nExec=x\nNoDisplay=true\n",
            ),
            (
                "oculto.desktop",
                "[Desktop Entry]\nType=Application\nName=Y\nExec=y\nHidden=true\n",
            ),
            (
                "link.desktop",
                "[Desktop Entry]\nType=Link\nName=Z\nURL=http://x\n",
            ),
            (
                "orfao.desktop",
                "[Desktop Entry]\nType=Application\nName=W\nExec=w\nTryExec=/nao/existe/mesmo\n",
            ),
            (
                "sem-exec.desktop",
                "[Desktop Entry]\nType=Application\nName=V\n",
            ),
        ]);
        assert_eq!(
            r.len(),
            1,
            "sobrou {:?}",
            r.iter().map(|e| &e.name).collect::<Vec<_>>()
        );
        assert_eq!(r[0].name, "Bom");
    }

    /// THE DEFECT: `read_dir` is the filesystem's order, and the item under the
    /// cursor swaps on its own between machines.
    #[test]
    fn the_order_is_alphabetical_and_ignores_accents() {
        let r = with_it(&[
            (
                "z.desktop",
                "[Desktop Entry]\nType=Application\nName=Zebra\nExec=z\n",
            ),
            (
                "a.desktop",
                "[Desktop Entry]\nType=Application\nName=Ágil\nExec=a\n",
            ),
            (
                "m.desktop",
                "[Desktop Entry]\nType=Application\nName=Meio\nExec=m\n",
            ),
        ]);
        let names: Vec<&str> = r.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["Ágil", "Meio", "Zebra"]);
    }

    /// THE DEFECT: a shallow `read_dir` loses the vendor subdirectories, and their
    /// id has to carry the path.
    #[test]
    fn the_subdirectory_goes_in_and_the_id_carries_the_path() {
        let r = with_it(&[(
            "kde4/foo.desktop",
            "[Desktop Entry]\nType=Application\nName=Foo\nExec=foo\n",
        )]);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].id, "kde4-foo");
    }

    #[test]
    fn the_description_falls_back_to_generic_name_when_there_is_no_comment() {
        let r = with_it(&[(
            "a.desktop",
            "[Desktop Entry]\nType=Application\nName=Firefox\nExec=firefox\n\
             GenericName=Navegador Web\n",
        )]);
        assert_eq!(r[0].description, "Navegador Web");
    }

    /// THE DEFECT: without `XDG_DATA_HOME` in front, the user's override is
    /// invisible — and it is the whole freedesktop mechanism.
    #[test]
    fn the_first_directory_wins_on_the_same_id() {
        let base = std::env::temp_dir().join(format!(
            "ronoctua-apps-prio-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ));
        let (meu, sistema) = (base.join("meu"), base.join("sistema"));
        for (d, name) in [(&meu, "O Meu"), (&sistema, "O Do Sistema")] {
            std::fs::create_dir_all(d).expect("mkdir");
            std::fs::write(
                d.join("x.desktop"),
                format!("[Desktop Entry]\nType=Application\nName={name}\nExec=x\n"),
            )
            .expect("write");
        }
        let dirs: Vec<PathBuf> = vec![meu, sistema];
        let r = scan_in(&dirs, "pt_BR");
        std::fs::remove_dir_all(&base).ok();
        assert_eq!(r.len(), 1, "o mesmo id nao pode aparecer duas vezes");
        assert_eq!(r[0].name, "O Meu");
    }

    /// THE DEFECT, SEEN ON SCREEN: half the applications showed the binary's
    /// path and the other half only the name, depending on what the vendor
    /// wrote in the `Exec=`.
    ///
    /// And the side that hurts more than the inconsistency: the application starts inside
    /// a `systemd-run --scope`, where `PATH` is minimal — a bare name there
    /// fails far from the cause.
    #[test]
    fn the_program_becomes_a_path_when_it_exists() {
        // `sh` is on every system that runs this test — including in the
        // Nix sandbox, where `PATH` is short but has a shell.
        let r = with_it(&[(
            "a.desktop",
            "[Desktop Entry]\nType=Application\nName=Shell\nExec=sh -c oi\n",
        )]);
        assert_eq!(r.len(), 1);
        let (program, args) = &r[0].command;
        assert!(
            std::path::Path::new(program).is_absolute(),
            "`sh` ficou como nome cru: {program}"
        );
        assert!(
            program.ends_with("/sh"),
            "resolveu para outra coisa: {program}"
        );
        assert_eq!(args, &["-c", "oi"], "os argumentos nao podem mudar");

        // And what does NOT resolve stays as it is: making the application disappear would be
        // worse than letting it try and fail saying why.
        let r = with_it(&[(
            "b.desktop",
            "[Desktop Entry]\nType=Application\nName=Fantasma\nExec=zzz-nao-existe-mesmo\n",
        )]);
        assert_eq!(r[0].command.0, "zzz-nao-existe-mesmo");
    }

    #[test]
    fn the_search_key_arrives_already_folded() {
        let r = with_it(&[(
            "a.desktop",
            "[Desktop Entry]\nType=Application\nName=Cálculo\nExec=galculator\n",
        )]);
        assert_eq!(r[0].key, "calculo");
    }
}
