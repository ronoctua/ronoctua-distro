//! The right column — the one piece of chrome beside the footer that
//! invariant 14 allows — as a SLOT with one tenant at a time.
//!
//! # Why a slot, and not the player's own pid file
//!
//! Because the rule is about the EDGE, not about the player: "at most one
//! column on the right, at a time". Written as the player's file it would
//! hold for the player only, and the next thing that wants the edge would
//! bring a second file, and two columns side by side — the chrome invariant 14
//! refuses. So the slot is declared here, once: whoever takes it ends whoever
//! had it, and the tenant is a variant of [`Tenant`], never a second file.
//!
//! # What the file holds
//!
//! `<pid> <tenant>` in the runtime directory (`column.pid`). A bare pid is
//! read as the player's, which is what the file held before there was a slot.
//!
//! # A living pid is not enough
//!
//! A pid is recycled. A column that died without its `Drop` — killed, or
//! taken down with a session — leaves its number in the file, and the next
//! process to get that number is somebody else's: asked only "is it alive",
//! the toggle would send it `TERM`, answer "closed", and raise nothing — a
//! click on the mark that did nothing, which is what the owner saw. So the
//! command line is read too: this program, started as that tenant.

/// The runtime file the slot is kept in.
pub const FILE: &str = "column.pid";

/// Who can hold the right column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tenant {
    /// The player (`ronoctua-shell column`).
    Player,
}

impl Tenant {
    /// Every tenant, for reading the file back.
    pub const ALL: [Self; 1] = [Self::Player];

    /// Its name in the file.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Player => "player",
        }
    }

    /// The word after the program's name that starts it.
    #[must_use]
    pub const fn subcommand(self) -> &'static str {
        match self {
            Self::Player => "column",
        }
    }

    fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.name() == name)
    }
}

/// Who holds the slot now, and its pid — only when that process is alive, is
/// not this one, and is this program started as that tenant.
#[must_use]
pub fn holder() -> Option<(i32, Tenant)> {
    let path = ronoctua_xdg::runtime_file(FILE)?;
    let (pid, tenant) = parse(&std::fs::read_to_string(path).ok()?)?;
    let me = i32::try_from(std::process::id()).ok()?;
    if pid == me {
        return None;
    }
    let line = std::fs::read(format!("/proc/{pid}/cmdline")).ok()?;
    is_the_tenant(&line, tenant).then_some((pid, tenant))
}

/// `<pid> <tenant>`, or a bare pid — the player's, from before the slot.
fn parse(text: &str) -> Option<(i32, Tenant)> {
    let mut words = text.split_whitespace();
    let pid = words.next()?.parse().ok()?;
    let tenant = match words.next() {
        Some(name) => Tenant::named(name)?,
        None => Tenant::Player,
    };
    Some((pid, tenant))
}

/// Whether a `/proc/<pid>/cmdline` is this program started as `tenant`.
fn is_the_tenant(cmdline: &[u8], tenant: Tenant) -> bool {
    started_as(cmdline, &[tenant.subcommand()])
}

/// Whether a `/proc/<pid>/cmdline` is this program started with `words`
/// after its name — the check every pid file of this program makes before
/// believing the pid is still the process that wrote it.
#[must_use]
pub fn started_as(cmdline: &[u8], words: &[&str]) -> bool {
    let mut given = cmdline.split(|b| *b == 0).filter(|w| !w.is_empty());
    let program = given
        .next()
        .map(|w| String::from_utf8_lossy(w).into_owned())
        .unwrap_or_default();
    // THE WRAPPED NAME TOO: the installed shell is `wrapProgram`'d, and a
    // column raised by `current_exe()` from inside it starts the real binary,
    // `.ronoctua-shell-wrapped`.
    let ours = std::path::Path::new(&program).file_name().is_some_and(|n| {
        n.to_string_lossy()
            .trim_start_matches('.')
            .starts_with("ronoctua-shell")
    });
    ours && words.iter().all(|w| given.next() == Some(w.as_bytes()))
}

/// Ends whoever holds the slot; whether there was one.
pub fn end() -> bool {
    holder().is_some_and(|(pid, _)| {
        rustix::process::Pid::from_raw(pid).is_some_and(|p| {
            rustix::process::kill_process(p, rustix::process::Signal::TERM).is_ok()
        })
    })
}

/// Takes the slot for `tenant`, ending whoever held it — one column at a
/// time, which is invariant 14.
pub fn take(tenant: Tenant) {
    end();
    if let Some(path) = ronoctua_xdg::runtime_file(FILE) {
        let _ = std::fs::write(path, format!("{} {}", std::process::id(), tenant.name()));
    }
}

/// Gives the slot up, if this process still holds it: a tenant that already
/// took over must not lose the file to the one leaving.
pub fn leave() {
    if let Some(path) = ronoctua_xdg::runtime_file(FILE)
        && std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| parse(&t))
            .is_some_and(|(pid, _)| u32::try_from(pid).ok() == Some(std::process::id()))
    {
        let _ = std::fs::remove_file(path);
    }
}

/// Raises the tenant again, fresh: a layer surface's exclusive zone is carved
/// from what the surfaces made BEFORE it leave, so a footer made after the
/// column would be narrowed by it. The footer asks this when it starts.
pub fn raise_again() {
    let Some((_, tenant)) = holder() else {
        return;
    };
    if end()
        && let Ok(me) = std::env::current_exe()
    {
        let _ = std::process::Command::new(me)
            .arg(tenant.subcommand())
            .stdin(std::process::Stdio::null())
            .spawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: a recycled pid is taken for the column — the toggle would
    /// kill somebody else's process and raise nothing.
    #[test]
    fn only_the_tenant_is_the_tenant() {
        let p = Tenant::Player;
        assert!(is_the_tenant(
            b"/nix/store/x/bin/ronoctua-shell\0column\0",
            p
        ));
        assert!(is_the_tenant(
            b"/nix/store/x/bin/.ronoctua-shell-wrapped\0column\0",
            p
        ));
        assert!(!is_the_tenant(
            b"/nix/store/x/bin/ronoctua-shell\0player\0",
            p
        ));
        assert!(!is_the_tenant(b"/usr/bin/zsh\0column\0", p));
        assert!(!is_the_tenant(b"", p));
    }

    /// FAILS IF: the file stops naming its tenant, or a file from before the
    /// slot — a bare pid — stops being read as the player's.
    #[test]
    fn the_file_names_its_tenant_and_a_bare_pid_is_the_player() {
        assert_eq!(parse("42 player"), Some((42, Tenant::Player)));
        assert_eq!(parse("42\n"), Some((42, Tenant::Player)));
        assert_eq!(parse("42 somebody"), None);
        assert_eq!(parse(""), None);
    }
}
