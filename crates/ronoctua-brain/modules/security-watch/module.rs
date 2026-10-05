//! What the security centre guards, watched: the connections the firewall
//! refuses, what sudo does, whether the firewall is what it was asked to be —
//! and ONE shield on the modeline, in the danger colour, only while
//! something is off.
//!
//! Why each piece is so is in `why.md`, beside this file. What is here is the
//! shape of it.
//!
//! # It runs nothing
//!
//! A watch (`schedule = null`): the journal feed the core runs, and inotify on
//! the two folders the root helper writes (`/run/ronoctua-security`,
//! `/var/lib/ronoctua-security`). No clock at rest; one is armed only while a
//! refused connection is recent, a passwordless window is open or the notice
//! is put off — each until the moment it stops being true.
//!
//! # What "off" is
//!
//! The table in the kernel is not the one asked for (the helper said so, or
//! the firewall's units logged an error after its last apply); the person's
//! firewall is switched off; sudo asks for no password until a time; the
//! shields are up; connections were refused in the last quarter hour. The
//! mark is the loudest of these ([`rules::Off`]), with that one's hint; the
//! page lists all of them.
//!
//! # A network born public is an event
//!
//! A connection that comes up on a network the person never placed is public
//! by default; the watch says so once on the minibuffer, with "treat as
//! home" (the shell's `security-home`). The networks up when it starts are
//! the baseline, not news.
//!
//! # The refused connections are a notice too
//!
//! "N connections refused on wlan0", which a person may dismiss (until the
//! next one) or put off. The page keeps the last ones grouped by who asked
//! for what, and the security centre offers "allow" on each.

mod rules;

use std::borrow::Cow;
use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::module::{
    Arg, Backfill, Birth, Cell, Ctx, Event, Face, Field, Heard, Look, Manifest, Mark, MarkId,
    Match, Module, Notice, Row, Screen, Section, Source, Tone, Watcher,
};
use crate::moment::Moment;

use rules::{Facts, Off, Refused, Sudo, SudoEvent};

/// The journal's sources, in the order [`Watcher::sources`] gives them.
const REFUSED: usize = 0;
const SUDO: usize = 1;
const FIREWALL_UNITS: usize = 2;

/// The entry.
#[must_use]
pub fn new(_birth: &Birth) -> impl Module + use<> {
    SecurityWatch::reading(Places::system())
}

/// Where the root helper writes what this reads — handed in, so a test reads
/// a folder of its own.
#[derive(Debug, Clone)]
struct Places {
    /// `/run/ronoctua-security`: the table's status and the window's end.
    run: PathBuf,
    /// `/var/lib/ronoctua-security`: the person's settings.
    lib: PathBuf,
}

impl Places {
    fn system() -> Self {
        let parent = |p: &str| {
            Path::new(p)
                .parent()
                .map_or_else(|| PathBuf::from("/"), Path::to_path_buf)
        };
        Self {
            run: parent(ronoctua_security::firewall::STATUS),
            lib: parent(ronoctua_security::firewall::FILE),
        }
    }

    fn status(&self) -> PathBuf {
        self.run
            .join(file_name(ronoctua_security::firewall::STATUS))
    }

    fn until(&self) -> PathBuf {
        self.run.join(file_name(ronoctua_security::relax::UNTIL))
    }

    fn settings(&self) -> PathBuf {
        self.lib.join(file_name(ronoctua_security::firewall::FILE))
    }

    fn links(&self) -> PathBuf {
        self.run.join(file_name(ronoctua_security::firewall::LINKS))
    }
}

fn file_name(p: &str) -> &str {
    Path::new(p)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
}

/// What the files say, read on every change of the two folders.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Files {
    /// The helper's last apply: when, and whether it held.
    status: Option<(Moment, bool)>,
    /// `firewall.json`: enabled, shields.
    settings: Option<(bool, bool)>,
    /// The passwordless window's end.
    until: Option<Moment>,
    /// The networks up on a public profile that the person never placed:
    /// connection and interface.
    public: Vec<(String, String)>,
}

impl Files {
    fn read(places: &Places) -> Self {
        let full = std::fs::read(places.status())
            .ok()
            .and_then(|b| serde_json::from_slice::<ronoctua_security::firewall::Status>(&b).ok());
        let links: ronoctua_security::firewall::Links = std::fs::read(places.links())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        let public = full
            .as_ref()
            .map(|s| ronoctua_security::firewall::public_unplaced(s, &links))
            .unwrap_or_default();
        let status = full.map(|s| (s.applied_s, s.ok));
        let settings = std::fs::read(places.settings())
            .ok()
            .and_then(|b| ronoctua_security::firewall::Settings::of_bytes(&b).ok())
            .map(|s| (s.enabled, s.shields));
        let until = std::fs::read_to_string(places.until())
            .ok()
            .and_then(|t| t.trim().parse().ok());
        Self {
            status,
            settings,
            until,
            public,
        }
    }
}

/// What the watch keeps.
#[derive(Debug, Default)]
struct Kept {
    files: Files,
    refused: VecDeque<Refused>,
    sudo: VecDeque<SudoEvent>,
    /// The newest error the firewall's units logged.
    unit_error: Option<Moment>,
    /// Sources that cannot be followed, with the machine's words.
    lost: BTreeMap<usize, String>,
    /// The newest refused connection a person dismissed the notice at.
    dismissed: Option<Moment>,
    /// Until when the notice is put off.
    snoozed: Option<Moment>,
    /// The page's generation: moves with every change.
    generation: u64,
    /// Every public network never placed that the watch has seen up — the
    /// first reading is the baseline, and a network is news once.
    networks_seen: std::collections::BTreeSet<String>,
}

impl Kept {
    fn facts(&self, now: Moment) -> Facts {
        let failed_apply = self.files.status.is_some_and(|(_, ok)| !ok);
        let failed_after = match (self.unit_error, self.files.status) {
            (Some(error), Some((applied, _))) => error > applied,
            (Some(_), None) => true,
            _ => false,
        };
        Facts {
            failed: failed_apply || failed_after,
            off: self.files.settings.is_some_and(|(enabled, _)| !enabled),
            sudo_until: self.files.until,
            shields: self.files.settings.is_some_and(|(_, shields)| shields),
            recent_refused: rules::recent(&self.refused, now),
        }
    }
}

/// The watch.
struct SecurityWatch {
    places: Places,
    kept: Mutex<Kept>,
}

impl SecurityWatch {
    fn reading(places: Places) -> Self {
        Self {
            places,
            kept: Mutex::new(Kept::default()),
        }
    }

    fn with<T>(&self, f: impl FnOnce(&mut Kept) -> T) -> T {
        let mut kept = self
            .kept
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        f(&mut kept)
    }

    /// Something changed: the page is asked again, and the face published.
    fn moved(&self, kept: &mut Kept, ctx: &Ctx) {
        kept.generation += 1;
        ctx.changed();
    }
}

/// The mark of what is off.
const fn mark_of(off: Off) -> MarkId {
    match off {
        Off::Failed => super::mark::FIREWALL_FAILED,
        Off::Disabled => super::mark::FIREWALL_OFF,
        Off::SudoOpen => super::mark::SUDO_OPEN,
        Off::Shields => super::mark::SHIELDS,
        Off::Refused => super::mark::REFUSED,
    }
}

/// The page's row of what is off.
const fn row_key(off: Off) -> &'static str {
    match off {
        Off::Failed => super::key::SECURITY_WATCH_ROW_FIREWALL_FAILED,
        Off::Disabled => super::key::SECURITY_WATCH_ROW_FIREWALL_OFF,
        Off::SudoOpen => super::key::SECURITY_WATCH_ROW_SUDO_OPEN,
        Off::Shields => super::key::SECURITY_WATCH_ROW_SHIELDS,
        Off::Refused => super::key::SECURITY_WATCH_ROW_REFUSED,
    }
}

/// The row id of what is off — data the security centre reads back to open
/// its matching section.
const fn row_id(off: Off) -> &'static str {
    match off {
        Off::Failed => "off:firewall-failed",
        Off::Disabled => "off:firewall-off",
        Off::SudoOpen => "off:sudo-open",
        Off::Shields => "off:shields",
        Off::Refused => "off:refused",
    }
}

const fn sudo_key(kind: Sudo) -> &'static str {
    match kind {
        Sudo::Used => super::key::SECURITY_WATCH_SUDO_USED,
        Sudo::Failed => super::key::SECURITY_WATCH_SUDO_FAILED,
        Sudo::Refused => super::key::SECURITY_WATCH_SUDO_REFUSED,
    }
}

fn key(k: &'static str) -> Cell {
    Cell::Key(Cow::Borrowed(k))
}

impl Module for SecurityWatch {
    fn manifest(&self) -> &'static Manifest {
        &super::MANIFEST
    }

    fn watcher(&self) -> Option<&dyn Watcher> {
        Some(self)
    }

    fn face(&self, look: &Look<'_>) -> Face {
        self.with(|kept| {
            let facts = kept.facts(look.now);
            let off = rules::off_at(&facts, look.now);
            let mut face = Face {
                screen: kept.generation,
                ..Face::default()
            };
            // ONE MARK, THE LOUDEST, and none at rest (invariant 14).
            if let Some(first) = off.first() {
                face.marks.push(Mark {
                    id: mark_of(*first),
                    tone: Tone::Danger,
                    count: if *first == Off::Refused {
                        facts.recent_refused
                    } else {
                        0
                    },
                    act: None,
                });
            }
            // THE NOTICE, until a person puts it away: a newer refusal than
            // the one dismissed at brings it back.
            let newest = kept
                .refused
                .front()
                .filter(|r| r.at > look.now - rules::RECENT_S);
            let put_off = kept.snoozed.is_some_and(|s| s > look.now);
            if let Some(newest) = newest
                && kept.dismissed.is_none_or(|d| newest.at > d)
                && !put_off
            {
                let since = kept
                    .refused
                    .iter()
                    .filter(|r| r.at > look.now - rules::RECENT_S)
                    .map(|r| r.at)
                    .min()
                    .unwrap_or(newest.at);
                face.notices.push(
                    Notice::of(
                        &super::notice::REFUSED,
                        super::key::SECURITY_WATCH_SAYS_REFUSED,
                        since,
                    )
                    .with("count", Arg::Number(i64::from(facts.recent_refused)))
                    .with("iface", Arg::Text(newest.iface.clone())),
                );
            }
            face
        })
    }

    fn screen(&self, look: &Look<'_>) -> Screen {
        self.with(|kept| {
            let facts = kept.facts(look.now);
            let mut now_rows: Vec<Row> = rules::off_at(&facts, look.now)
                .into_iter()
                .map(|off| Row {
                    id: row_id(off).to_owned(),
                    label: key(row_key(off)),
                    value: match off {
                        Off::SudoOpen => facts.sudo_until.map_or(Cell::Empty, Cell::Moment),
                        Off::Refused => Cell::Count(u64::from(facts.recent_refused)),
                        _ => Cell::Empty,
                    },
                    tone: Tone::Danger,
                    ..Row::default()
                })
                .collect();
            if now_rows.is_empty() {
                now_rows.push(Row {
                    id: "off:nothing".to_owned(),
                    label: key(super::key::SECURITY_WATCH_ROW_NOTHING),
                    ..Row::default()
                });
            }
            // A JOURNAL IT CANNOT READ IS SAID, never a quiet page.
            for why in kept.lost.values() {
                now_rows.push(Row {
                    id: "lost".to_owned(),
                    label: key(super::key::SECURITY_WATCH_ROW_JOURNAL_LOST),
                    note: Cell::Data(why.clone()),
                    tone: Tone::Warning,
                    ..Row::default()
                });
            }
            let blocked = rules::grouped(&kept.refused)
                .into_iter()
                .map(|(r, n)| Row {
                    id: r.row_id(),
                    label: Cell::Data(format!(
                        "{}/{} {} {}",
                        r.proto,
                        r.port.map_or_else(|| "-".to_owned(), |p| p.to_string()),
                        r.source,
                        r.iface
                    )),
                    value: Cell::Moment(r.at),
                    note: Cell::Count(n),
                    ..Row::default()
                })
                .collect();
            let admin = kept
                .sudo
                .iter()
                .enumerate()
                .map(|(n, e)| Row {
                    id: format!("sudo:{n}"),
                    label: key(sudo_key(e.kind)),
                    value: Cell::Moment(e.at),
                    note: Cell::Data(format!("{} {}", e.user, e.command).trim().to_owned()),
                    tone: match e.kind {
                        Sudo::Used => Tone::Quiet,
                        Sudo::Failed | Sudo::Refused => Tone::Warning,
                    },
                    ..Row::default()
                })
                .collect();
            Screen {
                sections: vec![
                    Section {
                        heading: Cow::Borrowed(super::key::SECURITY_WATCH_HEADING_NOW),
                        rows: now_rows,
                    },
                    Section {
                        heading: Cow::Borrowed(super::key::SECURITY_WATCH_HEADING_BLOCKED),
                        rows: blocked,
                    },
                    Section {
                        heading: Cow::Borrowed(super::key::SECURITY_WATCH_HEADING_ADMIN),
                        rows: admin,
                    },
                ],
            }
        })
    }

    fn dismissed(&self, _notice: &str, ctx: &Ctx) {
        self.with(|kept| {
            kept.dismissed = kept.refused.front().map(|r| r.at).or(Some(ctx.now()));
            self.moved(kept, ctx);
        });
    }

    fn snoozed(&self, _notice: &str, until: Moment, ctx: &Ctx) {
        self.with(|kept| {
            kept.snoozed = Some(until);
            self.moved(kept, ctx);
        });
    }
}

impl Watcher for SecurityWatch {
    fn sources(&self) -> Vec<Source> {
        let field = |name: &str, any_of: &[&str]| Field {
            name: name.to_owned(),
            any_of: any_of.iter().map(|v| (*v).to_owned()).collect(),
        };
        vec![
            // THE KERNEL'S LINES WITH OUR PREFIX — the declared firewall logs
            // them, rate-limited, just before it drops (`security.nix`).
            Source::Journal(Match {
                max_priority: Some(ronoctua_journal::INFO),
                fields: vec![field("_TRANSPORT", &["kernel"])],
                message_contains: Some(ronoctua_security::firewall::LOG_PREFIX.to_owned()),
                backfill: Backfill::Since {
                    seconds: rules::REFUSED_BACK_S,
                    most: 64,
                },
            }),
            // WHAT SUDO SAID — a week of it the first time, for the page.
            Source::Journal(Match {
                max_priority: Some(ronoctua_journal::INFO),
                fields: vec![field("SYSLOG_IDENTIFIER", &["sudo"])],
                message_contains: None,
                backfill: Backfill::Since {
                    seconds: rules::SUDO_BACK_S,
                    most: rules::SUDO_MOST,
                },
            }),
            // THE FIREWALL'S UNITS, when they fail.
            Source::Journal(Match {
                max_priority: Some(ronoctua_journal::ERR),
                fields: vec![field(
                    "_SYSTEMD_UNIT",
                    &["nftables.service", "ronoctua-firewall.service"],
                )],
                message_contains: None,
                backfill: Backfill::ThisBoot { most: 4 },
            }),
            Source::Path(self.places.run.clone()),
            Source::Path(self.places.lib.clone()),
        ]
    }

    fn started(&self, _now: Moment, ctx: &Ctx) {
        let files = Files::read(&self.places);
        self.with(|kept| {
            // THE BASELINE: the networks up when the watch starts are not
            // networks coming up.
            kept.networks_seen = files.public.iter().map(|(c, _)| c.clone()).collect();
            kept.files = files;
            self.moved(kept, ctx);
        });
    }

    fn heard(&self, heard: Heard<'_>, ctx: &Ctx) {
        match heard {
            Heard::Journal { source, entry } => {
                let at = i64::try_from(entry.realtime_us / 1_000_000).unwrap_or(ctx.now());
                self.with(|kept| {
                    let changed = match source {
                        REFUSED => rules::refused_of(
                            &entry.message,
                            ronoctua_security::firewall::LOG_PREFIX,
                            at,
                        )
                        .map(|r| rules::ring(&mut kept.refused, r, rules::REFUSED_MOST))
                        .is_some(),
                        SUDO => rules::sudo_of(&entry.message, at)
                            .map(|e| rules::ring(&mut kept.sudo, e, rules::SUDO_MOST))
                            .is_some(),
                        FIREWALL_UNITS => {
                            kept.unit_error = Some(kept.unit_error.map_or(at, |e| e.max(at)));
                            true
                        }
                        _ => false,
                    };
                    if changed {
                        self.moved(kept, ctx);
                    }
                });
            }
            Heard::Path { .. } => {
                let files = Files::read(&self.places);
                self.with(|kept| {
                    // A NETWORK BORN PUBLIC, said once on the minibuffer with
                    // "treat as home" (`security-home`).
                    for iface in rules::fresh_networks(&mut kept.networks_seen, &files.public) {
                        ctx.tell(
                            Event::of(&super::event::NETWORK_PUBLIC)
                                .with("iface", Arg::Text(iface)),
                        );
                    }
                    if kept.files != files {
                        kept.files = files;
                        self.moved(kept, ctx);
                    }
                });
            }
            Heard::Lost { source, why } => self.with(|kept| {
                if kept.lost.get(&source).map(String::as_str) != Some(why) {
                    kept.lost.insert(source, why.to_owned());
                    self.moved(kept, ctx);
                }
            }),
            Heard::Back { source } => self.with(|kept| {
                if kept.lost.remove(&source).is_some() {
                    self.moved(kept, ctx);
                }
            }),
            // THE CLOCK THIS ASKED FOR: something stopped being true — a
            // refusal aged out, the window ended, the notice came back.
            Heard::Wake | Heard::Line { .. } => self.with(|kept| self.moved(kept, ctx)),
        }
    }

    fn next_wake(&self, now: Moment) -> Option<Moment> {
        self.with(|kept| {
            [
                rules::recent_ends(&kept.refused, now),
                kept.files.until.filter(|u| *u > now),
                kept.snoozed.filter(|s| *s > now),
            ]
            .into_iter()
            .flatten()
            .min()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::Entry;
    use crate::pulse::Pulse;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(tag: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "ronoctua-security-watch-{tag}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(dir.join("run")).unwrap();
            std::fs::create_dir_all(dir.join("lib")).unwrap();
            Self(dir)
        }

        fn places(&self) -> Places {
            Places {
                run: self.0.join("run"),
                lib: self.0.join("lib"),
            }
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn face(m: &SecurityWatch, now: Moment) -> Face {
        let pulse = Pulse::default();
        let mine = super::super::MANIFEST.defaults();
        m.face(&Look {
            now,
            pulse: &pulse,
            mine: &mine,
            last: None,
        })
    }

    fn screen(m: &SecurityWatch, now: Moment) -> Screen {
        let pulse = Pulse::default();
        let mine = super::super::MANIFEST.defaults();
        m.screen(&Look {
            now,
            pulse: &pulse,
            mine: &mine,
            last: None,
        })
    }

    fn entry(message: &str, at_s: u64) -> Entry {
        Entry {
            message: message.to_owned(),
            realtime_us: at_s * 1_000_000,
            ..Entry::default()
        }
    }

    const LINE: &str =
        "ronoctua-refused: IN=wlan0 OUT= SRC=203.0.113.9 DST=192.168.1.5 PROTO=TCP SPT=5 DPT=22";

    /// FAILS IF: a mark shows at rest, or the loudest of what is off stops
    /// being the one drawn — with its own hint, in the danger colour.
    #[test]
    fn the_shield_is_up_only_while_something_is_off() {
        let dir = Scratch::new("mark");
        let m = SecurityWatch::reading(dir.places());
        let (ctx, _) = Ctx::kept(super::super::ID, 1000);
        m.started(1000, &ctx);
        assert!(face(&m, 1000).marks.is_empty(), "a mark at rest");

        // A REFUSED CONNECTION: the shield, counting it.
        m.heard(
            Heard::Journal {
                source: REFUSED,
                entry: &entry(LINE, 1000),
            },
            &ctx,
        );
        let f = face(&m, 1001);
        assert_eq!(f.marks.len(), 1);
        assert_eq!(f.marks[0].id, super::super::mark::REFUSED);
        assert_eq!(f.marks[0].tone, Tone::Danger);
        assert_eq!(f.marks[0].count, 1);
        assert!(f.marks[0].act.is_none(), "a click opens the page");
        // AND GONE A QUARTER HOUR LATER, by the clock it asked for.
        assert_eq!(m.next_wake(1001), Some(1000 + rules::RECENT_S));
        assert!(face(&m, 1000 + rules::RECENT_S).marks.is_empty());

        // THE WINDOW OPEN IS LOUDER, with its own hint.
        std::fs::write(dir.0.join("run/sudo-until"), "5000\n").unwrap();
        m.heard(
            Heard::Path {
                source: 3,
                path: &dir.0.join("run"),
            },
            &ctx,
        );
        let f = face(&m, 1001);
        assert_eq!(f.marks[0].id, super::super::mark::SUDO_OPEN);
        assert_eq!(f.marks[0].count, 0);
        assert_eq!(m.next_wake(1001), Some(1000 + rules::RECENT_S));
        assert_eq!(m.next_wake(1000 + rules::RECENT_S), Some(5000));
        assert!(face(&m, 5000).marks.is_empty(), "the window ended");

        // THE FIREWALL OFF, AND A FAILED APPLY, louder still.
        let settings = ronoctua_security::firewall::Settings {
            enabled: false,
            ..Default::default()
        };
        std::fs::write(
            dir.0.join("lib/firewall.json"),
            serde_json::to_vec(&settings).unwrap(),
        )
        .unwrap();
        m.heard(
            Heard::Path {
                source: 4,
                path: &dir.0.join("lib"),
            },
            &ctx,
        );
        assert_eq!(face(&m, 1001).marks[0].id, super::super::mark::FIREWALL_OFF);
        m.heard(
            Heard::Journal {
                source: FIREWALL_UNITS,
                entry: &entry("nft: syntax error", 1001),
            },
            &ctx,
        );
        assert_eq!(
            face(&m, 1002).marks[0].id,
            super::super::mark::FIREWALL_FAILED
        );
        // EVERY HINT IS ITS OWN, and the manifest's.
        for off in [
            Off::Failed,
            Off::Disabled,
            Off::SudoOpen,
            Off::Shields,
            Off::Refused,
        ] {
            assert!(
                super::super::MANIFEST.marks.contains(&mark_of(off)),
                "{off:?}"
            );
        }
    }

    /// FAILS IF: a network that comes up public and unplaced is not said once
    /// — or the networks up when the watch starts are taken for news.
    #[test]
    fn a_network_born_public_is_said_once() {
        use ronoctua_security::firewall::{Profile, Settings, Status};
        let dir = Scratch::new("network");
        let write = |links: &[(&str, &str)]| {
            let links: ronoctua_security::firewall::Links = links
                .iter()
                .map(|(i, c)| ((*i).to_owned(), (*c).to_owned()))
                .collect();
            let mut settings = Settings::default();
            settings
                .networks
                .insert("11111111-2222-3333-4444-555555555555".into(), Profile::Home);
            let status = Status::of(&settings, &links, Ok(()));
            std::fs::write(
                dir.0.join("run/links.json"),
                serde_json::to_vec(&links).unwrap(),
            )
            .unwrap();
            std::fs::write(
                dir.0.join("run/firewall.status"),
                serde_json::to_vec(&status).unwrap(),
            )
            .unwrap();
        };
        let path = |m: &SecurityWatch, ctx: &Ctx| {
            m.heard(
                Heard::Path {
                    source: 3,
                    path: &dir.0.join("run"),
                },
                ctx,
            );
        };
        // UP WHEN IT STARTS: the baseline.
        write(&[("wlan0", "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d")]);
        let m = SecurityWatch::reading(dir.places());
        let (ctx, kept) = Ctx::kept(super::super::ID, 1000);
        m.started(1000, &ctx);
        path(&m, &ctx);
        assert!(kept.events().is_empty(), "the baseline was said");
        // A NEW ONE, PUBLIC: said, with its interface.
        write(&[
            ("wlan0", "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d"),
            ("wlan1", "b1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d"),
            ("enp3s0", "11111111-2222-3333-4444-555555555555"),
        ]);
        path(&m, &ctx);
        let told = kept.events();
        assert_eq!(told.len(), 1, "{told:?}");
        assert_eq!(told[0].1.tag, super::super::event::NETWORK_PUBLIC.tag);
        assert!(
            told[0]
                .1
                .args
                .iter()
                .any(|(n, a)| n == "iface" && *a == Arg::Text("wlan1".into())),
            "{told:?}"
        );
        // ONCE: the same networks again say nothing, and a home one never.
        path(&m, &ctx);
        assert_eq!(kept.events().len(), 1);
    }

    /// FAILS IF: the notice stops saying how many and where, a dismissal does
    /// not put it away until the next refusal, or a snooze is not honoured.
    #[test]
    fn the_notice_counts_and_goes_away_when_asked() {
        let dir = Scratch::new("notice");
        let m = SecurityWatch::reading(dir.places());
        let (ctx, kept) = Ctx::kept(super::super::ID, 1000);
        for at in [1000, 1010] {
            m.heard(
                Heard::Journal {
                    source: REFUSED,
                    entry: &entry(LINE, at),
                },
                &ctx,
            );
        }
        let f = face(&m, 1020);
        assert_eq!(f.notices.len(), 1);
        let n = &f.notices[0];
        assert_eq!(n.says, super::super::key::SECURITY_WATCH_SAYS_REFUSED);
        assert!(
            n.args
                .iter()
                .any(|(k, v)| k == "count" && *v == Arg::Number(2))
        );
        assert!(
            n.args
                .iter()
                .any(|(k, v)| k == "iface" && *v == Arg::Text("wlan0".into()))
        );
        assert_eq!(n.since, 1000);
        assert!(n.dismiss);
        m.dismissed("refused", &ctx);
        assert!(face(&m, 1020).notices.is_empty(), "dismissed");
        m.heard(
            Heard::Journal {
                source: REFUSED,
                entry: &entry(LINE, 1030),
            },
            &ctx,
        );
        assert_eq!(
            face(&m, 1031).notices.len(),
            1,
            "a newer one brings it back"
        );
        m.snoozed("refused", 2000, &ctx);
        assert!(face(&m, 1031).notices.is_empty());
        assert!(!kept.changes().is_empty(), "the face was never republished");
    }

    /// FAILS IF: the page stops listing what is off, the blocked connections
    /// grouped with the row id the security centre reads, or sudo's uses and
    /// failures — or a journal it cannot read goes unsaid.
    #[test]
    fn the_page_says_what_is_off_what_was_blocked_and_what_sudo_did() {
        let dir = Scratch::new("page");
        let m = SecurityWatch::reading(dir.places());
        let (ctx, _) = Ctx::kept(super::super::ID, 1000);
        let calm = screen(&m, 1000);
        assert_eq!(calm.sections.len(), 3);
        assert_eq!(calm.sections[0].rows[0].id, "off:nothing");
        for at in [1000, 1005] {
            m.heard(
                Heard::Journal {
                    source: REFUSED,
                    entry: &entry(LINE, at),
                },
                &ctx,
            );
        }
        m.heard(
            Heard::Journal {
                source: SUDO,
                entry: &entry(
                    "noctua : 1 incorrect password attempt ; PWD=/ ; USER=root ; COMMAND=/bin/ls",
                    900,
                ),
            },
            &ctx,
        );
        m.heard(
            Heard::Lost {
                source: SUDO,
                why: "journalctl: permission denied",
            },
            &ctx,
        );
        let s = screen(&m, 1010);
        assert_eq!(s.sections[0].rows[0].id, "off:refused");
        assert_eq!(s.sections[0].rows[0].value, Cell::Count(2));
        assert!(s.sections[0].rows.iter().any(|r| r.id == "lost"));
        let blocked = &s.sections[1].rows;
        assert_eq!(blocked.len(), 1);
        assert_eq!(blocked[0].id, "blocked:tcp:22:wlan0:203.0.113.9");
        assert_eq!(blocked[0].value, Cell::Moment(1005));
        assert_eq!(blocked[0].note, Cell::Count(2));
        let admin = &s.sections[2].rows;
        assert_eq!(admin.len(), 1);
        assert_eq!(
            admin[0].label,
            Cell::Key(super::super::key::SECURITY_WATCH_SUDO_FAILED.into())
        );
        assert_eq!(admin[0].tone, Tone::Warning);
        let before = face(&m, 1010).screen;
        m.heard(Heard::Back { source: SUDO }, &ctx);
        assert!(face(&m, 1010).screen > before, "the page did not move");
    }

    #[test]
    fn it_listens_to_our_prefix_and_to_sudo() {
        let m = SecurityWatch::reading(Places::system());
        let sources = m.sources();
        let Source::Journal(refused) = &sources[REFUSED] else {
            panic!()
        };
        assert_eq!(
            refused.message_contains.as_deref(),
            Some(ronoctua_security::firewall::LOG_PREFIX)
        );
        let Source::Journal(sudo) = &sources[SUDO] else {
            panic!()
        };
        assert!(sudo.accepts(&Entry {
            fields: [("SYSLOG_IDENTIFIER".to_owned(), "sudo".to_owned())].into(),
            priority: Some(5),
            ..Entry::default()
        }));
        assert!(matches!(&sources[3], Source::Path(p) if p == Path::new("/run/ronoctua-security")));
        assert!(
            matches!(&sources[4], Source::Path(p) if p == Path::new("/var/lib/ronoctua-security"))
        );
    }
}
