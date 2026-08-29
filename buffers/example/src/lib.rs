//! A minimal native buffer — the first one end to end (milestone 7).
//!
//! It exists to prove the boundary, not to be useful: it draws a heading and
//! counts the keys it receives. The glyph browser (section 6.5) uses exactly
//! this interface, with real content.
//!
//! # What this file does NOT import
//!
//! No Wayland, no token value, no translated text — the three of invariant 11.
//! Note that there is no way of doing it even if one wanted to: the WIT's
//! `world` does not offer it.
//!
//! # And it no longer generates its own bindings
//!
//! It is the FIRST buffer through `ronoctua-design`, which is what invariant 12
//! names and what `docs/PENDING.md` said did not exist. Two things follow, and
//! both are the point of the crate:
//!
//!   - the heading is not written here. `Screen::heading` is the ONE heading,
//!     so this file cannot get its air wrong, cannot put a rule under it, and
//!     cannot start its content at a column of its own invention. Four buffers
//!     had four answers to that, which is the report this work came from.
//!   - `wit_bindgen::generate!` is gone, and with it the crate-wide
//!     `#![allow(unsafe_code, missing_docs)]` this file used to need. The
//!     exemption belongs to the one crate that generates bindings, and that is
//!     no longer this one.
//!
//! # Why `std`, and not `no_std`
//!
//! `wasm32-wasip2` has `std`, and a component without it would need its own
//! allocator and panic handler — ceremony to save a few KiB in a binary that is
//! already mapped copy-on-write. What matters for section 5.2's budget is the
//! size of the DATA, not that of the library.

use std::cell::Cell;

use ronoctua_design::guest::Wit;
use ronoctua_design::types::{
    Binding, BufferState, Capability, Color, Command, Context, Descriptor, Effect, Guest, Icon,
    Input, Run, Screen, Tone,
};

thread_local! {
    /// How many keys arrived.
    ///
    /// `thread_local` with a `Cell`, and not `static mut`: a component is
    /// single-threaded by construction, but `static mut` requires `unsafe` on
    /// every access and edition 2024's `static_mut_refs` complains with reason.
    static KEYS: Cell<u32> = const { Cell::new(0) };
}

/// The heading's text, and the only key this buffer needs.
mod key {
    /// Borrowed rather than invented: this buffer draws a heading to prove one
    /// can be drawn, and inventing a sentence for it would owe two `.ftl`
    /// lines that say nothing anybody reads.
    pub const TITLE: &str = "cmd-mx-open";
}

struct Component;

impl Guest for Component {
    fn init() -> Descriptor {
        Descriptor {
            name: "example".into(),
            mode: "example".into(),
            // The keymap as DATA (section 7.9), and only under `SPC m`: the host refuses
            // at load whoever tries to bind under a global prefix.
            keymap: vec![Binding {
                keys: "m r".into(),
                command: "exemplo-reiniciar".into(),
            }],
            // ONE STATE, which is the normal case: a counter is never doing
            // anything other than counting. Every buffer declares at least
            // `normal`, because that is where `Esc` returns to.
            states: vec![BufferState {
                id: "normal".into(),
                letter: 'n',
                tone: Tone::Standard,
            }],
            commands: vec!["exemplo-reiniciar".into()],
            // Nothing in the modeline: this buffer exists to prove the boundary.
            segments: vec![],
            // No capability: it needs neither a PTY nor pixels.
            capabilities: Vec::<Capability>::new(),
        }
    }

    fn render(ctx: Context) -> Vec<Command> {
        let n = KEYS.get();
        let mut screen = Screen::<Wit>::of(&ctx);

        // THE HEADING, AND NOTHING ABOUT IT IS DECIDED HERE. The air above and
        // below, the glyph's column, the title's budget and the row the content
        // starts on all come back from one function, which is the whole reason
        // that function exists.
        let line = screen.heading(Icon::Info, key::TITLE);

        // DATA, not a key: a counter has no translation.
        screen.put(line, Run::datum(&format!("keys: {n}")).quiet());
        screen.finish_wit()
    }

    fn event(e: Input) -> Vec<Effect> {
        // `Cancel` does NOT close. `Effect::Close` exists and stays in the
        // protocol — a buffer that ends by itself needs it —, but tying it to
        // `Esc` would make the key one presses by reflex to leave anything at
        // all DESTROY the buffer.
        //
        // Here it only does not count the key: there is no transient state to
        // cancel in a counter.
        if matches!(e, Input::Cancel) {
            return vec![];
        }
        // BEING SENT TO THE ONE STATE IT HAS IS NOT A KEYSTROKE. It arrives when
        // somebody taps `Esc` anywhere, and counting it would make the counter
        // measure the host's bookkeeping instead of the person's typing.
        if matches!(e, Input::EnterState(_)) {
            return vec![];
        }
        KEYS.set(KEYS.get() + 1);
        vec![Effect::Redraw]
    }
}

ronoctua_design::bindings::export!(Component with_types_in ronoctua_design::bindings);

// `Color` is re-exported by the design crate and this buffer names no colour of
// its own: the heading's tone and the counter's are roles, resolved by the
// palette. The import is kept so a future line here starts from the vocabulary
// rather than from a token name.
const _: Option<Color> = None;
