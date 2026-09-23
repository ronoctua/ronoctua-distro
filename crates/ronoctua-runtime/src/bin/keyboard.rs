//! Exercises the keyboards — `LEADER s k`.
//!
//! # Why a binary, and not a `#[test]` in the buffer
//!
//! The component compiles to `wasm32-wasip2` and lives in a workspace of its
//! own (invariant 12): a `cargo test` of it tests the logic OUTSIDE the
//! component, which is precisely what proves nothing. Here it is loaded the way
//! the host loads it, pushed the keyboards the way the host pushes them, and
//! read through `render`.
//!
//! # What only this can prove
//!
//! The screen is EMPTY until the host speaks, and what it speaks is a record
//! with three lists inside — the keyboards, the keys and the meanings — lowered
//! field by field. `wasmtime` compares field names only AFTER zipping them
//! positionally, so a field out of order fails here, with a message, instead of
//! on somebody's screen.
//!
//! And one thing that is this buffer's alone: what it hands BACK. A remap
//! carries three strings, and every one of them has to be a string the host
//! sent — the host refuses anything else, so a buffer that invented one would
//! be a screen where the arrows do nothing at all.
//!
//! Usage: `ronoctua-test-keyboard <component.wasm> <i18n-dir>`

use anyhow::{Context as _, Result, bail};
use ronoctua_protocol::{
    Board, Boards, Effect, Input, KeyboardMode, LayoutWord, UnitBoard, UnitSetting, Word,
};

/// The labels this buffer cuts ON PURPOSE, and why.
const ON_PURPOSE: &[(&str, &str)] = &[
    (
        "keyboard-global",
        "the sentence that says the layout and the repeat rate are not per \
         keyboard. It is a paragraph under the list, and it is meant to wrap \
         rather than to fit on one line.",
    ),
    (
        "keyboard-independent-note",
        "what Independent mode means, under the list — a sentence, meant to \
         wrap like the one about the global settings.",
    ),
    (
        "keyboard-keep-note",
        "the question's sentence after a switch of mode, a line of its own \
         under the title and above the countdown.",
    ),
    (
        "keyboard-same-model",
        "the warning that two keyboards of one model answer to one rule. Same \
         shape as the one above, and it appears under the keys.",
    ),
];

fn heights() -> ronoctua_protocol::Heights {
    ronoctua_protocol::Heights::from_sizes(15.0, 18.0, 20.0, 40.0, 116.0, 1.45)
}

const ROWS: u16 = 40;
const COLUMNS: u16 = 100;

/// This machine's own two keyboards, as the host sends them.
///
/// The vendor and product pairs are real (a laptop's i8042 keyboard and a
/// Bluetooth one); nothing here identifies anybody (invariant 13).
fn boards() -> Boards {
    let word = |keyd: &str, text: &str, holds: bool| Word {
        keyd: keyd.to_owned(),
        text: text.to_owned(),
        holds,
    };
    Boards {
        keys: vec![
            word("capslock", "keyboard-key-capslock", false),
            word("leftalt", "keyboard-key-leftalt", false),
            word("leftmeta", "keyboard-key-leftmeta", false),
        ],
        choices: vec![
            word("", "keyboard-choice-default", false),
            word("esc", "keyboard-choice-esc", false),
            word("control", "keyboard-choice-control", false),
            word(
                "overload(control, esc)",
                "keyboard-choice-control-esc",
                true,
            ),
        ],
        boards: vec![
            Board {
                id: "0001:0001".to_owned(),
                name: "AT Translated Set 2 keyboard".to_owned(),
                built_in: true,
                decided: Vec::new(),
            },
            Board {
                id: "3554:f605".to_owned(),
                name: "Bluetooth Keyboard".to_owned(),
                built_in: false,
                decided: vec![("capslock".to_owned(), "esc".to_owned())],
            },
        ],
        trouble: String::new(),
        leader: "leftmeta".to_owned(),
        ..Boards::default()
    }
}

/// Renders at three widths and refuses anything that collides.
fn look(inst: &mut ronoctua_runtime::Instance) -> Result<ronoctua_protocol::DisplayList> {
    let list = inst.render(COLUMNS, ROWS, heights())?;
    ronoctua_runtime::driven::note(&list);
    for columns in [46, 80, COLUMNS] {
        let at = if columns == COLUMNS {
            list.clone()
        } else {
            inst.render(columns, ROWS, heights())?
        };
        if let Err(e) = at.nothing_overlaps(heights(), columns) {
            bail!("at {columns} columns, {e}");
        }
        if let Err(e) = at.nothing_is_covered(heights(), columns) {
            bail!("at {columns} columns, {e}");
        }
    }
    Ok(list)
}

/// Draws a frame as text, for reading with `--draw`.
///
/// A buffer has no screenshot path of its own — `nix run .#png` renders one
/// with no host speaking, which for this buffer is a heading over nothing. So
/// the frame is laid out here in the cells the runs claim, which is the same
/// arithmetic the host would rasterise.
///
/// IT DRAWS THE KEYS AND NOT THE SENTENCES, because the host is what resolves
/// them — so a run reads as `[keyboard-built-in]`, which is longer than
/// `part of this machine` and will appear to collide with its neighbour at a
/// narrow width. What answers that question is `nothing_overlaps`, above, which
/// measures the BUDGETS. This is for looking at the shape.
fn draw(list: &ronoctua_protocol::DisplayList, columns: u16, what: &str) {
    println!("\n== {what} ==");
    let mut lines: std::collections::BTreeMap<u16, Vec<(u16, String)>> = Default::default();
    for command in &list.commands {
        if let ronoctua_protocol::Command::Text {
            line, column, text, ..
        } = command
        {
            let said = match text {
                ronoctua_protocol::Text::Data(t) => t.clone(),
                ronoctua_protocol::Text::Key(k) => format!("[{k}]"),
                ronoctua_protocol::Text::Icon(i) => format!("<{i}>"),
            };
            lines.entry(*line).or_default().push((*column, said));
        }
    }
    for (n, mut runs) in lines {
        runs.sort_by_key(|(at, _)| *at);
        let mut out = vec![' '; usize::from(columns) + 40];
        for (at, said) in runs {
            for (i, c) in said.chars().enumerate() {
                let cell = usize::from(at) + i;
                if cell < out.len() {
                    out[cell] = c;
                }
            }
        }
        println!("{n:>2}|{}|", out.into_iter().collect::<String>().trim_end());
    }
}

/// The text keys a frame drew, in order.
fn keys(list: &ronoctua_protocol::DisplayList) -> Vec<String> {
    ronoctua_runtime::driven::keys_of(list)
        .into_iter()
        .map(|(k, _)| k)
        .collect()
}

fn main() -> Result<()> {
    let path = std::env::args()
        .nth(1)
        .context("usage: ronoctua-test-keyboard <component.wasm> <i18n-dir>")?;
    let i18n = std::env::args()
        .nth(2)
        .context("usage: ronoctua-test-keyboard <component.wasm> <i18n-dir>")?;
    let wasm = std::fs::read(&path).with_context(|| format!("could not read {path}"))?;

    let rt = ronoctua_runtime::Runtime::fresh()?;
    let cwasm = rt.precompile(&wasm)?;
    let component = rt.load(&cwasm)?;
    let mut inst = rt.open(&component)?;

    let d = inst.init()?;
    ronoctua_runtime::driven::hand_phrases(&mut inst, &d, &i18n, "en-GB")?;
    if d.mode != "keyboard" {
        bail!("the buffer's mode is `{}`; the modeline shows that", d.mode);
    }
    // WITHOUT THE CAPABILITY THE SCREEN IS EMPTY, and the host would never
    // read the kernel. It is the one line of the descriptor this buffer
    // cannot lose.
    if !d
        .capabilities
        .contains(&ronoctua_protocol::Capability::Keyboard)
    {
        bail!("the buffer does not declare `keyboard`: the host would never read a thing");
    }
    if let Err(e) = d.validate() {
        bail!("the descriptor does not pass the section 7.9 guard: {e}");
    }
    for id in &d.commands {
        if !d.keymap.iter().any(|l| l.command == *id) {
            bail!("`{id}` has no key: nobody reaches it through which-key");
        }
    }
    for language in ronoctua_runtime::driven::languages(&i18n)? {
        let table = ronoctua_runtime::driven::ftl(&format!("{i18n}/{language}.ftl"))?;
        for id in &d.commands {
            let key = format!("cmd-{id}");
            if !table.contains_key(&key) {
                bail!("`{key}` is missing from {language}.ftl — SPC m would show [{key}]");
            }
        }
    }

    // ---- BEFORE THE HOST SPEAKS ----
    //
    // A heading and nothing else. It is the state a window opens in, for the
    // instant before the kernel has been read, and it has to be a screen and
    // not a panic.
    let empty = look(&mut inst)?;
    if !keys(&empty).iter().any(|k| k == "keyboard-title") {
        bail!("the buffer drew no heading before the keyboards arrived");
    }

    let drawing = std::env::args().any(|a| a == "--draw");

    // ---- THE LIST ----
    inst.event(&Input::Keyboards(boards()))?;
    let list = look(&mut inst)?;
    if drawing {
        draw(&list, COLUMNS, "the list of keyboards");
        draw(
            &inst.render(46, ROWS, heights())?,
            46,
            "the same, at 46 columns",
        );
    }
    let said = keys(&list);
    for wanted in ["keyboard-title", "keyboard-built-in", "keyboard-attached"] {
        if !said.iter().any(|k| k == wanted) {
            bail!("the list does not say `{wanted}`: {said:?}");
        }
    }
    // THE SENTENCE THAT SAYS WHAT THIS SCREEN CANNOT DO. A page that offered
    // only remapping and stayed quiet about the layout would read as a page
    // that had forgotten it.
    if !said.iter().any(|k| k == "keyboard-global") {
        bail!("the list does not say that the layout and the repeat rate are global");
    }

    // ---- THE SWITCH IS ROW ZERO ----
    //
    // Where the cursor starts, and `RET` there asks for the other mode — the
    // host makes it provisional; the buffer only asks.
    if !said.iter().any(|k| k == "keyboard-mode") {
        bail!("the list does not draw the switch between the two modes");
    }
    let effects = inst.event(&Input::Confirm)?;
    if !effects
        .iter()
        .any(|e| matches!(e, Effect::KeyboardMode(KeyboardMode::Independent)))
    {
        bail!("`RET` on the switch did not ask for Independent mode: {effects:?}");
    }

    // ---- ONE KEYBOARD ----
    //
    // One row down is the first keyboard, the built-in one; `RET` opens it and
    // the state changes, which is what the modeline draws.
    inst.event(&Input::Down)?;
    let effects = inst.event(&Input::Confirm)?;
    if !effects
        .iter()
        .any(|e| matches!(e, Effect::StateChanged(id) if id == "keyboard-editing"))
    {
        bail!("opening a keyboard did not report the state: the modeline would lie");
    }
    let keys_screen = look(&mut inst)?;
    if drawing {
        draw(
            &keys_screen,
            COLUMNS,
            "one keyboard, and what each key does",
        );
    }
    let said = keys(&keys_screen);
    for wanted in ["keyboard-one", "keyboard-key-capslock"] {
        if !said.iter().any(|k| k == wanted) {
            bail!("one keyboard's page does not say `{wanted}`: {said:?}");
        }
    }

    // ---- THE ONE KEY THAT COSTS SOMETHING TO MOVE ----
    //
    // Every file the shell renders claims the leader's key, and a rule written
    // after it wins — so remapping it is how somebody loses the key that opens
    // everything, on that keyboard. The crate's comment says the screen warns
    // before that happens; this is what makes that sentence true.
    let sent = boards();
    let at = sent
        .keys
        .iter()
        .position(|w| w.keyd == sent.leader)
        .context("the fixture offers the leader's key")?;
    for _ in 0..at {
        inst.event(&Input::Down)?;
    }
    let on_the_leader = look(&mut inst)?;
    if drawing {
        draw(
            &on_the_leader,
            COLUMNS,
            "the cursor on the leader's own key",
        );
    }
    if !keys(&on_the_leader)
        .iter()
        .any(|k| k == "keyboard-is-the-leader")
    {
        bail!(
            "the cursor is on `{}`, which IS the leader, and the screen does not say so",
            sent.leader
        );
    }
    // AND IT SAYS SO ONLY THERE. A warning on every row is a warning nobody
    // reads.
    for _ in 0..at {
        inst.event(&Input::Up)?;
    }
    if keys(&look(&mut inst)?)
        .iter()
        .any(|k| k == "keyboard-is-the-leader")
    {
        bail!("the leader's warning is drawn on a key that is not the leader");
    }

    // ---- THE RING, AND WHAT IT HANDS BACK ----
    //
    // THE THREE STRINGS HAVE TO BE STRINGS THE HOST SENT. It refuses anything
    // else — so a buffer that built one out of its own head would be a screen
    // where the arrows do nothing, silently.
    let effects = inst.event(&Input::Right)?;
    let Some(Effect::Remap(remap)) = effects
        .iter()
        .find(|e| matches!(e, Effect::Remap(_)))
        .cloned()
    else {
        bail!("the right arrow on a key asked for nothing: {effects:?}");
    };
    if !sent.boards.iter().any(|b| b.id == remap.device) {
        bail!("`{}` is not a keyboard the host sent", remap.device);
    }
    if !sent.keys.iter().any(|w| w.keyd == remap.key) {
        bail!("`{}` is not a key the host offered", remap.key);
    }
    if !sent.choices.iter().any(|w| w.keyd == remap.becomes) {
        bail!("`{}` is not a meaning the host offered", remap.becomes);
    }
    // AND IT MOVED ONE STEP, from "leave it alone" to the next one. The
    // built-in keyboard has nothing decided, so the ring starts at the
    // beginning.
    if remap.becomes != sent.choices[1].keyd {
        bail!(
            "the ring went to `{}` instead of `{}`",
            remap.becomes,
            sent.choices[1].keyd
        );
    }

    // AND THE OTHER WAY ROUND WRAPS. From the first choice, left is the last
    // one — a ring whose way back is only forwards is a ring nobody can leave.
    let effects = inst.event(&Input::Left)?;
    let Some(Effect::Remap(back)) = effects
        .iter()
        .find(|e| matches!(e, Effect::Remap(_)))
        .cloned()
    else {
        bail!("the left arrow on a key asked for nothing");
    };
    if back.becomes != sent.choices[sent.choices.len() - 1].keyd {
        bail!(
            "the ring does not wrap backwards: it went to `{}`",
            back.becomes
        );
    }

    // ---- WHAT WENT WRONG IS DRAWN ----
    //
    // A refusal nobody sees is a key that did nothing. The host puts the
    // reason in the record; the buffer has to draw it.
    let mut troubled = boards();
    troubled.trouble = "keyboard-trouble-refused".to_owned();
    inst.event(&Input::Keyboards(troubled))?;
    let list = look(&mut inst)?;
    if !keys(&list).iter().any(|k| k == "keyboard-trouble-refused") {
        bail!("the buffer does not draw why the last write was refused");
    }

    // ---- ESCAPE GOES BACK, AND NEVER CLOSES ----
    let effects = inst.event(&Input::Cancel)?;
    if effects.iter().any(|e| matches!(e, Effect::Close)) {
        bail!("`Esc` closed the buffer: the key one presses by reflex must not destroy it");
    }
    let list = look(&mut inst)?;
    if !keys(&list).iter().any(|k| k == "keyboard-title") {
        bail!("`Esc` did not go back to the list");
    }

    // ---- INDEPENDENT MODE: UNITS, NOT MODELS ----
    let mut independent = boards();
    independent.mode = KeyboardMode::Independent;
    independent.cursors = vec![1, 2, 3];
    independent.layouts = vec![
        LayoutWord {
            code: "gb".to_owned(),
            name: "English (UK)".to_owned(),
            variants: vec!["extd".to_owned()],
        },
        LayoutWord {
            code: "fr".to_owned(),
            name: "French".to_owned(),
            variants: Vec::new(),
        },
    ];
    let unit = |identity: &str, name: &str, plugged: bool, cursor: u16| UnitBoard {
        identity: identity.to_owned(),
        name: name.to_owned(),
        model: "3554:f605".to_owned(),
        built_in: false,
        plugged,
        cursor,
        layout: String::new(),
        variant: String::new(),
        leader: "leftmeta".to_owned(),
        repeat_rate: 0,
        repeat_delay: 0,
        decided: Vec::new(),
    };
    independent.units = vec![
        unit("0a1b2c3d", "Bluetooth Keyboard", true, 2),
        unit("4e5f6a7b", "USB Keyboard", false, 1),
    ];
    inst.event(&Input::Keyboards(independent.clone()))?;
    let list = look(&mut inst)?;
    if drawing {
        draw(&list, COLUMNS, "Independent mode: every unit");
    }
    let said = keys(&list);
    for wanted in [
        "keyboard-independent-note",
        "keyboard-unplugged",
        "keyboard-cursor",
    ] {
        if !said.iter().any(|k| k == wanted) {
            bail!("Independent mode's list does not say `{wanted}`: {said:?}");
        }
    }
    // FROM THE TOP, whatever the cursor was on: row zero is the switch and
    // the first unit is one below it.
    for _ in 0..4 {
        inst.event(&Input::Up)?;
    }
    inst.event(&Input::Down)?;
    inst.event(&Input::Confirm)?;
    let page = look(&mut inst)?;
    if drawing {
        draw(&page, COLUMNS, "one unit, and its settings");
    }
    let said = keys(&page);
    for wanted in [
        "keyboard-unit",
        "keyboard-types-through",
        "keyboard-layout",
        "keyboard-leader-key",
        "keyboard-repeat-rate",
    ] {
        if !said.iter().any(|k| k == wanted) {
            bail!("a unit's page does not say `{wanted}`: {said:?}");
        }
    }
    // THE CURSOR RING HANDS BACK A CURSOR THE HOST OFFERED, for a unit it sent.
    let effects = inst.event(&Input::Right)?;
    let Some(Effect::UnitChange(change)) = effects
        .iter()
        .find(|e| matches!(e, Effect::UnitChange(_)))
        .cloned()
    else {
        bail!("the right arrow on a unit's cursor asked for nothing: {effects:?}");
    };
    if change.unit != "0a1b2c3d" || change.setting != UnitSetting::Cursor(3) {
        bail!("the cursor ring handed back {change:?}");
    }

    // ---- THE QUESTION AFTER A SWITCH ----
    let mut asking = independent;
    asking.asking = 12;
    inst.event(&Input::Keyboards(asking))?;
    let question = look(&mut inst)?;
    if drawing {
        draw(&question, COLUMNS, "the question after a switch of mode");
    }
    let said = keys(&question);
    for wanted in ["keyboard-keep-title", "keyboard-go-back", "keyboard-keep"] {
        if !said.iter().any(|k| k == wanted) {
            bail!("the question does not say `{wanted}`: {said:?}");
        }
    }
    // AND THE FIRST `RET` GOES BACK: a question that appears by itself
    // answers NO.
    let effects = inst.event(&Input::Confirm)?;
    if !effects
        .iter()
        .any(|e| matches!(e, Effect::KeyboardMode(KeyboardMode::Together)))
    {
        bail!("the first `RET` on the question did not go back: {effects:?}");
    }

    ronoctua_runtime::driven::every_label_fits(&i18n, ON_PURPOSE, COLUMNS)?;
    println!(
        "OK: the keyboards draw, the ring turns both ways, and it hands back only what it was given"
    );
    Ok(())
}
