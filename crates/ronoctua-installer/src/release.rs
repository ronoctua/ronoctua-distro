//! The installer follows the `release` branch.
//!
//! # Why an installer asks the release before installing
//!
//! A machine follows the release: its flake's `ronoctua` input names the
//! `release` branch (`templates/machine/flake.nix`), and every update moves
//! it there. An ISO is a photograph of the distro on the day it was built —
//! its installer, its belongings CLI, its plan. Installing with an ISO from
//! three releases ago would make the new machine by the rules of three
//! releases ago (a question it no longer needs, a step that changed) and only
//! catch up at its first update. So, right after the network question, the
//! installer asks the release which revision it is at: when it is not the
//! ISO's own, the installer of THAT revision is started in its place
//! (`nix run <release>#install -- --from-release <rev> …`), with the answers
//! given so far, and that one never asks again (`--from-release`).
//!
//! # Only a NEWER release is followed
//!
//! The release's installer replaces this one only when the release was made
//! AFTER this installer's revision. An ISO built from a commit ahead of the
//! release — the way a new installer is tried before `.#release` publishes
//! it — keeps its own installer: following the release there would put an
//! older installer in the place of the one being tried. A release that went
//! BACK (`.#release --rollback-to`) is not recognised as such and is not
//! followed; following a rollback is the owner's to decide, and would need
//! to be told apart from an ISO that is simply ahead.
//!
//! # When it stays
//!
//! - the release IS this installer's revision — nothing to do;
//! - this installer was itself started from the release (`--from-release`);
//! - this installer is NEWER than the release (its tree is ahead of it);
//! - this installer is a DEVELOPMENT build (a dirty tree, or no revision at
//!   all — a test's node, the `.#dev` loop's ISO): what it carries is what is
//!   being tried, and following the release would install something else;
//! - the release cannot be reached (GitHub down, its answers rate-limited,
//!   no release published yet).
//!
//! In the last three the machine is installed from THIS installer's own
//! distro, and keeps it: its flake stays aimed at that tree, the flake and
//! its lock agreeing (`crate::plan::Origin`). The screen says which of the
//! three it was ([`Stay::key`]).
//!
//! The decision is [`follow`], pure: `installer-follows-release` runs its
//! test. Who asks the release is `crate::reach`, through `nix` — the distro's
//! flake reference is the template's, never spelled here (invariant 16).

use crate::key;
use serde::Deserialize;

/// The variable naming where the ISO keeps the distro's tree it was built
/// from (`nix/installer/iso.nix`).
pub const FLAKE_DIR: &str = "RONOCTUA_FLAKE_DIR";
/// The variable naming that tree's git revision (`<rev>`, `<rev>-dirty`).
pub const FLAKE_REV: &str = "RONOCTUA_FLAKE_REV";
/// The variable naming when that revision was made, in seconds since the
/// epoch (the flake's `lastModified`).
pub const FLAKE_LAST_MODIFIED: &str = "RONOCTUA_FLAKE_LAST_MODIFIED";

/// Where the machine's template is, under a distro's tree.
pub const TEMPLATE: &str = "templates/machine";

/// The template's flake file, under [`TEMPLATE`].
pub const TEMPLATE_FLAKE: &str = "flake.nix";

/// The flake attribute a machine's template reads its distro from — the
/// input the update moves.
pub const DISTRO_INPUT: &str = "ronoctua";

/// The app a release installs with (`apps.install` in `flake.nix`).
pub const INSTALL_APP: &str = "install";

/// The flags a hop hands the release's installer.
pub const FROM_RELEASE: &str = "--from-release";
/// See [`FROM_RELEASE`]: where the release's tree is.
pub const DISTRO: &str = "--distro";
/// See [`FROM_RELEASE`]: the answers given so far, in a `0600` file.
pub const ANSWERS: &str = "--answers";

/// A revision of the distro: a git commit, and when it was made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Revision {
    /// The commit, as `git` writes it; `-dirty` for a tree with changes.
    pub rev: String,
    /// When it was made, seconds since the epoch.
    pub last_modified_s: i64,
}

/// What `nix flake metadata --json <release>` said of the release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    /// Its revision.
    pub revision: Revision,
    /// The locked flake reference (`github:…/<rev>?narHash=…`): what the
    /// hop runs, with no second lookup of the branch.
    pub url: String,
    /// Its tree, already fetched into the store.
    pub path: String,
}

/// Why the installer goes on with its own distro.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stay {
    /// The release is this installer's revision.
    Current,
    /// This installer was started from the release.
    FromRelease,
    /// This installer is newer than the release: its tree is ahead of it.
    Ahead,
    /// This installer is a development build.
    Development,
    /// The release did not answer.
    Unreachable,
}

impl Stay {
    /// Whether the machine's flake is pointed at this installer's distro —
    /// and stays so: what it carries is not the release's, or the release
    /// cannot be fetched.
    #[must_use]
    pub const fn aims_at_own(self) -> bool {
        matches!(self, Self::Ahead | Self::Development | Self::Unreachable)
    }

    /// The sentence the person reads about it, when there is one to read.
    #[must_use]
    pub const fn key(self) -> Option<&'static str> {
        match self {
            Self::Current | Self::FromRelease => None,
            Self::Ahead => Some(key::INST_RELEASE_AHEAD),
            Self::Development => Some(key::INST_RELEASE_DEVELOPMENT),
            Self::Unreachable => Some(key::INST_RELEASE_UNREACHABLE),
        }
    }

    /// Its id, for the terminal-less mode's English line.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::FromRelease => "from-release",
            Self::Ahead => "ahead",
            Self::Development => "development",
            Self::Unreachable => "unreachable",
        }
    }
}

/// The release's installer to start instead of this one — always a newer
/// one ([`follow`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hop {
    /// The release.
    pub release: Release,
}

impl Hop {
    /// The sentence the person reads before the hop.
    #[must_use]
    pub const fn key(&self) -> &'static str {
        key::INST_RELEASE_NEWER
    }

    /// The revision, short, as the sentence shows it.
    #[must_use]
    pub fn short(&self) -> &str {
        short(&self.release.revision.rev)
    }
}

/// What the installer does about the release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Follow {
    /// It goes on, and why.
    Stay(Stay),
    /// It starts the release's installer.
    Hop(Hop),
}

/// The first twelve characters of a revision — what the screen shows.
#[must_use]
pub fn short(rev: &str) -> &str {
    rev.get(..12).unwrap_or(rev)
}

/// Whether a revision is a DEVELOPMENT build's: none at all, `-dirty`, or
/// anything that is not a full git commit. Pure.
#[must_use]
pub fn is_development(rev: &str) -> bool {
    !(rev.len() == 40 && rev.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// What to do, given this installer's revision (`None`: the ISO did not
/// say), whether it was started from the release, and what the release
/// answered (`None`: it did not). Pure — `installer-follows-release`.
#[must_use]
pub fn follow(own: Option<&Revision>, from_release: bool, release: Option<Release>) -> Follow {
    if from_release {
        return Follow::Stay(Stay::FromRelease);
    }
    let Some(own) = own.filter(|o| !is_development(&o.rev)) else {
        return Follow::Stay(Stay::Development);
    };
    let Some(release) = release.filter(|r| !is_development(&r.revision.rev)) else {
        return Follow::Stay(Stay::Unreachable);
    };
    if release.revision.rev == own.rev {
        return Follow::Stay(Stay::Current);
    }
    // NEWER, strictly: made after this installer's revision. Another
    // revision that is not newer is one this installer is ahead of.
    if release.revision.last_modified_s > own.last_modified_s {
        Follow::Hop(Hop { release })
    } else {
        Follow::Stay(Stay::Ahead)
    }
}

/// This installer's own revision, from the variables the ISO set; `None`
/// when it set none.
#[must_use]
pub fn own_from(rev: Option<String>, last_modified: Option<String>) -> Option<Revision> {
    let rev = rev.filter(|r| !r.is_empty())?;
    let last_modified_s = last_modified
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);
    Some(Revision {
        rev,
        last_modified_s,
    })
}

/// The answer of `nix flake metadata --json` — the parts of it read here.
#[derive(Debug, Deserialize)]
struct Metadata {
    #[serde(default)]
    locked: Locked,
    #[serde(rename = "lastModified", default)]
    last_modified: Option<i64>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    path: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct Locked {
    #[serde(default)]
    rev: Option<String>,
    #[serde(rename = "lastModified", default)]
    last_modified: Option<i64>,
}

/// The release, out of `nix flake metadata --json`'s answer; `None` for an
/// answer with no revision, no locked reference or no tree. Pure.
#[must_use]
pub fn release_of(metadata: &[u8]) -> Option<Release> {
    let m: Metadata = serde_json::from_slice(metadata).ok()?;
    let rev = m.locked.rev.filter(|r| !r.is_empty())?;
    let last_modified_s = m.locked.last_modified.or(m.last_modified).unwrap_or(0);
    Some(Release {
        revision: Revision {
            rev,
            last_modified_s,
        },
        url: m.url.filter(|u| !u.is_empty())?,
        path: m.path.filter(|p| p.starts_with('/'))?,
    })
}

/// The arguments of `nix` that start the release's installer: `run
/// <locked>#install -- --from-release <rev> --distro <tree>`, then what this
/// one was given (`kept`: `--execute`), then the answers' file when there
/// is one. A secret is never among them: the answers travel in a file.
#[must_use]
pub fn hop_arguments(hop: &Hop, kept: &[String], answers: Option<&str>) -> Vec<String> {
    let mut out = vec![
        "run".to_owned(),
        format!("{}#{INSTALL_APP}", hop.release.url),
        "--".to_owned(),
        FROM_RELEASE.to_owned(),
        hop.release.revision.rev.clone(),
        DISTRO.to_owned(),
        hop.release.path.clone(),
    ];
    out.extend(kept.iter().cloned());
    if let Some(file) = answers {
        out.push(ANSWERS.to_owned());
        out.push(file.to_owned());
    }
    out
}

/// The host a flake reference reaches: `github:` is `github.com`,
/// `gitlab:` is `gitlab.com`, `sourcehut:` is `git.sr.ht`, and a URL its own
/// host — what the network question names. `None` for one that reaches no
/// host (`path:`). Pure.
#[must_use]
pub fn host_of_flake(url: &str) -> Option<String> {
    let url = url.trim();
    let scheme_host = [
        ("github:", "github.com"),
        ("gitlab:", "gitlab.com"),
        ("sourcehut:", "git.sr.ht"),
    ];
    for (scheme, host) in scheme_host {
        if url.starts_with(scheme) {
            return Some(host.to_owned());
        }
    }
    let (scheme, rest) = url.split_once("://")?;
    if !(scheme.ends_with("https") || scheme.ends_with("ssh")) {
        return None;
    }
    let authority = rest.split(['/', '?']).next()?;
    let host = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    let host = host.split(':').next()?.to_ascii_lowercase();
    (!host.is_empty()).then_some(host)
}

/// The template's flake with its distro input aimed at `tree` instead of
/// the release `release_url` names: the one string literal that is exactly
/// the release's reference, replaced. `None` when the template does not
/// hold that literal exactly once — the template changed under this
/// installer, and a guess would install the wrong thing. Pure.
#[must_use]
pub fn aimed(template: &str, release_url: &str, tree: &str) -> Option<String> {
    let literal = format!("\"{release_url}\"");
    if release_url.is_empty() || template.matches(&literal).count() != 1 {
        return None;
    }
    Some(template.replacen(&literal, &format!("\"path:{tree}\""), 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    const OWN: &str = "1111111111111111111111111111111111111111";
    const OTHER: &str = "2222222222222222222222222222222222222222";

    fn own() -> Revision {
        Revision {
            rev: OWN.to_owned(),
            last_modified_s: 1_000,
        }
    }

    fn release(rev: &str, at: i64) -> Release {
        Release {
            revision: Revision {
                rev: rev.to_owned(),
                last_modified_s: at,
            },
            url: format!("github:someone/distro/{rev}?narHash=sha256-x"),
            path: "/nix/store/aaaa-source".to_owned(),
        }
    }

    /// FAILS IF: a newer release does not start its installer, the same
    /// one does, an unreachable release stops the install instead of going
    /// on with this one (and saying so), a development build follows the
    /// release, a hop hops again, or an OLDER release replaces an installer
    /// that is ahead of it. `installer-follows-release` runs this test by
    /// name.
    #[test]
    fn installer_follows_release() {
        // NEWER → the release's installer.
        match follow(Some(&own()), false, Some(release(OTHER, 2_000))) {
            Follow::Hop(hop) => {
                assert_eq!(hop.release.revision.rev, OTHER);
                assert_eq!(hop.key(), key::INST_RELEASE_NEWER);
            }
            other => panic!("a newer release was not followed: {other:?}"),
        }
        // THE SAME → go on, saying nothing.
        assert_eq!(
            follow(Some(&own()), false, Some(release(OWN, 1_000))),
            Follow::Stay(Stay::Current)
        );
        assert_eq!(Stay::Current.key(), None);
        // UNREACHABLE → go on with this one, and say why.
        let offline = follow(Some(&own()), false, None);
        assert_eq!(offline, Follow::Stay(Stay::Unreachable));
        assert_eq!(Stay::Unreachable.key(), Some(key::INST_RELEASE_UNREACHABLE));
        assert!(Stay::Unreachable.aims_at_own());
        // A DEVELOPMENT BUILD never follows: what it carries is what is
        // being tried.
        for dev in [format!("{OWN}-dirty"), "unknown".to_owned(), String::new()] {
            let mine = Revision {
                rev: dev.clone(),
                last_modified_s: 5_000,
            };
            assert_eq!(
                follow(Some(&mine), false, Some(release(OTHER, 9_000))),
                Follow::Stay(Stay::Development),
                "{dev:?}"
            );
        }
        assert_eq!(
            follow(None, false, Some(release(OTHER, 9_000))),
            Follow::Stay(Stay::Development),
            "an ISO that said nothing is a development build"
        );
        assert!(Stay::Development.aims_at_own());
        // ONE HOP: the release's installer never hops again.
        assert_eq!(
            follow(Some(&own()), true, Some(release(OTHER, 9_000))),
            Follow::Stay(Stay::FromRelease)
        );
        assert!(!Stay::FromRelease.aims_at_own() && !Stay::Current.aims_at_own());
        // AN OLDER RELEASE — this ISO is ahead of it, a new installer being
        // tried — keeps this installer, installs its tree, and says so. So
        // does another revision made in the same second: not newer.
        for at in [500, 1_000] {
            assert_eq!(
                follow(Some(&own()), false, Some(release(OTHER, at))),
                Follow::Stay(Stay::Ahead),
                "an older release ({at}) replaced the installer ahead of it"
            );
        }
        assert!(Stay::Ahead.aims_at_own());
        assert_eq!(Stay::Ahead.key(), Some(key::INST_RELEASE_AHEAD));
        // A release whose answer is not a commit is no answer.
        assert_eq!(
            follow(Some(&own()), false, Some(release("main", 9_000))),
            Follow::Stay(Stay::Unreachable)
        );
    }

    /// FAILS IF: the hop's command loses the locked reference, the revision
    /// or the tree, drops what this installer was given, or puts the
    /// answers anywhere but behind `--answers` (a file's name, never its
    /// content).
    #[test]
    fn the_hop_starts_the_release_installer_with_what_this_one_was_given() {
        let hop = Hop {
            release: release(OTHER, 2_000),
        };
        let args = hop_arguments(&hop, &["--execute".to_owned()], Some("/run/x/answers"));
        assert_eq!(
            args,
            [
                "run",
                &format!("github:someone/distro/{OTHER}?narHash=sha256-x#install"),
                "--",
                "--from-release",
                OTHER,
                "--distro",
                "/nix/store/aaaa-source",
                "--execute",
                "--answers",
                "/run/x/answers",
            ]
        );
        assert_eq!(hop.short(), &OTHER[..12]);
        assert!(!hop_arguments(&hop, &[], None).contains(&ANSWERS.to_owned()));
    }

    #[test]
    fn the_release_is_read_from_nix_s_own_answer() {
        let answer = format!(
            r#"{{"description":"x","lastModified":1700000000,"locked":{{"__final":true,"lastModified":1700000001,"narHash":"sha256-abc","owner":"o","repo":"r","rev":"{OTHER}","type":"github"}},"original":{{"owner":"o","ref":"release","repo":"r","type":"github"}},"path":"/nix/store/bbbb-source","url":"github:o/r/{OTHER}?narHash=sha256-abc"}}"#
        );
        let r = release_of(answer.as_bytes()).expect("a release");
        assert_eq!(r.revision.rev, OTHER);
        assert_eq!(r.revision.last_modified_s, 1_700_000_001);
        assert_eq!(r.path, "/nix/store/bbbb-source");
        assert!(r.url.ends_with("narHash=sha256-abc"));
        // No revision (a `path:` flake), no tree, not JSON: no release.
        assert_eq!(
            release_of(br#"{"locked":{"type":"path"},"url":"path:/x","path":"/nix/store/c"}"#),
            None
        );
        assert_eq!(release_of(b"error: unable to download"), None);
        assert_eq!(
            release_of(format!(r#"{{"locked":{{"rev":"{OTHER}"}},"url":"u"}}"#).as_bytes()),
            None
        );
    }

    #[test]
    fn the_hosts_a_flake_reaches_are_named() {
        assert_eq!(
            host_of_flake("github:someone/distro/release").as_deref(),
            Some("github.com")
        );
        assert_eq!(
            host_of_flake("gitlab:someone/distro").as_deref(),
            Some("gitlab.com")
        );
        assert_eq!(
            host_of_flake("git+https://Git.Example.org:8443/d.git?ref=release").as_deref(),
            Some("git.example.org")
        );
        assert_eq!(
            host_of_flake("git+ssh://git@example.org/d.git").as_deref(),
            Some("example.org")
        );
        assert_eq!(host_of_flake("path:/etc/ronoctua/distro"), None);
        assert_eq!(host_of_flake("git+http://example.org/d.git"), None);
    }

    /// FAILS IF: aiming touches anything but the release's one literal, or
    /// aims a template that holds it twice or not at all. (The template the
    /// distro ships is not in the Rust build's source; a template that stops
    /// holding the literal once makes the AIM step refuse, before the disk
    /// is touched.)
    #[test]
    fn a_flake_is_aimed_at_the_installers_tree_by_its_one_literal() {
        let url = "github:someone/distro/release";
        // The template's shape: a comment that names the branch in prose,
        // the input, the outputs.
        let template = "{\n  # follows the `release` branch\n  inputs.ronoctua.url = \"github:someone/distro/release\";\n  outputs = { ronoctua, ... }: ronoctua.lib.machineFlake { dir = ./.; };\n}\n";
        let aimed_text = aimed(template, url, "/nix/store/zzzz-source").expect("aimed");
        assert!(aimed_text.contains("\"path:/nix/store/zzzz-source\""));
        assert!(!aimed_text.contains(&format!("\"{url}\"")));
        // Everything else byte for byte.
        assert_eq!(
            aimed_text.replace("\"path:/nix/store/zzzz-source\"", &format!("\"{url}\"")),
            template
        );
        assert_eq!(aimed("x = \"u\"; y = \"u\";", "u", "/t"), None, "twice");
        assert_eq!(aimed("x = \"v\";", "u", "/t"), None, "absent");
        assert_eq!(aimed("x = \"\";", "", "/t"), None, "nothing to aim");
    }

    #[test]
    fn the_isos_revision_is_read_from_its_variables() {
        assert_eq!(
            own_from(Some(OWN.to_owned()), Some("1700000000".to_owned())),
            Some(Revision {
                rev: OWN.to_owned(),
                last_modified_s: 1_700_000_000
            })
        );
        assert_eq!(own_from(Some(String::new()), None), None);
        assert_eq!(own_from(None, Some("1".to_owned())), None);
        assert!(is_development("abc"));
        assert!(!is_development(OWN));
    }
}
