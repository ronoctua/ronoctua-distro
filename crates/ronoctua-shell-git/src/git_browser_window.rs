//! The git browser in a window of its own — the second door onto
//! `crate::git_browser`, opened by `C-h` in the file manager's window
//! (`ronoctua-shell git-browser <folder>`).
//!
//! A window and nothing else: what each column holds, what a key does and
//! what a point is are the view's, and the footer's region asks the same
//! view the same questions. What this door keeps is the repository it reads
//! on its own threads, the watch that says the repository moved, and the
//! header every window has — the keys, the drawer, the question.

use crate::door::{Canvas, Content, Door, Hand, LEFT_BUTTON, Ui};
use crate::git_browser::Errand;
use anyhow::Result;
use ronoctua_tokens as tokens;
use smithay_client_toolkit::{
    reexports::calloop::channel,
    seat::{
        keyboard::{KeyEvent, Modifiers},
        pointer::{PointerEvent, PointerEventKind},
    },
};

/// The window's `app_id`.
pub const APP_ID: &str = "br.ronoctua.git";

/// How a pointer's line and cell travel in the one number the door lights
/// every hand with (`Hand::row_under_pointer`): the line times this, plus
/// the cell — the file manager's window's arrangement.
const CELL_SPAN: usize = 1 << 16;

/// The history's glyph, before the title.
static GLYPH: std::sync::LazyLock<String> =
    std::sync::LazyLock::new(|| tokens::icon::GIT_BRANCH.to_string());

/// What a thread of this door hands back — the history's, in both doors
/// (`git_region::Back`).
pub use crate::git_region::Back;

/// Opens the browser over the repository `dir` is in.
///
/// # Errors
///
/// When there is no Wayland compositor, or it offers no `xdg_shell`.
pub fn host_it(dir: &std::path::Path) -> Result<()> {
    let dir = dir.to_path_buf();
    crate::door::run(move |w| {
        let (sender, backs) = channel::channel::<Back>();
        w.handle
            .insert_source(backs, |event, (), door: &mut Door<GitWindow>| {
                if let channel::Event::Msg(b) = event {
                    door.content.back(&mut door.ui, b);
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the git channel: {e}"))?;
        let zone = ronoctua_files::when::Zone::read_from(std::path::Path::new("/etc/localtime"));
        let mut window = GitWindow::over(&dir, w.texts.clone(), zone, sender);
        // THE REPOSITORY REPORTS ITSELF (`git_region::Git::watched`).
        window.git = window.git.watched();
        window.ask();
        Ok(window)
    })
}

/// The window: the history open in it — the view, the repository it reads
/// and the watch on it, as the footer's region keeps them too
/// (`git_region::Git`).
pub struct GitWindow {
    git: crate::git_region::Git,
}

/// What this window keeps for one pointer: which column's bar it holds.
#[derive(Debug, Default)]
pub struct Held {
    bar: Option<crate::git_browser::BarBox>,
}

impl GitWindow {
    /// The window over the repository `dir` is in — nothing watched, which
    /// is what the photograph draws.
    #[must_use]
    pub fn over(
        dir: &std::path::Path,
        texts: std::rc::Rc<ronoctua_i18n::Texts>,
        zone: ronoctua_files::when::Zone,
        sender: channel::Sender<Back>,
    ) -> Self {
        let sender = std::sync::Mutex::new(sender);
        Self {
            git: crate::git_region::Git::over(dir, true, texts, zone, move |b| {
                if let Ok(s) = sender.lock() {
                    let _ = s.send(b);
                }
            }),
        }
    }

    /// The same window, reading on this thread (`ask_now`) — the
    /// photograph's.
    #[must_use]
    pub fn at_once(mut self) -> Self {
        self.git = self.git.at_once();
        self
    }

    /// The repository this window reads, when the folder is in one.
    #[must_use]
    pub const fn repo(&self) -> Option<&ronoctua_git::Repo> {
        self.git.repo.as_ref()
    }

    /// Reads what the view wants, each on a thread of its own.
    pub fn ask(&mut self) {
        self.git.ask();
    }

    /// Reads what the view wants ON THIS THREAD, until it wants nothing —
    /// the photograph's, which must not draw before the answers it shows.
    pub fn ask_now(&mut self) {
        self.git.ask_now();
    }

    /// What a thread came back with.
    fn back(&mut self, ui: &mut Ui, b: Back) {
        let _ = self.git.back(b, ui);
    }

    /// Carries out what the view asked for, by the errand handler the
    /// footer's region shares.
    fn apply(&mut self, ui: &mut Ui, errand: Errand) {
        crate::git_region::carry_out(errand, &mut self.git, ui);
    }

    /// The region's shape: its width in cells, its lines, and its pixels.
    fn shape(ui: &Ui) -> (u16, usize, crate::git_browser::Px) {
        let first = ui.first_of_list();
        let f = ui.frame(first);
        #[allow(clippy::cast_precision_loss)]
        let edge = ui.width as f32;
        (
            ui.columns,
            ui.room_for_rows(),
            crate::git_browser::Px {
                lead: ui.margin,
                top: f.y_of(first),
                bottom: f.bottom(),
                edge,
                cell: f.cell(),
                line: f.line(),
            },
        )
    }

    /// The line and cell of the region a point is on.
    fn point(ui: &Ui, x: f64, y: f64) -> Option<(usize, u16)> {
        let (columns, room, px) = Self::shape(ui);
        #[allow(clippy::cast_possible_truncation)]
        let (x, y) = (x as f32, y as f32);
        if y < px.top || x < px.x(0) {
            return None;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let line = ((y - px.top) / px.line) as usize;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let cell = ((x - px.x(0)) / px.cell) as u16;
        (line < room && cell < columns.saturating_add(ui.margin)).then_some((line, cell))
    }
}

impl Content for GitWindow {
    type Hand = Held;

    fn app_id(&self) -> &'static str {
        APP_ID
    }

    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        let name = self
            .git
            .view
            .root()
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        texts.t_with(crate::key::FILES_GIT_TITLE, &[("name", &name)])
    }

    fn default_size(&self) -> (u32, u32) {
        (
            u32::from(ronoctua_tokens::space::WINDOW_GITWIDTHPX),
            u32::from(ronoctua_tokens::space::WINDOW_GITHEIGHTPX),
        )
    }

    /// A key, read by its whole notation through the view's own table.
    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers) {
        let Some(key) = crate::door::keys::key_of(event, mods) else {
            return;
        };
        ui.message = None;
        let errand = self.git.view.key(key);
        self.apply(ui, errand);
    }

    fn language_changed(&mut self, ui: &mut Ui) {
        self.git.view.speak(ui.texts.clone());
    }

    fn hint(&self, ui: &Ui) -> Vec<crate::chrome::Piece> {
        self.git.view.hint(&ui.texts)
    }

    /// A sweep of text stays in the column it started in.
    fn text_area(&self, ui: &Ui, line: u16, column: u16) -> Option<crate::text_select::Area> {
        let top = ui.first_of_list();
        if line < top {
            return None;
        }
        let drawn = self.git.view.drawn(ui.columns, ui.room_for_rows());
        let cell = column.checked_sub(ui.margin)?;
        let pane = drawn.pane_at(cell)?;
        Some(crate::text_select::Area {
            left: ui.margin + pane.from,
            right: ui.margin + pane.text_to(),
            top,
            bottom: ui.lines,
        })
    }

    fn paint(&mut self, ui: &mut Ui, canvas: &mut Canvas<'_>) {
        let name = self.title(&ui.texts);
        let hint = self.git.view.hint(&ui.texts);
        crate::door::list::header_of(
            ui,
            canvas,
            Some(crate::chrome::Title {
                glyph: Some(GLYPH.as_str()),
                label: None,
                name: Some(&name),
            }),
            |_| hint,
        );
        let (columns, room, px) = Self::shape(ui);
        let drawn = self.git.view.drawn(columns, room);
        let first = ui.first_of_list();
        let size = tokens::text::size();
        for (n, line) in drawn.lines().into_iter().enumerate() {
            for f in line {
                canvas.spans.push(ronoctua_render::Span {
                    size: crate::fields::run_size(f.ink, size, px.line),
                    text: f.text,
                    column: ui.margin + f.at,
                    line: first + u16::try_from(n).unwrap_or(u16::MAX),
                    color: f.color,
                    edge: false,
                    foot: false,
                });
            }
        }
        canvas.under.extend(crate::git_browser::bands(&drawn, &px));
        canvas
            .under
            .extend(crate::git_browser::dividers(&drawn, &px));
        // EVERY POINTER'S HOVER, in its own colour, on the row it is on —
        // and in that row's column only.
        for &(color, at) in canvas.lit {
            let Some(at) = at else { continue };
            let (line, cell) = (at / CELL_SPAN, u16::try_from(at % CELL_SPAN).unwrap_or(0));
            if let Some(q) = crate::git_browser::hover_band(&drawn, &px, line, cell, color) {
                canvas.under.push(q);
            }
        }
        for bar in crate::git_browser::bars(&drawn, &px) {
            canvas.bar(Some(crate::bars::Track::of(bar.track)));
            if let Some((x, y, width, height)) = bar.thumb {
                canvas.under.push(ronoctua_render::quad::Quad {
                    x,
                    y,
                    width,
                    height,
                    color: tokens::color::RULE_STRONG,
                });
            }
        }
    }

    fn pointer(&mut self, ui: &mut Ui, hand: &mut Hand<Held>, events: &[PointerEvent]) {
        ui.hinted.swap_hand(&mut hand.hint_drag);
        self.pointer_here(ui, hand, events);
        ui.hinted.swap_hand(&mut hand.hint_drag);
    }
}

impl GitWindow {
    #[allow(clippy::too_many_lines)]
    fn pointer_here(&mut self, ui: &mut Ui, hand: &mut Hand<Held>, events: &[PointerEvent]) {
        let head = ui.frame(0);
        let before = hand.row_under_pointer;
        let mut paint = false;
        for e in events {
            #[allow(clippy::cast_possible_truncation)]
            let (x, y) = (e.position.0 as f32, e.position.1 as f32);
            match e.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    if ui.hinted.dragged(head, e.position.1) {
                        paint = true;
                        continue;
                    }
                    let texts = ui.texts.clone();
                    let now = ui
                        .hinted
                        .under(e.position.0, e.position.1)
                        .and_then(|id| crate::chrome::hint_sentence(id, &texts));
                    paint |= ui.hinted.say(now);
                    // A BAR HELD follows the pointer.
                    if let Some(bar) = hand.extra.bar {
                        let (_, _, px) = Self::shape(ui);
                        paint |= self.git.view.scroll_bar(&bar, bar.offset_at(x, y, &px));
                        continue;
                    }
                    hand.row_under_pointer = Self::point(ui, e.position.0, e.position.1)
                        .map(|(line, cell)| line * CELL_SPAN + usize::from(cell));
                }
                PointerEventKind::Leave { .. } => {
                    ui.hinted.let_go();
                    paint |= ui.hinted.say(None);
                    hand.extra.bar = None;
                    hand.row_under_pointer = None;
                }
                PointerEventKind::Release { button, .. } if button == LEFT_BUTTON => {
                    ui.hinted.let_go();
                    hand.extra.bar = None;
                }
                PointerEventKind::Press { button, .. } if button == LEFT_BUTTON => {
                    if ui.hinted.pressed(head, e.position.0, e.position.1) {
                        paint = true;
                        continue;
                    }
                    // WHERE A BAR IS DRAWN IS WHERE IT IS GRABBED.
                    let (columns, room, px) = Self::shape(ui);
                    let drawn = self.git.view.drawn(columns, room);
                    if let Some(bar) = crate::git_browser::bars(&drawn, &px)
                        .into_iter()
                        .find(|b| b.holds(x, y))
                    {
                        hand.extra.bar = Some(bar);
                        paint |= self.git.view.scroll_bar(&bar, bar.offset_at(x, y, &px));
                        continue;
                    }
                    if let Some((line, cell)) = Self::point(ui, e.position.0, e.position.1) {
                        let errand = self.git.view.clicked(line, cell);
                        self.apply(ui, errand);
                    }
                }
                PointerEventKind::Axis {
                    vertical,
                    horizontal,
                    ..
                } => {
                    crate::wheel::wheel_arrived(&mut hand.lines_owed, &vertical, &horizontal);
                    let by = crate::wheel::wheel_lines(&mut hand.lines_owed);
                    if by == 0 {
                        continue;
                    }
                    // A WHEEL OVER THE OPEN DRAWER WALKS THE DRAWER.
                    if ui.hinted.turned(head, e.position.1, by) {
                        paint = true;
                        continue;
                    }
                    if let Some((_, cell)) = Self::point(ui, e.position.0, e.position.1) {
                        paint |= self.git.view.wheel(cell, by);
                    }
                }
                _ => {}
            }
        }
        if paint || before != hand.row_under_pointer {
            ui.ask_paint();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outside_a_repository_every_column_says_so() {
        let dir = std::env::temp_dir().join(format!("ronoctua-git-window-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // A `.git` folder that is not a repository keeps the walk up from
        // finding one around the scratch folder.
        let (sender, _backs) = channel::channel::<Back>();
        let texts = std::rc::Rc::new(ronoctua_i18n::Texts::fresh("en-GB").unwrap());
        let mut w = GitWindow::over(
            std::path::Path::new("/"),
            texts.clone(),
            ronoctua_files::when::Zone::utc(),
            sender,
        );
        w.git.repo = None;
        w.ask_now();
        let d = w.git.view.drawn(120, 20);
        let said: String = d
            .lines()
            .into_iter()
            .flatten()
            .map(|f| f.text)
            .collect::<Vec<_>>()
            .join(" ");
        let sentence = crate::git_line::why_said(ronoctua_git::Why::NotARepo, &texts);
        assert!(said.contains(&sentence), "{said}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
