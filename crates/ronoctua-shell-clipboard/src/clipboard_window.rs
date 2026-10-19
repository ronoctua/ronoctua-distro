//! The clipboard's history, in a window of its own — the second screen of
//! the ONE history the footer keeps (`LEADER LEADER c`, `c w` in the whole
//! tree).
//!
//! It watches nothing. The history is the footer's — its own
//! `zwlr_data_control` watcher and what ClipCascade brings fill it — and this
//! window follows it over the control socket (`crate::clipboard_wire`): the
//! whole history at once, and again after every change. What its keys do goes
//! back the same way, as an act aimed at the history it showed, and the
//! footer carries it out on its history by the one function its own region's
//! acts go through too (`clipboard_region::carry_out` here, the footer's
//! `clipboard_carry_out` there). A second watcher would be a second history.
//!
//! It DECIDES nothing either: which rows exist and what a key does to them is
//! `clipboard_view.rs`, and both doors ask the same view the same questions.
//! What the footer draws beside the rows — the entry's text, its picture, its
//! code — is drawn here under them.

use crate::door::{Canvas, Content, Door, Ui};
use anyhow::Result;
use smithay_client_toolkit::{
    reexports::calloop::channel,
    seat::keyboard::{KeyEvent, Modifiers},
};
use std::io::{BufRead as _, Write as _};

/// The window's `app_id`.
pub const APP_ID: &str = "br.ronoctua.clipboard";

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
        Ok(ClipboardWindow::default())
    })
}

/// Follows the footer's history on a thread of its own: one connection,
/// kept open, and each line a whole history.
fn follow(told: channel::Sender<Heard>) {
    let born = std::thread::Builder::new()
        .name("clipboard-follow".to_owned())
        .spawn(move || {
            let stream = crate::ipc::socket_path().and_then(|p| {
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
/// its camera (`Camera`) draws: no footer is followed, and an act is
/// an order left on the door's line.
pub(crate) fn photographed(mirror: crate::clipboard_wire::Mirror) -> ClipboardWindow {
    let mut window = ClipboardWindow::default();
    window.clip.took(
        std::rc::Rc::new(mirror.queue()),
        mirror.sync.snapshot(),
        mirror.generation,
    );
    window
}

/// The clipboard's window's camera (`photograph --door clipboard`): over a
/// history given, entry by entry — walked and drawn by
/// `crate::photo::scenes`.
#[derive(Debug, Default)]
pub(crate) struct Camera {
    /// The history, newest first.
    entries: Vec<ronoctua_clipboard::Entry>,
    /// The entries pinned.
    pinned: Vec<usize>,
}

impl crate::photo::scenes::Scene for Camera {
    const DOOR: &'static str = "clipboard";
    const IS: &'static str = "the clipboard's window (`LEADER LEADER c`) over a history given";
    const WORDS: &'static str = "\
  --entry <text>        an entry of the history, newest first — repeatable
  --image <png>         an image entry — repeatable
  --pinned <n>          entry n is pinned (from 0)";

    fn word(
        &mut self,
        flag: &str,
        value: &mut dyn FnMut() -> anyhow::Result<String>,
    ) -> anyhow::Result<bool> {
        use anyhow::Context as _;
        match flag {
            "--entry" => self
                .entries
                .push(ronoctua_clipboard::Entry::of_text(value()?)),
            "--image" => {
                let f = std::path::PathBuf::from(value()?);
                let bytes = std::fs::read(&f)
                    .with_context(|| format!("could not read `{}`", f.display()))?;
                self.entries
                    .push(ronoctua_clipboard::Entry::of_image(bytes, "image/png"));
            }
            "--pinned" => self
                .pinned
                .push(crate::photo::number("--pinned", &value()?, 0..=49)?),
            _ => return Ok(false),
        }
        Ok(true)
    }

    fn shoot(self, ui: Ui, asked: &crate::photo::scenes::Asked) -> anyhow::Result<()> {
        let mut queue = ronoctua_clipboard::Queue::of_entries(self.entries);
        for n in &self.pinned {
            anyhow::ensure!(
                queue.toggle_pinned(*n).is_some(),
                "`--pinned {n}` names no entry"
            );
        }
        let mirror = crate::clipboard_wire::Mirror::of_parts(
            queue.inputs().iter(),
            &ronoctua_clipcascade::Snapshot::default(),
            1,
        );
        crate::photo::scenes::walked(crate::door::Detached::new(ui, photographed(mirror)), asked)
    }
}

/// `ronoctua-shell photograph --door clipboard …`, to its end.
pub fn camera(rest: &[String]) -> ! {
    crate::photo::scenes::taken(rest, Camera::default())
}

/// The window: the history open in it — what the footer's region keeps
/// too (`clipboard_region::Clip`).
#[derive(Debug, Default)]
pub(crate) struct ClipboardWindow {
    clip: crate::clipboard_region::Clip,
}

impl ClipboardWindow {
    /// The list's hover bands, one per pointer, laid ONLY on a row a click
    /// takes (`Clip::takes`, which the footer's region asks too) — never on
    /// the heading, a fact or the air between, where the click takes
    /// nothing. Its own function, and not two lines of [`Content::paint`],
    /// so a test can hold it: a frame's canvas needs a GPU.
    fn hover(
        &self,
        ui: &Ui,
        lit: &[(ronoctua_tokens::Color, Option<usize>)],
    ) -> Vec<ronoctua_render::quad::Quad> {
        let takes = self.clip.takes(ui.columns, &ui.texts);
        crate::chrome::hover_bands(
            lit.iter().map(|&(colour, row)| {
                (
                    colour,
                    row.filter(|n| takes.get(*n).copied().unwrap_or(false)),
                )
            }),
            ui.offset,
            ui.room_for_rows(),
            takes.len(),
            ui.first_of_list(),
            ui.room.band_width(),
        )
    }

    /// What the footer's thread said.
    fn heard(&mut self, ui: &mut Ui, h: Heard) {
        match h {
            Heard::History(m) => {
                // WHAT THE FOOTER SAID TO THIS DOOR'S ACT, said here: a copy
                // made, a refusal, a key aimed at a history that had moved.
                if let Some(said) = &m.said {
                    ui.say(said.clone());
                }
                self.clip
                    .took(std::rc::Rc::new(m.queue()), m.sync.snapshot(), m.generation);
            }
            Heard::Gone => {
                self.clip.lost();
                ui.hold(ui.texts.t(crate::key::CLIPBOARD_WINDOW_NO_FOOTER));
            }
        }
        ui.ask_paint();
    }

    /// One press — from the keyboard, the hint, or `RET` from a second click.
    fn pressed(&mut self, ui: &mut Ui, press: crate::clipboard_view::Press) {
        self.clip.pressed(press, ui);
    }
}

impl Content for ClipboardWindow {
    type Hand = ();

    fn app_id(&self) -> &'static str {
        APP_ID
    }

    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        format!("*{}*", texts.t(crate::key::CLIPBOARD_HISTORY))
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
        self.clip.answered(id, text, ui);
    }

    /// The line was let go: a login half typed is forgotten.
    fn unanswered(&mut self, ui: &mut Ui, _: &'static str) {
        self.clip.unanswered(ui);
    }

    fn hint(&self, ui: &Ui) -> Vec<crate::chrome::Piece> {
        self.clip.view.hint(&ui.texts)
    }

    fn rows(&self, ui: &Ui) -> Vec<Vec<crate::fields::Field>> {
        self.clip.rows(ui.columns, &ui.texts)
    }

    fn cursor(&self) -> Option<usize> {
        Some(self.clip.cursor())
    }

    /// THE CLICK SELECTS, AND THE SECOND ONE ACTS — invariant 19, and what
    /// the footer does with a click on the same rows.
    fn clicked(&mut self, ui: &mut Ui, row: usize) {
        self.clip.clicked(row, ui);
    }

    /// The rows, and the shown entry's picture or its code in the box the
    /// view left for it — the footer's drawing, by the same functions.
    ///
    /// THE POINTER LIGHTS WHAT THE CLICK TAKES: the list is drawn without
    /// its hover, and the hover is [`ClipboardWindow::hover`]'s.
    fn paint(&mut self, ui: &mut Ui, canvas: &mut Canvas<'_>) {
        let lit = canvas.lit;
        canvas.lit = &[];
        crate::door::list::paint(self, ui, canvas);
        canvas.lit = lit;
        canvas.under.extend(self.hover(ui, lit));
        if !self.clip.heard() {
            return;
        }
        self.clip.decode_shown(ui.columns, &ui.texts);
        let Some(picture) = self
            .clip
            .view
            .picture(&self.clip.facts(), ui.columns, &ui.texts)
        else {
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
                if let Some((_, m)) = self.clip.qr().filter(|(at, _)| *at == n) {
                    canvas
                        .under
                        .extend(crate::clipboard_view::qr_quads(m, area));
                }
            }
            crate::clipboard_view::Shows::Image(n) => {
                if let Some((key, rgba)) = self.clip.picture_of(n) {
                    // CLONED ONLY WHEN IT IS NOT ON THE GPU YET: a capture
                    // is tens of megabytes, and a frame is not the place.
                    if canvas.upload(&key, || Some((*rgba).clone())) {
                        canvas
                            .pictures
                            .push((key, crate::sizes::picture_in(area, rgba.width, rgba.height)));
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
        let mut w = ClipboardWindow::default();
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
        assert_eq!(w.clip.queue().how_many(), 2);
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
        let stale = ui.texts.t(crate::key::CLIPBOARD_ACT_STALE);
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
            .clip
            .view
            .picture(&w.clip.facts(), ui.columns, &ui.texts)
            .expect("a code is shown");
        let (first, last) =
            crate::clipboard_view::picture_rows(box_, ui.offset, ui.room_for_rows())
                .expect("the box is on screen");
        assert!(first >= ui.offset && last <= ui.offset + ui.room_for_rows());
        assert!(w.clip.qr().is_some(), "no code was made");
        assert_ne!(
            ui.message.as_deref(),
            Some(ui.texts.t(crate::key::CLIPBOARD_NO_ROOM).as_str())
        );
    }

    /// FAILS IF: the window's resting pointer lights a row a click takes
    /// nothing on — the `History` heading, the air between the sections, a
    /// fact — or leaves unlit one the click takes. THE LIGHT IS THE CLICK:
    /// every row is clicked twice, on a window of its own, and is lit
    /// exactly where something happened. The footer's region has its own
    /// (`clipboard_region`'s `the_pointer_lights_what_the_click_takes`).
    #[test]
    fn the_windows_pointer_lights_what_the_click_takes() {
        let entries = ["one", "two", "three"].map(ronoctua_clipboard::Entry::of_text);
        let sized = |(w, mut ui): (ClipboardWindow, Ui)| {
            (ui.columns, ui.lines) = (100, 40);
            (w, ui)
        };
        let (w, ui) = sized(a_window(&entries));
        let rows = w.rows(&ui).len();
        assert!(
            ui.room_for_rows() >= rows,
            "the whole list is not on the page"
        );
        let lit: Vec<bool> = (0..rows)
            .map(|n| {
                !w.hover(&ui, &[(ronoctua_tokens::color::TEXT_HERE_ACTS, Some(n))])
                    .is_empty()
            })
            .collect();
        assert!(!lit[0], "the `History` heading lights");
        assert!(
            lit.iter().any(|l| *l) && lit.iter().any(|l| !*l),
            "no row lit, or every row lit: {lit:?}"
        );
        let seen = |w: &ClipboardWindow, ui: &Ui| {
            (
                w.rows(ui),
                ui.orders().to_vec(),
                ui.launches().len(),
                ui.asking.is_some(),
                ui.message.clone(),
            )
        };
        for (n, lit) in lit.iter().enumerate() {
            let (mut w, mut ui) = sized(a_window(&entries));
            let before = seen(&w, &ui);
            w.clicked(&mut ui, n);
            w.clicked(&mut ui, n);
            let something = seen(&w, &ui) != before;
            assert_eq!(
                *lit,
                something,
                "row {n} is lit {lit} and a click on it did {}",
                if something { "something" } else { "nothing" }
            );
        }
    }

    /// FAILS IF: the window draws a list before the footer sent one, or
    /// draws the footer's absence as an empty history.
    #[test]
    fn the_window_says_when_the_footer_is_not_there() {
        let texts = ronoctua_i18n::Texts::fresh("pt-BR").expect("pt-BR");
        let mut w = ClipboardWindow::default();
        let mut ui = Ui::for_tests(texts);
        let first = |w: &ClipboardWindow, ui: &Ui| w.rows(ui)[0][0].text.clone();
        assert_eq!(
            first(&w, &ui),
            ui.texts.t(crate::key::CLIPBOARD_WINDOW_WAITING)
        );
        w.heard(&mut ui, Heard::Gone);
        assert_eq!(
            first(&w, &ui),
            ui.texts.t(crate::key::CLIPBOARD_WINDOW_NO_FOOTER)
        );
    }

    /// FAILS IF: the login typed in the window leaves the password anywhere
    /// but the one act that carries it.
    #[test]
    fn the_login_is_typed_on_the_windows_line_and_sent_once() {
        let (mut w, mut ui) = a_window(&[]);
        crate::clipboard_region::carry_out(
            crate::clipboard_view::Errand::AskServer,
            &mut w.clip,
            &mut ui,
        );
        for said in ["https://clip.example.org", "someone", "s3nh4"] {
            let line = ui.asking.take().expect("a question is open");
            w.answered(&mut ui, line.id, said);
        }
        let orders = ui.orders().to_vec();
        assert_eq!(orders.len(), 1, "{orders:?}");
        assert!(orders[0].contains("s3nh4"), "the password did not go");
        assert!(!w.clip.logging_in(), "the login was kept");
        assert!(ui.asking.is_none());
    }

    fn words(all: &[&str]) -> Vec<String> {
        all.iter().map(|w| (*w).to_owned()).collect()
    }

    /// FAILS IF: the clipboard's camera loses an entry or a pin, or takes a
    /// word that is another camera's (how the screens' file read).
    #[test]
    fn the_cameras_words_are_the_history() {
        use crate::photo::scenes::parse;
        let mut camera = Camera::default();
        parse(
            &words(&["--door", "clipboard", "--entry", "one", "--pinned", "0"]),
            &mut camera,
        )
        .expect("the words");
        assert_eq!(
            (camera.entries.len(), camera.pinned.as_slice()),
            (1, &[0][..])
        );
        assert!(
            parse(
                &words(&["--door", "clipboard", "--settings", "failed"]),
                &mut Camera::default()
            )
            .is_err()
        );
    }
}
