//! A minimal native buffer — the first one end to end (milestone 7).
//!
//! It exists to prove the boundary, not to be useful: it draws a title and a
//! rule, and counts the keys it receives. The glyph browser (section 6.5) uses
//! exactly this interface, with real content.
//!
//! # What this file does NOT import
//!
//! Node Wayland, no token value, no translated text — the three of invariant 11.
//! Note that there is no way of doing it even if one wanted to: the WIT's
//! `world` does not offer it.
//!
//! # Why `std`, and not `no_std`
//!
//! `wasm32-wasip2` has `std`, and a component without it would need its own
//! allocator and panic handler — ceremony to save a few KiB in a binary that is
//! already mapped copy-on-write. What matters for section 5.2's budget is the
//! size of the DATA, not that of the library.

// The workspace's lints (`unsafe_code`, `missing_docs`) hit the code
// `wit_bindgen::generate!` emits: FFI glue with `export_name`, `unsafe fn` and
// modules with no docs. None of that is ours and none of it is optional — it is
// the target's shape. The exemptions stay here, in the only crate that
// generates bindings, and not in the workspace: the others stay under the rule.
#![allow(unsafe_code, missing_docs)]

use std::cell::Cell;

wit_bindgen::generate!({
    world: "buffer",
    path: "../../crates/ronoctua-protocol/wit",
    // `generate_all` because the world uses a dependency PACKAGE
    // (`ronoctua:tokens`, the enum generated from the tokens). Without this
    // `wit-bindgen` refuses with "missing `with` mapping" and the next error —
    // "cannot find module `exports`" — points at the `use`, not at the cause.
    generate_all,
});

// The types stay in the module of the interface that DEFINES them (`types`);
// in the exported interface's module there is only the `Guest` trait. A `use`
// in the WIT brings the type into the WIT's scope, not into a new Rust module.
use exports::ronoctua::buffer::view::Guest;
use ronoctua::buffer::types::{
    Binding, BufferState, Color, Command, Context, Descriptor, Effect, Input, LineCommand, Scale,
    Text, TextCommand, Tone,
};

thread_local! {
    /// How many keys arrived.
    ///
    /// `thread_local` with a `Cell`, and not `static mut`: a component is
    /// single-threaded by construction, but `static mut` requires `unsafe` on
    /// every access and edition 2024's `static_mut_refs` complains with reason.
    static KEYS: Cell<u32> = const { Cell::new(0) };
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
            // Node capability: it needs neither a PTY nor pixels.
            capabilities: vec![],
        }
    }

    fn render(ctx: Context) -> Vec<Command> {
        let n = KEYS.get();

        let mut out = vec![
            Command::Text(TextCommand {
                line: 0,
                column: 0,
                // A KEY, not a sentence: whoever translates is the host.
                content: Text::Key("cmd-mx-open".into()),
                color: Color::TextPrimary,
                scale: Scale::Normal,
                // Up to the edge: it is the only thing on the line.
                max_width: ctx.columns,
            }),
            // A rule of ONE line: the only separator that exists besides white space
            // (section 5.1).
            Command::Rule(LineCommand {
                line: 1,
                color: Color::RuleDefault,
            }),
        ];

        // DATA, not a key: a counter has no translation.
        if ctx.lines > 2 {
            out.push(Command::Text(TextCommand {
                line: 2,
                column: 0,
                content: Text::Data(format!("teclas: {n}")),
                color: Color::TextSecondary,
                scale: Scale::Normal,
                max_width: ctx.columns,
            }));
        }
        out
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

export!(Component);
