//! The screen capture: ONE command, and the choice of what to keep is made on
//! top of the picture — `ronoctua-shell capture`.
//!
//! # Why the picture is taken FIRST
//!
//! Because what a capture is for is the screen as it was when the key was
//! pressed. The whole of the screen with the focus is read before anything is
//! drawn (`grim -o`), and the overlay shows THAT picture, frozen: a menu that
//! closes when the keyboard moves, a tooltip, a video — they are all still
//! there to be chosen. And the choice is cut out of the same pixels, so what
//! is kept is exactly what was under the rectangle, with no second read that
//! could land a frame later.
//!
//! It is also what makes the overlay simple where it could have been fragile:
//! it is opaque (the picture fills it), and the magnifier reads real pixels
//! instead of asking the compositor for them again.
//!
//! # The three captures, from one surface
//!
//! - DRAG: the rectangle swept.
//! - CLICK on a window: that window, borders and title included.
//! - `RET`, or a click where there is no window: the whole screen.
//!
//! `Esc` or the second button gives up and keeps nothing.
//!
//! # Why this is a layer-shell surface
//!
//! It is the one exception to invariant 14 the owner approved for this
//! (2026-09-26): an overlay that exists ONLY while the capture is being
//! chosen, and is gone the moment it is — before the file is even written.
//!
//! # The guides
//!
//! Two lines through the pointer, one across the whole screen and one down
//! it, so the corner a rectangle starts at can be put exactly where it
//! should — and the magnifier beside the pointer shows the pixels under it,
//! with the one it is on outlined.

use crate::door::{
    self as base, Canvas, Content, Hand, LEFT_BUTTON, RIGHT_BUTTON, Shell, Ui, Wiring,
};
use anyhow::{Context as _, Result};
use ronoctua_render::image::{Area, Rgba};
use ronoctua_tokens as tokens;
use smithay_client_toolkit::{
    seat::{
        keyboard::{KeyEvent, Keysym, Modifiers},
        pointer::{PointerEvent, PointerEventKind},
    },
    shell::{
        WaylandSurface as _,
        wlr_layer::{Anchor, KeyboardInteractivity, Layer},
    },
};
use wayland_client::protocol::wl_surface;

/// The overlay's `app_id`.
pub const APP_ID: &str = "br.ronoctua.capture";

/// The layer-shell namespace.
const NAMESPACE: &str = "ronoctua-capture";

/// The frozen picture's texture.
const STILL: &str = "capture:still";

/// The magnifier's texture — one key, rewritten as the pointer moves.
const LOUPE: &str = "capture:loupe";

/// How many pixels of the picture the magnifier shows on each side of the one
/// under the pointer (`space.capture.loupeReach`).
const LOUPE_REACH: u32 = tokens::space::CAPTURE_LOUPEREACH as u32;

/// How many times the magnifier enlarges a pixel (`space.capture.loupeZoom`).
const LOUPE_ZOOM: u32 = tokens::space::CAPTURE_LOUPEZOOM as u32;

/// How far a held button has to travel, in logical pixels, before the press
/// is a rectangle rather than a click on a window (`space.capture.dragFrom`).
const DRAG_FROM: f32 = tokens::space::CAPTURE_DRAGFROM as f32;

/// What was chosen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Choice {
    /// Nothing: `Esc`, or the second button.
    Nothing,
    /// The whole screen.
    Whole,
    /// A rectangle, in the surface's logical pixels: `(x, y, width, height)`.
    Region(f32, f32, f32, f32),
}

/// A rectangle, in the surface's logical pixels.
type Box4 = (f32, f32, f32, f32);

/// Takes the picture, lets somebody choose, and writes what they chose.
///
/// Prints `saved <path>` when a file was written — the footer reads that and
/// raises the notification — and nothing when the capture was given up.
///
/// # Errors
///
/// No compositor, no screen with the focus, `grim` missing or failing, or the
/// file not written. Each is printed as `failed <why>` for the footer too.
pub fn host_it() -> Result<()> {
    match choose_and_keep() {
        Ok(Some(path)) => {
            println!("saved {}", path.display());
            Ok(())
        }
        Ok(None) => Ok(()),
        Err(e) => {
            println!("failed {e:#}");
            Err(e)
        }
    }
}

/// The whole of it, `None` when nothing was kept.
fn choose_and_keep() -> Result<Option<std::path::PathBuf>> {
    let grim = std::env::var("RONOCTUA_GRIM")
        .ok()
        .filter(|p| !p.is_empty())
        .context("RONOCTUA_GRIM is not set: there is no program to read the screen with")?;
    let mut wm = crate::daemon::open_the_compositor().context("no compositor to ask")?;
    let out = wm
        .focused_output()
        .context("the compositor did not say which screen has the focus")?
        .context("no screen has the focus")?;
    let at = out.logical.context("the screen with the focus is off")?;
    let windows: Vec<Box4> = wm
        .windows()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|w| w.rect)
        .map(|r| {
            #[allow(clippy::cast_precision_loss)]
            (
                (r.x - at.x) as f32,
                (r.y - at.y) as f32,
                r.width as f32,
                r.height as f32,
            )
        })
        .collect();
    let still = read_the_screen(&grim, &out.name)?;

    let chosen = std::rc::Rc::new(std::cell::Cell::new(Choice::Nothing));
    let answer = std::rc::Rc::clone(&chosen);
    let for_the_door = still.clone();
    base::run(move |_| {
        Ok(Capture {
            still: for_the_door,
            windows,
            pointer: None,
            pressed: None,
            chosen: answer,
            loupe_at: None,
            loupe: false,
            hint_hidden: false,
            marks: Vec::new(),
            hint_lit: None,
        })
    })?;

    let region = match chosen.get() {
        Choice::Nothing => return Ok(None),
        Choice::Whole => None,
        Choice::Region(x, y, w, h) => Some((x, y, w, h)),
    };
    // THE SURFACE WAS THE SCREEN, in logical pixels — which is what a
    // rectangle chosen on it is measured in.
    #[allow(clippy::cast_precision_loss)]
    let size = (at.width as f32, at.height as f32);
    let picture = match region {
        None => still,
        Some(r) => crop(&still, physical(r, size, (still.width, still.height)))
            .context("the rectangle is empty")?,
    };
    let path = crate::surface::capture_path().context("there is no folder for captures")?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("could not make {}", parent.display()))?;
    }
    image::save_buffer(
        &path,
        &picture.pixels,
        picture.width,
        picture.height,
        image::ExtendedColorType::Rgba8,
    )
    .with_context(|| format!("could not write {}", path.display()))?;
    Ok(Some(path))
}

/// Reads the whole of one screen, as `grim` sees it, into a picture.
///
/// PPM and not PNG: the bytes go straight from `grim` into memory, and
/// compressing a 4K screen only to decompress it here would be most of the
/// wait between the key and the overlay.
fn read_the_screen(grim: &str, output: &str) -> Result<Rgba> {
    let got = std::process::Command::new(grim)
        .args(["-o", output, "-t", "ppm", "-"])
        .stderr(std::process::Stdio::inherit())
        .output()
        .with_context(|| format!("could not run {grim}"))?;
    anyhow::ensure!(got.status.success(), "grim exited with {}", got.status);
    ppm(&got.stdout).context("grim's picture could not be read")
}

/// A binary PPM (`P6`, eight bits) as RGBA.
fn ppm(bytes: &[u8]) -> Option<Rgba> {
    let mut fields: Vec<u32> = Vec::with_capacity(3);
    let mut at = 2;
    if bytes.get(..2)? != b"P6" {
        return None;
    }
    while fields.len() < 3 {
        // WHITESPACE AND COMMENTS between the fields, as the format allows.
        while bytes.get(at)?.is_ascii_whitespace() {
            at += 1;
        }
        if *bytes.get(at)? == b'#' {
            while *bytes.get(at)? != b'\n' {
                at += 1;
            }
            continue;
        }
        let start = at;
        while bytes.get(at)?.is_ascii_digit() {
            at += 1;
        }
        fields.push(std::str::from_utf8(&bytes[start..at]).ok()?.parse().ok()?);
    }
    // ONE whitespace byte ends the header, and the pixels begin.
    at += 1;
    let (width, height, most) = (fields[0], fields[1], fields[2]);
    if most != 255 {
        return None;
    }
    let rgb = bytes.get(at..at + (width as usize) * (height as usize) * 3)?;
    let mut pixels = Vec::with_capacity(rgb.len() / 3 * 4);
    for p in rgb.chunks_exact(3) {
        pixels.extend_from_slice(&[p[0], p[1], p[2], 255]);
    }
    Rgba::new(width, height, pixels)
}

/// A logical rectangle on a surface of `surface` logical pixels, as pixels of
/// a picture of `picture` pixels — clamped to it.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
fn physical(r: Box4, surface: (f32, f32), picture: (u32, u32)) -> (u32, u32, u32, u32) {
    let sx = picture.0 as f32 / surface.0.max(1.0);
    let sy = picture.1 as f32 / surface.1.max(1.0);
    let x0 = (r.0 * sx).round().clamp(0.0, picture.0 as f32) as u32;
    let y0 = (r.1 * sy).round().clamp(0.0, picture.1 as f32) as u32;
    let x1 = ((r.0 + r.2) * sx).round().clamp(0.0, picture.0 as f32) as u32;
    let y1 = ((r.1 + r.3) * sy).round().clamp(0.0, picture.1 as f32) as u32;
    (x0, y0, x1.saturating_sub(x0), y1.saturating_sub(y0))
}

/// The part of a picture inside a rectangle of its own pixels.
fn crop(still: &Rgba, (x, y, w, h): (u32, u32, u32, u32)) -> Option<Rgba> {
    let row = still.width as usize * 4;
    let mut pixels = Vec::with_capacity(w as usize * h as usize * 4);
    for line in y..y + h {
        let from = line as usize * row + x as usize * 4;
        pixels.extend_from_slice(still.pixels.get(from..from + w as usize * 4)?);
    }
    Rgba::new(w, h, pixels)
}

/// The pixels around `(cx, cy)`, each drawn `LOUPE_ZOOM` times its size —
/// what is off the picture is black.
fn loupe(still: &Rgba, cx: u32, cy: u32) -> Option<Rgba> {
    let side = 2 * LOUPE_REACH + 1;
    let out_side = side * LOUPE_ZOOM;
    let mut pixels = vec![0u8; (out_side * out_side * 4) as usize];
    for oy in 0..out_side {
        for ox in 0..out_side {
            let sx = i64::from(cx) + i64::from(ox / LOUPE_ZOOM) - i64::from(LOUPE_REACH);
            let sy = i64::from(cy) + i64::from(oy / LOUPE_ZOOM) - i64::from(LOUPE_REACH);
            let o = ((oy * out_side + ox) * 4) as usize;
            if sx < 0 || sy < 0 || sx >= i64::from(still.width) || sy >= i64::from(still.height) {
                pixels[o + 3] = 255;
                continue;
            }
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let i = ((sy as usize) * still.width as usize + sx as usize) * 4;
            pixels[o..o + 4].copy_from_slice(&still.pixels[i..i + 4]);
        }
    }
    Rgba::new(out_side, out_side, pixels)
}

/// Two corners as a rectangle, whichever way the hand went.
fn spanned(a: (f32, f32), b: (f32, f32)) -> Box4 {
    (
        a.0.min(b.0),
        a.1.min(b.1),
        (a.0 - b.0).abs(),
        (a.1 - b.1).abs(),
    )
}

/// The window under a point, the SMALLEST that holds it: a floating window
/// is inside the tiled one under it, and it is the one on top.
fn window_at(windows: &[Box4], p: (f32, f32)) -> Option<Box4> {
    windows
        .iter()
        .copied()
        .filter(|r| p.0 >= r.0 && p.1 >= r.1 && p.0 < r.0 + r.2 && p.1 < r.1 + r.3)
        .min_by(|a, b| (a.2 * a.3).total_cmp(&(b.2 * b.3)))
}

/// Everything OUTSIDE a rectangle, as the four bands that dim it.
fn around(
    r: Box4,
    width: f32,
    height: f32,
    color: tokens::Color,
) -> [ronoctua_render::quad::Quad; 4] {
    let q = |x: f32, y: f32, w: f32, h: f32| ronoctua_render::quad::Quad {
        x,
        y,
        width: w.max(0.0),
        height: h.max(0.0),
        color,
    };
    [
        q(0.0, 0.0, width, r.1),
        q(0.0, r.1 + r.3, width, height - r.1 - r.3),
        q(0.0, r.1, r.0, r.3),
        q(r.0 + r.2, r.1, width - r.0 - r.2, r.3),
    ]
}

/// A rectangle's outline, `thick` logical pixels wide.
fn outline(r: Box4, thick: f32, color: tokens::Color) -> [ronoctua_render::quad::Quad; 4] {
    let q = |x: f32, y: f32, w: f32, h: f32| ronoctua_render::quad::Quad {
        x,
        y,
        width: w,
        height: h,
        color,
    };
    [
        q(r.0, r.1, r.2, thick),
        q(r.0, r.1 + r.3 - thick, r.2, thick),
        q(r.0, r.1, thick, r.3),
        q(r.0 + r.2 - thick, r.1, thick, r.3),
    ]
}

/// The overlay.
struct Capture {
    /// The screen, as it was when the key was pressed.
    still: Rgba,
    /// Every window that can be seen, in the surface's logical pixels.
    windows: Vec<Box4>,
    /// Where the pointer is.
    pointer: Option<(f32, f32)>,
    /// Where the button went down, while it is down.
    pressed: Option<(f32, f32)>,
    /// What was chosen — read by `host_it` after the overlay is gone.
    chosen: std::rc::Rc<std::cell::Cell<Choice>>,
    /// The picture pixel the magnifier last showed.
    loupe_at: Option<(u32, u32)>,
    /// Is the magnifier on? Off until `z` asks for it.
    loupe: bool,
    /// Has the shortcut line been put away (`h`)?
    hint_hidden: bool,
    /// Where each shortcut of that line landed, for the pointer — from what
    /// was DRAWN, on whichever line it was drawn.
    marks: Vec<crate::chrome::Mark>,
    /// The shortcut under the pointer, lit.
    hint_lit: Option<&'static str>,
}

/// The keys the overlay teaches, and what their words are.
const OF_THE_CAPTURE: &[(&str, &str)] = &[
    ("RET", "capture-whole"),
    ("z", "capture-loupe"),
    ("h", "capture-hide-hint"),
    ("ESC", "capture-cancel"),
];

impl Capture {
    /// The shortcut drawn under `(x, y)`, when the line is up.
    fn mark_at(&self, (x, y): (f64, f64)) -> Option<crate::chrome::Mark> {
        let (line, cell) = crate::text_select::Selecting::cell_of(x, y)?;
        self.marks.iter().find(|m| m.covers(line, cell)).cloned()
    }

    /// Ends the overlay with a choice.
    fn choose(&self, ui: &mut Ui, what: Choice) {
        self.chosen.set(what);
        ui.quit = true;
    }

    /// The pointer as a pixel of the picture.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn picture_pixel(&self, ui: &Ui, p: (f32, f32)) -> (u32, u32) {
        let (w, h) = surface_of(ui);
        let x = (p.0 * self.still.width as f32 / w.max(1.0)).max(0.0) as u32;
        let y = (p.1 * self.still.height as f32 / h.max(1.0)).max(0.0) as u32;
        (x.min(self.still.width - 1), y.min(self.still.height - 1))
    }

    /// The rectangle being swept, when the hand has gone far enough for it
    /// to be one.
    fn sweep(&self) -> Option<Box4> {
        let (a, b) = (self.pressed?, self.pointer?);
        ((a.0 - b.0).abs() >= DRAG_FROM || (a.1 - b.1).abs() >= DRAG_FROM).then(|| spanned(a, b))
    }
}

/// The surface's size in logical pixels.
#[allow(clippy::cast_precision_loss)]
fn surface_of(ui: &Ui) -> (f32, f32) {
    (ui.width as f32, ui.height as f32)
}

impl Content for Capture {
    type Hand = ();

    fn app_id(&self) -> &'static str {
        APP_ID
    }

    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        texts.t(ronoctua_i18n::key::CMD_SCREEN_CAPTURE)
    }

    /// THE WHOLE SCREEN, OVER EVERYTHING, and the keyboard with it: the
    /// overlay is the one thing a person can be doing while it is up.
    fn open(&self, wiring: &Wiring<'_, Self>, surface: wl_surface::WlSurface) -> Result<Shell> {
        let layer_shell = base::layer_shell(wiring)?;
        let layer = layer_shell.create_layer_surface(
            wiring.qh,
            surface,
            // OVER the footer and every window, and still under a lock.
            Layer::Overlay,
            Some(NAMESPACE),
            // THE SCREEN WITH THE FOCUS, which is the one that was read.
            None,
        );
        layer.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
        layer.set_size(0, 0);
        // OVER EVERY ZONE: the footer's reserved strip is part of the screen
        // that was read, and the picture has to lie exactly on it.
        layer.set_exclusive_zone(-1);
        layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        layer.commit();
        Ok(Shell::Layer(layer, layer_shell))
    }

    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, _: Modifiers) {
        match event.keysym {
            Keysym::Escape => self.choose(ui, Choice::Nothing),
            Keysym::Return | Keysym::KP_Enter => self.choose(ui, Choice::Whole),
            // THE MAGNIFIER and THE SHORTCUT LINE are switched, never held:
            // both are about what is in the way of the picture.
            Keysym::z | Keysym::Z => {
                self.loupe = !self.loupe;
                ui.ask_paint();
            }
            Keysym::h | Keysym::H => {
                self.hint_hidden = !self.hint_hidden;
                ui.ask_paint();
            }
            _ => {}
        }
    }

    /// NO SWEEP OF TEXT: a held button here is the rectangle.
    fn text_area(&self, _: &Ui, _: u16, _: u16) -> Option<crate::text_select::Area> {
        None
    }

    fn pointer(&mut self, ui: &mut Ui, _: &mut Hand<()>, events: &[PointerEvent]) {
        for e in events {
            #[allow(clippy::cast_possible_truncation)]
            let p = (e.position.0 as f32, e.position.1 as f32);
            match e.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    self.pointer = Some(p);
                    self.hint_lit = self.mark_at(e.position).map(|m| m.id);
                    ui.ask_paint();
                }
                PointerEventKind::Leave { .. } => {
                    self.pointer = None;
                    ui.ask_paint();
                }
                PointerEventKind::Press { button, .. } if button == LEFT_BUTTON => {
                    // A SHORTCUT OF THE LINE IS A CLICK, like every header's:
                    // it is pressed, and no rectangle starts under it.
                    if let Some(act) = self.mark_at(e.position).and_then(|m| m.press) {
                        if let crate::chrome::Act::Key(key) = act {
                            let (event, mods) = crate::door::keys::event_of(key);
                            self.typed(ui, &event, mods);
                        }
                        continue;
                    }
                    self.pressed = Some(p);
                    self.pointer = Some(p);
                }
                PointerEventKind::Press { button, .. } if button == RIGHT_BUTTON => {
                    self.choose(ui, Choice::Nothing);
                }
                PointerEventKind::Release { button, .. } if button == LEFT_BUTTON => {
                    if self.pressed.is_none() {
                        continue;
                    }
                    self.pointer = Some(p);
                    let choice = match self.sweep() {
                        Some(r) => Choice::Region(r.0, r.1, r.2, r.3),
                        // A CLICK: the window under it, or the whole screen
                        // where there is none.
                        None => window_at(&self.windows, p)
                            .map_or(Choice::Whole, |r| Choice::Region(r.0, r.1, r.2, r.3)),
                    };
                    self.pressed = None;
                    self.choose(ui, choice);
                }
                _ => {}
            }
        }
    }

    fn paint(&mut self, ui: &mut Ui, canvas: &mut Canvas<'_>) {
        let (w, h) = surface_of(ui);
        let f = ui.frame(0);
        // THE SCREEN AS IT WAS, filling the surface.
        let still = &self.still;
        if canvas.upload(STILL, || Some(still.clone())) {
            canvas.pictures.push((
                STILL.to_owned(),
                Area {
                    x: 0.0,
                    y: 0.0,
                    width: w,
                    height: h,
                },
            ));
        }

        // WHAT WOULD BE KEPT stays bright, and the rest is dimmed: the
        // rectangle being swept, or the window a click would take. With
        // neither — nothing under the pointer — a click keeps the whole
        // screen, and the whole screen stays bright.
        let kept = self
            .sweep()
            .or_else(|| self.pointer.and_then(|p| window_at(&self.windows, p)));
        let thick = f32::from(tokens::space::RULE_THICKNESS).max(1.0);
        if let Some(r) = kept {
            canvas
                .over
                .extend(around(r, w, h, tokens::color::CAPTURE_DIM));
            canvas
                .over
                .extend(outline(r, thick, tokens::color::CAPTURE_GUIDE));
        }

        // THE GUIDES, across the whole screen and down it, through the
        // pointer.
        if let Some(p) = self.pointer {
            canvas
                .over
                .extend(dotted(p.0.floor(), 0.0, h, false, thick));
            canvas.over.extend(dotted(0.0, p.1.floor(), w, true, thick));

            // THE MAGNIFIER, below and to the right of the pointer, and on
            // the other side of it near an edge — only when `z` asked for it.
            // Off, the numbers take its place beside the pointer.
            let at = self.picture_pixel(ui, p);
            #[allow(clippy::cast_precision_loss)]
            let side = if self.loupe {
                ((2 * LOUPE_REACH + 1) * LOUPE_ZOOM) as f32 / ui.scale.max(1) as f32
            } else {
                0.0
            };
            let gap = f.cell() * 2.0;
            let lx = if p.0 + gap + side > w {
                p.0 - gap - side
            } else {
                p.0 + gap
            };
            let ly = if p.1 + gap + side > h {
                p.1 - gap - side
            } else {
                p.1 + gap
            };
            if self.loupe {
                if self.loupe_at != Some(at)
                    && let Some(big) = loupe(&self.still, at.0, at.1)
                {
                    canvas.replace(LOUPE, &big);
                    self.loupe_at = Some(at);
                }
                canvas.pictures.push((
                    LOUPE.to_owned(),
                    Area {
                        x: lx,
                        y: ly,
                        width: side,
                        height: side,
                    },
                ));
                canvas.over.extend(outline(
                    (lx, ly, side, side),
                    thick,
                    tokens::color::CAPTURE_GUIDE,
                ));
                // AND THE PIXEL THE POINTER IS ON, outlined in the middle of it.
                #[allow(clippy::cast_precision_loss)]
                let one = side / (2 * LOUPE_REACH + 1) as f32;
                #[allow(clippy::cast_precision_loss)]
                let mid = LOUPE_REACH as f32 * one;
                canvas.over.extend(outline(
                    (lx + mid, ly + mid, one, one),
                    thick,
                    tokens::color::CAPTURE_GUIDE,
                ));
            }

            // THE NUMBERS: the size of the rectangle while one is swept, and
            // where the pointer is otherwise — in the picture's own pixels,
            // which is what the file will have.
            let said = match self.sweep() {
                Some(r) => {
                    let (_, _, pw, ph) = physical(r, (w, h), (self.still.width, self.still.height));
                    ui.texts.t_args(
                        ronoctua_i18n::key::CAPTURE_SIZE,
                        &[
                            ("width", ronoctua_i18n::Arg::Number(i64::from(pw))),
                            ("height", ronoctua_i18n::Arg::Number(i64::from(ph))),
                        ],
                    )
                }
                None => ui.texts.t_args(
                    ronoctua_i18n::key::CAPTURE_AT,
                    &[
                        ("x", ronoctua_i18n::Arg::Number(i64::from(at.0))),
                        ("y", ronoctua_i18n::Arg::Number(i64::from(at.1))),
                    ],
                ),
            };
            let lines = ui.lines.max(1);
            let columns = ui.columns + 2 * ui.margin;
            let wide = ronoctua_render::layout::width(&said);
            // UNDER THE MAGNIFIER, on the grid — text is laid in cells.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let (column, line) = (
                ((lx / f.cell()) as u16).min(columns.saturating_sub(wide + 1)),
                (((ly + side) / f.line()).ceil() as u16).min(lines - 1),
            );
            canvas.over.push(ronoctua_render::quad::Quad {
                x: f32::from(column) * f.cell(),
                y: f.y_of(line),
                width: f32::from(wide + 1) * f.cell(),
                height: f.line(),
                color: tokens::color::SURFACE_TRANSPORT,
            });
            canvas.spans.push(ronoctua_render::Span {
                size: ronoctua_render::Size::Grid,
                text: said,
                column,
                line,
                color: tokens::color::TEXT_PRIMARY,
                edge: false,
                foot: false,
            });
        }

        // THE LINE THAT SAYS HOW, on a band of its own so it reads over any
        // picture, and every key on it a click. It is ours and not the
        // door's header, because it MOVES: at the top, and at the bottom
        // while the pointer comes near the top (`space.capture.hintFlee`
        // lines), so it never covers the corner being chosen. `h` puts it
        // away wherever the pointer is.
        self.marks.clear();
        if !self.hint_hidden {
            let texts = ui.texts.clone();
            let mut pieces = vec![
                crate::chrome::Piece::prose(texts.t(ronoctua_i18n::key::CAPTURE_DRAG)),
                crate::chrome::Piece::separator(),
                crate::chrome::Piece::prose(texts.t(ronoctua_i18n::key::CAPTURE_CLICK)),
                crate::chrome::Piece::separator(),
            ];
            pieces.extend(crate::chrome::hint_of(OF_THE_CAPTURE, &texts));
            let pieces = crate::chrome::lit(&pieces, self.hint_lit);
            let flee = f.line() * f32::from(tokens::space::CAPTURE_HINTFLEE + 1);
            let near_the_top = self.pointer.is_some_and(|p| p.1 < flee);
            let line = if near_the_top {
                ui.lines.saturating_sub(1)
            } else {
                0
            };
            canvas.over.push(ronoctua_render::quad::Quad {
                x: 0.0,
                y: f.y_of(line),
                width: w,
                height: f.line(),
                color: tokens::color::SURFACE_TRANSPORT,
            });
            let room = ui.columns;
            canvas.spans.extend(crate::chrome::hint_spans(
                &pieces,
                ronoctua_render::Size::Grid,
                ui.margin,
                line,
                room,
            ));
            self.marks = crate::chrome::hint_marks(&pieces, ui.margin, line, room);
        }
    }
}

/// A guide line, DOTTED: `space.capture.dash` drawn, `space.capture.gap` not,
/// along `length` from `(x, y)`. A solid line one pixel wide is lost over a
/// photograph's own edges; a dotted one is not a thing a photograph has.
fn dotted(
    x: f32,
    y: f32,
    length: f32,
    across: bool,
    thick: f32,
) -> Vec<ronoctua_render::quad::Quad> {
    let dash = f32::from(tokens::space::CAPTURE_DASH).max(1.0);
    let step = dash + f32::from(tokens::space::CAPTURE_GAP);
    let mut out = Vec::new();
    let mut at = 0.0;
    while at < length {
        let run = dash.min(length - at);
        out.push(if across {
            ronoctua_render::quad::Quad {
                x: x + at,
                y,
                width: run,
                height: thick,
                color: tokens::color::CAPTURE_GUIDE,
            }
        } else {
            ronoctua_render::quad::Quad {
                x,
                y: y + at,
                width: thick,
                height: run,
                color: tokens::color::CAPTURE_GUIDE,
            }
        });
        at += step;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picture(w: u32, h: u32) -> Rgba {
        let mut px = Vec::new();
        for y in 0..h {
            for x in 0..w {
                px.extend_from_slice(&[u8::try_from(x).unwrap(), u8::try_from(y).unwrap(), 0, 255]);
            }
        }
        Rgba::new(w, h, px).unwrap()
    }

    #[test]
    fn a_ppm_reads_as_the_pixels_it_carries() {
        let mut bytes = b"P6\n# grim\n2 1\n255\n".to_vec();
        bytes.extend_from_slice(&[1, 2, 3, 4, 5, 6]);
        let p = ppm(&bytes).unwrap();
        assert_eq!((p.width, p.height), (2, 1));
        assert_eq!(p.pixels, vec![1, 2, 3, 255, 4, 5, 6, 255]);
        assert!(ppm(b"P5\n1 1\n255\n\0").is_none(), "only colour PPM");
        assert!(
            ppm(b"P6\n2 2\n255\n\0\0\0").is_none(),
            "a short picture is no picture"
        );
    }

    #[test]
    fn a_rectangle_is_cut_in_the_picture_s_own_pixels() {
        // A SCREEN AT SCALE TWO: the surface is half the picture's size, and
        // a rectangle chosen on it is twice as many pixels in the file.
        let still = picture(20, 10);
        let r = physical((2.0, 1.0, 3.0, 2.0), (10.0, 5.0), (20, 10));
        assert_eq!(r, (4, 2, 6, 4));
        let cut = crop(&still, r).unwrap();
        assert_eq!((cut.width, cut.height), (6, 4));
        assert_eq!(
            &cut.pixels[..4],
            &[4, 2, 0, 255],
            "the corner is the corner chosen"
        );
        // AND NOTHING OFF THE PICTURE: a sweep past the edge stops at it.
        let past = physical((8.0, 4.0, 10.0, 10.0), (10.0, 5.0), (20, 10));
        assert_eq!(past, (16, 8, 4, 2));
    }

    #[test]
    fn a_click_takes_the_smallest_window_under_it() {
        // A FLOATING WINDOW lies inside the tiled one under it, and it is the
        // one on top.
        let tiled = (0.0, 0.0, 100.0, 100.0);
        let floating = (10.0, 10.0, 20.0, 20.0);
        let windows = [tiled, floating];
        assert_eq!(window_at(&windows, (15.0, 15.0)), Some(floating));
        assert_eq!(window_at(&windows, (50.0, 50.0)), Some(tiled));
        assert_eq!(
            window_at(&windows, (150.0, 50.0)),
            None,
            "the desk is the whole screen"
        );
    }

    #[test]
    fn a_sweep_is_the_same_whichever_way_the_hand_went() {
        assert_eq!(spanned((10.0, 20.0), (4.0, 2.0)), (4.0, 2.0, 6.0, 18.0));
        assert_eq!(spanned((4.0, 2.0), (10.0, 20.0)), (4.0, 2.0, 6.0, 18.0));
    }

    #[test]
    fn the_magnifier_is_the_pixels_around_the_pointer() {
        let still = picture(40, 40);
        let big = loupe(&still, 20, 20).unwrap();
        let side = (2 * LOUPE_REACH + 1) * LOUPE_ZOOM;
        assert_eq!((big.width, big.height), (side, side));
        // THE MIDDLE is the pixel under the pointer.
        let mid = (LOUPE_REACH * LOUPE_ZOOM + LOUPE_ZOOM / 2) as usize;
        let i = (mid * side as usize + mid) * 4;
        assert_eq!(&big.pixels[i..i + 2], &[20, 20]);
        // AND OFF THE PICTURE IS BLACK, not a wrap to the other side.
        let corner = loupe(&still, 0, 0).unwrap();
        assert_eq!(&corner.pixels[..4], &[0, 0, 0, 255]);
    }

    #[test]
    fn a_guide_is_dotted_and_stays_on_its_line() {
        let dash = f32::from(tokens::space::CAPTURE_DASH);
        let gap = f32::from(tokens::space::CAPTURE_GAP);
        let across = dotted(0.0, 10.0, 100.0, true, 1.0);
        assert!(across.len() > 1, "one dash is a line, not dots");
        assert!(
            across
                .iter()
                .all(|q| (q.y - 10.0).abs() < f32::EPSILON && q.height == 1.0)
        );
        assert!((across[1].x - across[0].x - (dash + gap)).abs() < 0.01);
        assert!(
            across.iter().all(|q| q.x + q.width <= 100.0),
            "nothing past the end"
        );
        let down = dotted(7.0, 0.0, 50.0, false, 1.0);
        assert!(
            down.iter()
                .all(|q| (q.x - 7.0).abs() < f32::EPSILON && q.width == 1.0)
        );
        assert!(down.iter().all(|q| q.color == tokens::color::CAPTURE_GUIDE));
    }

    #[test]
    fn dimming_leaves_exactly_the_rectangle() {
        let bands = around(
            (10.0, 20.0, 30.0, 40.0),
            100.0,
            100.0,
            tokens::color::CAPTURE_DIM,
        );
        let area: f32 = bands.iter().map(|q| q.width * q.height).sum();
        assert!(
            (area - (100.0 * 100.0 - 30.0 * 40.0)).abs() < 0.01,
            "{area}"
        );
    }
}
