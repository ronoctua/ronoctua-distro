//! A terminal in an `xdg_toplevel` of its own — §7.5 and §7.10.
//!
//! # Where each piece lives
//!
//! ```text
//!   keyboard (Wayland)  ->  Pty::write_bytes  ->  process
//!                                                 |
//!   screen (wgpu)  <-  Grid  <-  Terminal  <-  Pty::pump
//!                       ^            ^
//!               VT thread ------------+  (section 7.10: `!Send` engine)
//! ```
//!
//! There are THREE threads and no lock: the compositor's (this one), the VT
//! engine's and the one that reads the PTY. Communication is by channel, and
//! the only meeting point is calloop's `ping`, which only wakes the loop — it
//! carries no data.
//!
//! # Why the window is an `xdg_toplevel`
//!
//! The same reason as the native buffer: invariant 4 says the shell never
//! reimplements window layout. A terminal is a window, and whoever positions
//! it is the compositor.
//!
//! # Why the drawing goes through the protocol
//!
//! The grid becomes a [`ronoctua_protocol::Command::Grid`] before becoming a
//! pixel, and not a direct path from `Grid` to `Span`. The WIT's `grid`
//! node exists precisely for this (§7.10, "from day one"): the day a wasip2
//! buffer asks for the `terminal` capability, it emits the same command and
//! falls into the same drawer. A shortcut here would be a second render path
//! to keep in sync.

use std::sync::Arc;

use crate::door::{Canvas, Content, Door, Hand, Ui};
use anyhow::{Context as _, Result};
use ronoctua_term::pty::{Pty, Stream};
use ronoctua_term::terminal::Terminal;
use ronoctua_tokens as tokens;
use smithay_client_toolkit::seat::{
    keyboard::{KeyEvent, Keysym, Modifiers},
    pointer::PointerEvent,
};

/// The terminal window's `app_id`.
///
/// Distinct from the native buffer's on purpose: it is by it that the modeline
/// and the test know which window is which, and an equal `app_id` would make
/// the two look like the same thing in the compositor's IPC.
pub const APP_ID: &str = "br.ronoctua.terminal";

/// The MONITOR window's `app_id` — the same terminal, another identity.
///
/// # Why a second one, and not the same
///
/// `Ctrl+Alt+Esc`, `LEADER+Esc` and `LEADER+S-Esc` open a FLOATING window, and
/// what floats it is a `for_window` rule matched on this string. The shell
/// does not place windows (invariant 4): it says WHAT the window is, and the
/// compositor decides where. With a single `app_id` the rule would float every
/// terminal, including `LEADER RET /`.
pub const APP_ID_MONITOR: &str = "br.ronoctua.monitor";

/// The RESCUE console's `app_id` — the third identity of the same terminal.
///
/// A third one and not the monitor's, for the reason the second exists: the
/// rule that floats it asks for a bigger window. What `Ctrl+Alt+Del` opens is
/// meant to be read while something is wrong, and 70% of the screen is a list
/// with the log cut off.
pub const APP_ID_RESCUE: &str = "br.ronoctua.rescue";

/// The rule that makes the monitor window float.
///
/// # Why it is generated from here, and not written in the two configs
///
/// The `app_id` above and the rule's regex have to be the same string, and it
/// would exist in THREE places — this file, `nix/modules/scroll.nix` and
/// `nix/dev.nix`. A string that crosses a border needs a check (`AGENTS.md`),
/// and generating it is the check that cannot go stale: `export-window-rules`
/// prints this, and both configs take it in — the same arrangement as the
/// chords in `nix/compositor-binds.nix`.
///
/// `r#"…"#` and not plain quotes: the criterion's value is a regex with a
/// backslash in it (`^br\.ronoctua\.monitor$`), and writing it in a raw string
/// is what keeps Rust's escapes out of a string the compositor parses.
///
/// The proportions are the compositor's geometry, like `gaps` and the border's
/// width in the same file — invariant 9's tokens are the design system's, and
/// no component of it draws this window.
#[must_use]
pub fn floating_rules() -> String {
    // The rescue console is WIDER, and that is the only difference: it is read
    // while something is wrong, and a list with the log cut off is a diagnosis
    // one has to scroll for.
    //
    // AND THE CAMERA APP floats too, at 60%: it is a picture of a face the
    // owner asked to see in a window of its own over the work, not a tile in
    // the strip that would push every other window aside.
    [
        (APP_ID_MONITOR, 70),
        (APP_ID_RESCUE, 90),
        (crate::camera::APP_ID, 60),
    ]
    .into_iter()
    .map(|(app_id, percent)| {
        let regex = app_id.replace('.', r"\.");
        // `ppt` IS NOT OPTIONAL: a floating container is sized in PIXELS
        // unless the unit says otherwise, so `resize set width 70` would
        // make a seventy-pixel window — which reads as the monitor having
        // failed to open rather than as a unit that was left out.
        //
        // And `move position center`, because a new floating window is
        // not centred on its own. There is no equivalent of asking for it
        // to open focused; one does, and that is the compositor's own
        // behaviour rather than something to spell.
        format!(
            "for_window [app_id=\"^{regex}$\"] \
                 floating enable, \
                 resize set width {percent} ppt height {percent} ppt, \
                 move position center\n"
        )
    })
    .collect()
}

/// It builds the engine INSIDE the VT thread — see `ronoctua_term::terminal`.
fn engine(columns: u16, lines: u16) -> Box<dyn ronoctua_term::Engine> {
    Box::new(ronoctua_term::alacritty::Alacritty::fresh(columns, lines))
}

/// Opens a terminal in a window of its own and runs until it closes.
///
/// An empty `program` uses the `$SHELL`.
///
/// # Errors
///
/// There is no compositor, the pty does not open, or the program does not exist.
pub fn host_it(program: Option<&str>, args: &[String]) -> Result<()> {
    host_as(APP_ID, "ronoctua: terminal", program, args)
}

/// The same terminal, under the monitor's `app_id` — so the compositor floats it.
///
/// # Errors
///
/// The same as [`host_it`].
pub fn host_monitor(program: &str, args: &[String]) -> Result<()> {
    host_as(APP_ID_MONITOR, "ronoctua: monitor", Some(program), args)
}

/// The same terminal again, under the rescue console's `app_id`.
///
/// # Errors
///
/// The same as [`host_it`].
pub fn host_rescue(program: &str, args: &[String]) -> Result<()> {
    host_as(APP_ID_RESCUE, "ronoctua: rescue", Some(program), args)
}

/// The body of both: everything is the same but the window's identity.
fn host_as(
    app_id: &'static str,
    title: &'static str,
    program: Option<&str>,
    args: &[String],
) -> Result<()> {
    crate::door::run(|w| {
        // 80x24 until the `configure` says the actual size. The following
        // `SIGWINCH` corrects the program; being born with a size is better
        // than being born without, because a 0x0 pty makes the shell print
        // wrong once.
        let (columns, lines) = (80u16, 24u16);
        let program = program.map_or_else(ronoctua_term::pty::default_program, ToOwned::to_owned);
        let pty = Arc::new(
            Pty::open(&program, args, columns, lines).with_context(|| format!("open {program}"))?,
        );
        let vt =
            Arc::new(Terminal::fresh(columns, lines, engine).context("start the engine's thread")?);
        tracing::info!(engine = vt.name(), %program, "terminal opened");

        // The `ping` wakes the loop when the PTY has news. It carries no data:
        // the bytes have already gone to the engine through its own queue.
        // Sending the bytes through here would make them go through two
        // queues, and the second one could reorder them against what the
        // keyboard wrote.
        let (ping, face) = smithay_client_toolkit::reexports::calloop::ping::make_ping()
            .context("create the pty's ping")?;
        w.handle
            .insert_source(face, |(), (), door: &mut Door<TerminalWindow>| {
                door.ui.ask_paint();
            })
            .map_err(|e| anyhow::anyhow!("hang the ping: {e}"))?;

        // The PTY's thread. It touches neither Wayland nor wgpu — it only
        // feeds the engine and rings the ping.
        {
            let pty = Arc::clone(&pty);
            let vt = Arc::clone(&vt);
            std::thread::Builder::new()
                .name("ronoctua-pty".into())
                .spawn(move || {
                    let r = pty.pump(|b| {
                        if vt.feed(b).is_err() {
                            // The engine's thread died: carrying on would read
                            // bytes to throw away.
                            return Stream::Para;
                        }
                        ping.ping();
                        Stream::Follow
                    });
                    if let Err(e) = r {
                        tracing::warn!(error = %e, "reading the pty ended with an error");
                    }
                    // End of the program: it wakes the loop one last time so
                    // it notices and closes the window.
                    ping.ping();
                })
                .context("start the pty's thread")?;
        }

        Ok(TerminalWindow {
            app_id,
            title,
            columns,
            lines,
            pty,
            vt,
        })
    })
}

/// How many cells fit in a window of `width` x `height` pixels.
///
/// # Why it is a free function and not a method
///
/// Because it is the one thing here that can be WRONG in a way nothing shows,
/// and a method on a struct that needs a Wayland connection cannot be tested.
/// Free, it is arithmetic over two numbers, and the test runs in microseconds.
///
/// **Integer arithmetic, without measuring text**: the cell is exactly 0.600 x
/// the font size at normal width, including in the Nerd Fonts. Measuring here
/// would be trading one calculation for a shaping call on every `configure`.
///
/// It is the BUFFER's cell, the same one the drawer uses. With the footer's,
/// the arithmetic would give more columns than fit and the text would go out
/// past the right edge with no error at all.
///
/// # And the margin comes off, TWICE — one on each side
///
/// The paragraph above says the text would otherwise leak past the right edge
/// with no error at all, and that is exactly what WAS happening:
/// `paint::spans_of` shifts every run by `CONTENT_INSET`, and this counted
/// columns across the whole window. The last two columns of every terminal
/// were written outside it, and the compositor does not clip.
///
/// `Host::context_of` in `buffer.rs` already subtracted it, and `render-png`
/// adds it back to work out a window's size from a cell count. Three places,
/// one arithmetic; this was the one that disagreed.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn cells_that_fit(width: u32, height: u32) -> (u16, u16) {
    let cell_width_px = tokens::text::size() * tokens::font::CELL_WIDTH_EM;
    let cell_height_px = tokens::text::size() * tokens::font::LINE_HEIGHT;
    let columns = ((width as f32 / cell_width_px) as u32)
        .saturating_sub(u32::from(tokens::space::CONTENT_INSET) * 2)
        .clamp(1, u32::from(u16::MAX)) as u16;
    let lines = ((height as f32 / cell_height_px) as u32)
        .saturating_sub(u32::from(tokens::space::CONTENT_INSETY) * 2)
        .clamp(1, u32::from(u16::MAX)) as u16;
    (columns, lines)
}

/// The terminal's window.
struct TerminalWindow {
    /// Which of the three identities this terminal is — see [`APP_ID`].
    app_id: &'static str,
    /// The title the compositor shows.
    title: &'static str,
    /// The grid's size in cells — the terminal's OWN count, and not
    /// `Ui::columns`: see [`cells_that_fit`] for why the margin comes off
    /// twice here.
    columns: u16,
    /// See [`TerminalWindow::columns`].
    lines: u16,
    pty: Arc<Pty>,
    vt: Arc<Terminal>,
}

/// The grid as a display list.
///
/// A single `Command::Grid`, and not a `Text` per line: the `grid` node
/// exists in the WIT exactly so that terminal content crosses the protocol in
/// one piece. See the top of the module.
fn grid_list(g: &ronoctua_term::Grid) -> ronoctua_protocol::DisplayList {
    let mut d = ronoctua_protocol::DisplayList::fresh_one();
    d.push(ronoctua_protocol::Command::Grid {
        line: 0,
        column: 0,
        // `visible_rows()` and not `canonical_f()`: the second one puts the cursor's
        // position on an extra line, for the differential diff. Drawing that would
        // show `cursor: 3,0` as if it were the program's output.
        lines: g.visible_rows(),
        color: ronoctua_protocol::Color::name("text-primary"),
    });
    d
}

impl TerminalWindow {
    /// Recomputes columns and rows from the size in pixels.
    ///
    /// **Integer arithmetic, without measuring text**: the cell is exactly
    /// 0.600 x the font size at normal width, including in the Nerd Fonts.
    /// Measuring here would be trading one calculation for a shaping call on
    /// every `configure`.
    fn recount(&mut self, ui: &Ui) {
        // A WINDOW WITH NO SIZE HAS NO GRID. `cells_that_fit` floors at one
        // cell rather than declining to answer, so at 0x0 this told the engine
        // AND the pty that the terminal was one column by one row — a
        // `SIGWINCH` of 1x1 to a shell that has just started, which is exactly
        // what `host_as`'s 80x24 guess exists to prevent.
        //
        // It is reachable before the first `configure`: the scale can arrive
        // first, and the size watch on `display.json` is installed before it
        // too. Both call this.
        if ui.width == 0 || ui.height == 0 {
            return;
        }
        let (columns, lines) = cells_that_fit(ui.width, ui.height);
        if (columns, lines) == (self.columns, self.lines) {
            return;
        }
        self.columns = columns;
        self.lines = lines;
        // BOTH sides. Only the engine would leave the program printing at the old
        // width; only the pty would leave the grid at the old size, and the
        // difference would show up as cut text with no error at all.
        if let Err(e) = self.vt.resize(columns, lines) {
            tracing::warn!(error = %e, "resizing the engine failed");
        }
        if let Err(e) = self.pty.resize(columns, lines) {
            tracing::warn!(error = %e, "resizing the pty failed");
        }
    }
}

/// The bytes a key sends to the program.
///
/// # Why there is no keymap here
///
/// What the terminal receives has already gone through the compositor and is
/// not a leader chord: it is text and the few control keys the VT
/// understands. Mapping this through `ronoctua-keymap` would give the
/// terminal a second keymap, and §7.4 wants only one — the shell's, which
/// does not even reach here.
#[must_use]
pub fn key_bytes(sym: Keysym, utf8: Option<&str>, ctrl: bool) -> Option<Vec<u8>> {
    // `\r` and not `\n`: a terminal sends a CARRIAGE RETURN on Enter, and the
    // line driver converts it. Sending `\n` makes the shell accept the line and
    // not echo the break — the cursor stays where it was, with no error at all.
    let especial = match sym {
        Keysym::Return | Keysym::KP_Enter => Some(b"\r".to_vec()),
        Keysym::BackSpace => Some(vec![0x7f]),
        Keysym::Tab => Some(b"\t".to_vec()),
        Keysym::Escape => Some(vec![0x1b]),
        // CSI: the sequences every full-screen program expects.
        Keysym::Up => Some(b"\x1b[A".to_vec()),
        Keysym::Down => Some(b"\x1b[B".to_vec()),
        Keysym::Right => Some(b"\x1b[C".to_vec()),
        Keysym::Left => Some(b"\x1b[D".to_vec()),
        Keysym::Home => Some(b"\x1b[H".to_vec()),
        Keysym::End => Some(b"\x1b[F".to_vec()),
        Keysym::Delete => Some(b"\x1b[3~".to_vec()),
        _ => None,
    };
    if especial.is_some() {
        return especial;
    }

    let text = utf8?;
    let mut c = text.chars().next()?;
    if !ctrl {
        return Some(text.as_bytes().to_vec());
    }
    // `Ctrl-A` is byte 1. Without this, `Ctrl-C` would become the letter `c` and
    // there would be no way of interrupting anything.
    c = c.to_ascii_lowercase();
    if c.is_ascii_lowercase() {
        return Some(vec![c as u8 - b'a' + 1]);
    }
    Some(text.as_bytes().to_vec())
}

impl Content for TerminalWindow {
    type Hand = ();

    fn app_id(&self) -> &'static str {
        self.app_id
    }

    fn title(&self, _: &ronoctua_i18n::Texts) -> String {
        self.title.to_owned()
    }

    /// One key, from a press or from a REPEAT.
    ///
    /// In a terminal the repeat IS the right behavior: holding the key
    /// deletes several characters, goes down several lines. Unlike the
    /// footer, where repeating a leader key is always a mistake.
    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers) {
        let Some(bytes) = key_bytes(event.keysym, event.utf8.as_deref(), mods.ctrl) else {
            return;
        };
        if let Err(e) = self.pty.write_bytes(&bytes) {
            tracing::warn!(error = %e, "writing to the pty failed; closing");
            ui.quit = true;
        }
        // It does NOT redraw here. The echo comes from the program through the pty,
        // and drawing now would show the screen from BEFORE the key — a late frame
        // that only shows up when one types fast.
    }

    /// THE HEADER'S KEYS ARE THE PROGRAM'S. This window draws no header, and
    /// the right `Ctrl` and `Esc` it would take are keys a program running
    /// here is owed — `Esc` above all, which is half of what vim is.
    fn header_keys(&self) -> bool {
        false
    }

    /// NOTHING UNDER THE POINTER ANSWERS IT: the grid is the program's, and
    /// the standard list's pointer would light rows and a header this window
    /// never draws.
    fn pointer(&mut self, _: &mut Ui, _: &mut Hand<()>, _: &[PointerEvent]) {}

    /// The window was given a new size, a new scale or a new size of text.
    ///
    /// A terminal's grid IS its content, so a cell that stopped matching its
    /// glyph is not cosmetic here: the engine is resized in CELLS, and the
    /// mismatch would be a wrong number of columns handed to whatever is
    /// running.
    fn resized(&mut self, ui: &mut Ui) {
        self.recount(ui);
    }

    fn paint(&mut self, ui: &mut Ui, canvas: &mut Canvas<'_>) {
        let Ok(grid) = self.vt.grid() else {
            tracing::info!("the engine's thread left; closing the terminal");
            ui.quit = true;
            return;
        };
        let list = grid_list(&grid);
        // Node i18n: EVERY terminal text is the program's data, never an interface
        // label. Translating the output of `ls` would be absurd, and that is why
        // `Command::Grid` only carries `String`.
        //
        // The window's COLUMNS: the host guarantees that nothing leaks past the
        // edge, and a terminal line wider than the window is the common case.
        canvas.spans.extend(crate::paint::spans_of(
            &list,
            &ui.texts,
            self.columns,
            self.lines,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::{grid_list, key_bytes};
    use ronoctua_tokens as tokens;
    use smithay_client_toolkit::seat::keyboard::Keysym;

    #[test]
    fn the_rule_matches_the_app_id_the_window_actually_sets() {
        // THE RULE AND THE WINDOW ARE ONE STRING. Written apart, an `app_id`
        // renamed here would give a rule that matches nothing — and the symptom
        // is the monitor opening TILED, which looks like the shortcut working.
        let rule = super::floating_rules();
        assert!(
            rule.contains(&super::APP_ID_MONITOR.replace('.', r"\.")),
            "the rule does not name the window's app id:\n{rule}"
        );
        // The dots are escaped: `br.ronoctua.monitor` as a regex would also
        // match `brXronoctuaYmonitor`, and — worse — is not what one reads.
        assert!(rule.contains(r"^br\.ronoctua\.monitor$"), "{rule}");
        assert!(rule.contains("floating enable"), "{rule}");
        // `ppt` IS NOT OPTIONAL. A floating container is sized in PIXELS
        // unless the unit says so, and a seventy-pixel window reads as the
        // monitor having failed to open rather than as a missing unit.
        assert!(rule.contains("ppt"), "the size has no unit:\n{rule}");
        // AND IT IS CENTRED, because a new floating window is not.
        assert!(rule.contains("move position center"), "{rule}");
    }

    #[test]
    fn enter_sends_a_carriage_return() {
        // `\n` would make the shell accept the line and NOT echo the break: the
        // cursor would stay where it was, with no error at all. A terminal sends `\r`.
        assert_eq!(
            key_bytes(Keysym::Return, Some("\r"), false),
            Some(b"\r".to_vec())
        );
    }

    #[test]
    fn ctrl_c_becomes_the_control_byte() {
        // Without this `Ctrl-C` would become the letter `c` and there would be no way
        // of interrupting anything — and the symptom would be "the terminal ignores Ctrl-C".
        assert_eq!(key_bytes(Keysym::c, Some("c"), true), Some(vec![3]));
        assert_eq!(key_bytes(Keysym::d, Some("d"), true), Some(vec![4]));
    }

    #[test]
    fn with_no_ctrl_the_letter_is_the_letter() {
        assert_eq!(key_bytes(Keysym::c, Some("c"), false), Some(b"c".to_vec()));
    }

    #[test]
    fn the_arrows_come_out_as_csi() {
        assert_eq!(key_bytes(Keysym::Up, None, false), Some(b"\x1b[A".to_vec()));
        assert_eq!(
            key_bytes(Keysym::Left, None, false),
            Some(b"\x1b[D".to_vec())
        );
    }

    #[test]
    fn a_key_with_no_text_and_no_case_sends_nothing() {
        // Shift on its own, for instance. Sending no byte is the right thing; sending
        // an empty one would make the `write` wake the program for nothing.
        assert_eq!(key_bytes(Keysym::Shift_L, None, false), None);
    }

    #[test]
    fn the_grid_becomes_a_single_grid_command() {
        // A `Command::Grid`, and not a `Text` per line: it is the node the WIT
        // already has for terminal content. If this becomes N commands, the wasip2
        // buffer with the `terminal` capability is going to need another drawing
        // path.
        let mut g = ronoctua_term::Grid::fresh_one(10, 2);
        g.put_at(0, 0, ronoctua_term::CellBox { c: 'o' });
        g.put_at(1, 0, ronoctua_term::CellBox { c: 'i' });
        let d = grid_list(&g);
        assert_eq!(d.commands.len(), 1);
        match &d.commands[0] {
            ronoctua_protocol::Command::Grid { lines, .. } => {
                // Without the `cursor: r,c` line of the canonical form: that one belongs to
                // the comparator, not to the screen.
                assert_eq!(lines, &vec!["o".to_owned(), "i".to_owned()]);
            }
            otherwise => panic!("it did not become a grid: {otherwise:?}"),
        }
    }

    #[test]
    fn the_terminal_counts_only_the_cells_it_can_draw_in() {
        // THE DEFECT: `paint::spans_of` shifts every run by `CONTENT_INSET`,
        // and this counted columns across the whole window. The last two
        // columns of every terminal were written outside it — and the
        // compositor does not clip, so there was no error anywhere.
        //
        // The assertion is the INVERSE of what `render-png` does to work out a
        // window's size from a cell count, which is what makes the two agree by
        // construction instead of by coincidence.
        let cell_w = tokens::text::size() * tokens::font::CELL_WIDTH_EM;
        let cell_h = tokens::text::size() * tokens::font::LINE_HEIGHT;
        for width in [200_u32, 640, 800, 1280, 1920, 3840] {
            for height in [200_u32, 600, 1080] {
                let (columns, lines) = super::cells_that_fit(width, height);
                let wanted = f32::from(columns + tokens::space::CONTENT_INSET * 2) * cell_w;
                assert!(
                    wanted <= width as f32,
                    "at {width} px the terminal asked for {columns} columns, \
                     which with the margin is {wanted} px"
                );
                let tall = f32::from(lines + tokens::space::CONTENT_INSETY * 2) * cell_h;
                assert!(
                    tall <= height as f32,
                    "at {height} px it asked for {lines} lines"
                );
            }
        }
        // AND NEVER ZERO. A window narrower than the margin is one the
        // compositor may still hand over, and a grid of zero columns is a panic
        // waiting inside the engine.
        let (columns, lines) = super::cells_that_fit(1, 1);
        assert!(columns >= 1 && lines >= 1, "the grid cannot be empty");
    }
}
