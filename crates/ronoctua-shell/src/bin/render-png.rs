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
//! Usage: `ronoctua-render-png <componente.wasm> [opcoes]`, `--help` for the list.

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
    /// In order: everything to send the component before drawing.
    inputs: Vec<Input>,
}

const USAGE: &str = "\
uso: ronoctua-render-png <componente.wasm> [opcoes]

  --out <arquivo>     onde escrever            (padrao: buffer.png)
  --columns <n>       largura em celulas       (padrao: 90)
  --lines <n>         altura em celulas        (padrao: 24)
  --language <id>     pt-BR ou en-GB           (padrao: pt-BR)

  --state <id>        entra num estado antes de desenhar
  --command <id>      dispara um comando do buffer
  --key <nome>        up, down, left, right, enter, esc, tab, backspace
  --type <texto>      digita, caractere por caractere

As quatro ultimas sao repetiveis e valem NA ORDEM em que aparecem: e assim que
se chega numa tela que so existe depois de andar ate ela.";

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
        inputs: Vec::new(),
    };
    while let Some(flag) = args.next() {
        // Each flag consumes its own argument, and a missing one is a mistake
        // worth naming: the alternative is a picture of a screen the caller did
        // not ask for. Taken here and not in a closure so the loop keeps the
        // iterator to itself.
        let mut value = || args.next().with_context(|| format!("`{flag}` sem valor"));
        match flag.as_str() {
            "--out" => me.out = std::path::PathBuf::from(value()?),
            "--columns" => me.columns = value()?.parse().context("`--columns` nao e numero")?,
            "--lines" => me.lines = value()?.parse().context("`--lines` nao e numero")?,
            "--language" => me.language = value()?,
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
                    other => bail!("tecla `{other}` desconhecida.\n\n{USAGE}"),
                });
            }
            other => bail!("opcao `{other}` desconhecida.\n\n{USAGE}"),
        }
    }
    Ok(me)
}

/// The scale heights the production host sends.
///
/// The same five tokens `Host::heights` reads. A run occupies as many cells as
/// its scale is worth, and a picture drawn with a different table would be a
/// picture of a different layout.
fn heights() -> ronoctua_protocol::Heights {
    ronoctua_protocol::Heights::from_sizes(
        tokens::font::SIZE_SMALL,
        tokens::font::SIZE_BASE,
        tokens::font::SIZE_LARGE,
        tokens::font::SIZE_XLARGE,
        tokens::font::SIZE_XXLARGE,
        tokens::font::LINE_HEIGHT,
    )
}

fn main() -> Result<()> {
    let ask = parse()?;
    let texts = ronoctua_i18n::Texts::fresh(&ask.language)
        .with_context(|| format!("nao existe o idioma `{}`", ask.language))?;

    let wasm =
        std::fs::read(&ask.wasm).with_context(|| format!("nao consegui ler {}", ask.wasm))?;
    let rt = ronoctua_runtime::Runtime::fresh()?;
    let cwasm = rt.precompile(&wasm)?;
    let component = rt.load(&cwasm)?;
    let mut inst = rt.open(&component)?;

    let d = inst.init()?;
    // THE SAME GUARD THE HOST APPLIES AT LOAD. A descriptor that would be
    // refused on the screen must be refused here too, or this would produce a
    // picture of a buffer nobody can open.
    if let Err(e) = d.validate() {
        bail!("{}: o descritor nao passa na guarda: {e}", d.name);
    }
    // A FIRST FRAME BEFORE ANYTHING, because the host draws one too — and a
    // component that only computes its screen lazily would otherwise be
    // photographed from a frame it never drew.
    inst.render(ask.columns, ask.lines, heights())?;
    for input in &ask.inputs {
        inst.event(input)?;
        inst.render(ask.columns, ask.lines, heights())?;
    }
    let list = inst.render(ask.columns, ask.lines, heights())?;
    if let Err(e) = list.validate(&d.capabilities) {
        bail!("o quadro foi recusado pela mesma trava do host: {e}");
    }

    // THE WINDOW'S SIZE IS THE INVERSE OF `Host::context_of`: it takes a window
    // and works out how many cells fit; here the cells are given and the window
    // follows. The margins count twice — one on each side — which is what makes
    // the picture have the same air around it as the screen does.
    let (cell_w, cell_h) = paint::cell();
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let width = (f32::from(ask.columns + tokens::space::BUFFER_MARGINX * 2) * cell_w) as u32;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let height = (f32::from(ask.lines + tokens::space::BUFFER_MARGINY * 2) * cell_h) as u32;

    let mut gpu = ronoctua_render::offscreen::Offscreen::new(width, height)?;
    let mut text = ronoctua_render::Text::new(
        &gpu.device,
        &gpu.queue,
        ronoctua_render::offscreen::FORMAT,
        tokens::font::SIZE_LARGE,
    )?;
    let mut quads =
        ronoctua_render::quad::Quads::new(&gpu.device, ronoctua_render::offscreen::FORMAT)?;

    let spans = paint::spans_of(&list, &texts, ask.columns);
    #[allow(clippy::cast_precision_loss)]
    let bands = paint::selections_of(&list, gpu.width as f32);
    text.prepare(
        &gpu.device,
        &gpu.queue,
        gpu.width,
        gpu.height,
        ronoctua_render::Atlas::Pinned,
        &spans,
    )?;
    quads.prepare(&gpu.device, &gpu.queue, gpu.width, gpu.height, &bands);

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
        println!("aviso: {svgs} desenho(s) svg nao entraram — esta ferramenta nao rasteriza vetor");
    }
    println!(
        "{}: {}x{} px, {} celulas por {}, {} trechos, idioma {}",
        ask.out.display(),
        gpu.width,
        gpu.height,
        ask.columns,
        ask.lines,
        spans.len(),
        texts.language()
    );
    Ok(())
}
