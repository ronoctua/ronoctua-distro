//! Scanning the person's YouTube channels every few hours, and marking the
//! modeline while a scan runs — or while the last one failed for every
//! channel.
//!
//! The whole story — why the brain and not a timer, why the power saver
//! waits, why the mark is red only when every channel failed — is in
//! `why.md` beside this file. What is here is the shape of it.
//!
//! # The brain owns the WHEN and nothing else
//!
//! The scan is the app's: `ronoctua-video-metrics scan`, a unit of its own
//! (`nix/modules/video-metrics.nix`) that blocks nothing here, runs at idle
//! priority and stays reachable by hand — the door's "Scan now" asks the
//! brain first, and systemd when there is no brain. What is left for the
//! brain is a cadence (every six hours), the network (held while there is no
//! way out), the power saver (a run while it is on is skipped, and says why)
//! and the mark.
//!
//! # The marks
//!
//! While a scan runs — started by this brain, or by anyone who started the
//! unit, which always writes its report where the brain reads — the modeline
//! carries one mark, in the watching colour. A report that says "begun" is
//! believed only while the scan HOLDS the catalogue (its lock, which goes
//! with its process): a scan killed before it wrote its end — the memory
//! killer, a stop the unit's `ExecStopPost` did not reach — is no scan
//! running, whenever this brain starts and reads it. After it, the catalogue's own
//! record of the scan decides: a scan in which EVERY channel failed leaves a
//! mark in the danger colour, until a scan in which one did not. At rest
//! there is none (invariant 14). A click opens this module's page.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::module::{
    Birth, Cell, Ctx, Face, Heard, Look, Manifest, Mark, Module, Row, Run, Scheduled, Screen,
    Section, Source, Tone, Watcher,
};
use crate::moment::Moment;
use crate::prefs::TaskPrefs;
use crate::pulse::Pulse;
use crate::systemd;
use crate::task::{Due, Hold, Outcome, Sheet, Verdict, Watched};

/// The entry: the real machine, the person's catalogue, and the brain's own
/// report directory.
#[must_use]
pub fn new(_birth: &Birth) -> impl Module + use<> {
    VideoMetrics::of(
        Box::new(Machine),
        ronoctua_video_metrics::default_path(),
        crate::runner::reports_dir(),
    )
}

/// What the module asks of the machine — a trait, so a test hands in a
/// machine of its own choosing.
trait Probe: Send + Sync {
    /// Whether there is a way out: `Some(false)` only when the network
    /// daemon SAID there is none; `None` when nobody could say.
    fn online(&self) -> Option<bool>;
    /// Whether the power saver is on.
    fn saving(&self) -> bool;
    /// The catalogue's record of its last scan.
    fn last_scan(&self, catalogue: &Path) -> Option<Last>;
    /// Whether a scan holds the catalogue now — its lock, not its record.
    fn holding(&self, catalogue: &Path) -> bool;
}

/// What the catalogue says of its last scan — what the page shows and the
/// danger mark reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Last {
    /// When it started.
    started: Moment,
    /// When it ended; `None` while it runs, or when it died.
    ended: Option<Moment>,
    /// Channels whose feeds were read.
    channels: u32,
    /// Channels whose feeds failed.
    failed: u32,
    /// Videos catalogued.
    added: u32,
}

impl Last {
    /// Whether it ended, and every channel it asked failed.
    const fn failed_everywhere(&self) -> bool {
        self.ended.is_some() && self.channels > 0 && self.failed == self.channels
    }
}

/// The real machine: NetworkManager, power-profiles-daemon, the catalogue.
struct Machine;

impl Probe for Machine {
    fn online(&self) -> Option<bool> {
        let look = ronoctua_network::Manager::fresh().ok()?.look();
        if !look.daemon {
            return None;
        }
        // THE AEROPLANE, OR NO ROUTE AT ALL, is offline. A portal or a
        // limited way out lets nothing through to YouTube either; an unknown
        // one is the daemon not having measured yet, and that is not a no.
        use ronoctua_network::Way;
        Some(look.networking && !matches!(look.way, Way::None | Way::Portal | Way::Limited))
    }

    fn saving(&self) -> bool {
        ronoctua_power::Manager::fresh()
            .ok()
            .and_then(|m| m.read().profiles)
            .is_some_and(|p| p.active == ronoctua_power::Profile::PowerSaver)
    }

    fn last_scan(&self, catalogue: &Path) -> Option<Last> {
        let record = ronoctua_video_metrics::read::Catalog::open_ro(catalogue)
            .ok()?
            .last_scan()
            .ok()??;
        Some(Last {
            started: record.started_at_s,
            ended: record.ended_at_s,
            channels: record.channels,
            failed: record.channels_failed,
            added: record.added,
        })
    }

    fn holding(&self, catalogue: &Path) -> bool {
        ronoctua_video_metrics::read::Catalog::open_ro(catalogue).is_ok_and(|c| c.scanning())
    }
}

/// The module, and what it remembers.
struct VideoMetrics {
    probe: Box<dyn Probe>,
    /// The catalogue — `None` with no home, and then nothing is read.
    catalogue: Option<PathBuf>,
    /// Where the brain reads the units' reports: the unit writes its own
    /// there, `<id>.json`.
    reports: PathBuf,
    kept: Mutex<Kept>,
}

/// What it remembers between two looks. In memory: a brain that restarted
/// reads the catalogue again when its watch starts.
#[derive(Debug, Default, Clone, Copy)]
struct Kept {
    /// A scan is running.
    scanning: bool,
    /// The catalogue's last scan, as last read.
    last: Option<Last>,
}

impl VideoMetrics {
    fn of(probe: Box<dyn Probe>, catalogue: Option<PathBuf>, reports: PathBuf) -> Self {
        Self {
            probe,
            catalogue,
            reports,
            kept: Mutex::new(Kept::default()),
        }
    }

    /// What it remembers now.
    fn kept(&self) -> Kept {
        self.kept.lock().map_or_else(|_| Kept::default(), |k| *k)
    }

    /// Says a scan runs, or does not — and publishes, if that changed.
    fn scanning(&self, on: bool, ctx: &Ctx) {
        let changed = self.kept.lock().is_ok_and(|mut k| {
            let was = k.scanning;
            k.scanning = on;
            was != on
        });
        if changed {
            ctx.changed();
        }
    }

    /// Reads the catalogue's last scan again — publishing when it moved.
    fn reread(&self, ctx: &Ctx) {
        let last = self
            .catalogue
            .as_deref()
            .and_then(|c| self.probe.last_scan(c));
        let changed = self.kept.lock().is_ok_and(|mut k| {
            let was = k.last;
            k.last = last;
            was != last
        });
        if changed {
            ctx.changed();
        }
    }

    /// The unit's report — its own file among the brain's reports.
    fn report(&self) -> PathBuf {
        crate::runner::report_in(&self.reports, super::ID, false)
    }
}

/// The page's rows: the last scan, or the sentence that there was none.
fn rows_of(last: Option<Last>) -> Vec<Row> {
    let key = |k: &'static str| Cell::Key(k.into());
    let Some(l) = last else {
        return vec![Row {
            id: "never".to_owned(),
            label: key(super::key::VIDEO_METRICS_BRAIN_NEVER),
            ..Row::default()
        }];
    };
    let count = |id: &str, label: &'static str, n: u32, tone: Tone| Row {
        id: id.to_owned(),
        label: key(label),
        value: Cell::Count(u64::from(n)),
        tone,
        ..Row::default()
    };
    let mut rows = vec![Row {
        id: "started".to_owned(),
        label: key(super::key::VIDEO_METRICS_BRAIN_STARTED),
        value: Cell::Moment(l.started),
        ..Row::default()
    }];
    if let Some(ended) = l.ended {
        rows.push(Row {
            id: "ended".to_owned(),
            label: key(super::key::VIDEO_METRICS_BRAIN_ENDED),
            value: Cell::Moment(ended),
            ..Row::default()
        });
    }
    rows.push(count(
        "channels",
        super::key::VIDEO_METRICS_BRAIN_CHANNELS,
        l.channels,
        Tone::Quiet,
    ));
    rows.push(count(
        "failed",
        super::key::VIDEO_METRICS_BRAIN_FAILED,
        l.failed,
        if l.failed_everywhere() {
            Tone::Danger
        } else if l.failed > 0 {
            Tone::Warning
        } else {
            Tone::Quiet
        },
    ));
    rows.push(count(
        "added",
        super::key::VIDEO_METRICS_BRAIN_ADDED,
        l.added,
        Tone::Quiet,
    ));
    rows
}

impl Module for VideoMetrics {
    fn manifest(&self) -> &'static Manifest {
        &super::MANIFEST
    }

    fn scheduled(&self) -> Option<&dyn Scheduled> {
        Some(self)
    }

    fn watcher(&self) -> Option<&dyn Watcher> {
        Some(self)
    }

    fn face(&self, _look: &Look<'_>) -> Face {
        let k = self.kept();
        let mut face = Face {
            // THE PAGE IS ASKED AGAIN WHEN THE RECORD MOVES: its start and its
            // end are what it shows.
            screen: k
                .last
                .map_or(0, |l| {
                    u64::try_from(l.ended.unwrap_or(l.started)).unwrap_or(0)
                })
                .wrapping_add(u64::from(k.scanning)),
            ..Face::default()
        };
        // A MARK EXISTS ONLY WHEN IT SAYS SOMETHING (invariant 14), and one at
        // a time: a scan running says more than the one before it failing.
        if k.scanning {
            face.marks.push(Mark {
                id: super::mark::SCANNING,
                tone: Tone::Watching,
                count: 0,
                act: None,
            });
        } else if k.last.is_some_and(|l| l.failed_everywhere()) {
            face.marks.push(Mark {
                id: super::mark::FAILED,
                tone: Tone::Danger,
                count: 0,
                act: None,
            });
        }
        face
    }

    fn screen(&self, _look: &Look<'_>) -> Screen {
        Screen {
            sections: vec![Section {
                heading: super::key::VIDEO_METRICS_BRAIN_LAST.into(),
                rows: rows_of(self.kept().last),
            }],
        }
    }
}

impl Scheduled for VideoMetrics {
    fn due(&self, now: Moment, mine: &TaskPrefs, last: Option<Moment>) -> Due {
        if !systemd::reachable() {
            return Due::Not {
                why: "no-systemctl",
            };
        }
        // A CADENCE IN MINUTES, and no zone is consulted: `next_moment`
        // short-circuits the calendar before one would be reached.
        let at = crate::schedule::next_moment(now, last, mine, &crate::moment::Fixed(0));
        if at <= now { Due::Now } else { Due::At(at) }
    }

    fn needs(&self, _pulse: &Pulse, _mine: &TaskPrefs) -> Verdict {
        // NO WAY OUT IS A HOLD, and the run waits for the network; a daemon
        // that could not say is no reason to wait (the scan stops by itself,
        // and records `offline`, when it finds none).
        if self.probe.online() == Some(false) {
            Verdict::Hold(Hold::Network)
        } else {
            Verdict::Go
        }
    }

    fn run(&self, run: &Run) -> Outcome {
        // A BRAIN THAT ONLY REHEARSES asks YouTube for nothing: a scan that
        // changed nothing would be one that asked nothing, and there is no
        // such scan.
        if run.is_rehearsal() {
            return Outcome::Skipped {
                why: "rehearsing".to_owned(),
            };
        }
        // THE POWER SAVER IS THE PERSON SAYING "SPEND LESS": this round is
        // skipped, and the next comes a cadence later.
        if self.probe.saving() {
            return Outcome::Skipped {
                why: "saver".to_owned(),
            };
        }
        let unit = super::unit::RUN;
        // THE MARK BEFORE THE START, and the report marked before it: a unit
        // quick enough to write its report before the watch began would
        // otherwise be missed.
        self.scanning(true, run.ctx());
        let mark = run.mark();
        let outcome = match systemd::start(unit) {
            Err(e) => Outcome::Failed {
                at: String::new(),
                why: e.to_string(),
            },
            Ok(()) => match run.watch(mark) {
                Watched::Ended(sheet) => outcome_of(&sheet),
                Watched::Runaway(_) => Outcome::Failed {
                    at: String::new(),
                    why: "runaway".to_owned(),
                },
                Watched::Aborted(_) => {
                    // GIVING UP IS WHAT STOPS IT: a scan the brain stopped
                    // watching would go on asking YouTube behind a row that
                    // says aborted.
                    if let Err(e) = systemd::stop(unit) {
                        tracing::warn!(error = %e, "could not stop the video scan");
                    }
                    Outcome::Failed {
                        at: String::new(),
                        why: "aborted".to_owned(),
                    }
                }
            },
        };
        self.scanning(false, run.ctx());
        self.reread(run.ctx());
        outcome
    }
}

/// What an ended report says the run came to: its trouble, verbatim, when
/// there is one — a stop, every channel failing.
fn outcome_of(sheet: &Sheet) -> Outcome {
    if sheet.trouble.is_empty() {
        Outcome::Done(sheet.steps.clone())
    } else {
        // NO `at`: the scan has no steps of the brain's, and the screen draws
        // `at` as a step's name.
        Outcome::Failed {
            at: String::new(),
            why: sheet.trouble.clone(),
        }
    }
}

impl Watcher for VideoMetrics {
    /// The brain's report directory: the unit writes its report there when
    /// it starts and when it ends, whoever started it.
    fn sources(&self) -> Vec<Source> {
        vec![Source::Path(self.reports.clone())]
    }

    /// Catching up: the catalogue's last scan, and a scan that may be
    /// running since before this brain.
    fn started(&self, _now: Moment, ctx: &Ctx) {
        self.reread(ctx);
        self.heard_report(ctx);
    }

    fn heard(&self, heard: Heard<'_>, ctx: &Ctx) {
        if let Heard::Path { .. } = heard {
            self.heard_report(ctx);
        }
    }
}

impl VideoMetrics {
    /// What the unit's report says now: running (it began, has not ended,
    /// and the scan holds the catalogue — the unit writes "begun" only once
    /// it does), or ended — and then the catalogue is read again. A begun
    /// report nobody holds the catalogue for is a scan that was killed: no
    /// mark for it, however long ago it was written.
    fn heard_report(&self, ctx: &Ctx) {
        let Some(sheet) = read_sheet(&self.report()) else {
            return;
        };
        let running = sheet.began != 0
            && sheet.ended == 0
            && self
                .catalogue
                .as_deref()
                .is_some_and(|c| self.probe.holding(c));
        self.scanning(running, ctx);
        if !running {
            self.reread(ctx);
        }
    }
}

/// The report at `path`, when there is one that reads.
fn read_sheet(path: &Path) -> Option<Sheet> {
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    /// A machine the test sets.
    #[derive(Default)]
    struct Set {
        offline: AtomicBool,
        unknown: AtomicBool,
        saving: AtomicBool,
        holding: AtomicBool,
        last: Mutex<Option<Last>>,
    }

    impl Probe for Arc<Set> {
        fn online(&self) -> Option<bool> {
            if self.unknown.load(Ordering::Relaxed) {
                None
            } else {
                Some(!self.offline.load(Ordering::Relaxed))
            }
        }

        fn saving(&self) -> bool {
            self.saving.load(Ordering::Relaxed)
        }

        fn last_scan(&self, _catalogue: &Path) -> Option<Last> {
            *self.last.lock().unwrap()
        }

        fn holding(&self, _catalogue: &Path) -> bool {
            self.holding.load(Ordering::Relaxed)
        }
    }

    fn module(set: &Arc<Set>, reports: PathBuf) -> VideoMetrics {
        VideoMetrics::of(
            Box::new(Arc::clone(set)),
            Some(PathBuf::from("/nonexistent/catalog.sqlite")),
            reports,
        )
    }

    fn face_of(m: &VideoMetrics) -> Face {
        let mine = super::super::MANIFEST.defaults();
        let pulse = Pulse::default();
        m.face(&Look {
            now: 100,
            pulse: &pulse,
            mine: &mine,
            last: None,
        })
    }

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-brain-vm-module-{}-{tag}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    const fn last(channels: u32, failed: u32, ended: bool) -> Last {
        Last {
            started: 10,
            ended: if ended { Some(20) } else { None },
            channels,
            failed,
            added: 3,
        }
    }

    /// FAILS IF: a mark shows at rest, the scanning mark stops following a
    /// scan, or the danger mark shows for a scan in which some channel
    /// answered — or does not for one in which every channel failed.
    #[test]
    fn a_mark_shows_only_while_scanning_or_after_every_channel_failed() {
        let set = Arc::new(Set::default());
        let m = module(&set, PathBuf::from("/nonexistent"));
        let (ctx, kept) = Ctx::kept(super::super::ID, 100);
        assert!(face_of(&m).marks.is_empty(), "a mark at rest");

        m.scanning(true, &ctx);
        let face = face_of(&m);
        assert_eq!(face.marks.len(), 1);
        assert_eq!(face.marks[0].id, super::super::mark::SCANNING);
        assert_eq!(face.marks[0].tone, Tone::Watching);
        assert!(face.marks[0].act.is_none(), "a click opens its page");
        assert_eq!(kept.changes().len(), 1, "the change was not published");

        m.scanning(false, &ctx);
        for (record, marked) in [
            (last(3, 1, true), false),
            (last(3, 3, false), false),
            (last(0, 0, true), false),
            (last(3, 3, true), true),
        ] {
            *set.last.lock().unwrap() = Some(record);
            m.reread(&ctx);
            let face = face_of(&m);
            assert_eq!(!face.marks.is_empty(), marked, "{record:?}");
            if marked {
                assert_eq!(face.marks[0].id, super::super::mark::FAILED);
                assert_eq!(face.marks[0].tone, Tone::Danger);
            }
        }
        // AND A SCAN RUNNING says more than the last one failing.
        m.scanning(true, &ctx);
        assert_eq!(face_of(&m).marks[0].id, super::super::mark::SCANNING);
        assert_eq!(
            super::super::mark::FAILED.glyph,
            "video-metrics-failed",
            "the glyph token is the module's"
        );
    }

    /// FAILS IF: a run goes on with no way out, waits when nobody could say,
    /// starts a unit while the power saver is on or while the brain only
    /// rehearses, or comes due off its six-hour cadence.
    #[test]
    fn it_waits_for_the_network_and_skips_the_power_saver() {
        let set = Arc::new(Set::default());
        let m = module(&set, PathBuf::from("/nonexistent"));
        let mine = super::super::MANIFEST.defaults();
        let pulse = Pulse::default();
        assert_eq!(m.needs(&pulse, &mine), Verdict::Go);
        set.offline.store(true, Ordering::Relaxed);
        assert_eq!(m.needs(&pulse, &mine), Verdict::Hold(Hold::Network));
        set.unknown.store(true, Ordering::Relaxed);
        assert_eq!(m.needs(&pulse, &mine), Verdict::Go, "nobody said no");

        assert_eq!(mine.every_minutes, Some(360), "six hours");
        if systemd::reachable() {
            assert_eq!(m.due(1_000, &mine, None), Due::Now, "never scanned: now");
            assert_eq!(m.due(1_000, &mine, Some(1_000)), Due::At(1_000 + 360 * 60));
        }
        assert!(
            super::super::MANIFEST.gated(),
            "a scan waits for a quiet machine"
        );
        assert!(
            super::super::MANIFEST.needs.network,
            "a scan needs the network"
        );

        // THE POWER SAVER, and a brain that only rehearses: no unit started.
        let dir = scratch("skips");
        for (saving, rehearsal, why) in [(true, false, "saver"), (false, true, "rehearsing")] {
            set.saving.store(saving, Ordering::Relaxed);
            let (ctx, _) = Ctx::kept(super::super::ID, 100);
            let report = crate::task::Report::at(
                crate::runner::report_in(&dir, super::super::ID, rehearsal),
                std::time::Duration::from_millis(10),
                std::time::Duration::from_millis(1),
                Arc::new(AtomicBool::new(false)),
            );
            let report = if rehearsal {
                report.rehearsing()
            } else {
                report
            };
            let outcome = m.run(&Run::new(report, ctx));
            assert_eq!(
                outcome,
                Outcome::Skipped {
                    why: why.to_owned()
                }
            );
            assert!(!m.kept().scanning, "a skipped round marked a scan");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: the report the unit writes stops being heard as a scan
    /// running (begun, not ended, the catalogue held) and one that ended —
    /// whoever started it — an ended report does not read the catalogue
    /// again, or a begun report a killed scan left is taken for a scan
    /// running when the brain starts.
    #[test]
    fn the_units_report_is_heard_whoever_started_it() {
        let set = Arc::new(Set::default());
        let dir = scratch("report");
        let m = module(&set, dir.clone());
        let (ctx, _) = Ctx::kept(super::super::ID, 100);
        let file = crate::runner::report_in(&dir, super::super::ID, false);
        std::fs::write(&file, r#"{"began":10,"ended":0,"steps":[],"trouble":""}"#).unwrap();
        // A KILLED SCAN'S REPORT, read when the brain starts: begun, never
        // ended, and nobody holds the catalogue.
        m.started(100, &ctx);
        assert!(!m.kept().scanning, "a killed scan's report marked a scan");
        set.holding.store(true, Ordering::Relaxed);
        m.heard(
            Heard::Path {
                source: 0,
                path: &file,
            },
            &ctx,
        );
        assert!(m.kept().scanning, "a report begun was not a scan running");
        *set.last.lock().unwrap() = Some(last(2, 2, true));
        std::fs::write(
            &file,
            r#"{"began":10,"ended":30,"steps":[],"trouble":"every channel failed"}"#,
        )
        .unwrap();
        m.heard(
            Heard::Path {
                source: 0,
                path: &file,
            },
            &ctx,
        );
        assert!(!m.kept().scanning, "an ended report kept the mark");
        assert_eq!(face_of(&m).marks[0].id, super::super::mark::FAILED);
        assert_eq!(
            m.sources(),
            vec![Source::Path(dir.clone())],
            "the report's folder is what is watched"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: an ended report's trouble is not what the run's record says
    /// went wrong, or a clean one is not done.
    #[test]
    fn a_report_is_an_outcome() {
        let clean = Sheet {
            began: 1,
            ended: 2,
            ..Sheet::default()
        };
        assert_eq!(outcome_of(&clean), Outcome::Done(Vec::new()));
        let failed = Sheet {
            trouble: "stopped: bot-check".to_owned(),
            ..clean
        };
        assert_eq!(
            outcome_of(&failed),
            Outcome::Failed {
                at: String::new(),
                why: "stopped: bot-check".to_owned()
            }
        );
    }

    /// FAILS IF: the page stops saying what the last scan did — its failed
    /// channels in the danger tone when every one failed — or draws a page of
    /// zeros when there was none.
    #[test]
    fn the_page_says_what_the_last_scan_did() {
        let ids = |rows: &[Row]| rows.iter().map(|r| r.id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(&rows_of(None)), ["never"]);
        let rows = rows_of(Some(last(3, 3, true)));
        assert_eq!(
            ids(&rows),
            ["started", "ended", "channels", "failed", "added"]
        );
        assert_eq!(rows[3].tone, Tone::Danger);
        assert_eq!(rows[3].value, Cell::Count(3));
        let rows = rows_of(Some(last(3, 1, false)));
        assert_eq!(ids(&rows), ["started", "channels", "failed", "added"]);
        assert_eq!(rows[2].tone, Tone::Warning);
    }
}
