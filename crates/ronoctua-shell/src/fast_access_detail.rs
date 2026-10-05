//! FAST-ACCESS'S DETAIL — what the cursor's hit is, beside the results (or
//! below them), and what can be done with it.
//!
//! # One detail per kind
//!
//! - A FILE: the file manager's own preview and facts (`inspection::Panel`):
//!   a picture, a page of text, a film that plays, a shape that turns — and
//!   under it the facts, every one that can be changed drawn as a setting
//!   and changed from here (`inspection_edit`): the name, when it was
//!   modified, a song's tags, a picture's or a PDF's details, where it was
//!   taken, a permission bit. The owner and the group ask the root's door
//!   the file manager has, so pressing them shows the file there.
//! - AN ICON: the icons app's page (`icons_page::Page`): the stage, its
//!   adjustments and every way to copy it.
//! - A VIDEO: its picture, its facts, and where it plays.
//! - AN APPLICATION, A COMMAND, A PROGRAM, A MANUAL PAGE, A SUM, A PAGE OF
//!   THE WEB: rows of what it is, then rows of what it does — each a press.
//!
//! # The keys
//!
//! `TAB` moves the keys here and back. Here, `↑`/`↓` walk the rows that act
//! (or the facts of a file), `RET` does what the row says, and the icon's
//! page keeps its own (`←`/`→` adjust or show the next icon). `C-w` gives
//! the detail the whole area.

use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::Arc;

use ronoctua_tokens as tokens;

use crate::fast_access::{Errand, Hit, Press, View, What};
use crate::fast_access_rows::{Drawing, Lay, Pic, Source};
use crate::icons_page::Area;

/// What the detail keeps.
#[derive(Default)]
pub struct Detail {
    /// The first row shown.
    pub offset: usize,
    /// The row the cursor is on — always one that acts.
    pub cursor: usize,
    /// The hit the detail was laid out for: a new one starts at the top.
    pub of: Option<String>,
    /// The icon's page, while an icon is the hit.
    pub icon: Option<crate::icons_page::Page>,
    /// What a person adjusted of icons here.
    pub icon_settings: crate::icons_page::Settings,
    /// The preview and the facts of a file, while one is the hit.
    pub panel: Option<FilePanel>,
    /// A question about the file, while it is asked (`no` first).
    pub confirm: Option<Confirm>,
    /// The change the question line answers, while it asks.
    pub asking: Option<Amend>,
    /// A manual page's first lines, when read.
    pub man: Option<(String, Vec<String>)>,
    /// The manual page asked for (`name.section`) — not to ask twice.
    pub man_asked: Option<String>,
    /// What a program to install is, once read, by its attribute.
    pub program: Option<(String, crate::fast_access_search::Program)>,
    /// The program asked for — not to ask twice.
    pub program_asked: Option<String>,
    /// The person's zone, for a date typed or shown.
    pub zone: Option<ronoctua_files::when::Zone>,
    /// The still picture of the file's preview, shared with the frame.
    still: RefCell<Option<(String, Arc<ronoctua_render::image::Rgba>)>>,
}

impl std::fmt::Debug for Detail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Detail")
            .field("offset", &self.offset)
            .field("cursor", &self.cursor)
            .field("of", &self.of)
            .field("panel", &self.panel.as_ref().map(|p| &p.path))
            .finish_non_exhaustive()
    }
}

/// The file manager's panel, about one file.
pub struct FilePanel {
    /// The panel.
    pub panel: crate::inspection::Panel,
    /// The file.
    pub path: PathBuf,
}

/// A change to the file under the detail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Amend {
    /// Its name.
    Rename(PathBuf),
    /// When it was modified.
    Modified(PathBuf),
    /// A song's or a film's tag.
    Tag(PathBuf, ronoctua_tags::Field),
    /// A picture's or a PDF's detail.
    Meta(PathBuf, ronoctua_inspect::amend::Meta),
    /// Its location taken out — `copies`: into a copy beside it.
    RemoveLocation(PathBuf, bool),
    /// Its permission set to this mode.
    Mode(PathBuf, u32),
    /// Its owner — the file manager's door.
    Owner(PathBuf),
    /// A folder's size, walked.
    Measure(PathBuf),
    /// A program installed — what its questions said yes to so far
    /// (`View::install` asks the next one, or queues it).
    Install {
        /// Its attribute.
        attr: String,
        /// Its name, as the row has it (data).
        name: String,
        /// What was said yes to.
        consent: ronoctua_software::order::Consent,
    },
}

/// The question a change asks first — `no` first, the cursor born on it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Confirm {
    /// What it asks.
    pub title: String,
    /// The answers' words, `no` first.
    pub answers: Vec<String>,
    /// What each answer does (`None`: nothing).
    pub does: Vec<Option<Amend>>,
    /// The answer under the cursor.
    pub cursor: usize,
}

/// What is under a point of the detail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// A row, by its place.
    Row(usize),
    /// The detail's bar.
    Bar,
    /// The icon's page.
    Icon(crate::icons_page::Target),
    /// A cell of a file's panels: its line in the detail and its column.
    Cell(u16, u16),
    /// An answer of the question.
    Answer(usize),
    /// A bar of a file's panels: the preview's or the facts', down or across
    /// — or the film's timeline ([`FileBar`]).
    FileBar(crate::files::Pane, crate::files::Axis),
}

/// The tag bits of a detail target's code.
const TAG: usize = 1 << 24;

impl Target {
    /// The target as one number.
    #[must_use]
    pub fn code(self) -> usize {
        match self {
            Self::Row(n) => n,
            Self::Bar => TAG,
            Self::Icon(crate::icons_page::Target::Row(n)) => 2 * TAG + n,
            Self::Icon(crate::icons_page::Target::Bar) => 3 * TAG,
            Self::Icon(crate::icons_page::Target::Stage) => 3 * TAG + 1,
            Self::Cell(line, column) => 4 * TAG + usize::from(line) * 4096 + usize::from(column),
            Self::Answer(n) => 5 * TAG + n,
            Self::FileBar(pane, axis) => {
                let pane = usize::from(pane == crate::files::Pane::Info);
                let axis = match axis {
                    crate::files::Axis::Down => 0,
                    crate::files::Axis::Across => 1,
                    crate::files::Axis::Time => 2,
                };
                6 * TAG + pane * 4 + axis
            }
        }
    }

    /// The target a number is.
    #[must_use]
    pub fn of_code(code: usize) -> Self {
        let (tag, n) = (code / TAG, code % TAG);
        match tag {
            0 => Self::Row(n),
            1 => Self::Bar,
            2 => Self::Icon(crate::icons_page::Target::Row(n)),
            3 if n == 0 => Self::Icon(crate::icons_page::Target::Bar),
            3 => Self::Icon(crate::icons_page::Target::Stage),
            4 => Self::Cell(
                u16::try_from(n / 4096).unwrap_or(0),
                u16::try_from(n % 4096).unwrap_or(0),
            ),
            6 => Self::FileBar(
                if n / 4 == 1 {
                    crate::files::Pane::Info
                } else {
                    crate::files::Pane::Preview
                },
                match n % 4 {
                    0 => crate::files::Axis::Down,
                    1 => crate::files::Axis::Across,
                    _ => crate::files::Axis::Time,
                },
            ),
            _ => Self::Answer(n),
        }
    }
}

/// What a row of the detail does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Act {
    /// Nothing: a fact.
    Nothing,
    /// What `RET` on the results would do.
    Enter,
    /// Put this on the clipboard.
    Copy(String),
    /// Play it in the player.
    Player(String),
    /// Open it in the kiosk.
    Kiosk(String),
    /// Narrow the search to a scope.
    Narrow,
    /// What `C-RET` on the results would do.
    Secondary,
}

/// One row of the detail: a label, a value, and what pressing it does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// What it is.
    pub label: String,
    /// Its value — empty for an action.
    pub value: String,
    /// What pressing it does.
    pub act: Act,
}

impl Row {
    fn fact(label: String, value: String) -> Self {
        Self {
            label,
            value,
            act: Act::Nothing,
        }
    }

    fn action(label: String, act: Act) -> Self {
        Self {
            label,
            value: String::new(),
            act,
        }
    }

    /// Whether the cursor rests on it.
    #[must_use]
    pub fn acts(&self) -> bool {
        self.act != Act::Nothing
    }
}

/// The rows of a hit that is neither a file nor an icon.
#[must_use]
pub fn rows_of(view: &View, hit: &Hit, texts: &ronoctua_i18n::Texts) -> Vec<Row> {
    let t = |k: &str| texts.t(k);
    let mut out = Vec::new();
    match &hit.what {
        What::App { desktop_id, .. } => {
            out.push(Row::fact(t("fast-access-detail-name"), hit.title.clone()));
            if let Some(n) = &hit.note {
                out.push(Row::fact(t("fast-access-detail-description"), n.clone()));
            }
            out.push(Row::fact(
                t("fast-access-detail-program"),
                desktop_id.clone(),
            ));
            out.push(Row::action(t("fast-access-act-open"), Act::Enter));
            out.push(Row::action(
                t("fast-access-act-copy"),
                Act::Copy(desktop_id.clone()),
            ));
        }
        What::Command { id, keys } => {
            out.push(Row::fact(t("fast-access-detail-command"), (*id).to_owned()));
            out.push(Row::fact(
                t("fast-access-detail-description"),
                hit.title.clone(),
            ));
            if let Some(k) = keys {
                out.push(Row::fact(t("fast-access-detail-keys"), k.clone()));
            }
            out.push(Row::action(t("fast-access-act-run"), Act::Enter));
        }
        What::Man { name, section } => {
            out.push(Row::fact(t("fast-access-detail-name"), name.clone()));
            out.push(Row::fact(t("fast-access-detail-section"), section.clone()));
            if let Some(n) = &hit.note {
                out.push(Row::fact(t("fast-access-detail-description"), n.clone()));
            }
            out.push(Row::action(t("fast-access-act-terminal"), Act::Enter));
            if let Some((page, lines)) = &view.detail.man
                && *page == format!("{name}.{section}")
            {
                for l in lines {
                    out.push(Row::fact(String::new(), l.clone()));
                }
            }
        }
        What::Package { attr } => {
            out.push(Row::fact(t("fast-access-detail-program"), attr.clone()));
            let read = view
                .detail
                .program
                .as_ref()
                .filter(|(a, _)| a == attr)
                .map(|(_, p)| &p.details);
            match read {
                // WHAT THE PROGRAMS' CENTRE READS of it, the same rows.
                Some(Ok(d)) => program_rows(d, texts, &mut out),
                // WHY THERE IS NOTHING MORE, said — and the index's line.
                Some(Err(why)) => {
                    if let Some(n) = &hit.note {
                        out.push(Row::fact(t("fast-access-detail-description"), n.clone()));
                    }
                    out.push(Row::fact(
                        t(crate::software_words::refusal_key(*why)),
                        String::new(),
                    ));
                }
                None => {
                    if let Some(n) = &hit.note {
                        out.push(Row::fact(t("fast-access-detail-description"), n.clone()));
                    }
                }
            }
            out.push(Row::action(t("fast-access-act-install"), Act::Enter));
            out.push(Row::action(t("fast-access-act-centre"), Act::Secondary));
            out.push(Row::action(
                t("fast-access-act-copy"),
                Act::Copy(attr.clone()),
            ));
        }
        What::Video(v) => {
            out.push(Row::fact(t("fast-access-detail-name"), hit.title.clone()));
            if let Some(c) = &v.channel {
                out.push(Row::fact(t("fast-access-detail-channel"), c.clone()));
            }
            if let Some(d) = v.duration_s {
                out.push(Row::fact(
                    t("fast-access-detail-duration"),
                    crate::fast_access_videos::length(d),
                ));
            }
            if let Some(n) = v.views {
                let n = i64::try_from(n).unwrap_or(i64::MAX);
                out.push(Row::fact(
                    t("fast-access-detail-views"),
                    texts.t_args(
                        ronoctua_i18n::key::FAST_ACCESS_VIEWS,
                        &[("n", ronoctua_i18n::Arg::Number(n))],
                    ),
                ));
            }
            out.push(Row::fact(
                t("fast-access-detail-from"),
                t(if v.local {
                    "fast-access-youtube-local"
                } else {
                    "fast-access-youtube-online"
                }),
            ));
            if v.short {
                out.push(Row::fact(t("fast-access-detail-short"), String::new()));
            }
            out.push(Row::action(
                t("fast-access-act-play-player"),
                Act::Player(v.url.clone()),
            ));
            out.push(Row::action(
                t("fast-access-act-play-kiosk"),
                Act::Kiosk(v.url.clone()),
            ));
            out.push(Row::action(
                t("fast-access-act-copy-address"),
                Act::Copy(v.url.clone()),
            ));
        }
        What::Web {
            url,
            snippet,
            source,
        } => {
            out.push(Row::fact(t("fast-access-detail-name"), hit.title.clone()));
            out.push(Row::fact(t("fast-access-detail-address"), url.clone()));
            out.push(Row::fact(
                t("fast-access-detail-source"),
                t(&format!("fast-access-web-{source}")),
            ));
            if let Some(s) = snippet {
                out.push(Row::fact(t("fast-access-detail-description"), s.clone()));
            }
            out.push(Row::action(t("fast-access-act-kiosk"), Act::Enter));
            out.push(Row::action(
                t("fast-access-act-copy-address"),
                Act::Copy(url.clone()),
            ));
        }
        What::Kiosk(url) => {
            out.push(Row::fact(t("fast-access-detail-address"), url.clone()));
            out.push(Row::action(t("fast-access-act-kiosk"), Act::Enter));
        }
        What::Math(answer) => {
            let mark = texts.decimal_mark();
            out.push(Row::fact(
                hit.title.clone(),
                hit.note.clone().unwrap_or_default(),
            ));
            if let Some(c) = crate::fast_access::copy_of(hit, texts) {
                out.push(Row::action(t("fast-access-act-copy"), Act::Copy(c)));
            }
            // A CURRENCY SAYS WHICH DAY'S RATES it was worked at — the older
            // ones on disk among them, when today's did not come.
            if answer.shape == ronoctua_calc::Shape::Currency
                && let Some(r) = view.rates.as_ref()
            {
                out.push(Row::fact(
                    texts.t_with(
                        ronoctua_i18n::key::CALC_RATES_OF,
                        &[("date", &crate::calc::rates_day(r, texts))],
                    ),
                    String::new(),
                ));
            }
            if !answer.also.is_empty() {
                out.push(Row::fact(t("fast-access-math-also"), String::new()));
                for v in &answer.also {
                    let said = crate::calc::answer_text(v, mark, texts);
                    let copied = said.trim_start_matches(['=', '≈', ' ']).to_owned();
                    out.push(Row {
                        label: String::new(),
                        value: said,
                        act: Act::Copy(copied),
                    });
                }
            }
        }
        What::Zone { name, offset_s } => {
            let now = crate::fast_access_rows::zone_time(*offset_s, ronoctua_index::now_s());
            out.push(Row::fact(t("fast-access-detail-zone"), name.clone()));
            out.push(Row::fact(t("fast-access-detail-time"), now.clone()));
            out.push(Row::action(t("fast-access-act-copy"), Act::Copy(now)));
        }
        What::Prefix(scope) => {
            out.push(Row::fact(t("fast-access-detail-scope"), scope.does(texts)));
            out.push(Row::action(t("fast-access-act-narrow"), Act::Narrow));
        }
        // THE CONSENT ROW, as the results draw it: what pressing does, and
        // what for — the note wrapped over the whole width.
        What::Consent { hosts, what } => {
            let hosts: Vec<&str> = hosts.iter().map(String::as_str).collect();
            out.push(Row::action(
                crate::consent_rows::label(&hosts, texts),
                Act::Enter,
            ));
            out.push(Row::fact(
                crate::consent_rows::note(what, texts),
                String::new(),
            ));
        }
        What::Shorts(shown) => out.push(Row::action(
            t(if *shown {
                "fast-access-shorts-shown"
            } else {
                "fast-access-shorts-hidden"
            }),
            Act::Enter,
        )),
        // A SETTING says what it is now, and `RET` turns it.
        What::Setting(_) => out.push(Row {
            label: hit.title.clone(),
            value: hit.note.clone().unwrap_or_default(),
            act: Act::Enter,
        }),
        What::Words(_) => out.push(Row::action(hit.title.clone(), Act::Enter)),
        What::File { .. } | What::Icon(_) => {}
    }
    out
}

/// The detail moved onto `hit`: a new one starts at the top, an icon gets its
/// page and a file its panels (pointed at it — the door's threads look).
pub fn follow_hit(
    view: &mut View,
    hit: Option<&Hit>,
    texts: &std::rc::Rc<ronoctua_i18n::Texts>,
    zone: &ronoctua_files::when::Zone,
) {
    let id = hit.map(|h| h.id.clone());
    if view.detail.of == id {
        return;
    }
    let keep = std::mem::take(&mut view.detail.panel);
    let page = std::mem::take(&mut view.detail.icon);
    let settings = view.detail.icon_settings.clone();
    view.detail = Detail {
        of: id,
        icon_settings: settings,
        zone: Some(zone.clone()),
        ..Detail::default()
    };
    let Some(hit) = hit else {
        return;
    };
    match &hit.what {
        What::Icon(icon) => {
            if let Some(set) = view.icons.clone() {
                // THE SAME PAGE, about another icon: the cursor stays on the
                // same kind of row (`Page::show`).
                view.detail.icon = Some(match page {
                    Some(mut p) => {
                        p.show(*icon, &set, &view.detail.icon_settings, texts);
                        p
                    }
                    None => {
                        crate::icons_page::Page::of(*icon, &set, &view.detail.icon_settings, texts)
                    }
                });
            }
        }
        What::File { path, .. } => {
            // ONE PANEL, pointed again: its threads' answers about the last
            // file are dropped by the panel itself (`Panel::looked`).
            let panel = keep.map_or_else(
                || crate::inspection::Panel::new(std::rc::Rc::clone(texts), zone.clone()),
                |p| p.panel,
            );
            let roster =
                ronoctua_files::who::Roster::read_from(&ronoctua_files::who::Files::default());
            panel.focus(crate::inspection::Subject::of_path(path, &roster));
            view.detail.panel = Some(FilePanel {
                panel,
                path: path.clone(),
            });
        }
        _ => {}
    }
    // THE FIRST ROW THAT ACTS is where the cursor is born.
    let rows = rows_of(view, hit, texts);
    view.detail.cursor = rows.iter().position(Row::acts).unwrap_or(0);
}

/// The box a file's preview takes in the detail, and the facts' under it.
fn file_boxes(area: Area) -> (Area, Area) {
    let preview_lines = (area.lines * 3 / 5).max(3).min(area.lines);
    let preview = Area {
        lines: preview_lines,
        bottom_px: Area::y_of(area.top + preview_lines),
        ..area
    };
    let info = Area {
        top: area.top + preview_lines + 1,
        lines: area.lines.saturating_sub(preview_lines + 1),
        ..area
    };
    (preview, info)
}

/// The two sections of a file's panels in `area`, laid out.
fn file_sections(
    p: &FilePanel,
    area: Area,
    focused: bool,
    texts: &ronoctua_i18n::Texts,
) -> (crate::files::Section, crate::files::Section, Area, Area) {
    use crate::files::{Pane, Panel as Which};
    let (pre, info) = file_boxes(area);
    p.panel.around(crate::inspection::Around {
        covered: false,
        keyboard: if focused { Pane::Info } else { Pane::Preview },
    });
    let sheet = p.panel.sheet();
    p.panel
        .laid_out(Which::Preview, pre.columns, usize::from(pre.lines));
    let preview = p.panel.section(
        Which::Preview,
        (pre.left, pre.columns),
        (0, usize::from(pre.lines)),
        &sheet,
        texts,
    );
    p.panel
        .laid_out(Which::Info, info.columns, usize::from(info.lines));
    let facts = p.panel.section(
        Which::Info,
        (info.left, info.columns),
        (0, usize::from(info.lines)),
        &sheet,
        texts,
    );
    (preview, facts, pre, info)
}

/// One bar of a file's panels: the pane and the axis it moves, the STRIP it
/// owns — what a press takes, what the bars' registry records — and what is
/// drawn in it: the thumb, or the film's cursor on its timeline.
#[derive(Debug, Clone, Copy, PartialEq)]
struct FileBar {
    pane: crate::files::Pane,
    axis: crate::files::Axis,
    strip: (f32, f32, f32, f32),
    mark: Option<(f32, f32, f32, f32)>,
}

impl FileBar {
    fn holds(&self, x: f32, y: f32) -> bool {
        let (sx, sy, sw, sh) = self.strip;
        x >= sx && x < sx + sw && y >= sy && y < sy + sh
    }
}

/// EVERY BAR of a file's panels, as the frame draws them — the preview's and
/// the facts' going down, the preview's going across (a line of code wider
/// than the box), and a film's timeline — from the sections' own rectangles
/// (`files::Section::bar_box`, `wide_bar_box`, `time_track_box`): ONE answer
/// for the drawing, the press, the drag and the registry of bars, as the
/// file manager's doors have (`files_window`'s `section_bars`).
fn bars_of(laid: [(&crate::files::Section, Area); 2]) -> Vec<FileBar> {
    use crate::files::Axis;
    let (cell, line) = (Area::cell(), Area::line());
    let mut out = Vec::new();
    for (sec, b) in laid {
        let top = Area::y_of(b.top);
        let bottom = Area::y_of(b.top + b.lines);
        let (left, wide) = (Area::x_of(b.left), f32::from(b.columns) * cell);
        if let Some(thumb) = sec.bar_box(Area::x_of(sec.to), top, bottom, cell, line) {
            // THE WHOLE COLUMN IS THE BAR'S: a press anywhere on it puts the
            // panel there, as the thumb's own inverse says (`offset_at`).
            let band_top = top + f32::from(u16::try_from(sec.top + sec.above).unwrap_or(0)) * line;
            let track = ronoctua_design::Scroll {
                lines: sec.visible(),
                ..sec.scroll()
            }
            .track(band_top, bottom, line);
            out.push(FileBar {
                pane: sec.pane,
                axis: Axis::Down,
                strip: (thumb.0, band_top, thumb.2, track),
                mark: Some(thumb),
            });
        }
        if let Some(thumb) = sec.wide_bar_box(left, top, bottom, wide, cell, line) {
            out.push(FileBar {
                pane: sec.pane,
                axis: Axis::Across,
                strip: (left, thumb.1, wide, thumb.3),
                mark: Some(thumb),
            });
        }
        // THE TIMELINE, from the transport's own cells: the section's columns
        // are the surface's here, so nothing leads them.
        let (fx, fwide) = sec.fields_px(0, cell);
        let foot = crate::files::Foot {
            left: fx,
            wide: fwide,
            cell,
            line_px: line,
            bottom,
        };
        if let Some(track) = sec.time_track_box(foot) {
            out.push(FileBar {
                pane: sec.pane,
                axis: Axis::Time,
                strip: track,
                mark: sec.time_bar_box(foot),
            });
        }
    }
    out
}

/// The bars of the file under the detail in `area`.
fn file_bars(
    p: &FilePanel,
    area: Area,
    focused: bool,
    texts: &ronoctua_i18n::Texts,
) -> Vec<FileBar> {
    let (preview, facts, pre, info) = file_sections(p, area, focused, texts);
    bars_of([(&preview, pre), (&facts, info)])
}

/// Which of a file's panels a point of the detail `area` is over.
fn pane_at(area: Area, y: f32) -> crate::files::Pane {
    let (_, info) = file_boxes(area);
    if y < Area::y_of(info.top.saturating_sub(1)) {
        crate::files::Pane::Preview
    } else {
        crate::files::Pane::Info
    }
}

/// The pointer rests on `target` of the detail in `area` — or left it: the
/// file's panel is told which of its panes it is over, which is what brings a
/// film's controls up (`inspection::Panel::pointing`).
pub fn pointed(view: &View, area: Area, target: Option<(Target, f32)>) {
    let Some(p) = view.detail.panel.as_ref() else {
        return;
    };
    p.panel.pointing(match target {
        Some((Target::FileBar(pane, _), _)) => Some(pane),
        Some((Target::Cell(..), y)) => Some(pane_at(area, y)),
        _ => None,
    });
}

/// What the field of a file's panels under `target` says WHOLE: its label and
/// its value put back together — the panel cuts a long value to its column,
/// as the file manager's does, and the line under the pointer says the rest
/// there as here (`inspection::Panel::fact_fields`). `None` for a target that
/// is not such a field.
#[must_use]
pub fn about(
    view: &View,
    area: Area,
    target: Target,
    texts: &ronoctua_i18n::Texts,
) -> Option<String> {
    let Target::Cell(line, column) = target else {
        return None;
    };
    if !matches!(view.current()?.what, What::File { .. }) {
        return None;
    }
    let p = view.detail.panel.as_ref()?;
    let focused = view.focus == crate::fast_access::Focus::Detail;
    let (preview, facts, pre, info) = file_sections(p, area, focused, texts);
    let at = area.top + line;
    [(&preview, pre.top), (&facts, info.top)]
        .into_iter()
        .find_map(|(s, top)| {
            let i = usize::from(at.checked_sub(top)?);
            s.content
                .get(i)?
                .fields
                .iter()
                .rev()
                .find(|f| f.covers(column))?
                .explain
                .clone()
        })
        .map(|e| texts.t_with(e.key, &e.borrowed()))
}

/// A file's bar held at `(x, y)`: the panel moved where the pointer is, along
/// the bar's own strip — the facts or the preview down, the preview across,
/// the film to that moment.
pub fn file_dragged(
    view: &mut View,
    area: Area,
    (pane, axis): (crate::files::Pane, crate::files::Axis),
    (x, y): (f32, f32),
    texts: &ronoctua_i18n::Texts,
) {
    use crate::files::{Axis, Panel as Which};
    let focused = view.focus == crate::fast_access::Focus::Detail;
    let Some(p) = view.detail.panel.as_mut() else {
        return;
    };
    let (preview, facts, pre, info) = file_sections(p, area, focused, texts);
    let Some(bar) = bars_of([(&preview, pre), (&facts, info)])
        .into_iter()
        .find(|b| b.pane == pane && b.axis == axis)
    else {
        return;
    };
    let (sx, sy, sw, sh) = bar.strip;
    let along =
        |at: f32, from: f32, span: f32| f64::from(((at - from) / span.max(1.0)).clamp(0.0, 1.0));
    let (which, sec) = if pane == crate::files::Pane::Info {
        (Which::Info, &facts)
    } else {
        (Which::Preview, &preview)
    };
    let _ = match axis {
        Axis::Down => p.panel.drag_down(which, along(y, sy, sh), sec.visible()),
        Axis::Across => p.panel.drag_across(along(x, sx, sw), pre.columns),
        Axis::Time => p.panel.drag_time(along(x, sx, sw)),
    };
}

/// Draws the detail of `hit` in `area`.
pub fn draw(
    view: &View,
    hit: &Hit,
    area: Area,
    lit: &[(tokens::Color, Target)],
    texts: &ronoctua_i18n::Texts,
    out: &mut Drawing,
) {
    let focused = view.focus == crate::fast_access::Focus::Detail;
    if let Some(c) = &view.detail.confirm {
        draw_confirm(c, area, out);
        return;
    }
    match &hit.what {
        What::Icon(_) => {
            if let (Some(page), Some(set)) = (&view.detail.icon, view.icons.as_deref()) {
                let rows: Vec<(tokens::Color, usize)> = lit
                    .iter()
                    .filter_map(|(c, t)| match t {
                        Target::Icon(crate::icons_page::Target::Row(n)) => Some((*c, *n)),
                        _ => None,
                    })
                    .collect();
                let mut drawing = crate::icons_page::Drawing::default();
                page.draw(
                    area,
                    set,
                    &view.detail.icon_settings,
                    texts,
                    &rows,
                    &mut drawing,
                );
                out.spans.extend(drawing.spans);
                out.under.extend(drawing.under);
                out.over.extend(drawing.thumbs);
                out.tracks.extend(drawing.tracks);
                out.pictures
                    .extend(drawing.pictures.into_iter().map(|p| Pic {
                        key: p.key.clone(),
                        area: p.area,
                        source: Source::Svg(p),
                    }));
            }
        }
        What::File { .. } => {
            if let Some(p) = &view.detail.panel {
                draw_file(view, p, area, focused, lit, texts, out);
            }
        }
        _ => draw_rows(view, hit, area, focused, lit, texts, out),
    }
}

/// A file's panels: the preview on top — its picture when it has one — and
/// the facts under it, every field where the panel placed it.
fn draw_file(
    view: &View,
    p: &FilePanel,
    area: Area,
    focused: bool,
    lit: &[(tokens::Color, Target)],
    texts: &ronoctua_i18n::Texts,
    out: &mut Drawing,
) {
    let (preview, facts, pre, info) = file_sections(p, area, focused, texts);
    // THE PREVIEW'S BOX, told to the panel: what it asks the inspector for.
    let wide = f32::from(pre.columns) * Area::cell();
    let tall = f32::from(pre.lines) * Area::line();
    p.panel.measured_preview(wide, tall);
    if let Some(key) = p.panel.preview_key() {
        let space = ronoctua_render::image::Area {
            x: Area::x_of(pre.left),
            y: Area::y_of(pre.top),
            width: wide,
            height: tall,
        };
        let moving = p.panel.live_frame();
        let still = moving.is_none().then(|| p.panel.preview_pixels()).flatten();
        if let Some(rgba) = moving {
            out.pictures.push(Pic {
                key,
                area: crate::files::picture_in(space, rgba.width, rgba.height),
                source: Source::Moving(Arc::new(rgba.clone())),
            });
        } else if let Some(rgba) = still {
            // THE STILL ONE IS SHARED, made once per key: a preview is up to a
            // mebibyte, and a frame is drawn on every event.
            let mut kept = view.detail.still.borrow_mut();
            let shared = match kept.as_ref() {
                Some((k, r)) if *k == key => Arc::clone(r),
                _ => {
                    let r = Arc::new(rgba.clone());
                    *kept = Some((key.clone(), Arc::clone(&r)));
                    r
                }
            };
            out.pictures.push(Pic {
                key,
                area: crate::files::picture_in(space, shared.width, shared.height),
                source: Source::Still(shared),
            });
        }
    }
    let lit_cells: Vec<(tokens::Color, u16, u16)> = lit
        .iter()
        .filter_map(|(c, t)| match t {
            Target::Cell(l, col) => Some((*c, *l, *col)),
            _ => None,
        })
        .collect();
    for (section, top) in [(&preview, pre.top), (&facts, info.top)] {
        for (i, row) in section.content.iter().enumerate() {
            let at = top + u16::try_from(i).unwrap_or(u16::MAX);
            let line_in_detail = at - area.top;
            for (colour, l, col) in &lit_cells {
                if *l == line_in_detail
                    && let Some(f) = row.fields.iter().find(|f| f.covers(*col))
                    && !matches!(f.hit, crate::files::Hit::Nothing)
                {
                    out.under.push(ronoctua_render::quad::Quad {
                        x: Area::x_of(f.at),
                        y: Area::y_of(at),
                        width: f32::from(ronoctua_render::layout::width(&f.text)) * Area::cell(),
                        height: Area::line(),
                        color: *colour,
                    });
                }
            }
            for f in &row.fields {
                out.spans.push(ronoctua_render::Span {
                    text: f.text.clone(),
                    column: f.at,
                    line: at,
                    color: f.color,
                    size: crate::files::run_size(f.ink, tokens::text::size(), Area::line()),
                    edge: false,
                    foot: false,
                });
            }
        }
    }
    // EVERY BAR, from the one answer the pointer reads (`bars_of`): its strip
    // recorded — the column is the bar's (`crate::bars`) — its thumb over the
    // picture. A FILM'S TRANSPORT goes over the picture too, with the pointer:
    // the band that makes the glyphs legible, the timeline and its cursor.
    let showing = p.panel.transport_showing();
    let quad = |rect: (f32, f32, f32, f32), color| ronoctua_render::quad::Quad {
        x: rect.0,
        y: rect.1,
        width: rect.2,
        height: rect.3,
        color,
    };
    for bar in bars_of([(&preview, pre), (&facts, info)]) {
        if bar.axis != crate::files::Axis::Time {
            out.tracks.push(crate::bars::Track::of(bar.strip));
            if let Some(m) = bar.mark {
                out.over.push(crate::chrome::thumb(m));
            }
            continue;
        }
        if !showing {
            continue;
        }
        let (left, wide) = preview.fields_px(0, Area::cell());
        let foot = crate::files::Foot {
            left,
            wide,
            cell: Area::cell(),
            line_px: Area::line(),
            bottom: Area::y_of(pre.top + pre.lines),
        };
        if let Some(r) = crate::files::transport_band(&preview, foot) {
            out.over.push(quad(r, tokens::color::SURFACE_TRANSPORT));
        }
        // THE TRACK IS THE QUIETER TONE, the cursor on it what says where
        // the film is — the file manager's two rectangles.
        out.over.push(quad(bar.strip, tokens::color::RULE_STRONG));
        if let Some(m) = bar.mark {
            out.over.push(quad(m, tokens::color::TEXT_PRIMARY));
        }
    }
    // THE RULE between the preview and the facts — across the detail only,
    // from the divider's side: the results beside it are another list.
    let from = Area::x_of(area.left.saturating_sub(crate::chrome::FIELDS_LEAD));
    out.under.push(ronoctua_render::quad::Quad {
        x: from,
        width: (area.right_px - from).max(0.0),
        ..crate::chrome::rule(
            area.right_px,
            Area::y_of(info.top.saturating_sub(1)) + Area::line() / 2.0,
        )
    });
}

/// The question about the file: its title, then each answer — `no` first.
fn draw_confirm(c: &Confirm, area: Area, out: &mut Drawing) {
    let mut line = area.top;
    for l in crate::chrome::note_lines(&c.title, area.columns) {
        out.spans.push(ronoctua_render::Span {
            text: l,
            column: area.left,
            line,
            color: tokens::color::TEXT_HEADING,
            size: ronoctua_render::Size::Px(tokens::text::size()),
            edge: false,
            foot: false,
        });
        line += 1;
    }
    for (n, a) in c.answers.iter().enumerate() {
        let (label, _) = crate::chrome::Kind::ListedSetting.tones(c.cursor == n);
        out.spans.push(ronoctua_render::Span {
            text: a.clone(),
            column: area.left,
            line,
            color: label,
            size: ronoctua_render::Size::Px(tokens::text::size()),
            edge: false,
            foot: false,
        });
        line += 1;
    }
}

/// The rows of every other hit: a video's picture first when it has one,
/// then each row as a label and its value — the cursor's in the accent.
/// The rows of a hit as they are DRAWN in `area`: each line the row it
/// belongs to and its pieces — the label on the row's first line, the value
/// from the value column on, WRAPPED and never cut (`a-note-is-wrapped-and-
/// not-cut`): a description is read whole. What the drawing, the pointer, the
/// wheel, the bar and the keys all read, so a line is pressed where it is
/// drawn. The lines a video's picture takes first, and the width the rows
/// have.
struct Laid {
    picture: u16,
    width: u16,
    lines: Vec<(usize, Vec<crate::files::Field>)>,
}

impl Laid {
    fn of(view: &View, hit: &Hit, area: Area, focused: bool, texts: &ronoctua_i18n::Texts) -> Self {
        let picture = match &hit.what {
            What::Video(v)
                if v.thumb
                    .as_ref()
                    .is_some_and(|k| view.pictures.contains_key(k)) =>
            {
                tokens::space::FASTACCESS_GRIDPICTURELINES.min(area.lines / 2)
            }
            _ => 0,
        };
        let rows = rows_of(view, hit, texts);
        let room = usize::from(area.lines.saturating_sub(picture));
        let lay = |width: u16| -> Vec<(usize, Vec<crate::files::Field>)> {
            let mut out = Vec::new();
            for (k, row) in rows.iter().enumerate() {
                let here = focused && view.detail.cursor == k;
                for line in wrapped_pair(area.left, width, row, here) {
                    out.push((k, line));
                }
            }
            out
        };
        // THE BAR'S CELL, only when the lines are more than the room — and
        // laid out again at the narrower width, which may wrap one more.
        let mut width = area.columns;
        let mut lines = lay(width);
        if lines.len() > room {
            width = area.columns.saturating_sub(crate::chrome::BAR_CELLS);
            lines = lay(width);
        }
        Self {
            picture,
            width,
            lines,
        }
    }

    /// How many lines the rows have on screen.
    fn room(&self, area: Area) -> usize {
        usize::from(area.lines.saturating_sub(self.picture))
    }

    /// The first line shown, at `offset` held to the end.
    fn first(&self, area: Area, offset: usize) -> usize {
        offset.min(self.lines.len().saturating_sub(self.room(area)))
    }
}

/// One row as its lines: the label wrapped in its own column, the value
/// wrapped in its — both from the row's first line, the row as tall as the
/// taller (`chrome::aligned_pair`'s rule, with a value that wraps too). A row
/// with no value is its label, wrapped over the whole width. Nothing is ever
/// cut (`a-note-is-wrapped-and-not-cut`).
fn wrapped_pair(left: u16, width: u16, row: &Row, focused: bool) -> Vec<Vec<crate::files::Field>> {
    let (label_colour, value_colour) = crate::chrome::Kind::of(row.acts()).tones(focused);
    let piece = |at: u16, text: String, color: tokens::Color| crate::files::Field {
        at,
        text,
        color,
        hit: crate::files::Hit::Nothing,
        explain: None,
        ink: crate::files::Ink::Grid,
    };
    let end = left + width;
    if row.value.is_empty() {
        return crate::chrome::note_lines(&row.label, width)
            .into_iter()
            .map(|l| vec![piece(left, l, label_colour)])
            .collect();
    }
    let label_cells = ronoctua_render::layout::width(&row.label);
    // THE VALUE'S COLUMN, as `chrome::field_pair` places it: the view's
    // column, pushed by a longer label, never past the room a value needs —
    // a label longer than that wraps in its own column instead.
    let column = crate::chrome::value_column(left, width)
        .max(if label_cells == 0 {
            left
        } else {
            left + label_cells + 2
        })
        .min(end.saturating_sub(width / 2).max(left));
    let labels: Vec<String> = if label_cells == 0 {
        Vec::new()
    } else {
        crate::chrome::note_lines(&row.label, column.saturating_sub(left + 2).max(1))
    };
    let values = crate::chrome::note_lines(&row.value, end.saturating_sub(column));
    (0..labels.len().max(values.len()))
        .map(|n| {
            let mut line = Vec::new();
            if let Some(l) = labels.get(n).filter(|l| !l.is_empty()) {
                line.push(piece(left, l.clone(), label_colour));
            }
            if let Some(v) = values.get(n) {
                line.push(piece(column, v.clone(), value_colour));
            }
            line
        })
        .collect()
}

/// The detail's offset that keeps the cursor's row on screen — after a key,
/// never for data.
fn follow_rows(view: &mut View, hit: &Hit, area: Area, texts: &ronoctua_i18n::Texts) {
    let laid = Laid::of(view, hit, area, true, texts);
    let room = laid.room(area).max(1);
    let cursor = view.detail.cursor;
    let (Some(first), Some(last)) = (
        laid.lines.iter().position(|(k, _)| *k == cursor),
        laid.lines.iter().rposition(|(k, _)| *k == cursor),
    ) else {
        return;
    };
    let at = laid.first(area, view.detail.offset);
    view.detail.offset = if first < at {
        first
    } else if last >= at + room {
        (last + 1).saturating_sub(room).min(first)
    } else {
        at
    };
}

fn draw_rows(
    view: &View,
    hit: &Hit,
    area: Area,
    focused: bool,
    lit: &[(tokens::Color, Target)],
    texts: &ronoctua_i18n::Texts,
    out: &mut Drawing,
) {
    let mut top = area.top;
    if let What::Video(v) = &hit.what
        && let Some(key) = &v.thumb
        && let Some(rgba) = view.pictures.get(key)
    {
        let lines = tokens::space::FASTACCESS_GRIDPICTURELINES.min(area.lines / 2);
        let space = ronoctua_render::image::Area {
            x: Area::x_of(area.left),
            y: Area::y_of(top),
            width: f32::from(area.columns) * Area::cell(),
            height: f32::from(lines) * Area::line(),
        };
        out.pictures.push(Pic {
            key: key.clone(),
            area: crate::files::picture_in(space, rgba.width, rgba.height),
            source: Source::Still(Arc::clone(rgba)),
        });
        top += lines;
    }
    let laid = Laid::of(view, hit, area, focused, texts);
    let rows = rows_of(view, hit, texts);
    let room = laid.room(area);
    let scroll = crate::chrome::list_scroll(laid.lines.len(), room, view.detail.offset);
    let offset = laid.first(area, view.detail.offset);
    let width = laid.width;
    for (n, (k, fields)) in laid.lines.iter().enumerate().skip(offset).take(room) {
        let at = top + u16::try_from(n - offset).unwrap_or(0);
        if rows.get(*k).is_some_and(Row::acts)
            && let Some((colour, _)) = lit.iter().find(|(_, t)| *t == Target::Row(*k))
        {
            out.under.push(ronoctua_render::quad::Quad {
                x: Area::x_of(area.left.saturating_sub(crate::chrome::FIELDS_LEAD)),
                y: Area::y_of(at),
                width: f32::from(width + crate::chrome::FIELDS_LEAD) * Area::cell(),
                height: Area::line(),
                color: *colour,
            });
        }
        for f in fields {
            out.spans.push(ronoctua_render::Span {
                text: f.text.clone(),
                column: f.at,
                line: at,
                color: f.color,
                size: ronoctua_render::Size::Px(tokens::text::size()),
                edge: false,
                foot: false,
            });
        }
    }
    if scroll.cut() {
        let (track, thumb) = crate::chrome::band_bar(
            ronoctua_design::Scroll { offset, ..scroll },
            area.right_px,
            Area::y_of(top),
            area.bottom_px,
            Area::cell(),
            Area::line(),
        );
        out.tracks.push(crate::bars::Track::of(track));
        if let Some(t) = thumb {
            out.over.push(crate::chrome::thumb(t));
        }
    }
}

/// What is under `(x, y)` of the detail `area`, by the layout its drawing
/// used.
#[must_use]
pub fn target_at(
    view: &View,
    area: Area,
    x: f32,
    y: f32,
    texts: &ronoctua_i18n::Texts,
) -> Option<Target> {
    let hit = view.current()?;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let line = ((y - Area::y_of(area.top)) / Area::line()).max(0.0) as u16;
    if let Some(c) = &view.detail.confirm {
        let title =
            u16::try_from(crate::chrome::note_lines(&c.title, area.columns).len()).unwrap_or(0);
        return line
            .checked_sub(title)
            .map(usize::from)
            .filter(|n| *n < c.answers.len())
            .map(Target::Answer);
    }
    match &hit.what {
        What::Icon(_) => {
            let (page, set) = (view.detail.icon.as_ref()?, view.icons.as_deref()?);
            let (_, _, lines) = page.list(area, set, &view.detail.icon_settings, texts);
            page.hit(area, &lines, x, y).map(Target::Icon)
        }
        What::File { .. } => {
            // A BAR FIRST: drawn over the panel's cells, it is a control and
            // not a line under it — the strips the drawing drew (`bars_of`).
            let p = view.detail.panel.as_ref()?;
            let focused = view.focus == crate::fast_access::Focus::Detail;
            if let Some(bar) = file_bars(p, area, focused, texts)
                .into_iter()
                .find(|b| b.holds(x, y))
            {
                return Some(Target::FileBar(bar.pane, bar.axis));
            }
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let column = (x / Area::cell()).max(0.0) as u16;
            Some(Target::Cell(line, column))
        }
        _ => {
            let laid = Laid::of(view, hit, area, true, texts);
            let rows = rows_of(view, hit, texts);
            let room = laid.room(area);
            let scroll = crate::chrome::list_scroll(laid.lines.len(), room, view.detail.offset);
            if scroll.cut()
                && x >= area.right_px - f32::from(crate::chrome::BAR_CELLS) * Area::cell()
            {
                return Some(Target::Bar);
            }
            let n =
                usize::from(line.checked_sub(laid.picture)?) + laid.first(area, view.detail.offset);
            let (k, _) = laid.lines.get(n)?;
            rows.get(*k).filter(|r| r.acts()).map(|_| Target::Row(*k))
        }
    }
}

/// A row's press: what it does, as errands.
fn act_of(view: &mut View, hit: &Hit, act: &Act, texts: &ronoctua_i18n::Texts) -> Vec<Errand> {
    let _ = texts;
    match act {
        Act::Nothing => Vec::new(),
        Act::Copy(t) => vec![Errand::Copy(t.clone())],
        Act::Player(url) => {
            let mut out = vec![Errand::Keep(view.remember(hit))];
            out.extend([Errand::Player(url.clone()), Errand::Done]);
            out
        }
        Act::Kiosk(url) => {
            let mut out = vec![Errand::Keep(view.remember(hit))];
            out.extend([Errand::Kiosk(url.clone()), Errand::Done]);
            out
        }
        Act::Enter | Act::Narrow | Act::Secondary => Vec::new(),
    }
}

/// A program's rows, from its details: what it is, its version, licence and
/// site, the command it runs, what it weighs, and its badges — each a
/// sentence of the programs' centre (`software_words::badge_key`).
fn program_rows(
    d: &ronoctua_software::details::Details,
    texts: &ronoctua_i18n::Texts,
    out: &mut Vec<Row>,
) {
    let t = |k: &str| texts.t(k);
    if !d.name.is_empty() {
        out.push(Row::fact(t("fast-access-detail-name"), d.name.clone()));
    }
    if !d.summary.is_empty() {
        out.push(Row::fact(
            t("fast-access-detail-description"),
            d.summary.clone(),
        ));
    }
    for p in d.description.iter().filter(|p| **p != d.summary) {
        out.push(Row::fact(String::new(), p.clone()));
    }
    if !d.version.is_empty() {
        out.push(Row::fact(
            t("fast-access-detail-version"),
            d.version.clone(),
        ));
    }
    if let Some(l) = &d.license {
        out.push(Row::fact(t("fast-access-detail-licence"), l.clone()));
    }
    if let Some(h) = &d.homepage {
        out.push(Row::fact(t("fast-access-detail-site"), h.clone()));
    }
    if let Some(c) = &d.program {
        out.push(Row::fact(t("fast-access-detail-runs"), c.clone()));
    }
    if let Some(b) = d.download_bytes {
        out.push(Row::fact(
            t("fast-access-detail-download"),
            crate::sizes::readable_bytes(b, texts.decimal_mark()),
        ));
    }
    for b in &d.badges {
        out.push(Row::fact(
            t(crate::software_words::badge_key(*b)),
            String::new(),
        ));
    }
}

/// What the detail's cursor row does, when the detail has rows.
#[must_use]
pub fn row_act(view: &View, hit: &Hit, texts: &ronoctua_i18n::Texts) -> Option<Act> {
    if view.detail.confirm.is_some() || matches!(hit.what, What::Icon(_) | What::File { .. }) {
        return None;
    }
    rows_of(view, hit, texts)
        .get(view.detail.cursor)
        .map(|r| r.act.clone())
}

/// A key in the detail.
pub fn key(
    view: &mut View,
    hit: Option<&Hit>,
    press: &Press,
    area: Area,
    texts: &ronoctua_i18n::Texts,
) -> Vec<Errand> {
    let Some(hit) = hit else {
        return Vec::new();
    };
    // THE QUESTION about the file takes the keys while it is up.
    if let Some(c) = view.detail.confirm.as_mut() {
        match press {
            Press::Up | Press::Left => c.cursor = c.cursor.saturating_sub(1),
            Press::Down | Press::Right => {
                c.cursor = (c.cursor + 1).min(c.answers.len().saturating_sub(1))
            }
            Press::Escape => view.detail.confirm = None,
            Press::Enter => {
                let does = c.does.get(c.cursor).cloned().flatten();
                view.detail.confirm = None;
                return does.map(|a| vec![Errand::Amend(a)]).unwrap_or_default();
            }
            _ => {}
        }
        return Vec::new();
    }
    match &hit.what {
        What::Icon(_) => icon_key(view, press, area, texts),
        What::File { .. } => file_key(view, press, texts),
        _ => {
            let rows = rows_of(view, hit, texts);
            let acting: Vec<usize> = (0..rows.len()).filter(|i| rows[*i].acts()).collect();
            let at = acting
                .iter()
                .position(|i| *i == view.detail.cursor)
                .unwrap_or(0);
            match press {
                Press::Up => {
                    if let Some(i) = acting.get(at.saturating_sub(1)) {
                        view.detail.cursor = *i;
                    }
                    follow_rows(view, hit, area, texts);
                }
                Press::Down => {
                    if let Some(i) = acting.get((at + 1).min(acting.len().saturating_sub(1))) {
                        view.detail.cursor = *i;
                    }
                    follow_rows(view, hit, area, texts);
                }
                Press::Copy => {
                    return crate::fast_access::copy_of(hit, texts)
                        .map(|t| vec![Errand::Copy(t)])
                        .unwrap_or_default();
                }
                Press::Enter => {
                    // WHAT `RET` ON THE RESULTS DOES is the view's (it holds
                    // what opening reaches): `row_act` tells it so first.
                    let Some(row) = rows.get(view.detail.cursor) else {
                        return Vec::new();
                    };
                    return act_of(view, hit, &row.act.clone(), texts);
                }
                _ => {}
            }
            Vec::new()
        }
    }
}

/// A key on the icon's page.
fn icon_key(
    view: &mut View,
    press: &Press,
    area: Area,
    texts: &ronoctua_i18n::Texts,
) -> Vec<Errand> {
    use crate::icons_page::Press as P;
    let p = match press {
        Press::Up => P::Up,
        Press::Down => P::Down,
        Press::Left => P::Less,
        Press::Right => P::More,
        Press::Enter => P::Enter,
        Press::Copy => P::Enter,
        _ => return Vec::new(),
    };
    let (Some(page), Some(set)) = (view.detail.icon.as_mut(), view.icons.clone()) else {
        return Vec::new();
    };
    let errand = page.key(p, &set, &mut view.detail.icon_settings, texts);
    page.follow(area, &set, &view.detail.icon_settings, texts);
    match errand {
        crate::icons_page::Errand::Copy(t) => vec![Errand::Copy(t)],
        // ITS OWN ID, in the icons app's words: the footer hands an answer
        // under `icons-colour` to the icons app, open or not.
        crate::icons_page::Errand::AskColour(initial) => {
            let mut line = crate::line::Line::new(ASK_COLOUR, false, Some(&initial));
            line.asks = Some("ask-icons-colour");
            vec![Errand::Ask(line)]
        }
        // THE NEXT ICON of the strip the page is about.
        // THE NEXT ICON OF ITS STRIP: the cursor walks the strip, and the
        // page follows it onto the same kind of row (`follow_hit`).
        crate::icons_page::Errand::Neighbour(by) => {
            let press = if by > 0 { Press::Right } else { Press::Left };
            crate::fast_access_rows::moved(view, &press, area, texts);
            view.focus = crate::fast_access::Focus::Detail;
            Vec::new()
        }
        crate::icons_page::Errand::Nothing => Vec::new(),
    }
}

/// A key on a file's panels: the facts walked, and the one under the cursor
/// changed.
fn file_key(view: &mut View, press: &Press, texts: &ronoctua_i18n::Texts) -> Vec<Errand> {
    let Some(p) = view.detail.panel.as_mut() else {
        return Vec::new();
    };
    let total = p.panel.sheet().len();
    match press {
        Press::Up => {
            let _ = p.panel.walk_panel(-1, total, false);
        }
        Press::Down => {
            let _ = p.panel.walk_panel(1, total, false);
        }
        Press::PageDown => {
            let room = p.panel.info_room().max(1);
            let _ = p.panel.wheel(
                crate::files::Panel::Info,
                i64::try_from(room).unwrap_or(1),
                room,
            );
        }
        Press::PageUp => {
            let room = p.panel.info_room().max(1);
            let _ = p.panel.wheel(
                crate::files::Panel::Info,
                -i64::try_from(room).unwrap_or(1),
                room,
            );
        }
        Press::Copy => return vec![Errand::Copy(p.path.display().to_string())],
        Press::Enter => {
            let sheet = p.panel.sheet();
            let Some(fact) = sheet.get(p.panel.info_cursor) else {
                return Vec::new();
            };
            let path = p.path.clone();
            return amend_of(view, fact.does, &path, texts);
        }
        _ => {}
    }
    Vec::new()
}

/// What pressing a fact that `does` something asks.
fn amend_of(
    view: &mut View,
    does: crate::inspection_sheet::Doable,
    path: &std::path::Path,
    texts: &ronoctua_i18n::Texts,
) -> Vec<Errand> {
    use crate::inspection_sheet::Doable as D;
    let path = path.to_path_buf();
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ask = |view: &mut View, amend: Amend, id: &'static str, now: Option<String>| {
        view.detail.asking = Some(amend);
        let mut line = crate::line::Line::new(id, false, now.as_deref());
        // THE FILE MANAGER'S OWN QUESTIONS, where they need no value: the same
        // change is asked in the same words in both apps. A tag's and a
        // detail's name a field, which a static key cannot carry — theirs are
        // `ask-<id>`.
        line.asks = match id {
            ASK_NAME => Some("files-ask-name"),
            ASK_MODIFIED => Some("files-ask-modified"),
            _ => None,
        };
        vec![Errand::Ask(line)]
    };
    match does {
        D::Nothing => Vec::new(),
        D::Rename => ask(view, Amend::Rename(path), ASK_NAME, Some(name)),
        D::Modified => {
            let now = std::fs::symlink_metadata(&path)
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .and_then(|d| i64::try_from(d.as_secs()).ok())
                .map(|s| {
                    let zone = view
                        .detail
                        .zone
                        .clone()
                        .unwrap_or_else(ronoctua_files::when::Zone::utc);
                    ronoctua_files::when::typed(s, &zone)
                });
            ask(view, Amend::Modified(path), ASK_MODIFIED, now)
        }
        D::Tag(field) => {
            let now = view
                .detail
                .panel
                .as_ref()
                .and_then(|p| amendable_of(p))
                .and_then(|a| crate::inspection_edit::tag_now(a, field))
                .map(|(v, _)| v);
            ask(view, Amend::Tag(path, field), ASK_TAG, now)
        }
        D::Meta(meta) => {
            let now = view
                .detail
                .panel
                .as_ref()
                .and_then(|p| amendable_of(p))
                .and_then(|a| crate::inspection_edit::detail_now(a, meta))
                .map(|(v, _)| v);
            ask(view, Amend::Meta(path, meta), ASK_META, now)
        }
        D::RemoveLocation => {
            view.detail.confirm = Some(Confirm {
                title: texts.t_args(
                    ronoctua_i18n::key::FILES_ASK_LOCATION,
                    &[
                        ("n", ronoctua_i18n::Arg::Number(1)),
                        ("name", ronoctua_i18n::Arg::Text(&name)),
                    ],
                ),
                answers: vec![
                    texts.t(ronoctua_i18n::key::FILES_NO),
                    texts.t(ronoctua_i18n::key::FILES_LOCATION_COPY),
                    texts.t(ronoctua_i18n::key::FILES_LOCATION_OVERWRITE),
                ],
                does: vec![
                    None,
                    Some(Amend::RemoveLocation(path.clone(), true)),
                    Some(Amend::RemoveLocation(path, false)),
                ],
                cursor: 0,
            });
            Vec::new()
        }
        D::Ownership => vec![Errand::Amend(Amend::Owner(path))],
        D::Measure => vec![Errand::Amend(Amend::Measure(path))],
        D::Mode => Vec::new(),
    }
}

/// What the panel read about its file that can be changed — only when the
/// panel's answer IS about that file.
pub(crate) fn amendable_of(p: &FilePanel) -> Option<&crate::inspection_edit::Amendable> {
    p.panel
        .looked
        .as_ref()
        .filter(|l| l.path == p.path)
        .map(|l| &l.amendable)
}

/// The question lines' ids: what the answer comes back under.
pub const ASK_NAME: &str = "fast-access-rename";
/// See [`ASK_NAME`].
pub const ASK_MODIFIED: &str = "fast-access-modified";
/// See [`ASK_NAME`].
pub const ASK_TAG: &str = "fast-access-tag";
/// See [`ASK_NAME`].
pub const ASK_META: &str = "fast-access-meta";
/// The Lucide figure's colour, asked from an icon's page.
pub const ASK_COLOUR: &str = "fast-access-colour";

/// A press in the detail.
pub fn pressed(
    view: &mut View,
    hit: Option<&Hit>,
    lay: &Lay,
    target: Target,
    (x, y): (f32, f32),
    texts: &ronoctua_i18n::Texts,
) -> Vec<Errand> {
    let Some(hit) = hit else {
        return Vec::new();
    };
    let area = lay.detail.unwrap_or(lay.results);
    match target {
        Target::Answer(n) => {
            let Some(c) = view.detail.confirm.as_mut() else {
                return Vec::new();
            };
            if c.cursor != n {
                c.cursor = n;
                return Vec::new();
            }
            return key(view, Some(hit), &Press::Enter, area, texts);
        }
        Target::Row(n) => {
            if view.detail.cursor == n {
                return key(view, Some(hit), &Press::Enter, area, texts);
            }
            view.detail.cursor = n;
        }
        Target::Bar => {
            let laid = Laid::of(view, hit, area, true, texts);
            let lines = laid.lines.len();
            let top = Area::y_of(area.top + laid.picture);
            let span = (area.bottom_px - top).max(1.0);
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                clippy::cast_precision_loss
            )]
            let at = (((y - top) / span).clamp(0.0, 1.0) * lines as f32) as usize;
            view.detail.offset = at.min(lines.saturating_sub(laid.room(area)));
            view.grabbed = Some(crate::fast_access_rows::Grab::Detail);
        }
        Target::Icon(t) => {
            let (Some(page), Some(set)) = (view.detail.icon.as_mut(), view.icons.clone()) else {
                return Vec::new();
            };
            if let crate::icons_page::Target::Row(n) = t {
                let errand = page.clicked(n, &set, &mut view.detail.icon_settings, texts);
                if let crate::icons_page::Errand::Copy(text) = errand {
                    return vec![Errand::Copy(text)];
                }
            }
        }
        Target::Cell(line, column) => return file_pressed(view, area, line, column, texts),
        // A FILE'S BAR, held: the panel goes where the pointer is, and on
        // with the drag (`fast_access_rows::held`).
        Target::FileBar(pane, axis) => {
            view.grabbed = Some(crate::fast_access_rows::Grab::File(pane, axis));
            file_dragged(view, area, (pane, axis), (x, y), texts);
        }
    }
    Vec::new()
}

/// A press on a cell of a file's panels: a fact's change, a permission's
/// bit, the film's transport — through the panel's own code where it has
/// one.
fn file_pressed(
    view: &mut View,
    area: Area,
    line: u16,
    column: u16,
    texts: &ronoctua_i18n::Texts,
) -> Vec<Errand> {
    let Some(p) = view.detail.panel.as_ref() else {
        return Vec::new();
    };
    let (preview, facts, pre, info) = file_sections(p, area, true, texts);
    let at = area.top + line;
    let found = [(&preview, pre.top), (&facts, info.top)]
        .into_iter()
        .find_map(|(s, top)| {
            let i = usize::from(at.checked_sub(top)?);
            s.content
                .get(i)?
                .fields
                .iter()
                .rev()
                .find(|f| f.covers(column))
                .map(|f| f.hit.clone())
        });
    let Some(hit) = found else {
        return Vec::new();
    };
    let path = p.path.clone();
    match hit {
        crate::files::Hit::Amend(_, does) => amend_of(view, does, &path, texts),
        crate::files::Hit::Bit(_, who, what) => {
            let Ok(meta) = std::fs::symlink_metadata(&path) else {
                return Vec::new();
            };
            let mode = std::os::unix::fs::PermissionsExt::mode(&meta.permissions());
            let wanted = ronoctua_files::mode::toggled(mode, who, what);
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            view.detail.confirm = Some(Confirm {
                title: texts.t_with(
                    ronoctua_i18n::key::FILES_CONFIRM_MODE,
                    &[
                        ("what", name.as_str()),
                        ("mode", &format!("{:04o}", wanted & 0o7777)),
                    ],
                ),
                answers: vec![
                    texts.t(ronoctua_i18n::key::FILES_NO),
                    texts.t(ronoctua_i18n::key::FILES_YES),
                ],
                does: vec![None, Some(Amend::Mode(path, wanted))],
                cursor: 0,
            });
            Vec::new()
        }
        crate::files::Hit::Owner(_) => vec![Errand::Amend(Amend::Owner(path))],
        other => {
            if let Some(p) = view.detail.panel.as_mut() {
                let _ = p.panel.click(&other);
            }
            Vec::new()
        }
    }
}

/// The wheel at `y` over the detail: its rows, the icon's list — or the one
/// of a file's panels under the pointer, the preview's text or the facts.
pub fn wheeled(view: &mut View, by: i64, area: Area, y: f32, texts: &ronoctua_i18n::Texts) {
    let Some(hit) = view.current().cloned() else {
        return;
    };
    match &hit.what {
        What::File { .. } => {
            let focused = view.focus == crate::fast_access::Focus::Detail;
            if let Some(p) = view.detail.panel.as_mut() {
                let (preview, _, _, _) = file_sections(p, area, focused, texts);
                let _ = match pane_at(area, y) {
                    crate::files::Pane::Info => {
                        let room = p.panel.info_room().max(1);
                        p.panel.wheel(crate::files::Panel::Info, by, room)
                    }
                    _ => p
                        .panel
                        .wheel(crate::files::Panel::Preview, by, preview.visible().max(1)),
                };
            }
        }
        What::Icon(_) => {
            if let (Some(page), Some(set)) = (view.detail.icon.as_mut(), view.icons.clone()) {
                let lines = page.rows(&set, &view.detail.icon_settings, texts).len();
                page.wheeled(by, lines, 1);
            }
        }
        _ => {
            let laid = Laid::of(view, &hit, area, true, texts);
            let last = laid.lines.len().saturating_sub(laid.room(area));
            let at = laid.first(area, view.detail.offset);
            let now = i64::try_from(at).unwrap_or(0).saturating_add(by).max(0);
            view.detail.offset = usize::try_from(now).unwrap_or(0).min(last);
        }
    }
}

/// The question line answered: the change it was asked for, with what was
/// typed.
#[must_use]
pub fn answered(view: &mut View, id: &str, text: &str) -> Option<(Amend, String)> {
    if ![ASK_NAME, ASK_MODIFIED, ASK_TAG, ASK_META].contains(&id) {
        return None;
    }
    view.detail.asking.take().map(|a| (a, text.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: a bar a file's detail draws is not a control where it is
    /// drawn — a press on it takes nothing, a hold does not follow — or the
    /// wheel over the preview scrolls the facts instead of the preview
    /// (`a-bar-is-grabbed-where-it-is-drawn`, invariant 19).
    #[test]
    fn a_files_bars_are_taken_where_they_are_drawn() {
        use crate::fast_access::{Filled, Section, SectionId, Settings};
        use crate::fast_access_rows::Target as On;
        use crate::files::{Axis, Pane};
        let dir =
            std::env::temp_dir().join(format!("ronoctua-fast-access-bars-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a folder");
        let file = dir.join("long.txt");
        let body: String = (0..400)
            .map(|n| format!("line {n} {}\n", "word ".repeat(60)))
            .collect();
        std::fs::write(&file, body).expect("a file");
        let texts = std::rc::Rc::new(ronoctua_i18n::Texts::fresh("en-GB").expect("the language"));
        let zone = ronoctua_files::when::Zone::utc();
        let mut view = View::new(Settings::default(), &texts);
        view.set_text("long");
        view.with_sections(vec![Section {
            id: SectionId::Files,
            filled: Filled::Hits(vec![Hit {
                id: format!("file:{}", file.display()),
                title: "long.txt".to_owned(),
                note: None,
                what: What::File {
                    path: file,
                    folder: false,
                    content: false,
                    mime: Some("text/plain".to_owned()),
                    size_bytes: None,
                    mtime_s: None,
                },
            }]),
        }]);
        let whole = Area {
            left: 1,
            top: 2,
            columns: 150,
            lines: 44,
            right_px: Area::x_of(151),
            bottom_px: Area::y_of(46),
        };
        let frame = |view: &View| {
            let mut d = Drawing::default();
            crate::fast_access_rows::draw(view, whole, true, &[], &texts, &mut d);
            d
        };
        // A FRAME, THE PANEL'S QUESTION, ITS ANSWER — as a door's threads.
        view.settle_detail(&texts, &zone);
        let _ = frame(&view);
        let ask = view
            .detail
            .panel
            .as_mut()
            .and_then(|p| {
                p.panel
                    .wants_look(u32::from(tokens::space::FILES_PANELPIXELS))
            })
            .expect("the panel asks");
        let looked = crate::inspection::look(ask);
        let p = view.detail.panel.as_mut().expect("a panel");
        assert!(
            p.panel
                .heard(crate::inspection::Heard::Looked(Box::new(looked)))
        );
        let drawn = frame(&view);
        let lay = Lay::of(&view, whole, &texts);
        let on = |view: &View, x: f32, y: f32| lay.target_at(view, x, y, &texts);
        // EVERY TRACK DRAWN IS A BAR'S, taken where it is.
        let mut seen = Vec::new();
        for t in &drawn.tracks {
            match on(&view, t.x + t.width / 2.0, t.y + t.height / 2.0) {
                Some(On::Detail(Target::FileBar(pane, axis))) => seen.push((pane, axis)),
                Some(On::ResultsBar) => {}
                other => panic!("a track drawn at {t:?} takes {other:?}"),
            }
        }
        assert!(seen.contains(&(Pane::Preview, Axis::Down)), "{seen:?}");
        assert!(seen.contains(&(Pane::Preview, Axis::Across)), "{seen:?}");
        let down = drawn
            .tracks
            .iter()
            .find(|t| {
                on(&view, t.x + t.width / 2.0, t.y + t.height / 2.0)
                    == Some(On::Detail(Target::FileBar(Pane::Preview, Axis::Down)))
            })
            .copied()
            .expect("the preview's bar");
        let offset = |view: &View| {
            view.detail
                .panel
                .as_ref()
                .expect("a panel")
                .panel
                .preview_offset
        };
        // PRESSED AT ITS FOOT, the preview goes to its end; HELD to the top,
        // back.
        let g = crate::fast_access::Ground {
            catalogue: std::sync::Arc::default(),
            commands: Vec::new(),
            zone: zone.clone(),
            zoneinfo: None,
            queue_keys: String::new(),
        };
        let x = down.x + down.width / 2.0;
        let _ = crate::fast_access_rows::pressed(
            &mut view,
            On::Detail(Target::FileBar(Pane::Preview, Axis::Down)),
            whole,
            (x, down.y + down.height - 1.0),
            &g,
            &texts,
        );
        assert!(
            offset(&view) > 0,
            "a press on the preview's bar moved nothing"
        );
        assert!(crate::fast_access_rows::held(
            &mut view,
            whole,
            (x, down.y),
            &texts
        ));
        assert_eq!(offset(&view), 0, "the hold was not followed");
        view.grabbed = None;
        // THE WHEEL OVER THE PREVIEW scrolls the preview, the facts stay.
        let d = lay.detail.expect("a detail");
        let facts = view
            .detail
            .panel
            .as_ref()
            .expect("a panel")
            .panel
            .info_offset;
        crate::fast_access_rows::wheel_at(
            &mut view,
            whole,
            Area::x_of(d.left + 2),
            Area::y_of(d.top + 2),
            5,
            &texts,
        );
        assert!(
            offset(&view) > 0,
            "the wheel over the preview moved nothing"
        );
        assert_eq!(
            view.detail
                .panel
                .as_ref()
                .expect("a panel")
                .panel
                .info_offset,
            facts,
            "the wheel over the preview scrolled the facts"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: a long value is cut with an ellipsis instead of wrapped, a
    /// wrapped line runs past the row, or the label is drawn on more than its
    /// row's first line (`a-note-is-wrapped-and-not-cut`).
    #[test]
    fn a_value_is_wrapped_and_never_cut() {
        let row = Row::fact(
            "About it".to_owned(),
            "A long description of what this command does, which a narrow detail \
             cannot hold on one line and must not cut"
                .to_owned(),
        );
        let (left, width) = (3, 40);
        let lines = wrapped_pair(left, width, &row, false);
        assert!(lines.len() > 2, "{lines:?}");
        for (n, line) in lines.iter().enumerate() {
            for f in line {
                assert!(!f.text.contains('…'), "cut: {:?}", f.text);
                assert!(
                    f.at + ronoctua_render::layout::width(&f.text) <= left + width,
                    "past the row: {f:?}"
                );
            }
            let labelled = line.iter().any(|f| f.text == "About it");
            assert_eq!(labelled, n == 0, "line {n}: {line:?}");
        }
        let all: String = lines
            .iter()
            .flat_map(|l| {
                l.iter()
                    .filter(|f| f.text != "About it")
                    .map(|f| f.text.as_str())
            })
            .collect::<Vec<_>>()
            .join(" ");
        assert!(all.ends_with("must not cut"), "{all}");
        // A ROW WITH NO VALUE is its label, wrapped over the whole width.
        let sentence = Row::fact("x ".repeat(40), String::new());
        assert!(wrapped_pair(left, 20, &sentence, false).len() > 1);
        // AND A LABEL LONGER THAN ITS COLUMN WRAPS IN IT, never cut: every word
        // of it is drawn, left of the value.
        let long = Row::fact(
            "Keep the window ready when it closes".to_owned(),
            "No".to_owned(),
        );
        let lines = wrapped_pair(left, 30, &long, false);
        let label: Vec<&str> = lines
            .iter()
            .flat_map(|l| l.iter().filter(|f| f.at == left).map(|f| f.text.as_str()))
            .collect();
        assert!(label.len() > 1, "{lines:?}");
        assert_eq!(label.join(" "), "Keep the window ready when it closes");
        let value_at = lines[0]
            .iter()
            .find(|f| f.text == "No")
            .expect("the value")
            .at;
        for l in &lines {
            for f in l.iter().filter(|f| f.at == left) {
                assert!(!f.text.contains('…'), "cut: {:?}", f.text);
                assert!(
                    f.at + ronoctua_render::layout::width(&f.text) < value_at,
                    "the label ran into the value: {f:?}"
                );
            }
        }
    }
}
