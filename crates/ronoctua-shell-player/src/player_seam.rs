//! THE PLAYER'S SEAM — the pages of the player written in crates ABOVE it
//! (the 2026-10-12 wave): channels by theme with its listing and its sources
//! (`ronoctua-shell-catalogue`) and the ranking of the video catalogue
//! (`ronoctua-shell-video-metrics`), each a [`Page`] the view holds beside
//! its own.
//!
//! # Why a seam
//!
//! The player was one cycle of forty modules: the view's enums named the
//! catalogue's keys, the catalogue's rows were methods of the view, and the
//! view called them. So a line of the catalogue's listing recompiled the
//! transport, the stage, the window, the footer's region — the whole player,
//! some thirty thousand lines — and a line of the transport the catalogue.
//! The seam cuts the cycle one way: the player's crate knows the WORDS of
//! those pages — their [`PageId`]s, their [`Target`]s, their keys
//! ([`ListingKey`], [`RankingKey`]), the errands and answers they hand a door
//! ([`ranking::Act`], [`ranking::Outcome`]) — and nothing of what they DO:
//! what they draw, what they read, what a key on them turns, which a crate
//! above writes as a [`Page`]. A line of the catalogue recompiles the
//! catalogue and the binary; a line of the transport, everything above it.
//!
//! # How a page is reached
//!
//! The binary INSTALLS the pages at start ([`install`], beside the keymap
//! and the theme), and every view made after holds one [`Plugged`] state of
//! each — made by the page itself ([`Page::state`]) — which the page reads
//! back through [`View::plugged`]. The view asks every page at the places it
//! asked the catalogue and the ranking before, in the same order: the rows of
//! a page it does not draw, a key on a target it does not turn, a question it
//! did not ask, what an answer off the loop means, a picture it does not
//! hold. A page answers only for its own and says `None` (or `false`) for
//! the rest — every method has that default.
//!
//! The page's behaviour is a `&'static` value and its state a box in the
//! view, so a page that calls back into the view (`settle_in`, `follow`,
//! `rows`), which asks every page again, re-enters nothing that is borrowed:
//! the state is borrowed only between those calls, as the field was.

use std::any::Any;
use std::cell::Cell;
use std::sync::OnceLock;

use crate::key;
use ronoctua_player::wire::State;

use crate::player::{Errand, PageId, Press, Target, Told, View};
use crate::player_rows::Builder;

/// A listing's own keys (`Press::Listing`) — a page of the catalogue's
/// (`ronoctua-shell-catalogue`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListingKey {
    /// The page before (`<prior>`).
    Earlier,
    /// The page after (`<next>`).
    Later,
    /// The first page (`<home>`).
    First,
    /// The last page (`<end>`).
    Last,
    /// The whole screen (`F`).
    Full,
    /// The next layout (`L`).
    Layout,
    /// The next source (`S`).
    Source,
    /// A letter, typed (`C-l`).
    Letter,
    /// The next order (`O`).
    Order,
}

/// The sentence of a trouble of the catalogue's files — what the player's
/// own refusal of a listing says ([`crate::player::trouble_sentence`]).
#[must_use]
pub const fn catalog_trouble_key(t: ronoctua_broadcast::catalog::Trouble) -> &'static str {
    use ronoctua_broadcast::catalog::Trouble as CatalogTrouble;
    match t {
        CatalogTrouble::NoHome => key::PLAYER_TROUBLE_CATALOG_NO_HOME,
        CatalogTrouble::Absent => key::PLAYER_TROUBLE_CATALOG_ABSENT,
        CatalogTrouble::OtherSchema => key::PLAYER_TROUBLE_CATALOG_OTHER_SCHEMA,
        CatalogTrouble::Unreadable => key::PLAYER_TROUBLE_CATALOG_UNREADABLE,
        CatalogTrouble::Unwritable => key::PLAYER_TROUBLE_CATALOG_UNWRITABLE,
        CatalogTrouble::TooLarge => key::PLAYER_TROUBLE_CATALOG_TOO_LARGE,
    }
}

/// The sentence of a source's refusal — the player's, about a source it was
/// asked to read.
#[must_use]
pub const fn source_trouble_key(r: ronoctua_broadcast::sources::Refused) -> &'static str {
    use ronoctua_broadcast::sources::Refused as R;
    match r {
        R::NotAnAddress => key::PLAYER_TROUBLE_SOURCE_NOT_AN_ADDRESS,
        R::NotAList => key::PLAYER_TROUBLE_SOURCE_NOT_A_LIST,
        R::Unknown => key::PLAYER_TROUBLE_SOURCE_UNKNOWN,
        R::NotLet => key::PLAYER_TROUBLE_SOURCE_NOT_LET,
        R::PlainHttp => key::PLAYER_TROUBLE_SOURCE_PLAIN_HTTP,
        R::Unreachable => key::PLAYER_TROUBLE_SOURCE_UNREACHABLE,
        R::Declined => key::PLAYER_TROUBLE_SOURCE_DECLINED,
        R::TooManyAsks => key::PLAYER_TROUBLE_SOURCE_TOO_MANY_ASKS,
        R::NothingFound => key::PLAYER_TROUBLE_SOURCE_NOTHING_FOUND,
        R::Malformed => key::PLAYER_TROUBLE_SOURCE_MALFORMED,
        R::TooLarge => key::PLAYER_TROUBLE_SOURCE_TOO_LARGE,
        R::NothingPlayable => key::PLAYER_TROUBLE_SOURCE_NOTHING_PLAYABLE,
        R::NoYtDlp => key::PLAYER_TROUBLE_SOURCE_NO_YT_DLP,
        R::ListingFailed => key::PLAYER_TROUBLE_SOURCE_LISTING_FAILED,
    }
}

/// The sentence of a PAM refusal (`player-adult-<id>`) — how the adult
/// catalogue's password check ended ([`Told::Unlock`]).
#[must_use]
pub const fn unlock_refused_key(r: ronoctua_pam::Refusal) -> &'static str {
    match r {
        ronoctua_pam::Refusal::Wrong => key::PLAYER_ADULT_WRONG,
        ronoctua_pam::Refusal::NoService => key::PLAYER_ADULT_NO_SERVICE,
        ronoctua_pam::Refusal::Unavailable => key::PLAYER_ADULT_UNAVAILABLE,
    }
}

/// The words of the RANKING of the video catalogue — a page of the
/// player's written in `ronoctua-shell-video-metrics`: its keys, the rows of
/// a video's own page, and the writes and errands a door carries out for it.
pub mod ranking {
    use std::path::PathBuf;

    use crate::key;
    use ronoctua_video_metrics::{VideoId, Why};

    /// The command `⋯` is, and `e`: the video's own page.
    pub const MORE: &str = "player-ranking-more";

    /// The command the title's glyph is, and `L`: the other layout.
    pub const LAYOUT: &str = "player-ranking-layout";

    /// The ranking's own keys (`Press::Ranking`).
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum RankingKey {
        /// The page before (`<prior>`).
        Earlier,
        /// The page after (`<next>`).
        Later,
        /// The first page (`<home>`).
        First,
        /// The last page (`<end>`).
        Last,
        /// The other layout (`L`).
        Layout,
        /// The video under the cursor's own page (`e`, the `⋯`).
        More,
    }

    impl RankingKey {
        /// The key of command `id`, when it is one of the ranking's.
        #[must_use]
        pub fn of_command(id: &str) -> Option<Self> {
            Some(match id {
                "player-ranking-earlier" => Self::Earlier,
                "player-ranking-later" => Self::Later,
                "player-ranking-first" => Self::First,
                "player-ranking-last" => Self::Last,
                LAYOUT => Self::Layout,
                MORE => Self::More,
                _ => return None,
            })
        }
    }

    /// What a row of a video's own page does.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum RankedAct {
        /// Played now, right after what plays; the queue stays.
        PlayNow,
        /// At the queue's end, once.
        Add,
        /// Watched in the Chrome kiosk.
        Kiosk,
        /// Downloaded, by the transfers' queue.
        Download,
        /// Hidden from the ranking.
        Hide,
        /// Deleted from the catalogue for good, after a question.
        Delete,
    }

    impl RankedAct {
        /// The six, in the page's order.
        pub const ALL: [Self; 6] = [
            Self::PlayNow,
            Self::Add,
            Self::Kiosk,
            Self::Download,
            Self::Hide,
            Self::Delete,
        ];

        /// Its row's label.
        #[must_use]
        pub const fn key(self) -> &'static str {
            match self {
                Self::PlayNow => key::PLAYER_RANKING_PLAY_NOW,
                Self::Add => key::PLAYER_RANKING_ADD,
                Self::Kiosk => key::PLAYER_RANKING_KIOSK,
                Self::Download => key::PLAYER_RANKING_DOWNLOAD,
                Self::Hide => key::PLAYER_RANKING_HIDE,
                Self::Delete => key::PLAYER_RANKING_DELETE,
            }
        }

        /// Its row's spot.
        #[must_use]
        pub const fn spot(self) -> &'static str {
            match self {
                Self::PlayNow => "ranked-play-now",
                Self::Add => "ranked-add",
                Self::Kiosk => "ranked-kiosk",
                Self::Download => "ranked-download",
                Self::Hide => "ranked-hide",
                Self::Delete => "ranked-delete",
            }
        }
    }

    /// A WRITE OR AN ERRAND for a video, carried out off the loop
    /// (`player::carry_out`, the ranking's [`super::Page::off_the_loop`])
    /// and said when it is done (`Told::Ranking`).
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum Act {
        /// Hidden from the ranking: it stays catalogued.
        Hide {
            /// The catalogue.
            catalogue: PathBuf,
            /// The video.
            id: VideoId,
            /// Its title, for the sentence.
            title: String,
        },
        /// Deleted for good: its tombstone keeps it from coming back.
        Delete {
            /// The catalogue.
            catalogue: PathBuf,
            /// The video.
            id: VideoId,
            /// Its title, for the sentence.
            title: String,
        },
        /// Downloaded by the transfers' queue, at its best, into the
        /// person's downloads.
        Download {
            /// Its address.
            url: String,
            /// Its title, for the sentence.
            title: String,
        },
    }

    /// What an [`Act`] came to.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum Outcome {
        /// Hidden, or why not.
        Hidden(VideoId, String, Result<(), Why>),
        /// Deleted, or why not.
        Deleted(VideoId, String, Result<(), Why>),
        /// Handed to the transfers' queue — `taken` — and how the queue is
        /// opened, as the leader teaches it.
        Downloading {
            /// Its title.
            title: String,
            /// The queue took it.
            taken: bool,
            /// `LEADER t t`, in the leader's words.
            keys: Option<String>,
        },
    }

    impl Act {
        /// What it comes to when it is done — a photograph's, which does
        /// nothing.
        #[must_use]
        pub fn done(&self) -> Outcome {
            match self {
                Self::Hide { id, title, .. } => Outcome::Hidden(id.clone(), title.clone(), Ok(())),
                Self::Delete { id, title, .. } => {
                    Outcome::Deleted(id.clone(), title.clone(), Ok(()))
                }
                Self::Download { title, .. } => Outcome::Downloading {
                    title: title.clone(),
                    taken: true,
                    keys: None,
                },
            }
        }
    }
}

pub use ranking::RankingKey;

/// AN ERRAND A PAGE CARRIES OUT OFF THE LOOP ([`Page::off_the_loop`]): a
/// write that waits for a scan, a password PAM checks for seconds — on a
/// thread of its own, and how it ended told back ([`Told`]).
pub struct OffTheLoop {
    /// What it is about to do, for the guard that keeps a test off the
    /// machine (`daemon::about_to_act`).
    pub about: &'static str,
    /// The thread's name.
    pub thread: &'static str,
    /// The work, and how it ended.
    pub work: Box<dyn FnOnce() -> Told + Send>,
    /// What the door says when no thread could be started — `None`, only
    /// the log.
    pub unstarted: Option<&'static str>,
}

/// A PAGE OF THE PLAYER written in a crate above it — see the module.
///
/// Every method but [`Page::name`] and [`Page::state`] has the default of a
/// page that has nothing to say about what it is asked: the view goes on
/// asking the others, and then does its own.
#[allow(unused_variables)]
pub trait Page: Sync {
    /// Its name, for the log and a test's message.
    fn name(&self) -> &'static str;

    /// Its state, held by every view: this machine's files when
    /// `this_machine` (a door on a screen), its defaults otherwise (a
    /// photograph's, a test's).
    fn state(&self, this_machine: bool) -> Box<dyn Any>;

    /// The title of page `page`, when it writes its own: whether it did.
    fn title(&self, view: &View, page: PageId, b: &mut Builder<'_>) -> bool {
        false
    }

    /// The rows of page `page`, when it is one of its own: whether it drew.
    fn rows(&self, view: &View, page: PageId, state: Option<&State>, b: &mut Builder<'_>) -> bool {
        false
    }

    /// WHAT ITS PAGES READ, on each settle of the view — off the loop in a
    /// door that has one ([`View::wake`]).
    fn look(
        &self,
        view: &mut View,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) {
    }

    /// A key or a click: the person is here.
    fn touched(&self, view: &mut View) {}

    /// A press that is its own — a key of its pages ([`Press::Listing`],
    /// [`Press::Ranking`]), or the search on one of them.
    fn pressed(
        &self,
        view: &mut View,
        press: Press,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Option<Errand> {
        None
    }

    /// A press on a row whose target is its own.
    fn turned(
        &self,
        view: &mut View,
        target: &Target,
        press: Press,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Option<Errand> {
        None
    }

    /// Whether question `id` is one of its own.
    fn asks(&self, id: &str) -> bool {
        false
    }

    /// What was typed for one of its questions ([`Page::asks`]).
    fn answered(
        &self,
        view: &mut View,
        id: &str,
        said: &str,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        Errand::Nothing
    }

    /// The door's line was let go: its question with it.
    fn let_go(&self, view: &mut View) {}

    /// The picture it holds under `key` — a poster, a thumbnail.
    fn picture<'v>(&self, view: &'v View, key: &str) -> Option<&'v crate::player_find::Picture> {
        None
    }

    /// How an ask told off the loop ended, for it: `Some(changed)` when the
    /// answer was its own.
    fn told(&self, view: &mut View, answer: &Told) -> Option<bool> {
        None
    }

    /// What a door says of an answer that is its own.
    fn said(&self, answer: &Told, texts: &ronoctua_i18n::Texts) -> Option<String> {
        None
    }

    /// An errand that is its own, carried out off the loop — or handed back.
    ///
    /// # Errors
    ///
    /// The errand is not its own.
    fn off_the_loop(&self, errand: Errand) -> Result<OffTheLoop, Errand> {
        Err(errand)
    }

    /// The question it owes the person now — the adult listing's password,
    /// asked by the door that waits for it.
    fn owed(&self, view: &mut View) -> Option<Errand> {
        None
    }

    /// The screen's scale the door draws at: `true` when that changed what
    /// it draws.
    fn scale_is(&self, view: &mut View, scale: u32) -> bool {
        false
    }

    /// This door takes the listing's stage itself — the window.
    fn takes_its_own_stage(&self, view: &mut View) {}

    /// Whether something of its own waits before the door's stage is right
    /// — an adult listing handed over, waiting for its password.
    fn waits(&self, view: &View) -> bool {
        false
    }

    /// The listing's stage was asked of a window: what another door handed
    /// over for it, opened — `true` when there was something.
    fn handed_over(&self, view: &mut View) -> bool {
        false
    }

    /// The window took the listing's stage itself: what another door handed
    /// over is let go.
    fn hand_back(&self) {}

    /// A detached door's view (a photograph's): what it is given to read
    /// instead of this machine's.
    fn detached(&self, view: &mut View, outbox: &crate::player_window::Outbox) {}

    /// `photograph --make-catalog <dir>`: what a photograph of the page
    /// reads (`--catalog`), written into `dir` — `None` when the page keeps
    /// nothing to write, `Some(Err(why))` when it was not written.
    fn write_fixture(&self, dir: &std::path::Path) -> Option<Result<(), String>> {
        None
    }

    /// `photograph --handed <file>`: the view opened on what a door of the
    /// page would hand over of the fixture [`Page::write_fixture`] wrote —
    /// everything of its person's source in `store`. `false` when the page
    /// hands nothing over.
    fn handed_fixture(&self, view: &mut View, store: ronoctua_broadcast::catalog::Store) -> bool {
        false
    }
}

/// One page, held by a view: the page and its state.
pub struct Plugged {
    /// What it does.
    pub page: &'static dyn Page,
    /// What it holds.
    pub state: Box<dyn Any>,
}

impl std::fmt::Debug for Plugged {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Plugged").field(&self.page.name()).finish()
    }
}

/// The pages the binary installed ([`install`]).
static INSTALLED: OnceLock<&'static [&'static dyn Page]> = OnceLock::new();

thread_local! {
    /// The pages of this thread's views, inside [`with_pages`] — a test's.
    static STUBBED: Cell<Option<&'static [&'static dyn Page]>> = const { Cell::new(None) };
}

/// THE PAGES every view of this process holds — the binary's to say, once,
/// at start.
pub fn install(pages: &'static [&'static dyn Page]) {
    if INSTALLED.set(pages).is_err() {
        tracing::warn!("the player's pages were installed twice; the first stays");
    }
}

/// The pages a view made now holds: this thread's within [`with_pages`],
/// else the installed ones — none in a test that installed nothing.
#[must_use]
pub fn installed() -> &'static [&'static dyn Page] {
    STUBBED
        .with(Cell::get)
        .or_else(|| INSTALLED.get().copied())
        .unwrap_or(&[])
}

/// `f` with `pages` as the pages of the views this thread makes — a test's,
/// whatever was installed, and only inside (`test-support`).
#[cfg(any(test, feature = "test-support"))]
pub fn with_pages<T>(pages: &'static [&'static dyn Page], f: impl FnOnce() -> T) -> T {
    let before = STUBBED.with(|s| s.replace(Some(pages)));
    let out = f();
    STUBBED.with(|s| s.set(before));
    out
}

/// The states a new view holds: one of each installed page's.
#[must_use]
pub fn plugs(this_machine: bool) -> Vec<Plugged> {
    installed()
        .iter()
        .map(|page| Plugged {
            page: *page,
            state: page.state(this_machine),
        })
        .collect()
}

/// Whether question `id` is a page's — what a door answers before it knows
/// whether a view is open (the footer's minibuffer, by name).
#[must_use]
pub fn asks(id: &str) -> bool {
    installed().iter().any(|p| p.asks(id))
}

/// What a door says of `answer`, when it is a page's.
#[must_use]
pub fn said(answer: &Told, texts: &ronoctua_i18n::Texts) -> Option<String> {
    installed().iter().find_map(|p| p.said(answer, texts))
}

/// The page that carries `errand` out off the loop, and how — or the errand
/// back when none does.
///
/// # Errors
///
/// No installed page carries it.
pub fn off_the_loop(errand: Errand) -> Result<OffTheLoop, Errand> {
    let mut errand = errand;
    for page in installed() {
        match page.off_the_loop(errand) {
            Ok(work) => return Ok(work),
            Err(back) => errand = back,
        }
    }
    Err(errand)
}

impl View {
    /// The state of the page whose state is a `T`, when the view holds one.
    #[must_use]
    pub fn plugged<T: Any>(&self) -> Option<&T> {
        self.plugs.iter().find_map(|p| p.state.downcast_ref::<T>())
    }

    /// The same, to change.
    pub fn plugged_mut<T: Any>(&mut self) -> Option<&mut T> {
        self.plugs
            .iter_mut()
            .find_map(|p| p.state.downcast_mut::<T>())
    }

    /// The view, holding `page` too — a test's, which installed nothing: a
    /// page it holds already is not held twice (`test-support`).
    #[cfg(any(test, feature = "test-support"))]
    #[must_use]
    pub fn plugging(mut self, page: &'static dyn Page) -> Self {
        if !self.plugs.iter().any(|p| p.page.name() == page.name()) {
            self.plugs.push(Plugged {
                page,
                state: page.state(false),
            });
        }
        self
    }

    /// The pages this view holds, in the order they were installed.
    pub(crate) fn pages(&self) -> Vec<&'static dyn Page> {
        self.plugs.iter().map(|p| p.page).collect()
    }

    /// The title a page writes for page `page`: whether one did.
    pub(crate) fn pages_title(&self, page: PageId, b: &mut Builder<'_>) -> bool {
        self.plugs.iter().any(|p| p.page.title(self, page, b))
    }

    /// The rows a page draws for page `page`: whether one did.
    pub(crate) fn pages_rows(
        &self,
        page: PageId,
        state: Option<&State>,
        b: &mut Builder<'_>,
    ) -> bool {
        self.plugs.iter().any(|p| p.page.rows(self, page, state, b))
    }

    /// Every page's look, in order.
    pub(crate) fn pages_look(
        &mut self,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) {
        for page in self.pages() {
            page.look(self, state, columns, texts);
        }
    }

    /// A key or a click, told to every page.
    pub(crate) fn pages_touched(&mut self) {
        for page in self.pages() {
            page.touched(self);
        }
    }

    /// A press a page takes as its own.
    pub(crate) fn pages_pressed(
        &mut self,
        press: Press,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Option<Errand> {
        self.pages()
            .into_iter()
            .find_map(|page| page.pressed(self, press, state, columns, texts))
    }

    /// A press on a row a page turns.
    pub(crate) fn pages_turned(
        &mut self,
        target: &Target,
        press: Press,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Option<Errand> {
        self.pages()
            .into_iter()
            .find_map(|page| page.turned(self, target, press, state, columns, texts))
    }

    /// Whether question `id` is one of a page's this view holds.
    pub(crate) fn pages_ask(&self, id: &str) -> bool {
        self.plugs.iter().any(|p| p.page.asks(id))
    }

    /// What was typed for a page's question.
    pub(crate) fn pages_answered(
        &mut self,
        id: &str,
        said: &str,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        match self.pages().into_iter().find(|p| p.asks(id)) {
            Some(page) => page.answered(self, id, said, state, columns, texts),
            None => Errand::Nothing,
        }
    }

    /// The door's line was let go: told to every page.
    pub(crate) fn pages_let_go(&mut self) {
        for page in self.pages() {
            page.let_go(self);
        }
    }

    /// A picture a page holds under `key`.
    pub(crate) fn pages_picture(&self, key: &str) -> Option<&crate::player_find::Picture> {
        self.plugs.iter().find_map(|p| p.page.picture(self, key))
    }

    /// An answer off the loop a page takes: `Some(changed)`.
    pub(crate) fn pages_told(&mut self, answer: &Told) -> Option<bool> {
        self.pages()
            .into_iter()
            .find_map(|page| page.told(self, answer))
    }

    /// The question a page owes the person now — the adult listing's
    /// password, asked by a window that waits for it.
    pub fn lock_owed(&mut self) -> Option<Errand> {
        self.pages().into_iter().find_map(|page| page.owed(self))
    }

    /// The screen's scale the door draws at — a poster is decoded to its
    /// pixels. `true` when it changed.
    pub fn scale_is(&mut self, scale: u32) -> bool {
        let mut changed = false;
        for page in self.pages() {
            changed |= page.scale_is(self, scale);
        }
        changed
    }

    /// This door takes the listing's stage itself — the window: `F` hands
    /// nothing over.
    pub fn takes_its_own_stage(&mut self) {
        for page in self.pages() {
            page.takes_its_own_stage(self);
        }
    }

    /// Whether an adult listing handed over waits for the person's
    /// password.
    #[must_use]
    pub fn listing_waits(&self) -> bool {
        self.plugs.iter().any(|p| p.page.waits(self))
    }

    /// A window opened for the listing's stage opens on what another door
    /// handed over — `true` when a page had something.
    pub fn handed_over(&mut self) -> bool {
        self.pages().into_iter().any(|page| page.handed_over(self))
    }

    /// A detached door's view reads what its photograph gave each page.
    pub fn detached_pages(&mut self, outbox: &crate::player_window::Outbox) {
        for page in self.pages() {
            page.detached(self, outbox);
        }
    }
}

/// The window took the listing's stage itself: what another door handed
/// over for it is let go.
pub fn hand_back() {
    for page in installed() {
        page.hand_back();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A page that draws one page of its own and turns one target.
    struct Fake;

    #[derive(Debug, Default)]
    struct FakeState {
        turned: u32,
    }

    impl Page for Fake {
        fn name(&self) -> &'static str {
            "fake"
        }
        fn state(&self, _: bool) -> Box<dyn Any> {
            Box::new(FakeState::default())
        }
        fn turned(
            &self,
            view: &mut View,
            target: &Target,
            _: Press,
            _: Option<&State>,
            _: u16,
            _: &ronoctua_i18n::Texts,
        ) -> Option<Errand> {
            if *target != Target::CatalogSource {
                return None;
            }
            view.plugged_mut::<FakeState>()?.turned += 1;
            Some(Errand::Said("turned".to_owned()))
        }
        fn asks(&self, id: &str) -> bool {
            id == "fake-question"
        }
    }

    static FAKE: Fake = Fake;
    const PAGES: &[&dyn Page] = &[&FAKE];

    /// FAILS IF: a view made inside `with_pages` does not hold the page, or
    /// one made outside it does — a test's page would leak into another
    /// thread's views.
    #[test]
    fn a_view_holds_the_pages_of_its_thread() {
        assert!(View::blank(None).plugged::<FakeState>().is_none());
        with_pages(PAGES, || {
            let v = View::blank(None);
            assert!(v.plugged::<FakeState>().is_some());
            assert!(asks("fake-question") && !asks("other"));
        });
        assert!(View::blank(None).plugged::<FakeState>().is_none());
    }

    /// FAILS IF: a target the page turns is not handed to it, or one it
    /// does not turn is taken for it.
    #[test]
    fn a_target_goes_to_the_page_that_turns_it() {
        let texts = ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT).expect("the base language");
        with_pages(PAGES, || {
            let mut v = View::blank(None);
            let turned = v.pages_turned(&Target::CatalogSource, Press::Enter, None, 80, &texts);
            assert_eq!(turned, Some(Errand::Said("turned".to_owned())));
            assert_eq!(v.plugged::<FakeState>().map(|s| s.turned), Some(1));
            assert_eq!(
                v.pages_turned(&Target::Nothing, Press::Enter, None, 80, &texts),
                None
            );
        });
    }
}
