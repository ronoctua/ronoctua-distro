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

/// What the command does with what it was given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// Ask the folder, naming the host the answer lets.
    Ask {
        /// The first answer offered.
        first: String,
        /// The host the address names — the question says it, because
        /// answering lets it.
        host: String,
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
    /// The host it names, which the person lets by answering.
    pub host: String,
    /// Where it lands (absolute).
    pub into: PathBuf,
}

/// The repository this build clones, when it was told one.
#[must_use]
pub fn address() -> Option<&'static str> {
    option_env!("RONOCTUA_DISTRO_REPOSITORY").filter(|a| !a.is_empty())
}

/// How long the "cloning" sentence is held on the line: longer than any
/// clone that is still moving, so it is the answer — or another message —
/// that takes it off, never a deadline. (Bounded only because an `Instant`
/// cannot be pushed to infinity.)
pub const HELD: std::time::Duration = std::time::Duration::from_secs(24 * 60 * 60);

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
/// repository's address, the home and the clone already running, if any.
/// Pure but for the folder's being there (a folder that is there and not
/// empty is refused before anything is asked).
#[must_use]
pub fn step(
    argument: Option<&str>,
    address: Option<&str>,
    home: Option<&Path>,
    running: Option<&Path>,
    texts: &ronoctua_i18n::Texts,
) -> Step {
    if let Some(back) = argument.and_then(|a| a.strip_prefix(BACK)) {
        return came_back(back, texts);
    }
    if let Some(folder) = running {
        return Step::Say(texts.t_with(
            ronoctua_i18n::key::DEV_CLONE_RUNNING,
            &[("folder", &folder.to_string_lossy())],
        ));
    }
    let Some(url) = address else {
        return Step::Say(texts.t(ronoctua_i18n::key::DEV_CLONE_NO_ADDRESS));
    };
    let Some(host) = ronoctua_git::host_of(url) else {
        return Step::Say(texts.t(ronoctua_i18n::key::DEV_CLONE_NO_ADDRESS));
    };
    let Some(typed) = argument else {
        return Step::Ask {
            first: first_answer(url, home),
            host,
        };
    };
    let Some(into) = folder_of(typed, home) else {
        return Step::Say(texts.t(ronoctua_i18n::key::DEV_CLONE_NO_FOLDER));
    };
    if occupied(&into) {
        return Step::Say(texts.t_with(
            ronoctua_i18n::key::DEV_CLONE_OCCUPIED,
            &[("folder", &into.to_string_lossy())],
        ));
    }
    Step::Clone(Plan {
        url: url.to_owned(),
        host,
        into,
    })
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
        .map_err(|_| texts.t_with(ronoctua_i18n::key::DEV_CLONE_NOT_LET, &[("host", host)]))
}

/// The sentence said while the clone runs.
#[must_use]
pub fn cloning(plan: &Plan, texts: &ronoctua_i18n::Texts) -> String {
    texts.t_with(
        ronoctua_i18n::key::DEV_CLONE_CLONING,
        &[
            ("folder", &plan.into.to_string_lossy()),
            ("host", &plan.host),
        ],
    )
}

/// Runs the clone (on the thread that called it) and gives back the
/// argument the command comes back with.
#[must_use]
pub fn run(plan: &Plan, pass: &ronoctua_consent::Pass) -> String {
    let outcome = ronoctua_git::Repo::clone(pass, &plan.url, &plan.into).map(|_| ());
    back_of(outcome, &plan.into)
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

/// What a finished clone's answer says.
fn came_back(back: &str, texts: &ronoctua_i18n::Texts) -> Step {
    let Some((word, folder)) = back.split_once(BACK) else {
        return Step::Say(texts.t(ronoctua_i18n::key::DEV_CLONE_NO_FOLDER));
    };
    let into = PathBuf::from(folder);
    let shown = into.to_string_lossy().into_owned();
    if word == "ok" {
        return Step::Done {
            said: texts.t_with(ronoctua_i18n::key::DEV_CLONE_DONE, &[("folder", &shown)]),
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
            ronoctua_i18n::key::DEV_CLONE_FAILED,
            &[("folder", &shown), ("why", &reason)],
        ),
        open: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const URL: &str = "https://github.com/ronoctua/ronoctua-distro.git";

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

    #[test]
    fn it_asks_where_first_and_offers_the_home() {
        let t = texts("en-GB");
        let home = Path::new("/home/ada");
        assert_eq!(
            step(None, Some(URL), Some(home), None, &t),
            Step::Ask {
                first: "/home/ada/ronoctua-distro".to_owned(),
                host: "github.com".to_owned(),
            }
        );
        assert_eq!(
            step(None, Some("git@example.org:a/b"), Some(home), None, &t),
            Step::Ask {
                first: "/home/ada/b".to_owned(),
                host: "example.org".to_owned(),
            }
        );
    }

    #[test]
    fn a_build_with_no_address_says_so_and_asks_nothing() {
        let t = texts("en-GB");
        for address in [None, Some("/srv/not-a-host.git")] {
            let Step::Say(said) = step(None, address, Some(Path::new("/h")), None, &t) else {
                panic!("{address:?} did not refuse");
            };
            assert_eq!(said, t.t(ronoctua_i18n::key::DEV_CLONE_NO_ADDRESS));
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

    #[test]
    fn an_answer_plans_the_clone_of_the_host_the_address_names() {
        let t = texts("en-GB");
        let dir = place("plan");
        let step = step(Some("fresh/distro"), Some(URL), Some(&dir), None, &t);
        assert_eq!(
            step,
            Step::Clone(Plan {
                url: URL.to_owned(),
                host: "github.com".to_owned(),
                into: dir.join("fresh/distro"),
            })
        );
        // An EMPTY folder is where a clone may land.
        std::fs::create_dir_all(dir.join("empty")).unwrap();
        assert!(matches!(
            super::step(Some("empty"), Some(URL), Some(&dir), None, &t),
            Step::Clone(_)
        ));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_folder_that_is_there_is_refused_before_anything_is_asked() {
        let t = texts("en-GB");
        let dir = place("occupied");
        std::fs::create_dir_all(dir.join("full")).unwrap();
        std::fs::write(dir.join("full/kept.txt"), "mine").unwrap();
        std::fs::write(dir.join("a-file"), "mine").unwrap();
        for typed in ["full", "a-file"] {
            let Step::Say(said) = step(Some(typed), Some(URL), Some(&dir), None, &t) else {
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
        let Step::Say(said) = step(Some("  "), Some(URL), Some(&dir), None, &t) else {
            panic!("an empty answer was not refused");
        };
        assert_eq!(said, t.t(ronoctua_i18n::key::DEV_CLONE_NO_FOLDER));
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
            let Step::Done { said, open } = step(Some(&ok), Some(URL), None, Some(into), &t) else {
                panic!("the clone's answer was taken for a folder");
            };
            assert_eq!(open.as_deref(), Some(into));
            assert!(said.contains("/home/ada/ronoctua-distro"), "{said}");
            for why in ronoctua_git::Why::ALL {
                let back = back_of(Err(ronoctua_git::Refusal { why }), into);
                let Step::Done { said, open } = step(Some(&back), None, None, None, &t) else {
                    panic!("{why:?} was taken for a folder");
                };
                assert_eq!(open, None);
                assert!(said.contains(&crate::git_line::why_said(why, &t)), "{said}");
                assert!(!said.contains("dev-clone-"), "{said}");
            }
        }
    }

    /// FAILS IF: the question stops naming the host the address names —
    /// answering it lets that host, so a word written in the key (a fork's
    /// address asked as "GitHub") would be consent to something not said.
    #[test]
    fn the_question_names_the_host_it_lets() {
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            let Step::Ask { first, host } = step(
                None,
                Some("https://forge.example.org/a/distro.git"),
                Some(Path::new("/home/ada")),
                None,
                &t,
            ) else {
                panic!("it did not ask");
            };
            let mut line = crate::line::Line::new(COMMAND, false, Some(&first));
            line.asks_with = vec![("host", host)];
            let asked = line.question(&t);
            assert!(asked.contains("forge.example.org"), "{language}: {asked}");
            assert!(!asked.contains("GitHub"), "{language}: {asked}");
        }
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
                &t,
            ) else {
                panic!("{argument:?} was not refused while a clone runs");
            };
            assert_eq!(
                said,
                t.t_with(
                    ronoctua_i18n::key::DEV_CLONE_RUNNING,
                    &[("folder", "/home/ada/ronoctua-distro")]
                )
            );
        }
    }

    #[test]
    fn every_sentence_is_in_both_languages() {
        let plan = Plan {
            url: URL.to_owned(),
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
                ronoctua_i18n::key::ASK_DEV_CLONE,
                ronoctua_i18n::key::DEV_CLONE_RUNNING,
                ronoctua_i18n::key::DEV_CLONE_NO_ADDRESS,
                ronoctua_i18n::key::DEV_CLONE_NO_FOLDER,
                ronoctua_i18n::key::DEV_CLONE_UNHEARD,
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
