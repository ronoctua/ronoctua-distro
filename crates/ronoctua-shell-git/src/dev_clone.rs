//! `LEADER / c` — the distro's own repository, cloned into a folder the
//! person names, and then opened.
//!
//! # The question is the consent
//!
//! The address is the build's (`RONOCTUA_DISTRO_REPOSITORY`, `nix/rust.nix`),
//! so the one thing asked is WHERE. The question says which host the clone
//! asks, and answering it is the person letting that host — kept in
//! `consent.json` like the belongings' repository typed into its row — and
//! only then is a [`ronoctua_consent::Pass`] minted and the clone started
//! ([`ronoctua_git::Repo::clone`]). Nothing reaches the host before the
//! answer.
//!
//! # The host is where the address LEADS
//!
//! The clone keeps the person's git settings (their SSH setup, their
//! rewrites), and a `url.<base>.insteadOf` of theirs can send it to another
//! host than the address names. So before asking, the address is read where
//! it leads ([`resolve`], `ronoctua_git::effective_url`: git's own
//! `ls-remote --get-url`, which asks the network nothing) — off the loop, on
//! a thread whose answer comes back like the clone's — and the question
//! names THAT host: the same one for the common https → ssh rewrite (asked as
//! ever), another when the settings send the address elsewhere (asked, and
//! the question names the address's own host beside it). The answer is tied
//! to the question that named the host ([`Asked`], kept by the door, and let
//! go with the question); an answer no question named a host for is asked
//! first. The clone makes the repository first and reads where ITS fetch
//! goes ([`ronoctua_git::Repo::prepare_clone`]): settings included only for
//! that folder can send it elsewhere, and then the question is asked again,
//! naming that host, the folder offered ([`run`]) — nothing is reached
//! before an answer lets it. `pushInsteadOf` is a push's: a clone never
//! reads it.
//!
//! # Off the loop, and back by the loop's own ear
//!
//! A clone takes minutes, so it runs on a thread; what it ends with comes
//! back as the same command with an argument no person types (it starts with
//! [`BACK`]), on the channel that already runs a command with its argument
//! on the loop. [`step`] tells the two apart.
//!
//! # Said for as long as it runs
//!
//! A clone is a state, not a gesture: the sentence that says it is running
//! is HELD on the line until the answer replaces it, and a second `/ c`
//! while it runs is told so ([`running`]) instead of finding the folder the
//! first clone already made "occupied". The clone itself gives up on a
//! stalled line (`ronoctua_git::CLONE_LOW_SPEED_S`), so the hold ends with a
//! sentence either way.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// The command (`cmd-dev-clone`, bound to `/ c`).
pub const COMMAND: &str = "dev-clone";

/// What starts an answer the clone's thread sends back: a typed line never
/// carries it.
pub const BACK: char = '\u{0}';

/// The word of an answer that says where the address leads ([`read_back`]).
const AT: &str = "at";
/// The word of an answer that says where the address leads could not be
/// read ([`read_back`]).
const UNREAD: &str = "unread";
/// The word of a clone's answer that says the address leads, from the new
/// repository, elsewhere than the question named ([`run`]): asked again.
const AGAIN: &str = "again";

/// What the command does with what it was given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// Read where the address leads ([`resolve`]) — off the loop — and then
    /// ask.
    Resolve {
        /// The address.
        url: String,
        /// The first answer the question will offer.
        first: String,
    },
    /// Ask the folder, naming the host the answer lets.
    Ask {
        /// The first answer offered.
        first: String,
        /// The host the address LEADS to — the question says it, because
        /// answering lets it.
        host: String,
        /// The host the address itself names, when the person's settings
        /// send it to another (`host`): the question says that too.
        from: Option<String>,
        /// Where the address leads, which the answer is for ([`Asked`]).
        effective: String,
        /// Asked again by a clone that found the address leading elsewhere
        /// from its folder: that clone is over.
        again: bool,
    },
    /// Say this, and do nothing else (a refusal).
    Say(String),
    /// Let the host, mint its pass, and clone.
    Clone(Plan),
    /// The clone ended: say this, and open the folder when it worked.
    Done {
        /// The sentence.
        said: String,
        /// The folder, when the clone made it.
        open: Option<PathBuf>,
    },
}

/// A clone about to start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// The repository's address.
    pub url: String,
    /// Where it leads, as the question that named its host found it.
    pub effective: String,
    /// The host it leads to, which the person lets by answering.
    pub host: String,
    /// Where it lands (absolute).
    pub into: PathBuf,
}

/// THE QUESTION A DOOR ASKED, kept until its answer: the address it was for
/// and where it was found to lead — so the answer lets the host the question
/// named, and no other. The footer keeps it for the session
/// (`git_region::RESIDING`); a question let go forgets it, and an answer
/// takes it.
#[derive(Debug, Default)]
pub struct Asked {
    for_address: Option<(String, String)>,
}

impl Asked {
    /// The question for `address` was asked, naming where it leads.
    pub fn asked(&mut self, address: &str, effective: &str) {
        self.for_address = Some((address.to_owned(), effective.to_owned()));
    }

    /// The answer takes what its question found, when the question was for
    /// `address`.
    pub fn take_for(&mut self, address: Option<&str>) -> Option<String> {
        match self.for_address.take() {
            Some((a, effective)) if Some(a.as_str()) == address => Some(effective),
            _ => None,
        }
    }

    /// The question was let go, unanswered.
    pub fn forget(&mut self) {
        self.for_address = None;
    }
}

/// The repository this build clones, when it was told one.
#[must_use]
pub fn address() -> Option<&'static str> {
    option_env!("RONOCTUA_DISTRO_REPOSITORY").filter(|a| !a.is_empty())
}

/// The folder a clone of this process is filling, while it runs.
static RUNNING: Mutex<Option<PathBuf>> = Mutex::new(None);

/// The folder a clone is filling right now, if one runs.
#[must_use]
pub fn running() -> Option<PathBuf> {
    RUNNING.lock().ok().and_then(|r| r.clone())
}

/// A clone into `into` started.
pub fn started(into: &Path) {
    if let Ok(mut r) = RUNNING.lock() {
        *r = Some(into.to_path_buf());
    }
}

/// The clone ended (its answer came back).
pub fn ended() {
    if let Ok(mut r) = RUNNING.lock() {
        *r = None;
    }
}

/// What `dev-clone` does, given what it was run with (`argument`), the
/// repository's address, the home, the clone already running, if any, and
/// where the question this answers found the address to lead (`asked`, the
/// door's [`Asked`]). Pure but for the folder's being there (a folder that
/// is there and not empty is refused before anything is asked).
#[must_use]
pub fn step(
    argument: Option<&str>,
    address: Option<&str>,
    home: Option<&Path>,
    running: Option<&Path>,
    asked: Option<&str>,
    texts: &ronoctua_i18n::Texts,
) -> Step {
    if let Some(back) = argument.and_then(|a| a.strip_prefix(BACK)) {
        return came_back(back, address, texts);
    }
    if let Some(folder) = running {
        return Step::Say(texts.t_with(
            crate::key::DEV_CLONE_RUNNING,
            &[("folder", &folder.to_string_lossy())],
        ));
    }
    let Some(url) = address.filter(|a| ronoctua_git::host_of(a).is_some()) else {
        return Step::Say(texts.t(crate::key::DEV_CLONE_NO_ADDRESS));
    };
    let Some(typed) = argument else {
        return Step::Resolve {
            url: url.to_owned(),
            first: first_answer(url, home),
        };
    };
    let Some(into) = folder_of(typed, home) else {
        return Step::Say(texts.t(crate::key::DEV_CLONE_NO_FOLDER));
    };
    if occupied(&into) {
        return Step::Say(texts.t_with(
            crate::key::DEV_CLONE_OCCUPIED,
            &[("folder", &into.to_string_lossy())],
        ));
    }
    // AN ANSWER NO QUESTION NAMED A HOST FOR — the command run with its
    // folder — is asked first, the folder offered: the host is let only by
    // answering the question that names it.
    let Some((effective, host)) =
        asked.and_then(|e| ronoctua_git::host_of(e).map(|host| (e, host)))
    else {
        return Step::Resolve {
            url: url.to_owned(),
            first: typed.trim().to_owned(),
        };
    };
    Step::Clone(Plan {
        url: url.to_owned(),
        effective: effective.to_owned(),
        host,
        into,
    })
}

/// Reads where `url` leads — the person's git settings applied
/// (`ronoctua_git::effective_url`) — and gives back the argument the command
/// comes back with ([`read_back`]). Runs git: on a thread, never the loop
/// (a door with no loop, a photograph's, calls it at once — nothing of the
/// network is asked either way).
#[must_use]
pub fn resolve(url: &str, first: &str) -> String {
    let read = ronoctua_git::Tools::from_env()
        .ok_or(ronoctua_git::Refusal {
            why: ronoctua_git::Why::NoGit,
        })
        .and_then(|tools| ronoctua_git::effective_url(&tools, url));
    read_back(read, first)
}

/// The argument a reading of where the address leads comes back with:
/// `BACK at BACK <where> BACK <first>`, or `BACK unread BACK <why>`.
#[must_use]
pub fn read_back(read: Result<String, ronoctua_git::Refusal>, first: &str) -> String {
    match read {
        Ok(effective) => format!("{BACK}{AT}{BACK}{effective}{BACK}{first}"),
        Err(refusal) => format!("{BACK}{UNREAD}{BACK}{}", refusal.why.id()),
    }
}

/// The folder offered first: the home, and the repository's own name.
fn first_answer(url: &str, home: Option<&Path>) -> String {
    let name = url
        .trim_end_matches('/')
        .rsplit(['/', ':'])
        .next()
        .map(|n| n.trim_end_matches(".git"))
        .filter(|n| !n.is_empty())
        .unwrap_or("distro");
    home.map_or_else(
        || name.to_owned(),
        |h| h.join(name).to_string_lossy().into_owned(),
    )
}

/// The folder a typed answer names: `~` and `~/…` are the home, a relative
/// one is under the home. `None` for an empty answer, or a relative one
/// with no home to put it under.
#[must_use]
pub fn folder_of(typed: &str, home: Option<&Path>) -> Option<PathBuf> {
    let typed = typed.trim();
    if typed.is_empty() {
        return None;
    }
    if typed == "~" {
        return home.map(Path::to_path_buf);
    }
    if let Some(rest) = typed.strip_prefix("~/") {
        return home.map(|h| h.join(rest));
    }
    let path = Path::new(typed);
    if path.is_absolute() {
        Some(path.to_path_buf())
    } else {
        home.map(|h| h.join(path))
    }
}

/// Whether something is already at `into`: a file, or a folder with
/// anything in it. An empty folder is where a clone may land.
#[must_use]
pub fn occupied(into: &Path) -> bool {
    match std::fs::symlink_metadata(into) {
        Err(_) => false,
        Ok(meta) if meta.is_dir() => {
            std::fs::read_dir(into).map_or(true, |mut entries| entries.next().is_some())
        }
        Ok(_) => true,
    }
}

/// The person answered the question that names `host`: it is let, and its
/// pass minted — or the sentence that says why not.
///
/// # Errors
///
/// The sentence, when the consent could not be kept or would not mint.
pub fn consent(host: &str, texts: &ronoctua_i18n::Texts) -> Result<ronoctua_consent::Pass, String> {
    if let Err(unkept) = ronoctua_consent::let_hosts(&[host], true) {
        return Err(texts.t(&format!("dev-clone-unkept-{}", unkept.id())));
    }
    ronoctua_consent::pass(host)
        .map_err(|_| texts.t_with(crate::key::DEV_CLONE_NOT_LET, &[("host", host)]))
}

/// The sentence said while the clone runs.
#[must_use]
pub fn cloning(plan: &Plan, texts: &ronoctua_i18n::Texts) -> String {
    texts.t_with(
        crate::key::DEV_CLONE_CLONING,
        &[
            ("folder", &plan.into.to_string_lossy()),
            ("host", &plan.host),
        ],
    )
}

/// Runs the clone (on the thread that called it) and gives back the
/// argument the command comes back with ([`run_in`], with the tools and the
/// person's settings this process has).
#[must_use]
pub fn run(plan: &Plan, pass: &ronoctua_consent::Pass) -> String {
    match ronoctua_git::Tools::from_env() {
        Some(tools) => run_in(&tools, &ronoctua_git::Settings::persons(), plan, pass),
        None => back_of(
            Err(ronoctua_git::Refusal {
                why: ronoctua_git::Why::NoGit,
            }),
            &plan.into,
        ),
    }
}

/// The clone, with these tools and settings (a test's): the repository made
/// and read where its fetch will go — and fetched only when that is the
/// host the question named and the person let. From that folder the
/// person's settings may send the address elsewhere (a file they include
/// only there); then what was made goes, nothing is reached, and the answer
/// asks again, naming that host, the folder offered ([`again_back`]).
#[must_use]
pub fn run_in(
    tools: &ronoctua_git::Tools,
    settings: &ronoctua_git::Settings,
    plan: &Plan,
    pass: &ronoctua_consent::Pass,
) -> String {
    let prepared = match ronoctua_git::Repo::prepare_clone(tools, settings, &plan.url, &plan.into) {
        Ok(prepared) => prepared,
        Err(refusal) => return back_of(Err(refusal), &plan.into),
    };
    if prepared.host().as_deref() != Some(plan.host.as_str()) {
        let effective = prepared.effective().to_owned();
        prepared.abandon();
        return again_back(&effective, &plan.into);
    }
    back_of(prepared.fetch(pass).map(|_| ()), &plan.into)
}

/// The argument a clone comes back with when, from its folder, the address
/// leads to `effective`, which its question did not name:
/// `BACK again BACK <where> BACK <folder>` — asked again.
#[must_use]
pub fn again_back(effective: &str, into: &Path) -> String {
    format!(
        "{BACK}{AGAIN}{BACK}{effective}{BACK}{}",
        into.to_string_lossy()
    )
}

/// The argument a finished clone comes back with: `BACK ok BACK <folder>`,
/// or `BACK <why> BACK <folder>`.
#[must_use]
pub fn back_of(outcome: Result<(), ronoctua_git::Refusal>, into: &Path) -> String {
    let word = match outcome {
        Ok(()) => "ok",
        Err(refusal) => refusal.why.id(),
    };
    format!("{BACK}{word}{BACK}{}", into.to_string_lossy())
}

/// What a thread's answer says: where the address leads (then the question),
/// that it could not be read, or how the clone ended.
fn came_back(back: &str, address: Option<&str>, texts: &ronoctua_i18n::Texts) -> Step {
    let Some((word, rest)) = back.split_once(BACK) else {
        return Step::Say(texts.t(crate::key::DEV_CLONE_NO_FOLDER));
    };
    match word {
        AT | AGAIN => {
            let Some((effective, first)) = rest.split_once(BACK) else {
                return Step::Say(texts.t(crate::key::DEV_CLONE_NO_FOLDER));
            };
            return asked_of(effective, first, address, word == AGAIN, texts);
        }
        // ITS OWN SENTENCE: what git could not do is read the person's
        // settings, which no sentence of the history's browser says.
        UNREAD => {
            tracing::warn!(
                why = rest,
                "where the distro's address leads could not be read"
            );
            return Step::Say(texts.t(crate::key::DEV_CLONE_UNREAD));
        }
        _ => {}
    }
    let folder = rest;
    let into = PathBuf::from(folder);
    let shown = into.to_string_lossy().into_owned();
    if word == "ok" {
        return Step::Done {
            said: texts.t_with(crate::key::DEV_CLONE_DONE, &[("folder", &shown)]),
            open: Some(into),
        };
    }
    let why = ronoctua_git::Why::ALL
        .into_iter()
        .find(|w| w.id() == word)
        .unwrap_or(ronoctua_git::Why::Failed);
    let reason = crate::git_line::why_said(why, texts);
    Step::Done {
        said: texts.t_with(
            crate::key::DEV_CLONE_FAILED,
            &[("folder", &shown), ("why", &reason)],
        ),
        open: None,
    }
}

/// The question for an address found to lead to `effective` — the host it
/// leads to, and the address's own when the two differ — or the sentence
/// when it leads to no host at all (a path).
fn asked_of(
    effective: &str,
    first: &str,
    address: Option<&str>,
    again: bool,
    texts: &ronoctua_i18n::Texts,
) -> Step {
    let Some(host) = ronoctua_git::host_of(effective) else {
        return Step::Say(texts.t_with(crate::key::DEV_CLONE_NO_HOST, &[("to", effective)]));
    };
    let from = address
        .and_then(ronoctua_git::host_of)
        .filter(|named| *named != host);
    Step::Ask {
        first: first.to_owned(),
        host,
        from,
        effective: effective.to_owned(),
        again,
    }
}

/// The sentence held while git reads where the address leads (a live door's
/// thread; a photograph's reads at once).
#[must_use]
pub fn reading(texts: &ronoctua_i18n::Texts) -> String {
    texts.t(crate::key::DEV_CLONE_READING)
}

/// The question an [`Step::Ask`] asks, on either door's line: "Clone from
/// <host> into", and when the settings send the address elsewhere, the one
/// that names the address's own host beside it
/// ([`crate::key::DEV_CLONE_ELSEWHERE`]) — short, so the line keeps its keys
/// at 1280 pixels.
#[must_use]
pub fn question(first: &str, host: String, from: Option<String>) -> crate::line::Line {
    let mut line = crate::line::Line::new(COMMAND, false, Some(first));
    line.asks_with = vec![("host", host)];
    if let Some(from) = from {
        line.asks = Some(crate::key::DEV_CLONE_ELSEWHERE);
        line.asks_with.push(("from", from));
    }
    line
}

/// What this crate adds to the leader's keymap ([`crate::bindings::Contribution`]):
/// its commands, the sequences and groups that reach them, the ones only
/// `LEADER :` reaches by name, and what the view's own keys and rows run.
/// The binary assembles every crate's (its `keymap`); the footer's own are
/// its `leader::BASE`.
pub const KEYS: crate::bindings::Contribution = crate::bindings::Contribution {
    owner: "git",
    commands: &[ronoctua_cmd::Command {
        id: "dev-clone",
        label: crate::key::CMD_DEV_CLONE,
    }],
    bindings: &[("/ c", "dev-clone", None)],
    ..crate::bindings::Contribution::NONE
};

#[cfg(test)]
mod tests {
    use super::*;

    const URL: &str = "https://github.com/ronoctua/ronoctua-distro.git";
    /// Where the person's settings send [`URL`] when they keep its host.
    const BY_SSH: &str = "git@github.com:ronoctua/ronoctua-distro.git";
    /// …and when they send it to another host.
    const ELSEWHERE: &str = "https://mirror.example.org/github/ronoctua/ronoctua-distro.git";

    fn texts(language: &str) -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh(language).unwrap()
    }

    fn place(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-dev-clone-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// What `/ c` does once git said where [`URL`] leads.
    fn asked_when_it_leads_to(effective: &str, t: &ronoctua_i18n::Texts) -> Step {
        let back = read_back(Ok(effective.to_owned()), "/home/ada/ronoctua-distro");
        step(Some(&back), Some(URL), None, None, None, t)
    }

    #[test]
    fn it_reads_where_the_address_leads_first_and_offers_the_home() {
        let t = texts("en-GB");
        let home = Path::new("/home/ada");
        assert_eq!(
            step(None, Some(URL), Some(home), None, None, &t),
            Step::Resolve {
                url: URL.to_owned(),
                first: "/home/ada/ronoctua-distro".to_owned(),
            }
        );
        assert_eq!(
            step(
                None,
                Some("git@example.org:a/b"),
                Some(home),
                None,
                None,
                &t
            ),
            Step::Resolve {
                url: "git@example.org:a/b".to_owned(),
                first: "/home/ada/b".to_owned(),
            }
        );
    }

    /// FAILS IF: the question stops naming the host the address LEADS to —
    /// the three cases a person's git settings make: no rewrite and the
    /// common https → ssh one ask the address's host as ever, with the very
    /// question of before; one that sends the address to another host asks
    /// THAT host and says the settings send it there; one that sends it to a
    /// path, which no consent covers, is said and nothing asked; and where it
    /// leads that could not be read is said, not asked.
    #[test]
    fn the_question_names_the_host_the_address_leads_to() {
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            let as_ever = t.t_with(crate::key::ASK_DEV_CLONE, &[("host", "github.com")]);
            for effective in [URL, BY_SSH] {
                let Step::Ask {
                    first,
                    host,
                    from,
                    effective: found,
                    again: false,
                } = asked_when_it_leads_to(effective, &t)
                else {
                    panic!("{effective} was not asked");
                };
                assert_eq!((host.as_str(), from.as_deref()), ("github.com", None));
                assert_eq!(found, effective);
                assert_eq!(first, "/home/ada/ronoctua-distro");
                assert_eq!(question(&first, host, from).question(&t), as_ever);
            }
            let Step::Ask {
                first, host, from, ..
            } = asked_when_it_leads_to(ELSEWHERE, &t)
            else {
                panic!("{ELSEWHERE} was not asked");
            };
            assert_eq!(
                (host.as_str(), from.as_deref()),
                ("mirror.example.org", Some("github.com"))
            );
            let asked = question(&first, host, from).question(&t);
            assert!(asked.contains("mirror.example.org"), "{language}: {asked}");
            assert!(asked.contains("github.com"), "{language}: {asked}");
            assert_ne!(asked, as_ever);
            // A PATH, or A REMOTE HELPER: no host to ask, and never one named
            // after the helper.
            for nowhere in [
                "/srv/mirror/distro.git",
                "gcrypt::git@example.org:distro.git",
            ] {
                let Step::Say(said) = asked_when_it_leads_to(nowhere, &t) else {
                    panic!("{nowhere} was asked");
                };
                assert!(said.contains(nowhere), "{said}");
            }
            // NOT READ: said in its own words, never the history's.
            let back = read_back(
                Err(ronoctua_git::Refusal {
                    why: ronoctua_git::Why::Failed,
                }),
                "/home/ada/x",
            );
            assert_eq!(
                step(Some(&back), Some(URL), None, None, None, &t),
                Step::Say(t.t(crate::key::DEV_CLONE_UNREAD))
            );
        }
    }

    #[test]
    fn a_build_with_no_address_says_so_and_asks_nothing() {
        let t = texts("en-GB");
        for address in [None, Some("/srv/not-a-host.git")] {
            let Step::Say(said) = step(None, address, Some(Path::new("/h")), None, None, &t) else {
                panic!("{address:?} did not refuse");
            };
            assert_eq!(said, t.t(crate::key::DEV_CLONE_NO_ADDRESS));
        }
    }

    #[test]
    fn the_answer_is_a_folder_under_the_home() {
        let home = Path::new("/home/ada");
        for (typed, folder) in [
            ("~", Some("/home/ada")),
            ("~/work/distro", Some("/home/ada/work/distro")),
            ("work/distro", Some("/home/ada/work/distro")),
            ("  /srv/distro  ", Some("/srv/distro")),
            ("", None),
            ("   ", None),
        ] {
            assert_eq!(
                folder_of(typed, Some(home)),
                folder.map(PathBuf::from),
                "{typed:?}"
            );
        }
        assert_eq!(folder_of("work", None), None);
        assert_eq!(folder_of("/srv/a", None), Some(PathBuf::from("/srv/a")));
    }

    /// FAILS IF: an answer lets another host than the one its question named
    /// — the address's own when the question named where it leads — or an
    /// answer no question named a host for (the command run with its folder)
    /// clones instead of being asked first, the folder offered.
    #[test]
    fn an_answer_plans_the_clone_of_the_host_its_question_named() {
        let t = texts("en-GB");
        let dir = place("plan");
        for (asked, host) in [
            (URL, "github.com"),
            (BY_SSH, "github.com"),
            (ELSEWHERE, "mirror.example.org"),
        ] {
            assert_eq!(
                step(
                    Some("fresh/distro"),
                    Some(URL),
                    Some(&dir),
                    None,
                    Some(asked),
                    &t
                ),
                Step::Clone(Plan {
                    url: URL.to_owned(),
                    effective: asked.to_owned(),
                    host: host.to_owned(),
                    into: dir.join("fresh/distro"),
                })
            );
        }
        assert_eq!(
            step(
                Some(" fresh/distro "),
                Some(URL),
                Some(&dir),
                None,
                None,
                &t
            ),
            Step::Resolve {
                url: URL.to_owned(),
                first: "fresh/distro".to_owned(),
            }
        );
        // An EMPTY folder is where a clone may land.
        std::fs::create_dir_all(dir.join("empty")).unwrap();
        assert!(matches!(
            super::step(Some("empty"), Some(URL), Some(&dir), None, Some(URL), &t),
            Step::Clone(_)
        ));
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: the answer a door keeps is taken for another address, or
    /// twice, or outlives a question let go.
    #[test]
    fn what_a_question_found_is_its_answers_once() {
        let mut asked = Asked::default();
        asked.asked(URL, ELSEWHERE);
        assert_eq!(asked.take_for(Some("https://example.org/fork.git")), None);
        asked.asked(URL, ELSEWHERE);
        assert_eq!(asked.take_for(Some(URL)).as_deref(), Some(ELSEWHERE));
        assert_eq!(asked.take_for(Some(URL)), None);
        asked.asked(URL, ELSEWHERE);
        asked.forget();
        assert_eq!(asked.take_for(Some(URL)), None);
    }

    #[test]
    fn a_folder_that_is_there_is_refused_before_anything_is_asked() {
        let t = texts("en-GB");
        let dir = place("occupied");
        std::fs::create_dir_all(dir.join("full")).unwrap();
        std::fs::write(dir.join("full/kept.txt"), "mine").unwrap();
        std::fs::write(dir.join("a-file"), "mine").unwrap();
        for typed in ["full", "a-file"] {
            let Step::Say(said) = step(Some(typed), Some(URL), Some(&dir), None, Some(URL), &t)
            else {
                panic!("{typed} was not refused");
            };
            assert!(
                said.contains(&dir.join(typed).to_string_lossy().into_owned()),
                "{said}"
            );
        }
        assert_eq!(
            std::fs::read_to_string(dir.join("full/kept.txt")).unwrap(),
            "mine"
        );
        let Step::Say(said) = step(Some("  "), Some(URL), Some(&dir), None, Some(URL), &t) else {
            panic!("an empty answer was not refused");
        };
        assert_eq!(said, t.t(crate::key::DEV_CLONE_NO_FOLDER));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn what_comes_back_is_said_and_only_a_clone_opens() {
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            let into = Path::new("/home/ada/ronoctua-distro");
            let ok = back_of(Ok(()), into);
            // The answer comes back while the clone is still marked as
            // running: it is the answer, not a second clone.
            let Step::Done { said, open } = step(Some(&ok), Some(URL), None, Some(into), None, &t)
            else {
                panic!("the clone's answer was taken for a folder");
            };
            assert_eq!(open.as_deref(), Some(into));
            assert!(said.contains("/home/ada/ronoctua-distro"), "{said}");
            for why in ronoctua_git::Why::ALL {
                let back = back_of(Err(ronoctua_git::Refusal { why }), into);
                let Step::Done { said, open } = step(Some(&back), None, None, None, None, &t)
                else {
                    panic!("{why:?} was taken for a folder");
                };
                assert_eq!(open, None);
                assert!(said.contains(&crate::git_line::why_said(why, &t)), "{said}");
                assert!(!said.contains("dev-clone-"), "{said}");
            }
        }
    }

    /// FAILS IF: a clone that found, from its folder, the address leading to
    /// a host its question did not name does not ask again — naming that
    /// host, the folder offered, the clone over — or opens a folder.
    #[test]
    fn a_clone_asked_elsewhere_from_its_folder_asks_again() {
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            let into = Path::new("/home/ada/work/ronoctua-distro");
            let back = again_back("ssh://git@forge.example.net/distro.git", into);
            assert_eq!(
                step(Some(&back), Some(URL), None, Some(into), None, &t),
                Step::Ask {
                    first: "/home/ada/work/ronoctua-distro".to_owned(),
                    host: "forge.example.net".to_owned(),
                    from: Some("github.com".to_owned()),
                    effective: "ssh://git@forge.example.net/distro.git".to_owned(),
                    again: true,
                }
            );
        }
    }

    /// A home of a test's own, its `.gitconfig` saying `config`, and the
    /// settings that read it.
    fn planted(dir: &Path, config: &str) -> ronoctua_git::Settings {
        let home = dir.join("home");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(home.join(".gitconfig"), config).unwrap();
        ronoctua_git::Settings::planted(&home, Path::new("/dev/null"))
    }

    /// A pass for `host`, over a consent file of the test's own.
    fn pass_for(dir: &Path, host: &str) -> ronoctua_consent::Pass {
        let store = ronoctua_consent::store_at(Some(dir.join("consent")));
        ronoctua_consent::let_hosts_in(&store, &[host], true).unwrap();
        ronoctua_consent::pass_in(&store, host).unwrap()
    }

    /// FAILS IF: the clone, with real git and the person's settings planted,
    /// fetches when its folder sends the address to another host than the
    /// question named — instead of asking again, naming that host, with
    /// nothing reached and nothing left — or does not go on to the fetch when
    /// the folder sends it where the question said (here a port nobody
    /// answers: the fetch is tried, fails, and what was made goes).
    #[test]
    fn the_clone_reads_its_folder_and_asks_again_when_it_leads_elsewhere() {
        let tools = ronoctua_git::Tools::from_env()
            .expect("RONOCTUA_GIT is not set: run inside `nix develop` or the tests derivation");
        let t = texts("en-GB");
        let dir = place("run");
        let closed = {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            listener.local_addr().unwrap().port()
        };
        let address = "git://distro.example.invalid/distro.git";
        let work = dir.join("work");
        let into = work.join("distro");
        let only_there = dir.join("work.gitconfig");
        std::fs::write(
            &only_there,
            format!("[url \"git://127.0.0.2:{closed}/\"]\n\tinsteadOf = {address}\n"),
        )
        .unwrap();
        let settings = planted(
            &dir,
            &format!(
                "[url \"git://127.0.0.1:{closed}/\"]\n\tinsteadOf = git://distro.example.invalid/\n\
                 [includeIf \"gitdir:{}/\"]\n\tpath = {}\n",
                work.display(),
                only_there.display()
            ),
        );
        // THE QUESTION, read from `/`: the served host.
        let asked = ronoctua_git::effective_url_in(&tools, &settings, address).unwrap();
        let plan = Plan {
            url: address.to_owned(),
            effective: asked,
            host: "127.0.0.1".to_owned(),
            into: into.clone(),
        };
        let pass = pass_for(&dir, "127.0.0.1");
        let back = run_in(&tools, &settings, &plan, &pass);
        let Step::Ask {
            host, first, again, ..
        } = step(Some(&back), Some(address), None, Some(&into), None, &t)
        else {
            panic!("{back:?} did not ask again");
        };
        assert_eq!((host.as_str(), again), ("127.0.0.2", true));
        assert_eq!(first, into.to_string_lossy());
        assert!(!into.exists(), "what the clone made was left");
        // ELSEWHERE, where the question said: the fetch is tried.
        let elsewhere = dir.join("elsewhere");
        let plan = Plan {
            into: elsewhere.clone(),
            ..plan
        };
        let back = run_in(&tools, &settings, &plan, &pass);
        assert_eq!(
            back,
            back_of(
                Err(ronoctua_git::Refusal {
                    why: ronoctua_git::Why::Unreachable
                }),
                &elsewhere
            )
        );
        assert!(!elsewhere.exists(), "a failed clone left its folder");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: a second `/ c` while a clone runs asks again, or finds the
    /// folder the first clone made "occupied" — instead of saying the clone
    /// is still running.
    #[test]
    fn a_second_clone_while_one_runs_is_told_so() {
        let t = texts("en-GB");
        let into = Path::new("/home/ada/ronoctua-distro");
        for argument in [None, Some("/home/ada/ronoctua-distro"), Some("elsewhere")] {
            let Step::Say(said) = step(
                argument,
                Some(URL),
                Some(Path::new("/home/ada")),
                Some(into),
                Some(URL),
                &t,
            ) else {
                panic!("{argument:?} was not refused while a clone runs");
            };
            assert_eq!(
                said,
                t.t_with(
                    crate::key::DEV_CLONE_RUNNING,
                    &[("folder", "/home/ada/ronoctua-distro")]
                )
            );
        }
    }

    #[test]
    fn every_sentence_is_in_both_languages() {
        let plan = Plan {
            url: URL.to_owned(),
            effective: URL.to_owned(),
            host: "github.com".to_owned(),
            into: PathBuf::from("/home/ada/ronoctua-distro"),
        };
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            let said = cloning(&plan, &t);
            assert!(
                said.contains("github.com") && said.contains("/home/ada"),
                "{said}"
            );
            assert!(
                said.matches("github.com").count() >= 2,
                "the consent is not said: {said}"
            );
            for key in [
                crate::key::ASK_DEV_CLONE,
                crate::key::DEV_CLONE_RUNNING,
                crate::key::DEV_CLONE_NO_ADDRESS,
                crate::key::DEV_CLONE_NO_FOLDER,
                crate::key::DEV_CLONE_UNHEARD,
                crate::key::DEV_CLONE_ELSEWHERE,
                crate::key::DEV_CLONE_UNREAD,
                crate::key::DEV_CLONE_READING,
                crate::key::DEV_CLONE_NO_HOST,
            ] {
                assert!(!t.t(key).contains(key), "{language}: {key}");
            }
            for unkept in [
                ronoctua_consent::Unkept::NoHome,
                ronoctua_consent::Unkept::Unparseable,
                ronoctua_consent::Unkept::Unwritable,
            ] {
                let key = format!("dev-clone-unkept-{}", unkept.id());
                assert!(!t.t(&key).contains(&key), "{language}: {key}");
            }
        }
    }
}
