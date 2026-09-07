//! Draws a native buffer into a PNG, with no compositor and no VM.
//!
//! # Why this exists
//!
//! The owner's standing rule is that green is not proof: something that appears
//! on the screen is opened on the screen before anybody says it is done. Until
//! this binary, obeying that rule cost a NixOS VM — boot, niri under `Xvfb`,
//! screenshot the X root, crop it by the geometry niri reported, OCR it. Three
//! minutes, to see whether a rule was touching a title's descenders. A rule
//! that expensive is a rule that gets skipped, and the title that prompted this
//! was drawn at twice the body's size for months with every check green.
//!
//! # What makes the picture worth trusting
//!
//! That nothing here is a second implementation. The component is loaded the
//! way the host loads it, driven through the same `Input`s, and its
//! `DisplayList` goes through `paint` — the same module `buffer.rs` uses to put
//! a buffer on an `xdg_toplevel`. The clear colour, the margins, the cell, the
//! atlas and the shaper are the production ones. What is different is only
//! where the pixels land: a texture instead of a `wl_surface`.
//!
//! Two things it does NOT draw, and it says so rather than pretending:
//!
//!   - `Command::Svg`, which needs the rasterised pixmap cache the window keeps;
//!   - anything that depends on the host answering the buffer back — a disk
//!     rack, a transfer queue, a memory survey. Those buffers draw their empty
//!     screen here, which is the honest picture of what they have.
//!
//! Usage: `ronoctua-render-png <component.wasm> [options]`, `--help` for the list.

#[path = "../paint.rs"]
mod paint;

use anyhow::{Context as _, Result, bail};
use ronoctua_protocol::Input;
use ronoctua_tokens as tokens;

/// What was asked for on the command line.
struct Ask {
    wasm: String,
    out: std::path::PathBuf,
    columns: u16,
    lines: u16,
    language: String,
    /// The theme the picture is drawn in. The FIRST declared one unless asked
    /// — never the file or the environment: a photograph is deterministic, as
    /// `--language` already is.
    theme: tokens::theme::Theme,
    /// How many physical pixels one logical pixel is.
    ///
    /// ONE unless asked. A photograph has no screen to be on, so this is not
    /// read from anywhere — it is what makes the doors' HiDPI path something
    /// this repository can photograph at all, which the working rule ("open it
    /// on the screen before the word `ready`") needs and did not have.
    ///
    /// It multiplies the PIXELS and not the layout: the same picture, the same
    /// number of columns, more pixels in every glyph.
    ///
    /// AN INTEGER, because `wl_surface.set_buffer_scale` takes one and every
    /// door holds an `i32`. A fractional value here would photograph a state
    /// no door can be in — and it is arithmetically unsound besides: the
    /// texture's size is a truncating cast, so `ronoctua_render::text::across` would stop
    /// round-tripping to the column count the layout was resolved against,
    /// which is the one-cell error it exists to prevent.
    scale: u16,
    /// How big the text is, in percent of the design's base size.
    ///
    /// A hundred unless asked — never the person's `display.json`, for the
    /// reason `theme` gives just above: a photograph is deterministic. It is
    /// what makes a size something this repository can photograph at all,
    /// since a cell is the size times a constant and every measurement in the
    /// picture moves with it.
    text: u16,
    /// In order: everything to send the component before drawing.
    inputs: Vec<Input>,
}

const USAGE: &str = "\
usage: ronoctua-render-png <component.wasm> [options]

  --out <file>        where to write            (default: buffer.png)
  --columns <n>       width in cells           (default: 90)
  --lines <n>         height in cells          (default: 24)
  --language <id>     pt-BR or en-GB           (default: pt-BR)
  --theme <id>        a theme of assets/themes  (default: the first)
  --text <percent>    text size, in percent     (default: 100)
  --scale <n>         physical px per logical   (default: 1)

  --state <id>        enters a state before drawing
  --command <id>      fires a command of the buffer
  --key <name>        up, down, left, right, enter, esc, tab, backspace
  --type <text>       types, character by character

The last four are repeatable and apply IN THE ORDER they appear: that is how
one reaches a screen that only exists after walking to it.";

/// A count of cells a person typed, refused if the size it implies would wrap.
///
/// The picture's size is the count PLUS the inset twice, in `u16`. At 65535 the
/// sum wrapped to 3 in release and the tool photographed a three-cell picture
/// without a word (in debug it panics, which is the same bug with a louder
/// noise). A thousand cells is past the widest screen anybody sells and far
/// short of the wrap.
fn cells(flag: &str, raw: &str) -> Result<u16> {
    const MOST: u16 = 1000;
    let asked: u16 = raw
        .parse()
        .with_context(|| format!("`{flag} {raw}` is not a whole number"))?;
    anyhow::ensure!(
        (1..=MOST).contains(&asked),
        "`{flag} {asked}` is not a size a screen has; the range is 1..={MOST}",
    );
    Ok(asked)
}

fn parse() -> Result<Ask> {
    let mut args = std::env::args().skip(1);
    let first = args.next().context(USAGE)?;
    if first == "--help" || first == "-h" {
        println!("{USAGE}");
        std::process::exit(0);
    }
    let mut me = Ask {
        wasm: first,
        out: std::path::PathBuf::from("buffer.png"),
        columns: 90,
        lines: 24,
        language: "pt-BR".to_owned(),
        theme: tokens::theme::default_one(),
        text: tokens::text::NORMAL,
        scale: 1,
        inputs: Vec::new(),
    };
    while let Some(flag) = args.next() {
        // Each flag consumes its own argument, and a missing one is a mistake
        // worth naming: the alternative is a picture of a screen the caller did
        // not ask for. Taken here and not in a closure so the loop keeps the
        // iterator to itself.
        let mut value = || {
            args.next()
                .with_context(|| format!("`{flag}` with no value"))
        };
        match flag.as_str() {
            "--out" => me.out = std::path::PathBuf::from(value()?),
            // BOUNDED, BOTH OF THEM. The window's size is the count plus the
            // inset TWICE, in `u16`, so 65535 columns wrapped to 3 in release
            // and photographed a three-cell picture with no complaint at all.
            // A thousand is past any screen and short of the wrap.
            "--columns" => me.columns = cells("--columns", &value()?)?,
            "--lines" => me.lines = cells("--lines", &value()?)?,
            "--language" => me.language = value()?,
            "--theme" => {
                let id = value()?;
                me.theme = tokens::theme::by_id(&id).with_context(|| {
                    let known: Vec<&str> = tokens::theme::all().map(|t| t.id()).collect();
                    format!("unknown theme `{id}`; the themes are {}", known.join(", "))
                })?;
            }
            "--text" => {
                // THE RANGE IS NOT REPEATED HERE. It is three tokens
                // (`font.scale.*`), and a usage line spelling them out is the
                // one place nobody updates when they move — so the line says
                // what the option is and the refusal below says what it
                // accepts, from the tokens themselves.
                let raw = value()?;
                let asked: u16 = raw.parse().with_context(|| {
                    format!(
                        "`--text {raw}` is not a whole percentage between {} and {}",
                        tokens::font::SCALE_FLOOR,
                        tokens::font::SCALE_CEILING,
                    )
                })?;
                // REFUSED RATHER THAN SNAPPED. `tokens::text::allowed` would
                // quietly move 300 to 200, and a photograph taken at a size
                // nobody asked for is worse than one that did not happen.
                let held = tokens::text::allowed(asked);
                anyhow::ensure!(
                    held == asked,
                    "`--text {asked}` is not a size the person can reach; \
                     the nearest is {held} (the range is {}..{} in steps of {})",
                    tokens::font::SCALE_FLOOR,
                    tokens::font::SCALE_CEILING,
                    tokens::font::SCALE_STEP,
                );
                me.text = held;
            }
            "--scale" => {
                let raw = value()?;
                let asked: u16 = raw
                    .parse()
                    .with_context(|| format!("`--scale {raw}` is not a whole number"))?;
                // THE ONE PLACE THE BOUND IS WRITTEN is
                // `ronoctua_render::MOST_SCALE`, which the shell's own
                // `hidpi::followed` reads too — this used to be an 8 here and
                // an 8 there, each comment calling itself the other's bound.
                anyhow::ensure!(
                    (1..=ronoctua_render::MOST_SCALE).contains(&i32::from(asked)),
                    "`--scale {asked}` is not a scale a compositor drives a screen at; \
                     the most is {}",
                    ronoctua_render::MOST_SCALE,
                );
                me.scale = asked;
            }
            "--state" => me.inputs.push(Input::EnterState(value()?)),
            "--command" => me.inputs.push(Input::Command(value()?)),
            "--type" => me.inputs.extend(value()?.chars().map(Input::Char)),
            "--key" => {
                let name = value()?;
                me.inputs.push(match name.as_str() {
                    "up" => Input::Up,
                    "down" => Input::Down,
                    "left" => Input::Left,
                    "right" => Input::Right,
                    "enter" => Input::Confirm,
                    "esc" => Input::Cancel,
                    "tab" => Input::Toggle,
                    "backspace" => Input::Delete,
                    other => bail!("unknown key `{other}`.\n\n{USAGE}"),
                });
            }
            other => bail!("unknown option `{other}`.\n\n{USAGE}"),
        }
    }
    Ok(me)
}

// THE CELL TABLE comes from `paint::heights`, which is the one place a scale
// becomes pixels — the same one the production host reads. There were two
// copies of it, each with a comment saying it had to agree with the other,
// and they stopped agreeing the day the size became something a person could
// move.

fn main() -> Result<()> {
    let ask = parse()?;
    // The theme, before anything resolves a colour: every `Color` is a role
    // that reads the process's theme when drawn, and this is that theme.
    tokens::theme::set(ask.theme);
    tokens::text::set(ask.text);
    let texts = ronoctua_i18n::Texts::fresh(&ask.language)
        .with_context(|| format!("the language `{}` does not exist", ask.language))?;

    let wasm = std::fs::read(&ask.wasm).with_context(|| format!("could not read {}", ask.wasm))?;
    let rt = ronoctua_runtime::Runtime::fresh()?;
    let cwasm = rt.precompile(&wasm)?;
    let component = rt.load(&cwasm)?;
    let mut inst = rt.open(&component)?;

    let d = inst.init()?;
    // THE SAME GUARD THE HOST APPLIES AT LOAD. A descriptor that would be
    // refused on the screen must be refused here too, or this would produce a
    // picture of a buffer nobody can open.
    if let Err(e) = d.validate() {
        bail!("{}: the descriptor does not pass the guard: {e}", d.name);
    }
    // THE PHRASES, exactly as the real host hands them over (`buffer.rs`).
    //
    // Without this every picture of a buffer that holds interface text would be
    // a picture of the fallback — the English word — whatever `--language` was
    // asked for. This tool exists to photograph what a person sees.
    if !d.phrases.is_empty() {
        let said = d
            .phrases
            .iter()
            .map(|k| ronoctua_protocol::Phrase {
                key: k.clone(),
                // The reserved key carries the TAG, never a translation — the
                // same answer `buffer.rs` gives, and for the same reason.
                text: if k == ronoctua_design::Phrases::LANGUAGE {
                    texts.language().to_owned()
                } else {
                    texts.t(k)
                },
            })
            .collect();
        inst.event(&ronoctua_protocol::Input::Phrases(said))?;
    }
    // A FIRST FRAME BEFORE ANYTHING, because the host draws one too — and a
    // component that only computes its screen lazily would otherwise be
    // photographed from a frame it never drew.
    inst.render(ask.columns, ask.lines, paint::heights())?;
    for input in &ask.inputs {
        inst.event(input)?;
        inst.render(ask.columns, ask.lines, paint::heights())?;
    }
    let list = inst.render(ask.columns, ask.lines, paint::heights())?;
    if let Err(e) = list.validate(&d.capabilities) {
        bail!("the frame was refused by the host's own gate: {e}");
    }

    // THE WINDOW'S SIZE IS THE INVERSE OF `Host::context_of`: it takes a window
    // and works out how many cells fit; here the cells are given and the window
    // follows. The margins count twice — one on each side — which is what makes
    // the picture have the same air around it as the screen does.
    let (cell_w, cell_h) = paint::cell();
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let width = (f32::from(ask.columns + tokens::space::CONTENT_INSET * 2) * cell_w) as u32;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let height = (f32::from(ask.lines + tokens::space::CONTENT_INSETY * 2) * cell_h) as u32;
    // AND THE TEXTURE IS PHYSICAL, exactly as a door's buffer is: the picture
    // is the same picture with more pixels in it. A door multiplies the size
    // the compositor configured; here the size comes from the cells asked for,
    // so the multiplication is the last step rather than the first.
    // THE DOORS' RULE, WRITTEN OUT — and this is the one place it is honest to
    // write it out. `crate::hidpi` is a module of the shell's own binary and
    // its other half takes a `Gpu` and a `wl_surface`, neither of which this
    // tool has: including it here would pull in a surface's worth of dead code
    // to reuse one multiplication. What is shared instead is the RULE, and
    // `Offscreen::new` does its own clamping at the same ceiling.
    let (physical_w, physical_h) = (
        width.saturating_mul(u32::from(ask.scale)),
        height.saturating_mul(u32::from(ask.scale)),
    );

    let mut gpu = ronoctua_render::offscreen::Offscreen::new(physical_w, physical_h)?;
    // AND A PICTURE THE GPU CANNOT HOLD IS REFUSED, not quietly cut.
    //
    // `Offscreen::new` clamps at `max_texture_dimension_2d` and says nothing to
    // whoever asked (`Gpu::new` does the same, and `hidpi::resized` exists so a
    // DOOR keeps what came back). A tool has the other answer available: the
    // number of cells was typed by a person, and a photograph taken at a size
    // nobody asked for is worse than one that did not happen — which is the
    // argument `--text` already makes one screen up. Without this the tool
    // measured the rectangles against the clamped texture and the runs against
    // the columns it was given, so an `edge` run landed off the right of the
    // picture and the printed line still named the count that was asked for.
    anyhow::ensure!(
        (gpu.width, gpu.height) == (physical_w, physical_h),
        "the picture asked for is {physical_w}x{physical_h} px and this GPU takes at most {} on \
         either side; ask for fewer cells or a smaller --scale",
        gpu.ceiling,
    );
    let mut text = ronoctua_render::Text::new(
        &gpu.device,
        &gpu.queue,
        ronoctua_render::offscreen::FORMAT,
        i32::from(ask.scale),
        tokens::text::size(),
    )?;
    let mut quads =
        ronoctua_render::quad::Quads::new(&gpu.device, ronoctua_render::offscreen::FORMAT)?;

    let spans = paint::spans_of(&list, &texts, ask.columns);
    // THE RECTANGLES ARE LOGICAL, like a door's — see `crate::hidpi`, which
    // carries the whole of why, and the text is given the PHYSICAL viewport.
    //
    // AND THE TWO ARE THE SAME SPACE, exactly. The
    // texture is exactly `physical` — the `ensure` above refused every other
    // case — so dividing it back by the scale is the window this picture
    // would have if it were a door, exactly, with no remainder for the two
    // halves to disagree over.
    let by = u32::from(ask.scale);
    let (logical_w, logical_h) = (gpu.width / by, gpu.height / by);
    #[allow(clippy::cast_precision_loss)]
    let bands = paint::quads_of(&list, logical_w as f32);
    text.prepare(
        &gpu.device,
        &gpu.queue,
        gpu.width,
        gpu.height,
        ronoctua_render::Atlas::Pinned,
        &spans,
    )?;
    quads.prepare(&gpu.device, &gpu.queue, logical_w, logical_h, &bands);

    // THE SAME ORDER AS THE WINDOW'S: the selection is a background, and
    // anything drawn after it stays on top. Drawing it last would hide the very
    // text it exists to highlight.
    let drawn = &text;
    let bands_ref = &quads;
    gpu.frame(tokens::color::SURFACE_BUFFER.to_linear(), |pass| {
        bands_ref.draw(pass);
        drawn.draw(ronoctua_render::Atlas::Pinned, pass)
    })?;

    let rgba = gpu.read_rgba()?;
    ronoctua_render::offscreen::write_png(&ask.out, &rgba, gpu.width, gpu.height)?;

    let svgs = list
        .commands
        .iter()
        .filter(|c| matches!(c, ronoctua_protocol::Command::Svg { .. }))
        .count();
    if svgs > 0 {
        // SAID OUT LOUD, because a missing drawing looks exactly like a drawing
        // that failed, and somebody comparing this picture with the screen has
        // to know which of the two they are looking at.
        println!(
            "notice: {svgs} svg drawing(s) did not get in — this tool does not rasterise vectors"
        );
    }
    println!(
        "{}: {}x{} px, {} cells by {}, scale {}, {} runs, language {}",
        ask.out.display(),
        gpu.width,
        gpu.height,
        ask.columns,
        ask.lines,
        ask.scale,
        spans.len(),
        texts.language()
    );
    Ok(())
}
