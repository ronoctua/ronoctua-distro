//! The camera of four system windows — the network, Bluetooth, the language
//! and region, and the camera's settings — over PLANTED machines.
//!
//! # Why one camera for four doors
//!
//! Each of these windows reads a daemon (NetworkManager, BlueZ, timedated,
//! the studio) that a photograph must not ask: what is drawn has to be the
//! same on every machine, and a test of the screen that read the developer's
//! own network would pass or fail by where it ran. So each window module
//! builds itself over a planted reading — its `photographed(scene)` — and
//! this module does the rest the way every camera here does it: a
//! [`Detached`] door, the keys and the pointer down the door's own paths,
//! one frame composed as the door composes it, and the PNG.
//!
//! `ronoctua-shell photograph --door network --scene lan|hotspot|vpn|phone`, and the same for
//! `bluetooth --scene phone`, `locale --scene currency|zone|watch-on|
//! watch-asking|watch-system` and
//! `camera-settings --scene virtual`.

use anyhow::{Context as _, Result, bail, ensure};
use ronoctua_tokens as tokens;
use smithay_client_toolkit::seat::pointer::{PointerEvent, PointerEventKind};

use crate::door::{Content, Detached, Ui};

/// The usage, printed by `--help`.
const USAGE: &str = "\
usage: ronoctua-shell photograph --door network|bluetooth|locale|camera-settings
         --scene <scene> [--out file.png] [--columns N] [--lines N]
         [--language en-GB|pt-BR] [--theme id] [--scale N] [--list]
         [--key notation]... [--type text]... [--hover column,line]...

scenes: network lan|hotspot|vpn|phone, bluetooth phone,
        locale currency|zone|watch-on|watch-asking|watch-system, camera-settings virtual";

/// One thing done to the window before the picture.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Step {
    /// A key, by its notation.
    Key(ronoctua_keymap::Key),
    /// Text, typed a character at a time.
    Type(String),
    /// The pointer, resting on a cell.
    Hover(u16, u16),
}

/// What the words asked for.
#[derive(Debug, Clone)]
struct Asked {
    door: String,
    scene: String,
    out: std::path::PathBuf,
    columns: u16,
    lines: u16,
    language: String,
    theme: tokens::theme::Theme,
    scale: u16,
    list: bool,
    steps: Vec<Step>,
}

fn number(flag: &str, raw: &str, most: u16) -> Result<u16> {
    let n: u16 = raw
        .parse()
        .ok()
        .with_context(|| format!("`{flag} {raw}` is not a whole number"))?;
    ensure!(
        (1..=most).contains(&n),
        "`{flag} {n}` is out of range; the range is 1..={most}"
    );
    Ok(n)
}

/// The words, `--door` included.
fn parse(words: &[String]) -> Result<Asked> {
    let mut me = Asked {
        door: String::new(),
        scene: String::new(),
        out: std::path::PathBuf::from("system.png"),
        columns: 100,
        lines: 60,
        language: "pt-BR".to_owned(),
        theme: tokens::theme::default_one(),
        scale: 1,
        list: false,
        steps: Vec::new(),
    };
    let mut words = words.iter();
    while let Some(flag) = words.next() {
        let mut value = || {
            words
                .next()
                .cloned()
                .with_context(|| format!("`{flag}` with no value"))
        };
        match flag.as_str() {
            "--help" | "-h" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            "--door" => me.door = value()?,
            "--scene" => me.scene = value()?,
            "--out" | "-o" => me.out = std::path::PathBuf::from(value()?),
            "--columns" => me.columns = number("--columns", &value()?, 1000)?,
            "--lines" => me.lines = number("--lines", &value()?, 1000)?,
            "--language" => me.language = value()?,
            "--theme" => {
                let id = value()?;
                me.theme =
                    tokens::theme::by_id(&id).with_context(|| format!("unknown theme `{id}`"))?;
            }
            "--scale" => {
                let most = u16::try_from(ronoctua_render::MOST_SCALE).unwrap_or(1);
                me.scale = number("--scale", &value()?, most)?;
            }
            "--list" => me.list = true,
            "--key" => {
                let notation = value()?;
                let key = ronoctua_keymap::Key::parse(&notation)
                    .map_err(|e| anyhow::anyhow!("`--key {notation}` is not a key: {e}"))?;
                me.steps.push(Step::Key(key));
            }
            "--type" => me.steps.push(Step::Type(value()?)),
            "--hover" => {
                let raw = value()?;
                let (c, l) = raw
                    .split_once(',')
                    .with_context(|| format!("`--hover` wants `column,line`, not `{raw}`"))?;
                me.steps.push(Step::Hover(
                    number("--hover", c, 999)?,
                    number("--hover", l, 999)?,
                ));
            }
            other => bail!("unknown option `{other}`.\n\n{USAGE}"),
        }
    }
    ensure!(!me.scene.is_empty(), "`--scene` is missing.\n\n{USAGE}");
    Ok(me)
}

/// `ronoctua-shell photograph --door network|bluetooth|locale|camera-settings
/// …`, to its end: the rest of the command line is this camera's.
pub fn taken(rest: &[String]) -> ! {
    let line: Vec<String> = std::env::args().skip(2).collect();
    let words = if line.ends_with(rest) {
        line
    } else {
        rest.to_vec()
    };
    match parse(&words).and_then(|asked| shoot_the_door(&asked)) {
        Ok(()) => std::process::exit(0),
        Err(e) => {
            eprintln!("Error: {e:?}");
            std::process::exit(1);
        }
    }
}

/// The window the words name, over its scene.
fn shoot_the_door(asked: &Asked) -> Result<()> {
    tokens::theme::set(asked.theme);
    tokens::theme::set_ground(tokens::theme::Ground::Chrome);
    tokens::text::set(tokens::text::NORMAL);
    crate::locale::prefs::set(crate::locale::Choice::default());
    match asked.door.as_str() {
        "network" => {
            let (content, line) = crate::network_window::photographed(&asked.scene)?;
            shoot(asked, content, line)
        }
        "bluetooth" => {
            let (content, line) = crate::bluetooth_window::photographed(&asked.scene)?;
            shoot(asked, content, line)
        }
        "locale" => {
            let (content, line) = crate::locale_window::photographed(&asked.scene)?;
            shoot(asked, content, line)
        }
        "camera-settings" => {
            let (content, line) = crate::camera_settings_window::photographed(&asked.scene)?;
            shoot(asked, content, line)
        }
        other => bail!("`{other}` is not one of this camera's doors.\n\n{USAGE}"),
    }
}

/// Where the middle of a cell is, in logical pixels.
fn middle_of(column: u16, line: u16) -> (f64, f64) {
    let px = tokens::text::size();
    (
        (f64::from(column) + 0.5) * f64::from(tokens::metrics::cell_width(px)),
        (f64::from(line) + 0.5) * f64::from(tokens::metrics::line_height(px)),
    )
}

/// One window, its steps, one frame, the PNG.
fn shoot<C: Content>(asked: &Asked, content: C, line: Option<crate::line::Line>) -> Result<()> {
    let texts = ronoctua_i18n::Texts::fresh(&asked.language)
        .with_context(|| format!("the language `{}` does not exist", asked.language))?;
    let ui = Ui::detached(texts, (1, 1), i32::from(asked.scale))?;
    let mut d = Detached::new(ui, content);
    let px = tokens::text::size();
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let size = (
        (f32::from(asked.columns + 2 * d.content.margin()) * tokens::metrics::cell_width(px)).ceil()
            as u32,
        (f32::from(asked.lines) * tokens::metrics::line_height(px)).ceil() as u32,
    );
    d.lay_out_at(size);
    if let Some(line) = line {
        d.ui.ask(line);
    }
    d.turn();
    let mut entered = false;
    for step in &asked.steps {
        match step {
            Step::Key(key) => d.press(*key),
            Step::Type(text) => {
                for c in text.chars() {
                    d.press(ronoctua_keymap::Key::simple(if c == ' ' {
                        ronoctua_keymap::Sym::Space
                    } else {
                        ronoctua_keymap::Sym::Char(c)
                    }));
                }
            }
            Step::Hover(c, l) => {
                let kind = if std::mem::replace(&mut entered, true) {
                    PointerEventKind::Motion { time: 0 }
                } else {
                    PointerEventKind::Enter { serial: 0 }
                };
                d.pointer(&[PointerEvent {
                    surface: d.ui.surface.clone(),
                    position: middle_of(*c, *l),
                    kind,
                }]);
            }
        }
        d.turn();
        if d.ui.quit {
            break;
        }
    }
    let mut gpu = ronoctua_render::offscreen::Offscreen::new(1, 1)?;
    let format = ronoctua_render::offscreen::FORMAT;
    let mut text = ronoctua_render::Text::new(
        &gpu.device,
        &gpu.queue,
        format,
        d.ui.scale,
        tokens::text::size(),
        &asked.language,
    )?;
    let mut quads = ronoctua_render::quad::Quads::new(&gpu.device, format)?;
    let mut images = ronoctua_render::image::Images::new(&gpu.device, format)?;
    let composed = d.compose(&gpu.device, &gpu.queue, &mut images);
    if asked.list {
        for s in &composed.spans {
            println!(
                "run line {} column {} {:?} {:?}",
                s.line, s.column, s.color, s.text
            );
        }
        return Ok(());
    }
    let logical = (d.ui.width, d.ui.height);
    let by = u32::from(asked.scale);
    gpu.resize(logical.0 * by, logical.1 * by);
    text.prepare(
        &gpu.device,
        &gpu.queue,
        gpu.width,
        gpu.height,
        ronoctua_render::Atlas::Pinned,
        &composed.spans,
    )?;
    quads.prepare(
        &gpu.device,
        &gpu.queue,
        logical.0,
        logical.1,
        &composed.quads,
    );
    images.prepare(&gpu.queue, logical.0, logical.1, &composed.pictures);
    let (drawn, rects, pictures) = (&text, &quads, &images);
    gpu.frame(tokens::color::SURFACE_CHROME.to_linear(), |pass| {
        composed.draw(pass, drawn, rects, pictures)
    })?;
    let rgba = gpu.read_rgba()?;
    ronoctua_render::offscreen::write_png(&asked.out, &rgba, gpu.width, gpu.height)?;
    println!(
        "{}: {}x{} px, {} columns by {} lines, language {}",
        asked.out.display(),
        gpu.width,
        gpu.height,
        d.ui.columns,
        d.ui.lines,
        d.ui.texts.language()
    );
    if let Some(m) = d.ui.message.as_ref() {
        println!("the message line says: {m}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: the camera stops taking its words, or takes a door's scene
    /// without the scene.
    #[test]
    fn the_camera_reads_its_words() {
        let words: Vec<String> = [
            "--door",
            "network",
            "--scene",
            "lan",
            "--key",
            "<down>",
            "--type",
            "Lis",
            "--hover",
            "3,4",
            "--columns",
            "90",
            "--language",
            "en-GB",
        ]
        .iter()
        .map(|w| (*w).to_owned())
        .collect();
        let asked = parse(&words).expect("the words parse");
        assert_eq!(
            (asked.door.as_str(), asked.scene.as_str()),
            ("network", "lan")
        );
        assert_eq!(asked.columns, 90);
        assert_eq!(asked.steps.len(), 3);
        assert!(parse(&["--door".to_owned(), "network".to_owned()]).is_err());
    }

    /// FAILS IF: a door's planted scene stops building — or builds for a
    /// scene the door does not know.
    #[test]
    fn every_scene_builds_and_an_unknown_one_is_refused() {
        for scene in ["lan", "hotspot", "vpn", "phone"] {
            assert!(
                crate::network_window::photographed(scene).is_ok(),
                "{scene}"
            );
        }
        assert!(crate::bluetooth_window::photographed("phone").is_ok());
        for scene in [
            "currency",
            "zone",
            "watch-on",
            "watch-asking",
            "watch-system",
        ] {
            assert!(crate::locale_window::photographed(scene).is_ok(), "{scene}");
        }
        assert!(crate::camera_settings_window::photographed("virtual").is_ok());
        assert!(crate::network_window::photographed("nothing").is_err());
    }
}
