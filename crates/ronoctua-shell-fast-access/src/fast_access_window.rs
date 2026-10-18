//! FAST-ACCESS IN A WINDOW AND IN THE COLUMN — the second and third doors
//! onto `crate::fast_access` (`LEADER d w`, `LEADER d c`, `LEADER SPC`), and
//! the glue all three doors share ([`Search`]).
//!
//! # One view, three doors
//!
//! The view is `fast_access.rs`'s: what is typed, what came back, the
//! cursor, what a key does. [`Search`] is what every door holds around it —
//! the feed (`fast_access_search::Feed`), the in-process sources' ground, the
//! file panel's threads (`inspection::Threads`) — and it turns the view's
//! errands into what a door does ([`Out`]): close, tell the footer, say,
//! ask. The footer's region (`fast_access_region`) holds one too, so a key, a
//! question answered, a click and a thread's answer do the same in all
//! three.
//!
//! # Launches go through the footer
//!
//! A window that starts a program and closes the next moment can take the
//! program with it; so every opening — an application, a file, a command, a
//! video — is an order for the footer, which outlives the window. The window
//! closes only when the footer TOOK the order (`Ui::hand_over`): a footer
//! that is not there leaves the window up, saying so. The column stays.
//!
//! # Floating over everything (`LEADER SPC`)
//!
//! The same window under another `app_id`
//! ([`crate::fast_access::APP_ID_FLOAT`]), which the compositor floats at the
//! whole room by the rule `crate::floating` generates — a window, not chrome
//! (invariant 14).
//!
//! # Kept ready (`fast-access.json`, `park`)
//!
//! With the setting on, closing does not end the process: the window's role
//! is given up (`Shell::Parked`), the index's reader closes with the feed,
//! and the process listens on its socket (`fast-access.sock`) — only while it
//! is parked. The next `ronoctua-shell fast-access` hands it the query in
//! one line and ends; the window comes back with a new feed.

use std::cell::RefCell;
use std::io::{BufRead as _, Read as _, Write as _};
use std::rc::Rc;
use std::sync::{Arc, LazyLock, Mutex};

use anyhow::{Context as _, Result};
use ronoctua_tokens as tokens;
use smithay_client_toolkit::{
    reexports::calloop::channel,
    seat::{
        keyboard::{KeyEvent, Modifiers},
        pointer::{PointerEvent, PointerEventKind},
    },
    shell::{
        WaylandSurface as _,
        wlr_layer::LayerShell,
        xdg::{XdgShell, window::WindowDecorations},
    },
};
use wayland_client::protocol::wl_surface;

use crate::door::{BACK_BUTTON, Canvas, Content, Door, Hand, LEFT_BUTTON, Shell, Ui, Wiring};
use crate::fast_access::{Errand, Ground, Press, View};
use crate::fast_access_rows::{Drawing, Lay, Target};
use crate::fast_access_search::{Back, Feed, Sink};
use crate::icons_page::Area;

/// The runtime socket a kept window listens on while it is parked.
pub const SOCKET: &str = "fast-access.sock";

/// The glyph in front of the header — the search's token.
static GLYPH: LazyLock<String> = LazyLock::new(|| tokens::icon::SEARCH.to_string());

/// What a door does for the view.
#[derive(Debug, Clone, PartialEq)]
pub enum Out {
    /// The view closes (`Esc`): every door closes it.
    Close,
    /// Something was opened: the footer's view and the window close; the
    /// column stays.
    Done,
    /// An order for the footer: a command and its argument.
    Footer(&'static str, Option<String>),
    /// The programs' centre on a search, by its attribute.
    Software(String),
    /// Say this on the door's line.
    Say(String),
    /// The door's question line.
    Ask(crate::line::Line),
    /// Read the clipboard into what is typed.
    Paste,
}

/// What every door holds around the view.
pub struct Search {
    /// The view.
    pub view: View,
    /// Its threads — `None` while the view is not on a screen (a kept window,
    /// a parked column).
    pub feed: Option<Feed>,
    /// What the in-process sources read.
    pub ground: Ground,
    /// The file panel's threads (the preview, the film, the shape).
    pub threads: crate::inspection::Threads,
    /// The person's zone.
    pub zone: ronoctua_files::when::Zone,
    /// Where the settings are kept (`fast-access.json`) — a test's keeps
    /// nothing.
    pub store: ronoctua_store::Store<crate::fast_access::Settings>,
}

impl std::fmt::Debug for Search {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Search")
            .field("view", &self.view)
            .finish_non_exhaustive()
    }
}

impl Search {
    /// A search over `view`, its threads `feed`.
    #[must_use]
    pub fn new(
        view: View,
        feed: Option<Feed>,
        ground: Ground,
        threads: crate::inspection::Threads,
    ) -> Self {
        let zone = ground.zone.clone();
        Self {
            view,
            feed,
            ground,
            threads,
            zone,
            store: crate::fast_access::store(),
        }
    }

    /// What a thread came back with. `None` when nothing moved; else what the
    /// door is to do (often nothing but a frame).
    pub fn back(&mut self, back: Back, texts: &Rc<ronoctua_i18n::Texts>) -> Option<Vec<Out>> {
        let moved = match back {
            Back::Answer(a) => self.view.arrived(a),
            Back::Icons(set) => {
                self.view.took_icons(set, texts);
                true
            }
            Back::Picture { video, key, rgba } => self.view.pictured(&video, key, rgba),
            Back::Rates(r) => {
                self.view.rates_are(Some(r), &self.ground, texts);
                true
            }
            Back::RatesFailed {
                generation,
                why,
                stale,
            } => self
                .view
                .rates_failed(generation, why, stale, &self.ground, texts),
            Back::Apps(catalogue) => {
                self.ground.catalogue = catalogue;
                // THE APPLICATIONS ARE IN-PROCESS: their section answered
                // again in place for what is typed now — data, not a key:
                // nothing asked again, the page and the cursor where they are.
                self.view.apps_are(&self.ground)
            }
            Back::Man { page, lines } => {
                self.view.detail.man = Some((page, lines));
                true
            }
            Back::Program { attr, program } => {
                self.view.detail.program = Some((attr, *program));
                true
            }
            Back::Amended(amended) => {
                // THE PANEL READS THE FILE AGAIN: its key carries the mtime.
                let said = crate::inspection_edit::sentence(&amended, texts);
                self.repoint(texts);
                return Some(said.map(Out::Say).into_iter().collect());
            }
            Back::Measured(path, bytes) => {
                if let Some(p) = self.view.detail.panel.as_ref()
                    && p.path == path
                {
                    let roster = ronoctua_files::who::Roster::read_from(
                        &ronoctua_files::who::Files::default(),
                    );
                    let mut subject = crate::inspection::Subject::of_path(&path, &roster);
                    if let Some(s) = subject.as_mut() {
                        s.measured = Some(bytes);
                    }
                    p.panel.focus(subject);
                }
                true
            }
        };
        moved.then(Vec::new)
    }

    /// The panel pointed at its file again — after a change to it.
    fn repoint(&mut self, texts: &Rc<ronoctua_i18n::Texts>) {
        self.view.detail.of = None;
        self.view.settle_detail(texts, &self.zone);
    }

    /// The panels' question asked and answered AT ONCE, down the path an
    /// answer from the inspector's thread takes — a photograph's, whose door
    /// has no threads (the window's camera, the footer's region in a
    /// photograph). `true` when there was a question.
    pub fn inspect_now(&mut self, texts: &Rc<ronoctua_i18n::Texts>) -> bool {
        self.view.settle_detail(texts, &self.zone);
        let Some(ask) = self.view.detail.panel.as_mut().and_then(|p| {
            p.panel
                .wants_look(u32::from(tokens::space::FILES_PANELPIXELS))
        }) else {
            return false;
        };
        let looked = crate::inspection::look(ask);
        self.panels_heard(crate::inspection::Heard::Looked(Box::new(looked)))
    }

    /// The file panel's threads came back. Answers whether a frame is owed.
    pub fn panels_heard(&mut self, heard: crate::inspection::Heard) -> bool {
        let Some(p) = self.view.detail.panel.as_mut() else {
            return false;
        };
        if !p.panel.heard(heard) {
            return false;
        }
        self.threads.play(|| p.panel.wants_motion());
        self.threads.turn(|| p.panel.wants_turn());
        true
    }

    /// What the frame about to be drawn needs asked: the detail on the
    /// cursor's hit, the file panel's preview, film and shape, the videos'
    /// pictures and a manual page's lines. Each a comparison of values: a
    /// wish already told costs nothing.
    pub fn look_again(&mut self, texts: &Rc<ronoctua_i18n::Texts>) {
        self.view.settle_detail(texts, &self.zone);
        if let Some(p) = self.view.detail.panel.as_mut() {
            self.threads.look(|| {
                p.panel
                    .wants_look(u32::from(tokens::space::FILES_PANELPIXELS))
            });
            self.threads.play(|| p.panel.wants_motion());
            self.threads.turn(|| p.panel.wants_turn());
        }
        let Some(feed) = self.feed.as_ref() else {
            return;
        };
        for (video, file) in self.view.pictures_wanted() {
            feed.picture(&video, file);
        }
        match self.view.current().map(|h| h.what.clone()) {
            Some(crate::fast_access::What::Man { name, section }) => {
                let page = format!("{name}.{section}");
                if self.view.detail.man_asked.as_deref() != Some(page.as_str()) {
                    self.view.detail.man_asked = Some(page);
                    feed.man(&name, &section);
                }
            }
            // A PROGRAM'S DETAILS, asked once per program on screen.
            Some(crate::fast_access::What::Package { attr })
                if self.view.detail.program_asked.as_deref() != Some(attr.as_str()) =>
            {
                self.view.detail.program_asked = Some(attr.clone());
                feed.program(&attr, texts.language());
            }
            _ => {}
        }
    }

    /// A key, read and done.
    pub fn key(&mut self, press: Press, area: Area, texts: &Rc<ronoctua_i18n::Texts>) -> Vec<Out> {
        let errands = self.view.key(press, area, &self.ground, texts);
        self.errands(errands, texts)
    }

    /// What was typed on the door's question line, under the id that asked.
    pub fn answered(&mut self, id: &str, text: &str, texts: &Rc<ronoctua_i18n::Texts>) -> Vec<Out> {
        // THE PERSON'S SEARXNG: kept in `fast-access.json` when it is an
        // address, said when it is not.
        if id == crate::fast_access::ASK_SEARXNG {
            return match crate::fast_access::searxng_typed(&self.view.settings, text, texts) {
                Ok(settings) => {
                    self.view.settings_are(settings.clone(), texts);
                    self.errands(vec![crate::fast_access::Errand::Keep(settings)], texts)
                }
                Err(said) => vec![Out::Say(said)],
            };
        }
        // THE BRAVE KEY: a `0600` file of its own, never `fast-access.json`;
        // nothing typed forgets it.
        if id == crate::fast_access::ASK_BRAVE {
            let typed = text.trim();
            let kept = if typed.is_empty() {
                ronoctua_websearch::keys::forget_brave().map(|()| "fast-access-brave-forgotten")
            } else {
                ronoctua_websearch::keys::keep_brave(&ronoctua_consent::keys::Secret::new(typed))
                    .map(|()| "fast-access-brave-kept")
            };
            let said = match kept {
                Ok(key) => texts.t(key),
                Err(why) => texts.t(&format!("fast-access-web-key-{}", why.id())),
            };
            let settings = self.view.settings.clone();
            self.view.settings_are(settings, texts);
            return vec![Out::Say(said)];
        }
        if id == crate::fast_access_detail::ASK_COLOUR {
            text.trim()
                .clone_into(&mut self.view.detail.icon_settings.lucide.colour);
            return Vec::new();
        }
        let Some((amend, typed)) = crate::fast_access_detail::answered(&mut self.view, id, text)
        else {
            return Vec::new();
        };
        self.amend(amend, Some(&typed), texts)
    }

    /// A change to the file under the detail — done here when it is quick, on
    /// a thread when it is a child or a walk.
    fn amend(
        &mut self,
        amend: crate::fast_access_detail::Amend,
        typed: Option<&str>,
        texts: &Rc<ronoctua_i18n::Texts>,
    ) -> Vec<Out> {
        use crate::fast_access_detail::Amend;
        let refused = |e: &dyn std::fmt::Display| {
            vec![Out::Say(texts.t_with(
                crate::key::FILES_REFUSED,
                &[("why", &e.to_string())],
            ))]
        };
        let typed = typed.unwrap_or_default();
        match amend {
            Amend::Rename(path) => {
                let name = typed.trim();
                if name.is_empty() || name.contains('/') {
                    return Vec::new();
                }
                match ronoctua_files::act::rename(&path, &path.with_file_name(name)) {
                    // THE RESULTS ASK AGAIN: the old name is gone.
                    Ok(()) => {
                        let errands = self.view.retyped(&self.ground, texts);
                        self.errands(errands, texts)
                    }
                    Err(e) => refused(&e),
                }
            }
            Amend::Modified(path) => {
                let Some(secs) = ronoctua_files::when::instant_of(typed, &self.zone) else {
                    return vec![Out::Say(texts.t(crate::key::FILES_MODIFIED_BAD))];
                };
                match ronoctua_files::act::set_modified(&path, secs) {
                    Ok(()) => {
                        self.repoint(texts);
                        Vec::new()
                    }
                    Err(e) => refused(&e),
                }
            }
            Amend::Tag(path, field) => {
                let stamp = self
                    .view
                    .detail
                    .panel
                    .as_ref()
                    .and_then(crate::fast_access_detail::amendable_of)
                    .and_then(|a| crate::inspection_edit::tag_now(a, field))
                    .map(|(_, stamp)| stamp);
                let Some(stamp) = stamp else {
                    return Vec::new();
                };
                match crate::inspection_edit::tag_answer(field, typed, texts) {
                    Ok(edit) => {
                        if let Some(feed) = self.feed.as_ref() {
                            feed.amend(move || {
                                crate::inspection_edit::write_tags(&path, stamp, &[edit])
                            });
                        }
                        Vec::new()
                    }
                    Err(said) => vec![Out::Say(said)],
                }
            }
            Amend::Meta(path, meta) => {
                let holder = self
                    .view
                    .detail
                    .panel
                    .as_ref()
                    .and_then(crate::fast_access_detail::amendable_of)
                    .and_then(|a| crate::inspection_edit::detail_now(a, meta))
                    .map(|(_, holder)| holder);
                let Some(holder) = holder else {
                    return Vec::new();
                };
                let value = typed.to_owned();
                if let Some(feed) = self.feed.as_ref() {
                    feed.amend(move || {
                        crate::inspection_edit::write_detail(&path, holder, meta, &value)
                    });
                }
                Vec::new()
            }
            Amend::RemoveLocation(path, copies) => {
                if let Some(feed) = self.feed.as_ref() {
                    feed.amend(move || crate::inspection_edit::remove_locations(&[path], copies));
                }
                Vec::new()
            }
            Amend::Mode(path, mode) => match ronoctua_files::act::chmod(&path, mode) {
                Ok(()) => {
                    self.repoint(texts);
                    Vec::new()
                }
                Err(e) => refused(&e),
            },
            // THE OWNER IS CHANGED WHERE IT CAN BE ASKED FOR: the file
            // manager's door, shown on the file.
            Amend::Owner(path) => vec![
                Out::Footer("fast-access-reveal", Some(path.display().to_string())),
                Out::Done,
            ],
            Amend::Measure(path) => {
                if let Some(feed) = self.feed.as_ref() {
                    feed.measure(path);
                }
                Vec::new()
            }
            // A QUESTION OF AN INSTALL ANSWERED YES: the next one, or the
            // queue.
            Amend::Install {
                attr,
                name,
                consent,
            } => {
                let errands = self.view.install(&attr, &name, consent, texts);
                self.errands(errands, texts)
            }
        }
    }

    /// The view's errands, done where they can be and handed to the door
    /// where they cannot.
    pub fn errands(&mut self, errands: Vec<Errand>, texts: &Rc<ronoctua_i18n::Texts>) -> Vec<Out> {
        let mut out = Vec::new();
        for e in errands {
            match e {
                Errand::Close => out.push(Out::Close),
                Errand::Done => out.push(Out::Done),
                Errand::Search(ask) => match self.feed.as_ref() {
                    Some(feed) => feed.ask(&ask),
                    // NO THREADS, AND SAID: never a page that waits for ever.
                    None => self.view.refuse_pending(&crate::fast_access::Said::key(
                        "fast-access-index-why-unreadable".to_owned(),
                    )),
                },
                Errand::Launch(id) => out.push(Out::Footer("fast-access-launch", Some(id))),
                Errand::OpenPath(p) => out.push(Out::Footer(
                    "fast-access-open-path",
                    Some(p.display().to_string()),
                )),
                Errand::Reveal(p) => out.push(Out::Footer(
                    "fast-access-reveal",
                    Some(p.display().to_string()),
                )),
                Errand::Run(id) => out.push(Out::Footer(id, None)),
                Errand::Man(page) => out.push(Out::Footer("fast-access-man", Some(page))),
                Errand::Player(url) => out.push(Out::Footer("player-play-file", Some(url))),
                Errand::Kiosk(url) => out.push(Out::Footer("kiosk-open", Some(url))),
                Errand::Software(attr) => out.push(Out::Software(attr)),
                Errand::Install {
                    attr,
                    name,
                    consent,
                } => out.push(Out::Say(queue_install(
                    attr,
                    &name,
                    consent,
                    &self.ground.queue_keys,
                    texts,
                ))),
                Errand::Say(text) => out.push(Out::Say(text)),
                Errand::Copy(text) => out.push(Out::Say(crate::chrome::copied(&text, texts))),
                Errand::Paste => out.push(Out::Paste),
                Errand::Allow(hosts) => {
                    let hosts: Vec<&str> = hosts.iter().map(String::as_str).collect();
                    match crate::consent_rows::allow(&hosts, texts) {
                        Ok(()) => {
                            // LET, AND ASKED AGAIN by the same words: the rates, too,
                            // come by `/$`'s own fetch, stopped by the next key.
                            let again = self.view.retyped(&self.ground, texts);
                            out.extend(self.errands(again, texts));
                        }
                        Err(said) => out.push(Out::Say(said)),
                    }
                }
                Errand::Keep(settings) => {
                    if let Err(why) = keep(&self.store, settings) {
                        out.push(Out::Say(
                            texts.t_with(crate::key::FAST_ACCESS_UNKEPT, &[("why", &why)]),
                        ));
                    }
                }
                Errand::Ask(line) => out.push(Out::Ask(line)),
                Errand::Amend(a) => out.extend(self.amend(a, None, texts)),
            }
        }
        out
    }

    /// The view's keys: its own, the strip's and the YouTube scope's while
    /// they act, the consent row's while one is drawn — `Esc` first.
    #[must_use]
    pub fn hint(&self, texts: &ronoctua_i18n::Texts) -> Vec<crate::chrome::Piece> {
        let mut table: Vec<(&'static str, &'static str)> = crate::fast_access::OF_THE_VIEW.to_vec();
        if self.view.query().scope == crate::fast_access::Scope::YouTube {
            table.extend_from_slice(crate::fast_access::OF_YOUTUBE);
        }
        if matches!(
            self.view.current().map(|h| &h.what),
            Some(crate::fast_access::What::Icon(_))
        ) {
            table.extend_from_slice(crate::fast_access::OF_A_STRIP);
        }
        if self
            .view
            .hits()
            .any(|h| matches!(h.what, crate::fast_access::What::Consent { .. }))
        {
            table.extend_from_slice(crate::consent_rows::OF_THE_CONSENT);
        }
        crate::chrome::with_the_way_back(crate::chrome::hint_of(&table, texts), texts)
    }

    /// What the pointer rests on has to add, whole: a fact of the file's
    /// panels that its column cut (`fast_access_detail::about`).
    #[must_use]
    ///
    /// AND A STRIP'S: an icon says its collection and its name, a page mark
    /// which collection it turns — the strips are icons only, so this is
    /// where a strip says whose it is.
    pub fn about(&self, area: Area, texts: &ronoctua_i18n::Texts) -> Option<String> {
        let lay = Lay::of(&self.view, area, texts);
        let collection = |c: usize| {
            ronoctua_iconset::Collection::ALL
                .get(c)
                .map(|c| texts.t(&crate::fast_access_rows::strip_key(*c)))
        };
        match self.view.hover? {
            Target::Detail(t) => {
                crate::fast_access_detail::about(&self.view, lay.detail?, t, texts)
            }
            Target::Hit(h) => {
                let crate::fast_access::What::Icon(icon) = lay.hits.get(h)?.what else {
                    return None;
                };
                let set = self.view.icons.as_deref()?;
                let name = crate::fast_access::icon_name(set, icon);
                Some(texts.t_with(
                    crate::key::FAST_ACCESS_STRIP_ABOUT,
                    &[
                        (
                            "collection",
                            &texts.t(&crate::fast_access_rows::strip_key(icon.collection)),
                        ),
                        ("name", &name),
                    ],
                ))
            }
            Target::Earlier(c) => Some(texts.t_with(
                crate::key::FAST_ACCESS_STRIP_EARLIER,
                &[("collection", &collection(c)?)],
            )),
            Target::Later(c) => Some(texts.t_with(
                crate::key::FAST_ACCESS_STRIP_LATER,
                &[("collection", &collection(c)?)],
            )),
            _ => None,
        }
    }

    /// The view leaves the screen: its threads end with the feed — the
    /// index's reader closes — and the film and the shape stop.
    pub fn let_go(&mut self) {
        self.feed = None;
        self.threads.play(|| Some(crate::motion::Ask::Stop));
        self.threads.turn(|| Some(crate::stage::Ask::Stop));
    }

    /// The settings file changed — this door's write, or another's.
    pub fn settings_changed(&mut self, texts: &ronoctua_i18n::Texts) {
        if let Some(s) = self.store.parsed() {
            self.view.settings_are(s, texts);
        }
    }

    /// The feed raised again over `sink` — a kept window back on a screen, a
    /// column shown again — and what is typed asked again.
    pub fn raise(&mut self, sink: &Sink, texts: &Rc<ronoctua_i18n::Texts>) -> Vec<Out> {
        let feed = Feed::start(Arc::clone(sink));
        feed.apps(texts.language());
        self.feed = Some(feed);
        let errands = self.view.retyped(&self.ground, texts);
        self.errands(errands, texts)
    }
}

/// A program's install, sent to the queue as the programs' engine's work
/// (`Work::Software`) through the client every door uses — and the sentence
/// that says where to watch it, or that the queue is not there.
fn queue_install(
    attr: String,
    name: &str,
    consent: ronoctua_software::order::Consent,
    queue_keys: &str,
    texts: &ronoctua_i18n::Texts,
) -> String {
    let work = ronoctua_transfers::Work::Software {
        order: Box::new(ronoctua_software::Order::Install {
            key: ronoctua_software::AppKey::Nix { attr },
            consent,
        }),
        label: name.to_owned(),
    };
    match ronoctua_transfers::client::tell(&ronoctua_transfers::Ask::Add { work }) {
        Ok(_) => texts.t_with(
            crate::key::SOFTWARE_ECHO_QUEUED,
            &[("app", name), ("keys", queue_keys)],
        ),
        Err(e) => {
            tracing::warn!(error = %e, "the transfer queue did not take the install");
            texts.t(crate::key::TRANSFERS_OFFLINE)
        }
    }
}

/// `settings` written into `fast-access.json`, MERGED over what the file says
/// now: two doors write it (the footer and a window), and what one
/// remembered opening must not erase what the other did. A file that does
/// not parse is left alone and said.
///
/// # Errors
///
/// Why it was not written, in the machine's words.
pub fn keep(
    store: &ronoctua_store::Store<crate::fast_access::Settings>,
    mine: crate::fast_access::Settings,
) -> std::result::Result<(), String> {
    let Some(on_disk) = store.parsed() else {
        return Err(format!(
            "{} does not parse; it was left as it is",
            crate::fast_access::FILE
        ));
    };
    store
        .write(&merged(on_disk, mine))
        .map_err(|e| e.to_string())
}

/// The file's settings with `mine` over them: the switches are mine, the
/// recent are both, by id — each with the most uses and the latest time.
#[must_use]
pub fn merged(
    file: crate::fast_access::Settings,
    mine: crate::fast_access::Settings,
) -> crate::fast_access::Settings {
    let mut recent = file.recent;
    for o in mine.recent.iter().cloned() {
        match recent.iter_mut().find(|r| r.id == o.id) {
            Some(r) => {
                r.uses = r.uses.max(o.uses);
                if o.at_s > r.at_s {
                    r.at_s = o.at_s;
                    r.title = o.title;
                }
            }
            None => recent.push(o),
        }
    }
    recent.sort_by_key(|r| r.at_s);
    let over = recent
        .len()
        .saturating_sub(crate::fast_access::recent_most());
    recent.drain(..over);
    crate::fast_access::Settings { recent, ..mine }
}

/// What the in-process sources read in a window or a column: the commands
/// with their keys from the default tables, the person's zone, and an empty
/// catalogue the feed fills.
///
/// # Errors
///
/// The installed keymap did not close (a build defect, said).
pub fn ground() -> Result<Ground> {
    let commands = crate::bindings::read(crate::fast_access_search::commands_of)?;
    Ok(Ground {
        catalogue: Arc::default(),
        commands,
        zone: ronoctua_files::when::Zone::read_from(std::path::Path::new("/etc/localtime")),
        zoneinfo: std::env::var_os("RONOCTUA_ZONEINFO").map(std::path::PathBuf::from),
        queue_keys: crate::queue::queue_keys(),
    })
}

// ---------------------------------------------------------------------------
// The window, and the column's tenant.
// ---------------------------------------------------------------------------

/// What one pointer keeps of its own over the view: nothing past the door's
/// — the bar it holds is the view's (`View::grabbed`).
pub type PerPointer = ();

/// The window — or the column's tenant.
pub struct FastAccess {
    /// The search.
    pub search: Search,
    /// Floating over the whole room (`LEADER SPC`).
    float: bool,
    /// Whether this is the column's tenant.
    column: bool,
    /// Where the threads answer — kept to raise a feed again.
    sink: Option<Sink>,
    /// A kept window's way to its door (`park`), and back from its socket.
    parking: Option<channel::Sender<()>>,
    woken: Option<channel::Sender<Wake>>,
    /// The globals a kept window comes back with: the `xdg_wm_base` its
    /// window was made by, kept while it is parked, and the layer shell a
    /// parked surface keeps.
    xdg: RefCell<Option<XdgShell>>,
    layer: RefCell<Option<Rc<LayerShell>>>,
    /// Whether the window is put away.
    pub parked: bool,
}

impl FastAccess {
    /// A door over `search` — what the photograph draws, its answers in.
    #[must_use]
    pub const fn over(search: Search, float: bool, column: bool) -> Self {
        Self {
            search,
            float,
            column,
            sink: None,
            parking: None,
            woken: None,
            xdg: RefCell::new(None),
            layer: RefCell::new(None),
            parked: false,
        }
    }

    /// Where the view is drawn in a window: from the line under the header —
    /// its drawer and its sentence included — to the bottom.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn area(ui: &Ui) -> Area {
        let top = ui.first_of_list();
        Area {
            left: ui.margin,
            top,
            columns: ui.columns,
            lines: ui.lines.saturating_sub(top),
            right_px: ui.width as f32,
            bottom_px: ui.height as f32,
        }
    }

    /// What a thread came back with, done in this door.
    pub(crate) fn took(&mut self, ui: &mut Ui, back: Back) {
        let texts = Rc::clone(&ui.texts);
        if let Some(outs) = self.search.back(back, &texts) {
            self.apply(ui, outs);
        }
    }

    /// Whether closing puts the window away instead.
    fn keeps(&self) -> bool {
        !self.column
            && self.search.view.settings.park
            && self.parking.is_some()
            && self.layer.borrow().is_some()
    }

    /// What the view asked, done in this door.
    fn apply(&mut self, ui: &mut Ui, outs: Vec<Out>) {
        let done = outs.contains(&Out::Done);
        for o in outs {
            match o {
                Out::Close => {
                    // `Esc` ON AN EMPTY BOX closes every door — the column
                    // too, which gives its slot up.
                    if self.column {
                        ui.quit = true;
                    } else {
                        self.close(ui);
                    }
                }
                Out::Done => {
                    // THE COLUMN STAYS. A window closes once the footer took
                    // what it was told — `hand_over` below — or at once
                    // when there was nothing to tell.
                }
                Out::Footer(id, arg) => self.order(ui, done, id, arg.as_deref()),
                Out::Software(attr) => {
                    let words = format!("search {attr}");
                    self.order(ui, done, "software-window", Some(&words));
                }
                Out::Say(text) => ui.say(text),
                Out::Ask(line) => ui.ask(line),
                Out::Paste => ui.paste(),
            }
        }
        ui.ask_paint();
    }

    /// One order for the footer — handed over when it closes the window.
    fn order(&mut self, ui: &mut Ui, done: bool, id: &str, arg: Option<&str>) {
        if !done || self.column {
            ui.tell_the_footer(id, arg);
        } else if self.keeps() {
            ui.tell_the_footer(id, arg);
            self.close(ui);
        } else {
            // THE WINDOW CLOSES ONLY WHEN THE FOOTER TOOK IT (`Ui::hand_over`).
            ui.hand_over(id, arg);
        }
    }

    /// Closes the window — or puts it away, when the person keeps it ready.
    fn close(&mut self, ui: &mut Ui) {
        if !self.keeps() {
            ui.quit = true;
            return;
        }
        if self.parked {
            return;
        }
        if let Some(p) = self.parking.as_ref()
            && p.send(()).is_ok()
        {
            self.parked = true;
            // THE READER CLOSES WITH THE FEED: nothing of the index stays
            // open behind a window nobody sees.
            self.search.let_go();
        } else {
            ui.quit = true;
        }
    }

    /// A key, read by its notation through the reader the footer calls too
    /// (`fast_access::press_of_key`).
    fn typed_here(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers, area: Area) {
        let Some(press) =
            crate::door::keys::key_of(event, mods).and_then(crate::fast_access::press_of_key)
        else {
            return;
        };
        ui.message = None;
        let texts = Rc::clone(&ui.texts);
        let outs = self.search.key(press, area, &texts);
        self.apply(ui, outs);
    }

    /// One pointer's frame: the header's controls first, a bar held, then a
    /// target, then the wheel.
    fn pointer_here(
        &mut self,
        ui: &mut Ui,
        hand: &mut Hand<()>,
        events: &[PointerEvent],
        area: Area,
    ) {
        let texts = Rc::clone(&ui.texts);
        let head = ui.frame(0);
        let before = (
            hand.row_under_pointer,
            self.search.view.cursor_id().map(str::to_owned),
        );
        let mut header = false;
        let mut moved = false;
        let mut outs: Vec<Out> = Vec::new();
        let mut turned: Option<(f32, f32)> = None;
        for e in events {
            #[allow(clippy::cast_possible_truncation)]
            let (px, py) = (e.position.0 as f32, e.position.1 as f32);
            match e.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    if ui.hinted.dragged(head, e.position.1) {
                        header = true;
                        continue;
                    }
                    let now = ui
                        .hinted
                        .under(e.position.0, e.position.1)
                        .and_then(|id| crate::chrome::hint_sentence(id, &texts));
                    moved |= ui.hinted.say(now);
                    if crate::fast_access_rows::held(&mut self.search.view, area, (px, py), &texts)
                    {
                        moved = true;
                    } else {
                        let lay = Lay::of(&self.search.view, area, &texts);
                        let target = lay.target_at(&self.search.view, px, py, &texts);
                        hand.row_under_pointer = target.map(Target::code);
                        crate::fast_access_rows::hovered(
                            &mut self.search.view,
                            area,
                            target,
                            py,
                            &texts,
                        );
                    }
                }
                PointerEventKind::Leave { .. } => {
                    ui.hinted.let_go();
                    moved |= ui.hinted.say(None);
                    self.search.view.grabbed = None;
                    crate::fast_access_rows::hovered(&mut self.search.view, area, None, py, &texts);
                    hand.row_under_pointer = None;
                }
                PointerEventKind::Release { button, .. } if button == LEFT_BUTTON => {
                    ui.hinted.let_go();
                    self.search.view.grabbed = None;
                }
                PointerEventKind::Press { button, .. } if button == LEFT_BUTTON => {
                    if ui.hinted.pressed(head, e.position.0, e.position.1) {
                        header = true;
                        continue;
                    }
                    let lay = Lay::of(&self.search.view, area, &texts);
                    if let Some(t) = lay.target_at(&self.search.view, px, py, &texts) {
                        let errands = crate::fast_access_rows::pressed(
                            &mut self.search.view,
                            t,
                            area,
                            (px, py),
                            &self.search.ground,
                            &texts,
                        );
                        outs.extend(self.search.errands(errands, &texts));
                        moved = true;
                    }
                }
                // THE BACK BUTTON IS `Esc` over the window, as everywhere.
                PointerEventKind::Press { button, .. } if button == BACK_BUTTON => {
                    outs.extend(self.search.key(Press::Escape, area, &texts));
                    moved = true;
                }
                PointerEventKind::Axis {
                    vertical,
                    horizontal,
                    ..
                } => {
                    crate::wheel::wheel_arrived(&mut hand.lines_owed, &vertical, &horizontal);
                    turned = Some((px, py));
                }
                _ => {}
            }
        }
        if header {
            ui.ask_paint();
            return;
        }
        if let Some((x, y)) = turned {
            let by = crate::wheel::wheel_lines(&mut hand.lines_owed);
            // A WHEEL OVER THE OPEN DRAWER WALKS THE DRAWER.
            if by != 0 && ui.hinted.turned(head, f64::from(y), by) {
                ui.ask_paint();
                return;
            }
            if by != 0 {
                crate::fast_access_rows::wheel_at(&mut self.search.view, area, x, y, by, &texts);
                moved = true;
            }
        }
        if !outs.is_empty() {
            self.apply(ui, outs);
        } else if moved
            || before
                != (
                    hand.row_under_pointer,
                    self.search.view.cursor_id().map(str::to_owned),
                )
        {
            ui.ask_paint();
        }
    }

    /// The frame: the header with `extra` keys after the view's, then the
    /// view in `area`.
    fn paint_in(
        &mut self,
        ui: &mut Ui,
        canvas: &mut Canvas<'_>,
        area: Area,
        extra: Vec<crate::chrome::Piece>,
    ) {
        let texts = Rc::clone(&ui.texts);
        self.search.look_again(&texts);
        let title = texts.t(crate::key::FAST_ACCESS_TITLE);
        let search = &self.search;
        // WHAT THE POINTER RESTS ON says it whole on line 0 — a fact of the
        // file's panels the column cut — as the file manager's door does.
        let about = search.about(area, &texts);
        crate::door::list::header_about(
            ui,
            canvas,
            Some(crate::chrome::Title {
                glyph: Some(GLYPH.as_str()),
                label: None,
                name: Some(&title),
            }),
            about,
            |ui| {
                let mut hint = search.hint(&ui.texts);
                if !hint.is_empty() && !extra.is_empty() {
                    hint.push(crate::chrome::Piece::separator());
                }
                hint.extend(extra);
                hint
            },
        );
        // EVERY HAND'S TARGET, in its own colour.
        let lit: Vec<(tokens::Color, Target)> = canvas
            .lit
            .iter()
            .filter_map(|(c, code)| code.map(|n| (*c, Target::of_code(n))))
            .collect();
        let mut drawing = Drawing::default();
        crate::fast_access_rows::draw(&self.search.view, area, true, &lit, &texts, &mut drawing);
        paint_drawing(drawing, canvas);
    }
}

/// A [`Drawing`] on a door's canvas: the grounds under, the pictures up (a
/// moving one every frame, a still one when missing), the thumbs over, the
/// text, and every bar's track recorded — its column is the bar's
/// (`crate::bars`).
pub fn paint_drawing(drawing: Drawing, canvas: &mut Canvas<'_>) {
    canvas.under.extend(drawing.under);
    for p in drawing.pictures {
        let missing = canvas.images.missing(&p.key);
        if let Some(px) = p.pixels(missing) {
            canvas.replace(&p.key, &px);
        } else if missing {
            // A PICTURE WITH NO PIXELS (an SVG that did not draw) is not
            // drawn: a texture missing is a hole, never a stale one.
            continue;
        }
        canvas.pictures.push((p.key, p.area));
    }
    canvas.over.extend(drawing.over);
    for t in drawing.tracks {
        canvas.bar(Some(t));
    }
    canvas.spans.extend(drawing.spans);
}

impl Content for FastAccess {
    type Hand = PerPointer;

    fn app_id(&self) -> &'static str {
        if self.float {
            crate::fast_access::APP_ID_FLOAT
        } else {
            crate::fast_access::APP_ID
        }
    }

    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        texts.t(crate::key::FAST_ACCESS_TITLE)
    }

    /// A window of its own — and the layer shell bound beside it: a parked
    /// surface keeps its `Rc` (`Shell::Parked`).
    fn open(&self, wiring: &Wiring<'_, Self>, surface: wl_surface::WlSurface) -> Result<Shell> {
        let xdg = XdgShell::bind(wiring.globals, wiring.qh)
            .context("the compositor does not expose xdg_shell")?;
        let window = xdg.create_window(surface, WindowDecorations::RequestServer, wiring.qh);
        window.set_title(Content::title(self, &wiring.texts));
        window.set_app_id(Content::app_id(self));
        window.commit();
        *self.layer.borrow_mut() = crate::door::layer_shell(wiring).ok();
        Ok(Shell::Window {
            window,
            _global: xdg,
        })
    }

    fn default_size(&self) -> (u32, u32) {
        (
            u32::from(ronoctua_tokens::space::WINDOW_FASTACCESSWIDTHPX),
            u32::from(ronoctua_tokens::space::WINDOW_FASTACCESSHEIGHTPX),
        )
    }

    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers) {
        let area = Self::area(ui);
        self.typed_here(ui, event, mods, area);
    }

    fn answered(&mut self, ui: &mut Ui, id: &'static str, text: &str) {
        let texts = Rc::clone(&ui.texts);
        let outs = self.search.answered(id, text, &texts);
        self.apply(ui, outs);
    }

    fn unanswered(&mut self, ui: &mut Ui, _: &'static str) {
        self.search.view.detail.asking = None;
        ui.ask_paint();
    }

    fn pasted(&mut self, ui: &mut Ui, text: &str) {
        let texts = Rc::clone(&ui.texts);
        let errands = self.search.view.pasted(text, &self.search.ground, &texts);
        let outs = self.search.errands(errands, &texts);
        self.apply(ui, outs);
    }

    fn hint(&self, ui: &Ui) -> Vec<crate::chrome::Piece> {
        self.search.hint(&ui.texts)
    }

    /// No rows: the view draws itself.
    fn rows(&self, _: &Ui) -> Vec<Vec<crate::fields::Field>> {
        Vec::new()
    }

    /// A held button on the view is a press or a drag of a bar, never a
    /// sweep of text.
    fn text_area(&self, _: &Ui, _: u16, _: u16) -> Option<crate::text_select::Area> {
        None
    }

    fn pointer(&mut self, ui: &mut Ui, hand: &mut Hand<PerPointer>, events: &[PointerEvent]) {
        ui.hinted.swap_hand(&mut hand.hint_drag);
        let area = Self::area(ui);
        self.pointer_here(ui, hand, events, area);
        ui.hinted.swap_hand(&mut hand.hint_drag);
    }

    fn paint(&mut self, ui: &mut Ui, canvas: &mut Canvas<'_>) {
        let area = Self::area(ui);
        self.paint_in(ui, canvas, area, Vec::new());
    }

    fn language_changed(&mut self, ui: &mut Ui) {
        let texts = Rc::clone(&ui.texts);
        let errands = self.search.view.retyped(&self.search.ground, &texts);
        let outs = self.search.errands(errands, &texts);
        self.apply(ui, outs);
    }

    /// THE COMPOSITOR CLOSED IT: a kept window is put away instead.
    fn closing(&mut self, ui: &mut Ui) -> bool {
        if self.keeps() {
            self.close(ui);
            return ui.quit;
        }
        true
    }
}

impl crate::column::Tenant for FastAccess {
    type Hand = PerPointer;

    const WHO: crate::right_column::Tenant = crate::right_column::Tenant::FastAccess;

    fn app_id(&self) -> &'static str {
        crate::fast_access::APP_ID
    }

    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        texts.t(crate::key::FAST_ACCESS_TITLE)
    }

    fn paint(&mut self, ui: &mut Ui, canvas: &mut Canvas<'_>, area: &crate::column::Area) {
        let view_area = column_area(ui, area);
        let teach = area.teach(&ui.texts);
        self.paint_in(ui, canvas, view_area, teach);
    }

    fn pointer(
        &mut self,
        ui: &mut Ui,
        hand: &mut Hand<PerPointer>,
        events: &[PointerEvent],
        area: &crate::column::Area,
    ) {
        ui.hinted.swap_hand(&mut hand.hint_drag);
        let view_area = column_area(ui, area);
        self.pointer_here(ui, hand, events, view_area);
        ui.hinted.swap_hand(&mut hand.hint_drag);
    }

    fn typed(
        &mut self,
        ui: &mut Ui,
        event: &KeyEvent,
        mods: Modifiers,
        area: &crate::column::Area,
    ) {
        let view_area = column_area(ui, area);
        self.typed_here(ui, event, mods, view_area);
    }

    fn answered(&mut self, ui: &mut Ui, id: &'static str, text: &str) {
        Content::answered(self, ui, id, text);
    }

    fn unanswered(&mut self, ui: &mut Ui, id: &'static str) {
        Content::unanswered(self, ui, id);
    }

    fn pasted(&mut self, ui: &mut Ui, text: &str) {
        Content::pasted(self, ui, text);
    }

    fn text_area(
        &self,
        _: &Ui,
        _: u16,
        _: u16,
        _: &crate::column::Area,
    ) -> Option<crate::text_select::Area> {
        None
    }

    fn language_changed(&mut self, ui: &mut Ui) {
        Content::language_changed(self, ui);
    }

    /// OFF THE SCREEN the column's reader closes: its feed goes, and a new
    /// one is raised when it is shown again.
    fn parked(&mut self, _: &mut Ui) {
        self.search.let_go();
    }

    fn shown(&mut self, ui: &mut Ui) {
        if self.search.feed.is_none()
            && let Some(sink) = self.sink.clone()
        {
            let texts = Rc::clone(&ui.texts);
            let outs = self.search.raise(&sink, &texts);
            self.apply(ui, outs);
        }
    }

    fn closing(&mut self, _: &mut Ui) {
        self.search.let_go();
    }
}

/// The view's area inside a column: under its control line and its header,
/// down to the tenant's last line.
#[allow(clippy::cast_precision_loss)]
fn column_area(ui: &Ui, area: &crate::column::Area) -> Area {
    let top = ui.first_of_list().max(area.top + 1);
    Area {
        left: ui.margin,
        top,
        columns: ui.columns,
        lines: area.end(ui.lines).saturating_sub(top),
        right_px: ui.width as f32,
        bottom_px: ui.height as f32,
    }
}

/// A door the search is in — the window, or the column around it — and how
/// to reach it inside, so the channels hung on its loop (and the camera's
/// walk) are written once.
pub(crate) trait Host: Content {
    /// The search's door inside the content.
    fn tenant(content: &mut Self) -> &mut FastAccess;
}

impl Host for FastAccess {
    fn tenant(content: &mut Self) -> &mut FastAccess {
        content
    }
}

impl Host for crate::column::Column<FastAccess> {
    fn tenant(content: &mut Self) -> &mut FastAccess {
        &mut content.tenant
    }
}

/// The window, or the column's tenant, with its channels hung on the loop
/// of the door `C` it is in: the threads' answers, the file panel's, and
/// the settings file watched.
fn wired<C: Host>(
    w: &Wiring<'_, C>,
    float: bool,
    column: bool,
    query: Option<&str>,
) -> Result<(FastAccess, Option<ronoctua_files::notice::Notice>)> {
    let texts = Rc::clone(&w.texts);
    let (sender, heard) = channel::channel::<Back>();
    let sender = Mutex::new(sender);
    let sink: Sink = Arc::new(move |b| {
        if let Ok(s) = sender.lock() {
            let _ = s.send(b);
        }
    });
    w.handle
        .insert_source(heard, |event, (), d: &mut Door<C>| {
            if let channel::Event::Msg(back) = event {
                let (me, ui) = (C::tenant(&mut d.content), &mut d.ui);
                let texts = Rc::clone(&ui.texts);
                if let Some(outs) = me.search.back(back, &texts) {
                    me.apply(ui, outs);
                }
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the search's channel: {e}"))?;
    let threads = crate::inspection::Threads::raise(&w.handle, |d: &mut Door<C>, h| {
        let (me, ui) = (C::tenant(&mut d.content), &mut d.ui);
        if me.search.panels_heard(h) {
            ui.ask_paint();
        }
    })?;
    // `fast-access.json`, WATCHED: the recent another door remembered, the
    // switches it turned (AGENTS: the file is the broadcast).
    let (told, changed) = channel::channel::<()>();
    w.handle
        .insert_source(changed, |event, (), d: &mut Door<C>| {
            if let channel::Event::Msg(()) = event {
                let (me, ui) = (C::tenant(&mut d.content), &mut d.ui);
                let texts = Rc::clone(&ui.texts);
                me.search.settings_changed(&texts);
                // KEPT READY NO MORE: a parked window whose setting was
                // turned off ends.
                if me.parked && !me.search.view.settings.park {
                    ui.quit = true;
                }
                ui.ask_paint();
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the settings' watch: {e}"))?;
    // NO WATCH IS NO FAILURE: the door works, and only another door's write
    // waits for the next time this one reads.
    let watch = ronoctua_store::config_dir().and_then(|dir| {
        ronoctua_files::notice::Notice::on_file(
            &dir,
            crate::fast_access::FILE,
            ronoctua_files::notice::Pace::Coalesced,
            move || told.send(()).is_ok(),
        )
        .map_err(|e| tracing::info!(error = %e, "fast-access: the settings are not watched"))
        .ok()
    });
    let feed = Feed::start(Arc::clone(&sink));
    feed.apps(texts.language());
    let view = View::new(crate::fast_access::store().read(), &texts);
    let mut search = Search::new(view, Some(feed), ground()?, threads);
    if let Some(q) = query.filter(|q| !q.trim().is_empty()) {
        let errands = search.view.typed(q, &search.ground, &texts);
        let _ = search.errands(errands, &texts);
    }
    let mut me = FastAccess::over(search, float, column);
    me.sink = Some(sink);
    Ok((me, watch))
}

/// Opens fast-access in a window — floating over the whole room when
/// `float` — typed with `query`. A window kept ready takes the query on its
/// socket instead, and this process ends.
///
/// # Errors
///
/// No Wayland compositor, or no `xdg_shell`.
pub fn host_window(float: bool, query: Option<String>) -> Result<()> {
    if let Some(path) = ronoctua_xdg::runtime_file(SOCKET)
        && hand_to_the_kept(&path, float, query.as_deref())
    {
        return Ok(());
    }
    // THE WATCH LIVES AS LONG AS THE DOOR: held by this frame, which the
    // door's run outlives only by its return.
    let held: RefCell<Option<ronoctua_files::notice::Notice>> = RefCell::new(None);
    crate::door::run(|w: &Wiring<'_, FastAccess>| {
        let (mut me, watch) = wired(w, float, false, query.as_deref())?;
        *held.borrow_mut() = watch;
        // THE WAY TO ITS DOOR, for a window that puts itself away, and back.
        let (parking, park_asked) = channel::channel::<()>();
        w.handle
            .insert_source(park_asked, |event, (), d: &mut Door<FastAccess>| {
                if let channel::Event::Msg(()) = event {
                    park(d);
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the parking channel: {e}"))?;
        let (woken, wakes) = channel::channel::<Wake>();
        w.handle
            .insert_source(wakes, |event, (), d: &mut Door<FastAccess>| {
                if let channel::Event::Msg(wake) = event {
                    unpark(d, &wake);
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the wake channel: {e}"))?;
        me.parking = Some(parking);
        me.woken = Some(woken);
        Ok(me)
    })
}

/// Opens fast-access in the column on the right, as `opening` says, typed
/// with `query`.
///
/// # Errors
///
/// No Wayland compositor, or no layer shell.
pub fn host_column(opening: &crate::column::Opening, query: Option<String>) -> Result<()> {
    let held: RefCell<Option<ronoctua_files::notice::Notice>> = RefCell::new(None);
    crate::column::run::<FastAccess>(opening, |w| {
        let (me, watch) = wired(w, false, true, query.as_deref())?;
        *held.borrow_mut() = watch;
        Ok(me)
    })
}

/// What a second `ronoctua-shell fast-access` hands a kept window: one line.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Wake {
    /// Floating over the whole room.
    pub float: bool,
    /// What to type.
    pub query: Option<String>,
}

/// The line a wake is sent as — whole, so it is written with ONE `write_all`.
#[must_use]
pub fn wake_line(wake: &Wake) -> Option<String> {
    let mut line = serde_json::to_string(wake).ok()?;
    line.push('\n');
    Some(line)
}

/// A wake read from one line — at most [`WAKE_MOST`] bytes.
#[must_use]
pub fn wake_of(line: &str) -> Option<Wake> {
    serde_json::from_str(line.trim()).ok()
}

/// The longest wake read: a query is a line typed, never a file.
pub const WAKE_MOST: u64 = 16 * 1024;

/// Hands the query to a window kept ready, when one listens on `path`.
/// Answers whether one took it.
fn hand_to_the_kept(path: &std::path::Path, float: bool, query: Option<&str>) -> bool {
    let Ok(mut stream) = std::os::unix::net::UnixStream::connect(path) else {
        return false;
    };
    let wake = Wake {
        float,
        query: query.map(ToOwned::to_owned),
    };
    let Some(line) = wake_line(&wake) else {
        return false;
    };
    // ONE `write_all` FOR THE WHOLE LINE (AGENTS: a JSON line is written
    // whole, or a reader gets half of it).
    stream.write_all(line.as_bytes()).is_ok()
}

/// Puts a kept window away: the window's role is given up — its surface goes
/// with it — and a bare surface keeps the door alive, listening on its
/// socket for the next `ronoctua-shell fast-access`.
fn park(d: &mut Door<FastAccess>) {
    let Some(layer) = d.content.layer.borrow().clone() else {
        d.ui.quit = true;
        return;
    };
    let Some(path) = ronoctua_xdg::runtime_file(SOCKET) else {
        d.ui.quit = true;
        return;
    };
    let Some(woken) = d.content.woken.clone() else {
        d.ui.quit = true;
        return;
    };
    // A SOCKET LEFT BY A WINDOW THAT DIED is in the way: nobody answered it
    // (`hand_to_the_kept` failed before this window opened), so it goes.
    let _ = std::fs::remove_file(&path);
    let listener = match std::os::unix::net::UnixListener::bind(&path) {
        Ok(l) => l,
        Err(e) => {
            tracing::warn!(error = %e, "fast-access: no socket to be kept ready on; closing");
            d.ui.quit = true;
            return;
        }
    };
    // THE LISTENER IS A THREAD, as the control socket's is: it takes ONE
    // wake, hands it to the loop and ends — the window listens only while
    // it is parked.
    let born = std::thread::Builder::new()
        .name("fast-access-kept".into())
        .spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(500)));
                let mut line = String::new();
                if std::io::BufReader::new(stream.take(WAKE_MOST))
                    .read_line(&mut line)
                    .is_err()
                {
                    continue;
                }
                if let Some(wake) = wake_of(&line) {
                    let _ = std::fs::remove_file(&path);
                    let _ = woken.send(wake);
                    return;
                }
            }
        });
    if born.is_err() {
        d.ui.quit = true;
        return;
    }
    // THE WINDOW GOES AND ITS GLOBAL STAYS: the window it comes back as is
    // made by the same `xdg_wm_base`, which outlives every window it made.
    let bare = d.compositor().create_surface(&d.qh);
    let old = std::mem::replace(&mut d.shell, Shell::Parked(bare.clone(), Rc::clone(&layer)));
    if let Shell::Window {
        window,
        _global: global,
    } = old
    {
        drop(window);
        *d.content.xdg.borrow_mut() = Some(global);
    }
    d.reopen(Shell::Parked(bare, layer));
}

/// A kept window comes back with a new feed and the query typed.
fn unpark(d: &mut Door<FastAccess>, wake: &Wake) {
    let texts = Rc::clone(&d.ui.texts);
    if d.content.parked {
        let Some(xdg) = d.content.xdg.borrow_mut().take() else {
            d.ui.quit = true;
            return;
        };
        d.content.float = wake.float;
        let surface = d.compositor().create_surface(&d.qh);
        let window = xdg.create_window(surface, WindowDecorations::RequestServer, &d.qh);
        window.set_title(Content::title(&d.content, &texts));
        window.set_app_id(Content::app_id(&d.content));
        window.commit();
        d.reopen(Shell::Window {
            window,
            _global: xdg,
        });
        d.content.parked = false;
    }
    // THE BOX STARTS FROM WHAT WAS ASKED: a kept window shows nothing of the
    // last search.
    d.content
        .search
        .view
        .set_text(wake.query.as_deref().unwrap_or_default());
    let outs = match d.content.sink.clone() {
        Some(sink) => d.content.search.raise(&sink, &texts),
        None => Vec::new(),
    };
    d.content.apply(&mut d.ui, outs);
}

/// `ronoctua-shell fast-access [--float|--column] [query]`.
///
/// # Errors
///
/// A word not understood, or the door could not open.
pub fn invoked(words: &[String]) -> Result<()> {
    let (door, query) = parse_words(words)?;
    match door {
        Opened::Column => {
            let opening = crate::column::Opening {
                tenant: crate::right_column::Tenant::FastAccess,
                on: None,
                size: None,
            };
            host_column(&opening, query)
        }
        Opened::Float => host_window(true, query),
        Opened::Window => host_window(false, query),
    }
}

/// Which door a command line opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opened {
    /// A window.
    Window,
    /// The window floating over the whole room.
    Float,
    /// The column on the right.
    Column,
}

/// The words after `fast-access`: a door flag, then the query. A flag nobody
/// knows is REFUSED, never read as words of the query.
///
/// # Errors
///
/// A flag not understood, or two doors asked.
pub fn parse_words(words: &[String]) -> Result<(Opened, Option<String>)> {
    let mut door = Opened::Window;
    let mut query: Vec<&str> = Vec::new();
    for w in words {
        match w.as_str() {
            "--float" | "--column" if door != Opened::Window => {
                anyhow::bail!("--float and --column are two doors; give one")
            }
            "--float" => door = Opened::Float,
            "--column" => door = Opened::Column,
            other if other.starts_with("--") => anyhow::bail!(
                "usage: ronoctua-shell fast-access [--float|--column] [query] — `{other}` is not understood"
            ),
            other => query.push(other),
        }
    }
    let query = (!query.is_empty()).then(|| query.join(" "));
    Ok((door, query))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(all: &[&str]) -> Vec<String> {
        all.iter().map(|w| (*w).to_owned()).collect()
    }

    /// FAILS IF: a door flag is read as words of the query, an unknown flag
    /// is taken for one, or two doors are accepted.
    #[test]
    fn the_command_line_names_one_door_and_the_rest_is_the_query() {
        assert_eq!(parse_words(&words(&[])).unwrap(), (Opened::Window, None));
        assert_eq!(
            parse_words(&words(&["--float", "/y", "lofi"])).unwrap(),
            (Opened::Float, Some("/y lofi".to_owned()))
        );
        assert_eq!(
            parse_words(&words(&["--column"])).unwrap(),
            (Opened::Column, None)
        );
        assert!(parse_words(&words(&["--floaty"])).is_err());
        assert!(parse_words(&words(&["--float", "--column"])).is_err());
    }

    /// FAILS IF: a wake does not come back as sent, or is not one line.
    #[test]
    fn a_wake_is_one_line_and_reads_back() {
        let wake = Wake {
            float: true,
            query: Some("2+2".to_owned()),
        };
        let line = wake_line(&wake).unwrap();
        assert_eq!(line.matches('\n').count(), 1);
        assert!(line.ends_with('\n'));
        assert_eq!(wake_of(&line), Some(wake));
        assert_eq!(wake_of("not json"), None);
    }

    fn opened(id: &str, uses: u32, at_s: i64) -> crate::fast_access::Opened {
        crate::fast_access::Opened {
            id: id.to_owned(),
            title: id.to_owned(),
            uses,
            at_s,
        }
    }

    fn ground() -> Ground {
        Ground {
            catalogue: Arc::default(),
            commands: Vec::new(),
            zone: ronoctua_files::when::Zone::utc(),
            zoneinfo: None,
            queue_keys: String::new(),
        }
    }

    /// A search with one application found, whose settings are kept nowhere.
    fn found(texts: &ronoctua_i18n::Texts) -> Search {
        use crate::fast_access::{Filled, Hit, Section, SectionId, What};
        let mut view = View::new(crate::fast_access::Settings::default(), texts);
        view.set_text("fire");
        view.with_sections(vec![Section {
            id: SectionId::Apps,
            filled: Filled::Hits(vec![Hit {
                id: "app:firefox.desktop".to_owned(),
                title: "Firefox".to_owned(),
                note: None,
                what: What::App {
                    desktop_id: "firefox.desktop".to_owned(),
                    glyph: tokens::icon::GENERIC,
                },
            }]),
        }]);
        let mut s = Search::new(view, None, ground(), crate::inspection::Threads::none());
        s.store = ronoctua_store::Store::at(None, crate::fast_access::FILE);
        s
    }

    /// FAILS IF: `RET` on a hit does not hand the opening to the footer, or
    /// the door stays when it is the window — or goes when it is the column,
    /// which stays for the next search (`enter-closes-but-the-column`).
    #[test]
    fn enter_closes_but_the_column() {
        for column in [false, true] {
            let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("the language");
            let mut ui = Ui::detached(texts, (1100, 720), 1).expect("a detached door");
            let mut door = FastAccess::over(found(&ui.texts), false, column);
            let texts = Rc::clone(&ui.texts);
            let outs = door.search.key(Press::Enter, FastAccess::area(&ui), &texts);
            assert!(
                outs.contains(&Out::Footer(
                    "fast-access-launch",
                    Some("firefox.desktop".to_owned())
                )),
                "{outs:?}"
            );
            assert!(outs.contains(&Out::Done), "{outs:?}");
            door.apply(&mut ui, outs);
            assert_eq!(ui.quit, !column, "column: {column}");
        }
    }

    /// FAILS IF: `Esc` leaves any door of the search open — the column
    /// included, which gives its slot up.
    #[test]
    fn escape_closes_every_door() {
        for column in [false, true] {
            let texts = ronoctua_i18n::Texts::fresh("pt-BR").expect("the language");
            let mut ui = Ui::detached(texts, (1100, 720), 1).expect("a detached door");
            let mut door = FastAccess::over(found(&ui.texts), false, column);
            let texts = Rc::clone(&ui.texts);
            let outs = door
                .search
                .key(Press::Escape, FastAccess::area(&ui), &texts);
            assert_eq!(outs, vec![Out::Close]);
            door.apply(&mut ui, outs);
            assert!(ui.quit, "column: {column}");
        }
    }

    /// FAILS IF: a door's write erases what another door remembered, or the
    /// switches are not the writer's.
    #[test]
    fn two_doors_remember_both() {
        let file = crate::fast_access::Settings {
            park: false,
            recent: vec![opened("a", 3, 10), opened("b", 1, 20)],
            ..Default::default()
        };
        let mine = crate::fast_access::Settings {
            park: true,
            recent: vec![opened("a", 4, 30), opened("c", 1, 25)],
            ..Default::default()
        };
        let m = merged(file, mine);
        assert!(m.park, "the switches are the writer's");
        let ids: Vec<&str> = m.recent.iter().map(|o| o.id.as_str()).collect();
        assert_eq!(ids, ["b", "c", "a"], "newest last, every door's");
        assert_eq!(m.recent[2].uses, 4);
        assert_eq!(m.recent[2].at_s, 30);
    }
}
