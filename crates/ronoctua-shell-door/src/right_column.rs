//! The right column — the one piece of chrome beside the footer that
//! invariant 14 allows — as a SLOT per workspace, one tenant each.
//!
//! # Why a slot, and not the player's own pid file
//!
//! Because the rule is about the EDGE, not about the player: "at most one
//! column on the right of a workspace". Written as the player's file it would
//! hold for the player only, and the next thing that wants the edge would
//! bring a second file, and two columns side by side — the chrome invariant 14
//! refuses. So the slot is declared here, once: whoever takes a workspace ends
//! whoever had it, and the tenant is a variant of [`Tenant`], never a second
//! rule.
//!
//! # One per workspace, and a tenant exists once
//!
//! Each screen shows the column of the workspace on it, so two screens may
//! show two columns (`crate::column`). A tenant is ONE process: opening the
//! player's column on workspace 2 while it is on workspace 1 ends the one on
//! 1 — the column moves, it is not copied.
//!
//! # What the files hold
//!
//! One file per tenant in the runtime directory (`column-<tenant>.pid`),
//! `<pid> <size> <workspace…>`: the workspace LAST, so a name with spaces in
//! it survives, and absent when there was no compositor to name one. Each
//! process writes only its own, through a draft renamed into place.
//!
//! # A living pid is not enough
//!
//! A pid is recycled. A column that died without its `Drop` — killed, or
//! taken down with a session — leaves its number in the file, and the next
//! process to get that number is somebody else's: asked only "is it alive",
//! the toggle would send it `TERM`, answer "closed", and raise nothing — a
//! click on the mark that did nothing, which is what the owner saw. So the
//! command line is read too: this program, started as that tenant's column.

use crate::column::{Opening, Size};

/// Who can hold a right column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tenant {
    /// The player (`ronoctua-shell column player`, or `column` alone).
    Player,

    /// The search (`ronoctua-shell column fast-access`, `LEADER d c`).
    FastAccess,
}

impl Tenant {
    /// Every tenant, for reading the files back.
    pub const ALL: [Self; 2] = [Self::Player, Self::FastAccess];

    /// Its name in the files and on the command line.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Player => "player",

            Self::FastAccess => "fast-access",
        }
    }

    /// The tenant by its name; `None` for a name that is not one — refused,
    /// never read as the player.
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.name() == name)
    }

    /// The runtime file its slot is kept in.
    fn file(self) -> String {
        format!("column-{}.pid", self.name())
    }
}

/// A column that is up: its process, its tenant, its size and the workspace
/// it belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Holder {
    /// Its process.
    pub pid: i32,
    /// Who it is.
    pub tenant: Tenant,
    /// The size it is at.
    pub size: Size,
    /// The workspace it belongs to, by name — `None` when no compositor
    /// named one.
    pub workspace: Option<String>,
}

/// `<pid> <size> <workspace…>` — the workspace optional, and last.
fn parse(text: &str, tenant: Tenant) -> Option<Holder> {
    let mut words = text.trim_end_matches('\n').splitn(3, ' ');
    let pid = words.next()?.parse().ok()?;
    let size = Size::named(words.next()?)?;
    let workspace = words
        .next()
        .filter(|w| !w.is_empty())
        .map(ToOwned::to_owned);
    Some(Holder {
        pid,
        tenant,
        size,
        workspace,
    })
}

/// The line a holder is written as — [`parse`]'s other half.
fn line_of(h: &Holder) -> String {
    match &h.workspace {
        Some(w) => format!("{} {} {w}", h.pid, h.size.name()),
        None => format!("{} {}", h.pid, h.size.name()),
    }
}

/// This process's pid, as the files write it.
fn me() -> Option<i32> {
    i32::try_from(std::process::id()).ok()
}

/// Whether a holder read from a file is a column that is up: alive, not this
/// process, and this program started as that tenant's column.
fn believed(h: Holder) -> Option<Holder> {
    if Some(h.pid) == me() {
        return None;
    }
    let line = std::fs::read(format!("/proc/{}/cmdline", h.pid)).ok()?;
    is_the_tenant(&line, h.tenant).then_some(h)
}

/// The tenant's column, when one is up.
#[must_use]
pub fn holder(tenant: Tenant) -> Option<Holder> {
    let read =
        |leaf: &str| ronoctua_xdg::runtime_file(leaf).and_then(|p| std::fs::read_to_string(p).ok());
    read(&tenant.file())
        .and_then(|t| parse(&t, tenant))
        .and_then(believed)
}

/// Every column that is up.
#[must_use]
pub fn holders() -> Vec<Holder> {
    Tenant::ALL.into_iter().filter_map(holder).collect()
}

/// Whether a `/proc/<pid>/cmdline` is this program started as `tenant`'s
/// column — `column`, `column <tenant>`, and whatever else its opening says.
fn is_the_tenant(cmdline: &[u8], tenant: Tenant) -> bool {
    if !started_as(cmdline, &["column"]) {
        return false;
    }
    let words: Vec<String> = cmdline
        .split(|b| *b == 0)
        .filter(|w| !w.is_empty())
        .skip(2)
        .map(|w| String::from_utf8_lossy(w).into_owned())
        .collect();
    Opening::parse(&words[..]).is_ok_and(|o| o.tenant == tenant)
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

/// Sends `TERM` to a column; whether it went.
fn end_one(pid: i32) -> bool {
    rustix::process::Pid::from_raw(pid)
        .is_some_and(|p| rustix::process::kill_process(p, rustix::process::Signal::TERM).is_ok())
}

/// Whether `h` holds the workspace `here`.
///
/// A column whose workspace nobody knows — no compositor named one, or it
/// was started before the slot was per workspace — could be here, and so
/// could any column when `here` is unknown: the old rule, one column, is the
/// safe reading of "unknown".
fn holds(h: &Holder, here: Option<&str>) -> bool {
    here.is_none() || h.workspace.is_none() || h.workspace.as_deref() == here
}

/// Which columns a column of `me` taking `here` ends: the same tenant
/// WHEREVER it is — it exists once, so it moves — and whoever holds `here`.
fn to_end(me: Tenant, here: Option<&str>, all: &[Holder]) -> Vec<i32> {
    all.iter()
        .filter(|h| h.tenant == me || holds(h, here))
        .map(|h| h.pid)
        .collect()
}

/// Writes this process's own record, through a draft renamed into place:
/// a reader never sees half a line.
fn write_own(h: &Holder) {
    let Some(path) = ronoctua_xdg::runtime_file(&h.tenant.file()) else {
        return;
    };
    let draft = path.with_extension(format!("pid.{}", h.pid));
    if std::fs::write(&draft, line_of(h)).is_ok() && std::fs::rename(&draft, &path).is_err() {
        let _ = std::fs::remove_file(&draft);
    }
}

/// Takes `workspace` for `tenant`, at `size`: the same tenant ends wherever
/// it is, and so does whoever held the workspace — at most one column per
/// workspace, which is invariant 14.
pub fn take(tenant: Tenant, workspace: Option<&str>, size: Size) {
    for pid in to_end(tenant, workspace, &holders()) {
        let _ = end_one(pid);
    }
    let Some(pid) = me() else {
        return;
    };
    write_own(&Holder {
        pid,
        tenant,
        size,
        workspace: workspace.map(ToOwned::to_owned),
    });
}

/// The record this process wrote for `tenant`, while it is still this
/// process's — a column that already took over must not lose its file to
/// the one leaving.
fn own(tenant: Tenant) -> Option<(std::path::PathBuf, Holder)> {
    let path = ronoctua_xdg::runtime_file(&tenant.file())?;
    let h = parse(&std::fs::read_to_string(&path).ok()?, tenant)?;
    (Some(h.pid) == me()).then_some((path, h))
}

/// The column changed size: its record says so, for a footer that raises it
/// again (`raise_again`).
pub fn resize(tenant: Tenant, size: Size) {
    if let Some((_, h)) = own(tenant) {
        write_own(&Holder { size, ..h });
    }
}

/// Gives the slot up, if this process still holds it.
pub fn leave(tenant: Tenant) {
    if let Some((path, _)) = own(tenant) {
        let _ = std::fs::remove_file(path);
    }
}

/// What a toggle of a column did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Asked {
    /// It was up here, and it was ended.
    Closed,
    /// It was not up here — nowhere, or on another workspace — and is to be
    /// raised here, which ends it wherever it was.
    Raise,
}

/// The toggle's decision: the column is closed when it is HERE — or when
/// where it is or where here is is unknown, the toggle it always was — and
/// raised here otherwise, which moves it.
fn toggled(holder: Option<&Holder>, here: Option<&str>) -> Option<i32> {
    holder.filter(|h| holds(h, here)).map(|h| h.pid)
}

/// The mark, the gesture and `LEADER p c`: the tenant's column closed when
/// it is on `here`, and asked for here otherwise.
#[must_use]
pub fn toggle(tenant: Tenant, here: Option<&str>) -> Asked {
    match toggled(holder(tenant).as_ref(), here) {
        Some(pid) if end_one(pid) => Asked::Closed,
        _ => Asked::Raise,
    }
}

/// Whether the tenant's column is up HERE already — what the swap asks
/// before it raises one, so asking twice is asking once and never the
/// toggle's close. The toggle's own reading of "here" ([`toggled`]).
#[must_use]
pub fn is_here(tenant: Tenant, here: Option<&str>) -> bool {
    toggled(holder(tenant).as_ref(), here).is_some()
}

/// Ends the tenant's column WHEREVER it is — on any workspace, parked or
/// not; whether one went. What closing the player everywhere does to the
/// column when no daemon is there to say farewell. Never this process: a
/// column that closes itself quits.
pub fn end(tenant: Tenant) -> bool {
    holder(tenant).is_some_and(|h| end_one(h.pid))
}

/// The words after the program's name that raise this holder again, where
/// it was and at the size it had.
fn raised_as(h: &Holder) -> Vec<String> {
    Opening {
        tenant: h.tenant,
        on: h.workspace.clone(),
        size: Some(h.size),
    }
    .args()
}

/// Raises every column again, fresh: a layer surface's exclusive zone is
/// carved from what the surfaces made BEFORE it leave, so a footer made after
/// a column would be narrowed by it. The footer asks this when it starts.
///
/// Each where it was and at its size. One whose workspace no screen shows is
/// born parked (`crate::column`); one on screen is made after the footer.
pub fn raise_again() {
    let Ok(program) = std::env::current_exe() else {
        return;
    };
    for h in holders() {
        if !end_one(h.pid) {
            continue;
        }
        // WAITED FOR, aside (`foreign::waited`): a column the footer raises
        // and then forgets would end as a zombie of the footer.
        if let Ok(child) = crate::watchdog::unshared(&mut std::process::Command::new(&program))
            .args(raised_as(&h))
            .stdin(std::process::Stdio::null())
            .spawn()
        {
            let _ = crate::foreign::waited(child);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn held(pid: i32, tenant: Tenant, workspace: Option<&str>) -> Holder {
        Holder {
            pid,
            tenant,
            size: Size::Medium,
            workspace: workspace.map(ToOwned::to_owned),
        }
    }

    /// FAILS IF: a recycled pid is taken for the column — the toggle would
    /// kill somebody else's process and raise nothing — or a column started
    /// with its tenant, its workspace or its size named stops being read as
    /// that tenant's.
    #[test]
    fn only_the_tenant_is_the_tenant() {
        let p = Tenant::Player;
        for yes in [
            &b"/nix/store/x/bin/ronoctua-shell\0column\0"[..],
            b"/nix/store/x/bin/.ronoctua-shell-wrapped\0column\0",
            b"/nix/store/x/bin/ronoctua-shell\0column\0player\0",
            b"/nix/store/x/bin/ronoctua-shell\0column\0player\0--on\0mail box\0--size\0bar\0",
        ] {
            assert!(is_the_tenant(yes, p), "{}", String::from_utf8_lossy(yes));
        }
        for no in [
            &b"/nix/store/x/bin/ronoctua-shell\0player\0"[..],
            b"/nix/store/x/bin/ronoctua-shell\0column\0somebody\0",
            b"/nix/store/x/bin/ronoctua-shell\0column\0--size\0huge\0",
            b"/usr/bin/zsh\0column\0",
            b"",
        ] {
            assert!(!is_the_tenant(no, p), "{}", String::from_utf8_lossy(no));
        }
    }

    /// FAILS IF: a record stops reading back as what was written — the
    /// workspace last, a name with spaces whole, and none when no
    /// compositor named one.
    #[test]
    fn a_record_reads_back_as_it_was_written() {
        for h in [
            Holder {
                pid: 42,
                tenant: Tenant::Player,
                size: Size::Bar,
                workspace: Some("mail  and news".to_owned()),
            },
            held(7, Tenant::Player, Some("2")),
            held(9, Tenant::Player, None),
        ] {
            assert_eq!(parse(&line_of(&h), h.tenant), Some(h.clone()), "{h:?}");
        }
        assert_eq!(parse("42 huge 2", Tenant::Player), None, "an unknown size");
        assert_eq!(parse("", Tenant::Player), None);
    }

    /// FAILS IF: taking a workspace leaves the same tenant up elsewhere (it
    /// exists once, so it moves), or the workspace's occupant is not the one
    /// that holds it — whoever is on it, and whoever might be.
    #[test]
    fn taking_a_workspace_ends_the_tenant_elsewhere_and_the_occupant_here() {
        // THE SAME TENANT on another workspace, ended: the column moves.
        let player_on_1 = held(10, Tenant::Player, Some("1"));
        assert_eq!(
            to_end(
                Tenant::Player,
                Some("2"),
                std::slice::from_ref(&player_on_1)
            ),
            [10]
        );
        assert!(to_end(Tenant::Player, Some("2"), &[]).is_empty());
        // THE OCCUPANT, whoever it is: the one on this workspace holds it,
        // the one on another does not, and an unknown place might.
        assert!(holds(&player_on_1, Some("1")));
        assert!(!holds(&player_on_1, Some("2")));
        assert!(holds(&held(11, Tenant::Player, None), Some("2")));
        assert!(
            holds(&player_on_1, None),
            "here unknown: one column, the old rule"
        );
    }

    /// FAILS IF: the toggle stops closing the column that is HERE, or closes
    /// one on another workspace instead of bringing it here.
    #[test]
    fn the_toggle_closes_here_and_brings_from_elsewhere() {
        let on_1 = held(10, Tenant::Player, Some("1"));
        assert_eq!(toggled(Some(&on_1), Some("1")), Some(10), "here: closed");
        assert_eq!(
            toggled(Some(&on_1), Some("2")),
            None,
            "elsewhere: raised here"
        );
        assert_eq!(
            toggled(Some(&on_1), None),
            Some(10),
            "here unknown: the old toggle"
        );
        assert_eq!(toggled(None, Some("1")), None, "none: raised");
    }

    /// FAILS IF: a column raised again by a footer that restarted forgets
    /// where it was or the size it had.
    #[test]
    fn a_column_raised_again_keeps_its_place_and_its_size() {
        let h = Holder {
            pid: 3,
            tenant: Tenant::Player,
            size: Size::Small,
            workspace: Some("mail box".to_owned()),
        };
        let args = raised_as(&h);
        assert_eq!(args[0], "column");
        let back = Opening::parse(&args[1..]).expect("its own words parse");
        assert_eq!(
            (back.tenant, back.on.as_deref(), back.size),
            (Tenant::Player, Some("mail box"), Some(Size::Small))
        );
    }
}
