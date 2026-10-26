//! Modules that only exist in tests, and the manifests they declare.
//!
//! FICTIONAL IDS, on purpose: the core's fixtures never name a real module
//! (the sealed check refuses a module's name outside its folder), and a test
//! that borrowed one would be testing that module instead of the core.
//!
//! A [`Fake`] answers the contract with closures a test hands in, so each test
//! writes only the one answer it is about and the rest are the contract's own
//! quiet defaults.

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

use crate::module::{
    Act, Acted, Ctx, Face, Heard, Kind, Look, Manifest, Module, Needs, Run, Schedule, Scheduled,
    Screen, Source, Units, Watcher,
};
use crate::moment::Moment;
use crate::prefs::{PartOfDay, TaskPrefs};
use crate::pulse::Pulse;
use crate::task::{Due, Outcome, Step, Verdict};

/// A clock the test moves by hand.
#[derive(Debug, Default, Clone)]
pub struct Hand(pub Arc<AtomicI64>);

impl Hand {
    /// One reading `at`.
    pub fn at(at: Moment) -> Self {
        Self(Arc::new(AtomicI64::new(at)))
    }

    /// Moves it.
    pub fn set(&self, at: Moment) {
        self.0.store(at, Ordering::Relaxed);
    }
}

impl crate::moment::Clock for Hand {
    fn now(&self) -> Moment {
        self.0.load(Ordering::Relaxed)
    }
}

/// Something a test builds once and keeps for the rest of the process —
/// which is what a `&'static` manifest is.
pub fn leak<T>(t: T) -> &'static T {
    Box::leak(Box::new(t))
}

/// The schedule of a fake: weekly, in the afternoon, no countdown, gated.
pub const ROUTINE: Schedule = Schedule {
    kind: Kind::Routine,
    calendar: true,
    interval_days: 7,
    part_of_day: PartOfDay::Afternoon,
    every_minutes: None,
    countdown_seconds: 0,
    runaway_minutes: 60,
    gated: true,
};

/// A manifest that declares a routine and nothing else.
pub fn manifest(id: &'static str) -> Manifest {
    Manifest {
        id,
        what: "A module that only exists in a test.",
        name_key: leak(format!("brain-task-{id}")),
        does_key: leak(format!("brain-task-{id}-does")),
        needs: Needs::default(),
        schedule: Some(ROUTINE),
        starts_off: false,
        units: Units::default(),
        whys: &[],
        marks: &[],
        notices: &[],
        actions: &[],
        events: &[],
    }
}

type DueFn = dyn Fn(Moment, &TaskPrefs, Option<Moment>) -> Due + Send + Sync;
type WantedFn = dyn Fn(Moment, &Pulse, &TaskPrefs, Option<Moment>) -> bool + Send + Sync;
type NeedsFn = dyn Fn(&Pulse, &TaskPrefs) -> Verdict + Send + Sync;
type RunFn = dyn Fn(&Run) -> Outcome + Send + Sync;
type LookFn<T> = dyn Fn(&Look<'_>) -> T + Send + Sync;
type ActFn = dyn Fn(&Act, &Ctx) -> Acted + Send + Sync;
type HeardFn = dyn Fn(Heard<'_>, &Ctx) + Send + Sync;
type WakeFn = dyn Fn(Moment) -> Option<Moment> + Send + Sync;
type StartedFn = dyn Fn(Moment, &Ctx) + Send + Sync;

/// A module made of closures.
pub struct Fake {
    manifest: &'static Manifest,
    steps: &'static [Step],
    due: Box<DueFn>,
    wanted: Box<WantedFn>,
    needs: Box<NeedsFn>,
    run: Box<RunFn>,
    face: Box<LookFn<Face>>,
    screen: Box<LookFn<Screen>>,
    act: Box<ActFn>,
    sources: Vec<Source>,
    /// Whether it watches at all — a module that only wants a clock has no
    /// source and is a watcher all the same.
    listening: bool,
    heard: Box<HeardFn>,
    wake: Box<WakeFn>,
    started: Box<StartedFn>,
    /// Every dismissal and snooze the core passed on, in order.
    pub put_away: Mutex<Vec<(String, Option<Moment>)>>,
}

impl Fake {
    /// One answering only its manifest; every other answer is the default.
    pub fn new(manifest: Manifest) -> Self {
        Self {
            manifest: leak(manifest),
            steps: &[],
            due: Box::new(|_, _, _| Due::Not { why: "test" }),
            wanted: Box::new(|_, _, _, _| false),
            needs: Box::new(|_, _| Verdict::Go),
            run: Box::new(|_| Outcome::Done(Vec::new())),
            face: Box::new(|_| Face::default()),
            screen: Box::new(|_| Screen::default()),
            act: Box::new(|_, _| Acted::Refused("unknown")),
            sources: Vec::new(),
            listening: false,
            heard: Box::new(|_, _| {}),
            wake: Box::new(|_| None),
            started: Box::new(|_, _| {}),
            put_away: Mutex::new(Vec::new()),
        }
    }

    /// The same closures, under another manifest.
    #[must_use]
    pub fn declaring(mut self, manifest: Manifest) -> Self {
        self.manifest = leak(manifest);
        self
    }

    /// A routine by this id.
    pub fn routine(id: &'static str) -> Self {
        Self::new(manifest(id))
    }

    /// A watch by this id: no schedule at all.
    pub fn watch(id: &'static str) -> Self {
        Self::new(Manifest {
            schedule: None,
            ..manifest(id)
        })
    }

    /// When it is due.
    #[must_use]
    pub fn due(
        mut self,
        f: impl Fn(Moment, &TaskPrefs, Option<Moment>) -> Due + Send + Sync + 'static,
    ) -> Self {
        self.due = Box::new(f);
        self
    }

    /// When the machine wants it.
    #[must_use]
    pub fn wanted(
        mut self,
        f: impl Fn(Moment, &Pulse, &TaskPrefs, Option<Moment>) -> bool + Send + Sync + 'static,
    ) -> Self {
        self.wanted = Box::new(f);
        self
    }

    /// What only it knows about whether it may run.
    #[must_use]
    pub fn needs(
        mut self,
        f: impl Fn(&Pulse, &TaskPrefs) -> Verdict + Send + Sync + 'static,
    ) -> Self {
        self.needs = Box::new(f);
        self
    }

    /// What its run does.
    #[must_use]
    pub fn runs(mut self, f: impl Fn(&Run) -> Outcome + Send + Sync + 'static) -> Self {
        self.run = Box::new(f);
        self
    }

    /// Its face.
    #[must_use]
    pub fn face(mut self, f: impl Fn(&Look<'_>) -> Face + Send + Sync + 'static) -> Self {
        self.face = Box::new(f);
        self
    }

    /// Its screen.
    #[must_use]
    pub fn screen(mut self, f: impl Fn(&Look<'_>) -> Screen + Send + Sync + 'static) -> Self {
        self.screen = Box::new(f);
        self
    }

    /// What its actions do.
    #[must_use]
    pub fn acts(mut self, f: impl Fn(&Act, &Ctx) -> Acted + Send + Sync + 'static) -> Self {
        self.act = Box::new(f);
        self
    }

    /// Its steps.
    #[must_use]
    pub const fn steps(mut self, steps: &'static [Step]) -> Self {
        self.steps = steps;
        self
    }

    /// What it listens to, and what it does with what it hears.
    #[must_use]
    pub fn listens(
        mut self,
        sources: Vec<Source>,
        f: impl Fn(Heard<'_>, &Ctx) + Send + Sync + 'static,
    ) -> Self {
        self.sources = sources;
        self.listening = true;
        self.heard = Box::new(f);
        self
    }

    /// When it wants a wake.
    #[must_use]
    pub fn wakes(mut self, f: impl Fn(Moment) -> Option<Moment> + Send + Sync + 'static) -> Self {
        self.wake = Box::new(f);
        self.listening = true;
        self
    }

    /// What it does when the watch thread starts.
    #[must_use]
    pub fn starts(mut self, f: impl Fn(Moment, &Ctx) + Send + Sync + 'static) -> Self {
        self.started = Box::new(f);
        self
    }

    /// Boxed, for a registry.
    pub fn boxed(self) -> Box<dyn Module> {
        Box::new(self)
    }
}

impl Module for Fake {
    fn manifest(&self) -> &'static Manifest {
        self.manifest
    }

    fn steps(&self) -> &'static [Step] {
        self.steps
    }

    fn scheduled(&self) -> Option<&dyn Scheduled> {
        self.manifest.schedule.map(|_| self as &dyn Scheduled)
    }

    fn watcher(&self) -> Option<&dyn Watcher> {
        self.listening.then_some(self as &dyn Watcher)
    }

    fn face(&self, look: &Look<'_>) -> Face {
        (self.face)(look)
    }

    fn screen(&self, look: &Look<'_>) -> Screen {
        (self.screen)(look)
    }

    fn act(&self, act: &Act, ctx: &Ctx) -> Acted {
        (self.act)(act, ctx)
    }

    fn dismissed(&self, notice: &str, _ctx: &Ctx) {
        self.put_away
            .lock()
            .expect("put away")
            .push((notice.to_owned(), None));
    }

    fn snoozed(&self, notice: &str, until: Moment, _ctx: &Ctx) {
        self.put_away
            .lock()
            .expect("put away")
            .push((notice.to_owned(), Some(until)));
    }
}

impl Scheduled for Fake {
    fn due(&self, now: Moment, mine: &TaskPrefs, last: Option<Moment>) -> Due {
        (self.due)(now, mine, last)
    }

    fn needs(&self, pulse: &Pulse, mine: &TaskPrefs) -> Verdict {
        (self.needs)(pulse, mine)
    }

    fn wanted_now(
        &self,
        now: Moment,
        pulse: &Pulse,
        mine: &TaskPrefs,
        last: Option<Moment>,
    ) -> bool {
        (self.wanted)(now, pulse, mine, last)
    }

    fn run(&self, run: &Run) -> Outcome {
        (self.run)(run)
    }
}

impl Watcher for Fake {
    fn sources(&self) -> Vec<Source> {
        self.sources.clone()
    }

    fn started(&self, now: Moment, ctx: &Ctx) {
        (self.started)(now, ctx);
    }

    fn heard(&self, heard: Heard<'_>, ctx: &Ctx) {
        (self.heard)(heard, ctx);
    }

    fn next_wake(&self, now: Moment) -> Option<Moment> {
        (self.wake)(now)
    }
}
