//! The clipboard's history, in a window of its own — the second screen of
//! the ONE history the footer keeps (`LEADER LEADER c`, `c w` in the whole
//! tree).
//!
//! It watches nothing. The history is the footer's — its own
//! `zwlr_data_control` watcher and what ClipCascade brings fill it — and this
//! window follows it over the control socket (`crate::clipboard_wire`): the
//! whole history at once, and again after every change. What its keys do goes
//! back the same way and the footer carries it out on its history, by the
//! errands its own view uses. A second watcher would be a second history.
//!
//! It DECIDES nothing either: which rows exist and what a key does to them is
//! `clipboard_view.rs`, and both doors ask the same view the same questions.
//! What the footer draws beside the rows — the entry's text, its picture, its
//! code — is drawn here under them.

use crate::door::{Canvas, Content, Door, Ui};
use anyhow::Result;
use ronoctua_tokens as tokens;
use smithay_client_toolkit::{
    reexports::calloop::channel,
    seat::keyboard::{KeyEvent, Modifiers},
};
use std::io::{BufRead as _, Write as _};

/// The window's `app_id`.
pub const APP_ID: &str = "br.ronoctua.clipboard";

/// The key the focused entry's picture is uploaded under: the generation
/// and the entry, so a new history is a new picture.
fn picture_key(generation: u64, n: usize) -> String {
    format!("clipboard-window:{generation}:{n}")
}

/// What the footer's thread hands the loop.
enum Heard {
    /// A whole history.
    History(Box<crate::clipboard_wire::Mirror>),
    /// The footer closed the connection, or there was none.
    Gone,
}

/// Opens the clipboard's history in a window.
///
/// # Errors
///
/// When there is no Wayland compositor, or the compositor offers no
/// `xdg_shell`.
pub fn host_it() -> Result<()> {
    crate::door::run(|w| {
        let (told, heard) = channel::channel::<Heard>();
        w.handle
            .insert_source(heard, |event, (), door: &mut Door<ClipboardWindow>| {
                if let channel::Event::Msg(h) = event {
                    door.content.heard(&mut door.ui, h);
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the clipboard's channel: {e}"))?;
        follow(told);
        Ok(ClipboardWindow {
            view: crate::clipboard_view::View::fresh(),
            mirror: crate::clipboard_wire::Mirror::default(),
            queue: ronoctua_clipboard::Queue::default(),
            sync: ronoctua_clipcascade::Snapshot::default(),
            heard: false,
            gone: false,
            qr: None,
            picture: None,
            login: None,
        })
    })
}

/// Follows the footer's history on a thread of its own: one connection,
/// kept open, and each line a whole history.
fn follow(told: channel::Sender<Heard>) {
    let born = std::thread::Builder::new()
        .name("clipboard-follow".to_owned())
        .spawn(move || {
            let stream = crate::leader::socket_path().and_then(|p| {
                let mut s = std::os::unix::net::UnixStream::connect(&p)?;
                writeln!(s, "{}", crate::clipboard_wire::FOLLOW)?;
                Ok(s)
            });
            let stream = match stream {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(error = %e, "no footer to follow the clipboard from");
                    let _ = told.send(Heard::Gone);
                    return;
                }
            };
            for line in std::io::BufReader::new(stream).lines() {
                let Ok(line) = line else { break };
                match serde_json::from_str::<crate::clipboard_wire::Mirror>(&line) {
                    Ok(m) => {
                        if told.send(Heard::History(Box::new(m))).is_err() {
                            return;
                        }
                    }
                    Err(e) => tracing::warn!(error = %e, "a clipboard history that does not read"),
                }
            }
            let _ = told.send(Heard::Gone);
        });
    if let Err(e) = born {
        tracing::error!(error = %e, "could not start the thread that follows the clipboard");
    }
}

/// The window, DETACHED, over a history handed in whole — what
/// `crate::scene_photograph` draws: no footer is followed, and an act is
/// an order left on the door's line.
pub(crate) fn photographed(mirror: crate::clipboard_wire::Mirror) -> ClipboardWindow {
    ClipboardWindow {
        view: crate::clipboard_view::View::fresh(),
        queue: mirror.queue(),
        sync: mirror.sync.snapshot(),
        mirror,
        heard: true,
        gone: false,
        qr: None,
        picture: None,
        login: None,
    }
}

/// The window.
pub(crate) struct ClipboardWindow {
    view: crate::clipboard_view::View,
    /// The last history the footer sent, as it came.
    mirror: crate::clipboard_wire::Mirror,
    /// That history, for the view.
    queue: ronoctua_clipboard::Queue,
    /// ClipCascade's reading, for the view.
    sync: ronoctua_clipcascade::Snapshot,
    /// Whether a history has arrived at all.
    heard: bool,
    /// Whether the footer went away (or was never there).
    gone: bool,
    /// The shown entry's code, while `q` shows one — by the entry it is of.
    qr: Option<(usize, ronoctua_clipboard::qr::Matrix)>,
    /// The focused entry's picture, decoded — by the entry it is of.
    picture: Option<(usize, ronoctua_render::image::Rgba)>,
    /// A login being typed: the server, then the account.
    login: Option<(String, Option<String>)>,
}

impl ClipboardWindow {
    /// What the footer's thread said.
    fn heard(&mut self, ui: &mut Ui, h: Heard) {
        match h {
            Heard::History(m) => {
                // WHAT THE FOOTER SAID TO THIS DOOR'S ACT, said here: a copy
                // made, a refusal, a key aimed at a history that had moved.
                if let Some(said) = &m.said {
                    ui.say(said.clone());
                }
                let moved = m.generation != self.mirror.generation;
                self.queue = m.queue();
                self.sync = m.sync.snapshot();
                self.mirror = *m;
                self.heard = true;
                self.gone = false;
                // A NEW HISTORY is a new list under the same cursor: what the
                // code and the picture were of may have moved. A ClipCascade
                // reading moves no entry, and leaves them.
                if moved {
                    self.qr = None;
                    self.view.no_qr();
                    self.picture = None;
                }
            }
            Heard::Gone => {
                self.gone = true;
                ui.hold(ui.texts.t(ronoctua_i18n::key::CLIPBOARD_WINDOW_NO_FOOTER));
            }
        }
        ui.ask_paint();
    }

    fn facts(&self) -> crate::clipboard_view::Facts<'_> {
        crate::clipboard_view::Facts {
            queue: &self.queue,
            sync: &self.sync,
        }
    }

    /// The entry the cursor is on, when it is on one.
    fn entry_here(&self, ui: &Ui) -> Option<usize> {
        match self.view.target_at(&self.facts(), ui.columns, &ui.texts) {
            crate::clipboard_view::Target::Entry(n) => Some(n),
            _ => None,
        }
    }

    /// Sends an act to the footer, for the history this window is showing.
    fn act(&self, ui: &mut Ui, act: &crate::clipboard_wire::Act) {
        match act.line(self.mirror.generation) {
            Ok(line) => ui.tell_the_footer_line(line),
            Err(e) => tracing::warn!(error = %e, "a clipboard act that does not write"),
        }
    }

    /// Carries out what the view asked for — the footer's errands, sent to
    /// the footer.
    fn apply(&mut self, ui: &mut Ui, errand: crate::clipboard_view::Errand) {
        use crate::clipboard_view::Errand;
        use crate::clipboard_wire::Act;
        match errand {
            Errand::Nothing => {}
            Errand::Close => {
                ui.quit = true;
                return;
            }
            Errand::Copy(n) => self.act(ui, &Act::Copy { n }),
            Errand::Remove(n) => self.act(ui, &Act::Remove { n }),
            Errand::Pin(n) => self.act(ui, &Act::Pin { n }),
            Errand::Clear => self.act(ui, &Act::Clear),
            Errand::Sync(on) => self.act(ui, &Act::Sync { on }),
            Errand::LogOut => self.act(ui, &Act::LogOut),
            Errand::Keep(settings) => self.act(ui, &Act::of_settings(&settings)),
            // THE CODE IS DRAWN HERE, from the entry this window holds: it is
            // a picture of text, and nothing about it is the footer's.
            Errand::Qr(n) => {
                self.qr = if self.view.qr().is_some() {
                    let m = self
                        .queue
                        .at(n)
                        .and_then(ronoctua_clipboard::Entry::text)
                        .and_then(ronoctua_clipboard::qr::generate);
                    if m.is_none() {
                        ui.say(ui.texts.t(ronoctua_i18n::key::ECHO_QR_LARGE));
                    } else if !self.picture_fits(ui) {
                        // A CODE NOBODY CAN SEE IS SAID, never drawn off
                        // screen in silence.
                        ui.say(ui.texts.t(ronoctua_i18n::key::CLIPBOARD_NO_ROOM));
                    }
                    m.map(|m| (n, m))
                } else {
                    None
                };
            }
            // THE TYPED ONES, on this window's own line, under the ids the
            // footer's minibuffer asks under; the password hidden.
            Errand::AskServer => ui.ask(crate::line::Line::new("clipboard-server", false, None)),
            Errand::AskAccount => {
                ui.ask(crate::line::Line::new("clipboard-account", false, None));
            }
            Errand::AskSalt => ui.ask(crate::line::Line::new("clipboard-salt", false, None)),
            Errand::AskRounds => ui.ask(crate::line::Line::new("clipboard-rounds", false, None)),
            Errand::AskCap => ui.ask(crate::line::Line::new("clipboard-cap", false, None)),
        }
        ui.ask_paint();
    }

    /// One press — from the keyboard, the hint, or `RET` from a second click.
    fn pressed(&mut self, ui: &mut Ui, press: crate::clipboard_view::Press) {
        let before = self.entry_here(ui);
        let errand = {
            let facts = crate::clipboard_view::Facts {
                queue: &self.queue,
                sync: &self.sync,
            };
            self.view.key(press, &facts, ui.columns, &ui.texts)
        };
        // THE CODE BELONGED TO THE ENTRY THE CURSOR LEFT.
        if self.entry_here(ui) != before && self.view.qr().is_none() {
            self.qr = None;
        }
        self.apply(ui, errand);
    }

    /// Whether the view's picture box has rows on screen at this scroll.
    fn picture_fits(&self, ui: &Ui) -> bool {
        self.view
            .picture(&self.facts(), ui.columns, &ui.texts)
            .is_none_or(|p| {
                crate::clipboard_view::picture_rows(p, ui.offset, ui.room_for_rows()).is_some()
            })
    }

    /// Entry `n`'s picture, decoded once per entry: its size, and the key
    /// it is uploaded under.
    fn decoded(&mut self, n: usize) -> Option<(String, u32, u32)> {
        if self.picture.as_ref().map(|(at, _)| *at) != Some(n) {
            let bytes = self.queue.at(n)?.image()?;
            let r = ronoctua_clipboard::image::decode(bytes)?;
            self.picture = Some((
                n,
                ronoctua_render::image::Rgba {
                    width: r.width,
                    height: r.height,
                    pixels: r.pixels,
                },
            ));
        }
        let (at, rgba) = self.picture.as_ref()?;
        Some((
            picture_key(self.mirror.generation, *at),
            rgba.width,
            rgba.height,
        ))
    }

    /// The rows the view draws — the shown entry's text among them, in the
    /// right half, as the footer draws it.
    fn all_rows(&self, ui: &Ui) -> Vec<Vec<crate::files::Field>> {
        let mut rows: Vec<Vec<crate::files::Field>> = self
            .view
            .rows(&self.facts(), ui.columns, &ui.texts)
            .into_iter()
            .map(|r| r.fields)
            .collect();
        // BEFORE THE FIRST HISTORY, a sentence: the window opened on a list
        // that is the footer's and has not come yet.
        if !self.heard {
            rows.insert(
                0,
                vec![crate::files::Field {
                    at: crate::chrome::INSET,
                    text: ui.texts.t(if self.gone {
                        ronoctua_i18n::key::CLIPBOARD_WINDOW_NO_FOOTER
                    } else {
                        ronoctua_i18n::key::CLIPBOARD_WINDOW_WAITING
                    }),
                    color: tokens::color::TEXT_DIM,
                    hit: crate::files::Hit::Nothing,
                    explain: None,
                    ink: crate::files::Ink::Grid,
                }],
            );
            return rows;
        }
        rows
    }
}

impl Content for ClipboardWindow {
    type Hand = ();

    fn app_id(&self) -> &'static str {
        APP_ID
    }

    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        format!("*{}*", texts.t(ronoctua_i18n::key::CLIPBOARD_HISTORY))
    }

    /// A key, read by its whole notation in the view's own table — the
    /// footer's reader, and `Esc` through the view so a code on screen is
    /// put away before the window closes.
    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers) {
        let Some(key) = crate::door::keys::key_of(event, mods) else {
            return;
        };
        let press = if key.sym == ronoctua_keymap::Sym::Esc {
            Some(crate::clipboard_view::Press::Escape)
        } else {
            let notation = key.notation();
            crate::clipboard_view::OF_THE_VIEW
                .iter()
                .find(|(t, _)| *t == notation)
                .and_then(|(_, id)| crate::clipboard_view::press_of(id))
        };
        if let Some(press) = press {
            ui.message = None;
            self.pressed(ui, press);
        }
    }

    /// What was typed on this window's line, under the footer's own ids: a
    /// login's three answers, one at a time, and the typed settings.
    fn answered(&mut self, ui: &mut Ui, id: &'static str, text: &str) {
        use crate::clipboard_wire::Act;
        let said = text.trim();
        match id {
            "clipboard-server" if !said.is_empty() => {
                self.login = Some((said.to_owned(), None));
                ui.ask(crate::line::Line::new("clipboard-account", false, None));
            }
            "clipboard-account" if !said.is_empty() => {
                if let Some((server, _)) = self.login.take() {
                    self.login = Some((server, Some(said.to_owned())));
                    ui.ask(crate::line::Line::new("clipboard-password", true, None));
                }
            }
            // THE PASSWORD IS NOT KEPT: it goes into the act, which the
            // footer hands to ClipCascade, and leaves this process with it.
            "clipboard-password" => {
                if let Some((server, Some(username))) = self.login.take() {
                    self.act(
                        ui,
                        &Act::LogIn {
                            server,
                            username,
                            password: text.to_owned(),
                        },
                    );
                }
            }
            "clipboard-salt" | "clipboard-rounds" | "clipboard-cap" => self.act(
                ui,
                &Act::Typed {
                    id: id.to_owned(),
                    text: said.to_owned(),
                },
            ),
            _ => {}
        }
        ui.ask_paint();
    }

    /// The line was let go: a login half typed is forgotten.
    fn unanswered(&mut self, ui: &mut Ui, _: &'static str) {
        self.login = None;
        ui.ask_paint();
    }

    fn hint(&self, ui: &Ui) -> Vec<crate::chrome::Piece> {
        self.view.hint(&ui.texts)
    }

    fn rows(&self, ui: &Ui) -> Vec<Vec<crate::files::Field>> {
        self.all_rows(ui)
    }

    fn cursor(&self) -> Option<usize> {
        Some(self.view.cursor() + usize::from(!self.heard))
    }

    /// THE CLICK SELECTS, AND THE SECOND ONE ACTS — invariant 19, and what
    /// the footer does with a click on the same rows.
    fn clicked(&mut self, ui: &mut Ui, row: usize) {
        if !self.heard {
            return;
        }
        let rows = self.view.rows(&self.facts(), ui.columns, &ui.texts).len();
        if row >= rows {
            return;
        }
        if row == self.view.cursor() {
            self.pressed(ui, crate::clipboard_view::Press::Enter);
        } else {
            self.view.clicked(row);
            if self.view.qr().is_none() {
                self.qr = None;
            }
            ui.ask_paint();
        }
    }

    /// The rows, and the shown entry's picture or its code in the box the
    /// view left for it — the footer's drawing, by the same functions.
    fn paint(&mut self, ui: &mut Ui, canvas: &mut Canvas<'_>) {
        crate::door::list::paint(self, ui, canvas);
        if !self.heard {
            return;
        }
        let Some(picture) = self.view.picture(&self.facts(), ui.columns, &ui.texts) else {
            return;
        };
        let first = ui.first_of_list();
        let f = ui.frame(0);
        let Some(area) = crate::clipboard_view::picture_area(
            picture,
            ui.offset,
            ui.room_for_rows(),
            ui.margin,
            f.cell(),
            |line| f.y_of(first.saturating_add(line)),
        ) else {
            return;
        };
        match picture.shows {
            crate::clipboard_view::Shows::Qr(n) => {
                if let Some((_, m)) = self.qr.as_ref().filter(|(at, _)| *at == n) {
                    canvas
                        .under
                        .extend(crate::clipboard_view::qr_quads(m, area));
                }
            }
            crate::clipboard_view::Shows::Image(n) => {
                if let Some((key, w, h)) = self.decoded(n) {
                    // CLONED ONLY WHEN IT IS NOT ON THE GPU YET: a capture
                    // is tens of megabytes, and a frame is not the place.
                    let picture = &self.picture;
                    if canvas.upload(&key, || picture.as_ref().map(|(_, r)| r.clone())) {
                        canvas
                            .pictures
                            .push((key, crate::files::picture_in(area, w, h)));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_window(entries: &[ronoctua_clipboard::Entry]) -> (ClipboardWindow, Ui) {
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let queue = ronoctua_clipboard::Queue::of_entries(entries.iter().cloned());
        let sync = ronoctua_clipcascade::Snapshot::default();
        let mirror = crate::clipboard_wire::Mirror::of(&queue, &sync, 4);
        let mut w = ClipboardWindow {
            view: crate::clipboard_view::View::fresh(),
            mirror: crate::clipboard_wire::Mirror::default(),
            queue: ronoctua_clipboard::Queue::default(),
            sync,
            heard: false,
            gone: false,
            qr: None,
            picture: None,
            login: None,
        };
        let mut ui = Ui::for_tests(texts);
        w.heard(&mut ui, Heard::History(Box::new(mirror)));
        (w, ui)
    }

    /// FAILS IF: a key in the window does something to a history of its own
    /// instead of asking the footer's — the act names the entry and the
    /// generation it was shown.
    #[test]
    fn a_key_in_the_window_asks_the_footers_history() {
        let (mut w, mut ui) = a_window(&[
            ronoctua_clipboard::Entry::of_text("one"),
            ronoctua_clipboard::Entry::of_text("two"),
        ]);
        w.pressed(&mut ui, crate::clipboard_view::Press::Down);
        w.pressed(&mut ui, crate::clipboard_view::Press::Enter);
        let orders = ui.orders().to_vec();
        assert_eq!(orders.len(), 1, "{orders:?}");
        let rest = orders[0]
            .strip_prefix(crate::clipboard_wire::ACT)
            .expect("an act");
        assert_eq!(
            crate::clipboard_wire::Act::parse(rest),
            Some((4, crate::clipboard_wire::Act::Copy { n: 1 }))
        );
        // AND THE WINDOW'S OWN LIST DID NOT MOVE: it is the footer's.
        assert_eq!(w.queue.how_many(), 2);
    }

    /// FAILS IF: what the footer said in answer to this window's act — a
    /// refusal above all — is not said on the window's own line.
    #[test]
    fn the_footers_answer_is_said_on_the_windows_line() {
        let (mut w, mut ui) = a_window(&[ronoctua_clipboard::Entry::of_text("one")]);
        let queue =
            ronoctua_clipboard::Queue::of_entries([ronoctua_clipboard::Entry::of_text("one")]);
        let mut m = crate::clipboard_wire::Mirror::of(
            &queue,
            &ronoctua_clipcascade::Snapshot::default(),
            4,
        );
        let stale = ui.texts.t(ronoctua_i18n::key::CLIPBOARD_ACT_STALE);
        m.said = Some(stale.clone());
        w.heard(&mut ui, Heard::History(Box::new(m)));
        assert_eq!(ui.message.as_deref(), Some(stale.as_str()));
    }

    /// FAILS IF: with a history taller than the window, the entry's picture
    /// box has no rows on screen — `q` drew nothing and said nothing.
    #[test]
    fn a_long_history_still_shows_the_picture_box() {
        let many: Vec<_> = (0..50)
            .map(|n| ronoctua_clipboard::Entry::of_text(format!("entry {n}")))
            .collect();
        let (mut w, mut ui) = a_window(&many);
        ui.columns = 100;
        ui.lines = 20;
        for _ in 0..40 {
            w.pressed(&mut ui, crate::clipboard_view::Press::Down);
        }
        ui.offset = 30;
        w.pressed(&mut ui, crate::clipboard_view::Press::Qr);
        let box_ = w
            .view
            .picture(&w.facts(), ui.columns, &ui.texts)
            .expect("a code is shown");
        let (first, last) =
            crate::clipboard_view::picture_rows(box_, ui.offset, ui.room_for_rows())
                .expect("the box is on screen");
        assert!(first >= ui.offset && last <= ui.offset + ui.room_for_rows());
        assert!(w.qr.is_some(), "no code was made");
        assert_ne!(
            ui.message.as_deref(),
            Some(ui.texts.t(ronoctua_i18n::key::CLIPBOARD_NO_ROOM).as_str())
        );
    }

    /// FAILS IF: the window draws a list before the footer sent one, or
    /// draws the footer's absence as an empty history.
    #[test]
    fn the_window_says_when_the_footer_is_not_there() {
        let texts = ronoctua_i18n::Texts::fresh("pt-BR").expect("pt-BR");
        let mut w = ClipboardWindow {
            view: crate::clipboard_view::View::fresh(),
            mirror: crate::clipboard_wire::Mirror::default(),
            queue: ronoctua_clipboard::Queue::default(),
            sync: ronoctua_clipcascade::Snapshot::default(),
            heard: false,
            gone: false,
            qr: None,
            picture: None,
            login: None,
        };
        let mut ui = Ui::for_tests(texts);
        let first = |w: &ClipboardWindow, ui: &Ui| w.rows(ui)[0][0].text.clone();
        assert_eq!(
            first(&w, &ui),
            ui.texts.t(ronoctua_i18n::key::CLIPBOARD_WINDOW_WAITING)
        );
        w.heard(&mut ui, Heard::Gone);
        assert_eq!(
            first(&w, &ui),
            ui.texts.t(ronoctua_i18n::key::CLIPBOARD_WINDOW_NO_FOOTER)
        );
    }

    /// FAILS IF: the login typed in the window leaves the password anywhere
    /// but the one act that carries it.
    #[test]
    fn the_login_is_typed_on_the_windows_line_and_sent_once() {
        let (mut w, mut ui) = a_window(&[]);
        w.apply(&mut ui, crate::clipboard_view::Errand::AskServer);
        for said in ["https://clip.example.org", "someone", "s3nh4"] {
            let line = ui.asking.take().expect("a question is open");
            w.answered(&mut ui, line.id, said);
        }
        let orders = ui.orders().to_vec();
        assert_eq!(orders.len(), 1, "{orders:?}");
        assert!(orders[0].contains("s3nh4"), "the password did not go");
        assert!(w.login.is_none(), "the login was kept");
        assert!(ui.asking.is_none());
    }
}
