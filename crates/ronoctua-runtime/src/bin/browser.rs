//! Exercises the glyph browser's five filters — section 6.5.
//!
//! # Why a binary, and not a `#[test]` in the buffer
//!
//! The buffer compiles to `wasm32-wasip2` and lives in a workspace of its own
//! (invariant 12): a `cargo test` of it would need a wasm runner, and
//! would test the logic **outside** the component — precisely what proves nothing.
//! Here the component is loaded the way the host loads it, driven by
//! `event` and read by `render`. If the filter only worked in native Rust,
//! this binary would not pass.
//!
//! # What it asserts
//!
//! One per section 6.5 criterion: source set, Unicode block, tag, "in use
//! as a token" and "assigned to an app id". Plus the descriptor, because a filter
//! that is not in the keymap is reachable by nobody.
//!
//! Usage: `ronoctua-test-browser <component.wasm>`

use anyhow::{Context as _, Result, bail};
use ronoctua_protocol::{Command, Input, Text};

/// The scale heights the production host sends.
///
/// The numbers are the tokens' (`typography.nix`); the ARITHMETIC is the same
/// `Heights::from_sizes` the host uses, and it is what this test exercises —
/// repeating the formula here would give two calculations that can diverge.
fn heights() -> ronoctua_protocol::Heights {
    ronoctua_protocol::Heights::from_sizes(15.0, 18.0, 20.0, 40.0, 116.0, 1.45)
}

/// The test window's height. Tall so `render` emits many rows — and
/// the list is virtualised, so only what fits is emitted.
const ROWS: u16 = 60;

/// The filter's icon token, as it arrives from the component.
///
/// The buffer names it through the enum GENERATED from the tokens (`Icon::Filter`), and the
/// runtime delivers it by its WIT name. Writing the name here is what ties the
/// two ends together: if the token disappears from `assets/tokens/icons.nix`, the component
/// does not even compile; if it changes name, this test does not find it.
const FILTER_ICON: &str = "filter";

/// The prefix of the sets' text keys, the same one the buffer assembles.
const SET_PREFIX_OF: &str = "glyphs-set-";

/// The command that opens and closes the panel.
const PANEL: &str = "glyphs-filter-panel";

/// What is seen on screen after a query.
struct Vision {
    /// How many results the header says there are.
    total_filtered: usize,
    /// How many glyphs there are in total.
    total_in_db: usize,
    /// The FOCUSED glyph's name, from the last row.
    ///
    /// The grid does not show one name per glyph — it shows the focused one, below. Reading
    /// "every visible name" was a thing of the list; here one walks the grid
    /// and reads the focus at each step, which as a bonus exercises the navigation.
    focus: Option<String>,
    /// The visible glyphs, in reading order.
    glyphs: Vec<char>,
    /// The focused glyph's TAGS, as they come out on screen. `None` when they did not fit.
    tags: Option<String>,
    /// Where each visible glyph was DRAWN, in the same order.
    ///
    /// It is what makes it possible to test the click without repeating the component's grid
    /// geometry here: one clicks where it said it put the glyph. A copy of the
    /// arithmetic (`STEP`, `HEIGHT`, starting row) would lie the day the
    /// header gained a line.
    positions: Vec<(u16, u16)>,
}

/// Reads the display list: the header on row 0, the grid below, the focus on the last.
fn read(list: &ronoctua_protocol::DisplayList) -> Result<Vision> {
    // `(column, text)`: the piece with the largest column on row 0 is the counter.
    let mut header: Option<(u16, String)> = None;
    // A `BTreeMap` so it comes out in reading order: the component emits row by
    // row, but depending on that would tie the test to the emission order.
    let mut cells: std::collections::BTreeMap<(u16, u16), String> =
        std::collections::BTreeMap::new();

    for c in &list.commands {
        let Command::Text {
            line, column, text, ..
        } = c
        else {
            continue;
        };
        let Text::Data(s) = text else { continue };
        if *line == 0 {
            // ROW 0 HAS THREE PIECES: the magnifying glass (which is a `Text::Icon` and does not
            // get here), what was typed, and the COUNTER flush
            // right. The one with the largest column is the counter — and it is the one this
            // driver reads, because it is the one that says how many matched.
            header = Some(match header {
                Some((col, _)) if col > *column => (col, header.unwrap().1),
                _ => (*column, s.clone()),
            });
        } else {
            cells.insert((*line, *column), s.clone());
        }
    }

    // THE LAST ROW HAS THREE RUNS, in column order: the focused GLYPH at
    // icon scale, its NAME, and the TAGS in whatever is left.
    //
    // There used to be two — `<glyph>  <name>` in a single string — and the glyph moved
    // into a run of its own when it gained the icon scale: next to the name it
    // inherited the text's size.
    //
    // Reading the row's last run (which is what this driver did) started returning
    // a TAG where the name was expected — and the symptom was the source-set filter
    // "letting through" a glyph that was never wrong. What decides is the
    // COLUMN, and that is why the reading is positional.
    let last_line = cells.keys().next_back().map(|(l, _)| *l);
    let mut focus = None;
    let mut tags = None;
    if let Some(l) = last_line {
        let on_line: Vec<(u16, String)> = cells
            .range((l, 0)..=(l, u16::MAX))
            .map(|((_, c), s)| (*c, s.clone()))
            .collect();
        for (column, _) in &on_line {
            cells.remove(&(l, *column));
        }
        let mut it = on_line.into_iter();
        // The glyph. It is not read — what this driver compares is the NAME — but
        // skipping its position is what makes the name the name.
        let _ = it.next();
        focus = it.next().map(|(_, name)| name);
        tags = it.next().map(|(_, t)| t);
    }

    let (positions, glyphs): (Vec<_>, Vec<_>) = cells
        .iter()
        .filter_map(|(k, s)| s.chars().next().map(|c| (*k, c)))
        .unzip();

    // THE COUNTER: `N / M`, flush against the right edge.
    //
    // It used to be `/search  (N of M)` stuck on the left. It changed because the counter
    // competed for the eye with what matters on screen — the icons' shape — and
    // because the `/` became a actual magnifying glass, which is an icon token.
    let (_, cab) = header.context("`render` emitted no counter on line 0")?;
    let (n, m) = cab
        .split_once('/')
        .with_context(|| format!("the counter is not in the `N / M` shape: {cab}"))?;

    Ok(Vision {
        total_filtered: n.trim().parse().context("the result count")?,
        total_in_db: m.trim().parse().context("the database total")?,
        focus,
        tags,
        glyphs,
        positions,
    })
}

/// Moves `steps` to the right, collecting the focused name at each stop.
///
/// It is what replaces the list's "read every visible name": it samples the
/// actual grid AND proves that lateral navigation works.
fn names_while_stepping(
    inst: &mut ronoctua_runtime::Instance,
    steps: usize,
) -> Result<Vec<String>> {
    let mut out = Vec::new();
    if let Some(n) = read(&inst.render(80, ROWS, heights())?)?.focus {
        out.push(n);
    }
    for _ in 0..steps {
        if inst.event(&Input::Right)?.is_empty() {
            break; // reached the end of the list
        }
        if let Some(n) = read(&inst.render(80, ROWS, heights())?)?.focus {
            out.push(n);
        }
    }
    Ok(out)
}

/// Types a query from scratch and returns what was left on screen.
fn query(inst: &mut ronoctua_runtime::Instance, q: &str) -> Result<Vision> {
    clear(inst)?;
    for c in q.chars() {
        inst.event(&Input::Char(c))?;
    }
    read(&inst.render(80, ROWS, heights())?)
}

/// Fires a named command and returns what was left on screen.
fn run_by_id(inst: &mut ronoctua_runtime::Instance, id: &str) -> Result<Vision> {
    let effects = inst.event(&Input::Command(id.to_owned()))?;
    if effects.is_empty() {
        bail!("the command `{id}` asked for no effect — did it reach the buffer?");
    }
    read(&inst.render(80, ROWS, heights())?)
}

/// Clears the search through the clear command.
fn clear(inst: &mut ronoctua_runtime::Instance) -> Result<()> {
    inst.event(&Input::Command("glyphs-filter-clear".to_owned()))?;
    Ok(())
}

fn main() -> Result<()> {
    let path = std::env::args()
        .nth(1)
        .context("usage: ronoctua-test-browser <component.wasm> <i18n-dir>")?;
    let i18n = std::env::args()
        .nth(2)
        .context("usage: ronoctua-test-browser <component.wasm> <i18n-dir>")?;
    let wasm = std::fs::read(&path).with_context(|| format!("could not read {path}"))?;

    let rt = ronoctua_runtime::Runtime::fresh()?;
    let cwasm = rt.precompile(&wasm)?;
    let component = rt.load(&cwasm)?;
    let mut inst = rt.open(&component)?;

    // ---- the descriptor: a filter outside the keymap is not reachable ----
    let d = inst.init()?;

    // THE PHRASES, exactly as the host hands them over at load. Without this a
    // buffer that holds interface text would be driven against its fallback,
    // and the screen this test reads would not be the screen anybody sees.
    // en-GB is the canonical one; the pt-BR round trip is checked separately.
    ronoctua_runtime::driven::hand_phrases(&mut inst, &d, &i18n, "en-GB")?;
    for id in [
        "glyphs-filter-set",
        "glyphs-filter-block",
        "glyphs-filter-tag",
        "glyphs-filter-token",
        "glyphs-filter-app",
        PANEL,
    ] {
        if !d.commands.iter().any(|c| c == id) {
            bail!("`{id}` is not in the descriptor's command list");
        }
        if !d.keymap.iter().any(|l| l.command == id) {
            bail!("`{id}` has no key: nobody reaches it through which-key");
        }
    }
    // The host refuses whoever binds outside `SPC m`; better to find out here.
    if let Err(e) = d.validate() {
        bail!("the browser's descriptor does not pass the section 7.9 guard: {e}");
    }
    // EVERY COMMAND HAS A KEY IN BOTH LANGUAGES. The buffer writes the `cmd-`
    // prefix as a literal because it does not depend on the host's crate
    // (invariant 11) — this is what stops the two from diverging. Without it
    // which-key shows `[cmd-glyphs-copy-glyph]`, the usual silent fallback.
    for language in ["pt-BR", "en-GB"] {
        let table = ftl(&format!("{i18n}/{language}.ftl"))?;
        for id in &d.commands {
            let key = format!("cmd-{id}");
            if !table.contains_key(&key) {
                bail!("`{key}` does not exist in {language}.ftl");
            }
        }
    }
    println!(
        "descriptor {} commands, all with a key and a text key",
        d.commands.len()
    );

    let base = query(&mut inst, "")?;
    if base.total_filtered != base.total_in_db {
        bail!(
            "an empty search should show the whole database: {} of {}",
            base.total_filtered,
            base.total_in_db
        );
    }
    // THE DATABASE MUST HAVE BEEN PARSED. The buffer has no JSON parser
    // (invariant 11) and splits the database on punctuation; when the generator puts a
    // space between the objects, it finds ONE record of 2.8 MB — with no error,
    // no log, and the list showing one item. That is what happened, and
    // no check saw it, because none looked at the RESULT.
    if base.total_in_db < 10_000 {
        bail!(
            "the database has {} glyphs; there are 10,473. The buffer's parser did not split \
             os registros — ver o `split` em `carregar`",
            base.total_in_db
        );
    }
    println!("database: {} glyphs", base.total_in_db);

    // ---- 1. source set ----
    let fa = query(&mut inst, "fa:")?;
    if fa.total_filtered == 0 || fa.total_filtered >= base.total_in_db {
        bail!(
            "`fa:` did not narrow: {} out of {}",
            fa.total_filtered,
            base.total_in_db
        );
    }
    let names = names_while_stepping(&mut inst, 30)?;
    if names.is_empty() {
        bail!("`fa:` showed no name at all in focus");
    }
    if let Some(bad) = names.iter().find(|n| !n.starts_with("fa-")) {
        bail!("`fa:` let a glyph of another set through: {bad}");
    }
    println!("set       walked over  -> {} names, all fa-", names.len());
    println!("set       `fa:`        -> {}", fa.total_filtered);

    // The free term applies WITHIN the set. If the filter swallowed the search,
    // this count would equal the one above — and nobody would notice.
    let fa_folder = query(&mut inst, "fa:folder")?;
    if fa_folder.total_filtered == 0 || fa_folder.total_filtered >= fa.total_filtered {
        bail!(
            "`fa:folder` did not narrow within the set: {} against {}",
            fa_folder.total_filtered,
            fa.total_filtered
        );
    }
    let names = names_while_stepping(&mut inst, 30)?;
    if let Some(bad) = names
        .iter()
        .find(|n| !n.starts_with("fa-") || !n.contains("folder"))
    {
        bail!("`fa:folder` deixou passar {bad}");
    }
    println!("set       `fa:folder`  -> {}", fa_folder.total_filtered);

    // ---- 2. Unicode block ----
    let block = query(&mut inst, "bloco:f0")?;
    if block.total_filtered == 0 || block.total_filtered >= base.total_in_db {
        bail!("`bloco:f0` did not narrow: {}", block.total_filtered);
    }
    if let Some(bad) = block
        .glyphs
        .iter()
        .find(|c| !format!("{:x}", **c as u32).starts_with("f0"))
    {
        bail!("`bloco:f0` let U+{:04X} through", *bad as u32);
    }
    println!("block     `bloco:f0`   -> {}", block.total_filtered);

    // ---- 3. tag ----
    let tag = query(&mut inst, "tag:folder")?;
    if tag.total_filtered == 0 || tag.total_filtered >= base.total_in_db {
        bail!("`tag:folder` did not narrow: {}", tag.total_filtered);
    }
    // The tag is bilingual (section 6.5): the same query in pt has to find
    // something. If only English worked, the curated overlay would not be being
    // read — and that would go unnoticed in every English search.
    let tag_pt = query(&mut inst, "tag:pasta")?;
    if tag_pt.total_filtered == 0 {
        bail!("`tag:pasta` found nothing: the pt-BR tags did not arrive");
    }
    println!(
        "tag       `tag:folder` -> {}   `tag:pasta` -> {}",
        tag.total_filtered, tag_pt.total_filtered
    );

    // ---- 4. in use as a token, through the NAMED command ----
    let tok = run_by_id(&mut inst, "glyphs-filter-token")?;
    if tok.total_filtered == 0 || tok.total_filtered >= base.total_in_db {
        bail!("`token:` did not narrow: {}", tok.total_filtered);
    }
    println!("token     command      -> {}", tok.total_filtered);

    // ---- 5. assigned to an app id, through the NAMED command ----
    let app = run_by_id(&mut inst, "glyphs-filter-app")?;
    if app.total_filtered == 0 {
        bail!("`app:` found nothing: the tray table did not reach the database");
    }
    // Every glyph assigned to an app id IS an icon token — the tray
    // table points at TOKENS, never at a raw glyph (a `gen-glyphdb.py`
    // guard). If this subset grew, it would be a sign that
    // one of the two fields had stopped being computed.
    if app.total_filtered > tok.total_filtered {
        bail!(
            "more glyphs assigned to an app id ({}) than tokens ({})",
            app.total_filtered,
            tok.total_filtered
        );
    }
    println!("app id    command      -> {}", app.total_filtered);

    // ---- the filter command takes the value from the focused glyph ----
    // Without this, `m f c` would have to come with a set written into the code — and the
    // filter would only serve that set.
    clear(&mut inst)?;
    for c in "md-".chars() {
        inst.event(&Input::Char(c))?;
    }
    let set_filter = run_by_id(&mut inst, "glyphs-filter-set")?;
    let names = names_while_stepping(&mut inst, 20)?;
    if let Some(bad) = names.iter().find(|n| !n.starts_with("md-")) {
        bail!("`glyphs-filter-set` did not take the set of the focused glyph: {bad}");
    }
    println!("set       of the focus -> {}", set_filter.total_filtered);

    // ---- clearing returns the whole database ----
    let back = query(&mut inst, "")?;
    if back.total_filtered != base.total_in_db {
        bail!(
            "clearing did not bring back the whole database: {} of {}",
            back.total_filtered,
            base.total_in_db
        );
    }

    // ---- the CLICK lands on the glyph that was drawn there ----
    //
    // The click reaches the component as a CELL (invariant 11: a pixel is a Wayland
    // thing). What resolves which glyph is in the cell is the buffer itself,
    // because only it knows its own grid — and this test does NOT repeat that
    // arithmetic: it clicks where `render` said it put the glyph.
    clear(&mut inst)?;
    let screen = read(&inst.render(80, ROWS, heights())?)?;
    // The fourth visible glyph: far enough from the origin for a one-cell or
    // one-row error to show, and close enough to fit in
    // any grid.
    const TARGET: usize = 3;
    let (line, column) = *screen
        .positions
        .get(TARGET)
        .context("the render did not emit enough glyphs to test the click")?;

    // The expected name, obtained by WALKING there — the path already known to
    // work. Comparing the click against the keyboard is what makes the test about
    // the click, and not about the glyph database.
    let running = names_while_stepping(&mut inst, TARGET)?;
    let expected = running
        .get(TARGET)
        .context("the sideways navigation did not reach the target")?
        .clone();

    clear(&mut inst)?;
    if inst.event(&Input::Click { column, line })?.is_empty() {
        bail!("the click at ({column},{line}) asked for no redraw — did it reach the buffer?");
    }
    let after = read(&inst.render(80, ROWS, heights())?)?;
    let focus = after.focus.context("no glyph in focus after the click")?;
    if focus != expected {
        bail!("the click at ({column},{line}) focused `{focus}`, and `{expected}` was drawn there");
    }
    println!("click    ({column},{line})       -> {focus}");

    // ---- and a click OUTSIDE the grid does not move the cursor ----
    //
    // Row 0 is the search header. Without this guard, clicking to focus the
    // search field would jump the cursor to the first glyph — a movement
    // nobody asked for, in the middle of something else.
    if !inst.event(&Input::Click { column: 0, line: 0 })?.is_empty() {
        bail!("a click on the header moved the cursor");
    }
    let stopped = read(&inst.render(80, ROWS, heights())?)?;
    if stopped.focus.as_deref() != Some(focus.as_str()) {
        bail!(
            "the click on the header moved the focus from `{focus}` to {:?}",
            stopped.focus
        );
    }

    // ---- THE FILTER PANEL: the icon, the options, and what they do ----
    //
    // The request, literally: a filter icon to the RIGHT of the counter, and on
    // interacting with it the set options ("material design", "font
    // awesome", "devicons") uncollapse BELOW the search.
    //
    // The test does not repeat the panel's geometry: it finds the icon where
    // `render` said it put it, clicks there, and reads the keys that came out. A
    // copy of the arithmetic here would lie the day the header changed.
    clear(&mut inst)?;
    let header = inst.render(80, ROWS, heights())?;
    let (icon_line, icon_column) = header
        .commands
        .iter()
        .find_map(|c| match c {
            Command::Text {
                line,
                column,
                text: Text::Icon(n),
                ..
            } if n == FILTER_ICON => Some((*line, *column)),
            _ => None,
        })
        .context("the header did not emit the filter icon")?;
    // TO THE RIGHT OF THE COUNTER, and not on top of it: it is where the owner asked for it, and it is
    // the only corner of the line that does not compete for space with what one reads.
    let (counter_column, counter) = header
        .commands
        .iter()
        .filter_map(|c| match c {
            Command::Text {
                line: 0,
                column,
                text: Text::Data(s),
                ..
            } => Some((*column, s.clone())),
            _ => None,
        })
        .max_by_key(|(c, _)| *c)
        .context("the header did not emit the counter")?;
    let counter_end = counter_column + u16::try_from(counter.chars().count()).unwrap_or(0);
    if icon_line != 0 || icon_column < counter_end {
        bail!(
            "the filter icon came out at ({icon_column},{icon_line}) and the counter ends \
             at column {counter_end}: it has to sit TO THE RIGHT of the counter, on the search's line"
        );
    }
    println!("filter    icon          -> column {icon_column}, after the counter");

    // CLICKING THE ICON OPENS THE PANEL.
    let before_the_panel = read(&inst.render(80, ROWS, heights())?)?;
    if inst
        .event(&Input::Click {
            column: icon_column,
            line: icon_line,
        })?
        .is_empty()
    {
        bail!("clicking the filter icon asked for no redraw — the panel did not open");
    }
    let open = inst.render(80, ROWS, heights())?;
    let options = panel_of(&open);
    if options.is_empty() {
        bail!("the panel opened with no set option at all");
    }
    // THE THREE EXAMPLES FROM THE REQUEST, plus "no filter" — which is the way
    // back: a panel that only knows how to restrict is a trap.
    for example in ["md", "fa", "dev", "all"] {
        let key = format!("{SET_PREFIX_OF}{example}");
        if !options.iter().any(|(k, ..)| *k == key) {
            bail!(
                "the panel did not offer `{key}`. The options come out of the DATABASE, so\n\
                 either the set went away from it or the panel stopped walking it."
            );
        }
    }
    println!("filter    panel         -> {} sets", options.len());

    // EVERY OPTION HAS A KEY IN BOTH LANGUAGES, AND FITS THE BUDGET.
    //
    // The same guard as the sheet's labels, and for the same reason: the buffer sends a
    // KEY and never sees the translated sentence. Here it counts double — the key is
    // ASSEMBLED at run time (`glyphs-set-` plus the prefix), so
    // the compiler protects nothing, and a new Nerd Fonts set would come out
    // on screen as `[glyphs-set-xyz]`.
    for language in ["pt-BR", "en-GB"] {
        let table = ftl(&format!("{i18n}/{language}.ftl"))?;
        for (key, _, _, budget) in &options {
            let Some(text) = table.get(key) else {
                bail!(
                    "the panel's option `{key}` does not exist in {language}.ftl — it would come out\n\
                     on screen as `[{key}]`, which is the usual silent fallback."
                );
            };
            if text.chars().count() > usize::from(*budget) {
                bail!(
                    "in {language}, the option `{key}` = {text:?} is {} cells and the panel\n\
                     reserved {budget}. It will come out CUT in any window.",
                    text.chars().count()
                );
            }
        }
        println!(
            "filter    {language:8}      -> the {} options fit",
            options.len()
        );
    }

    // THE PANEL PUSHES THE GRID DOWN — and the click GOES WITH IT.
    //
    // This is the defect the `Panel` structure exists to prevent: the row
    // the grid starts on was written in two places, the drawing and the
    // click's arithmetic. The panel opening would be enough for one of them to fall behind
    // and the click to start hitting the row above.
    let with_panel = read(&open)?;
    let (line_before, _) = *before_the_panel
        .positions
        .first()
        .context("the closed grid drew no glyph at all")?;
    let (line_after, _) = *with_panel
        .positions
        .first()
        .context("with the panel open the grid drew no glyph at all")?;
    if line_after <= line_before {
        bail!(
            "the grid used to start on line {line_before} and with the panel open it starts on \
             {line_after}: the panel is drawing ON TOP of it"
        );
    }
    const PANEL_TARGET: usize = 3;
    let (line, column) = *with_panel
        .positions
        .get(PANEL_TARGET)
        .context("with the panel open the grid does not have enough glyphs")?;
    inst.event(&Input::Click { column, line })?;
    let focused = drawn_focus(&inst.render(80, ROWS, heights())?)
        .context("no glyph came out in focus after the click")?;
    if focused != (line, column) {
        bail!(
            "with the panel open, the click at ({column},{line}) put the focus on {focused:?}.\n\
             The click's arithmetic did not follow the line the grid starts on."
        );
    }
    println!("filter    grid moved    -> the click lands at ({column},{line})");

    // CHOOSING A SET FILTERS BY IT — and by what is written there.
    //
    // One clicks "Material Design" WHERE IT WAS DRAWN, and what has to be
    // left are `md-` glyphs. Without this, a one-index error between the drawing
    // and the click would give the neighbouring set, and the screen would stay coherent
    // with itself while saying the wrong thing.
    let (_, md_line, md_column, _) = options
        .iter()
        .find(|(k, ..)| k == &format!("{SET_PREFIX_OF}md"))
        .context("the panel does not have the Material Design option")?;
    if inst
        .event(&Input::Click {
            column: *md_column,
            line: *md_line,
        })?
        .is_empty()
    {
        bail!("clicking an option in the panel asked for no redraw");
    }
    let chosen = read(&inst.render(80, ROWS, heights())?)?;
    let md_focus = chosen
        .focus
        .clone()
        .context("after choosing the set there is no glyph in focus")?;
    if !md_focus.starts_with("md-") {
        bail!("choosing `Material Design` in the panel left `{md_focus}` in focus");
    }
    let by_prefix = query(&mut inst, "md:")?;
    if chosen.total_filtered != by_prefix.total_filtered {
        bail!(
            "the panel filtered {} glyphs and `md:` filters {}",
            chosen.total_filtered,
            by_prefix.total_filtered
        );
    }
    println!(
        "filter    choice        -> {} glyphs, all md-",
        chosen.total_filtered
    );

    // THE PANEL CLOSES ON `Esc`, AND THE SEARCH STAYS.
    //
    // In the grid `Esc` clears the search. With the panel open it undoes what
    // is in progress, which is the panel — if it cleared the search too, leaving the
    // list would cost the query one was building.
    inst.event(&Input::Command(PANEL.to_owned()))?;
    if panel_of(&inst.render(80, ROWS, heights())?).is_empty() {
        bail!("the command `{PANEL}` did not open the panel — does it only exist for the mouse?");
    }
    inst.event(&Input::Cancel)?;
    let closed = inst.render(80, ROWS, heights())?;
    if !panel_of(&closed).is_empty() {
        bail!("`Esc` did not close the panel");
    }
    if read(&closed)?.total_filtered != by_prefix.total_filtered {
        bail!("`Esc` closed the panel AND cleared the search along with it");
    }
    println!("filter    esc           -> closes the panel, and the search stays");

    // ---- `Tab` OPENS AND CLOSES THE FILTERS ----
    //
    // The cheapest key on the screen for the panel. What this test guards, besides
    // opening and closing, is that it does NOT GO INTO THE SEARCH: without a case of its own in the
    // protocol `Tab` falls into the host's `_` and arrives as `character('\t')`, which
    // the buffer pushes into the field. It is invisible — and the search's `trim` on top of that
    // keeps the COUNT from changing, so only the invitation gives it away.
    clear(&mut inst)?;
    if inst.event(&Input::Toggle)?.is_empty() {
        bail!("`Tab` asked for no redraw — the panel did not open");
    }
    if panel_of(&inst.render(80, ROWS, heights())?).is_empty() {
        bail!("`Tab` did not open the filter panel");
    }
    inst.event(&Input::Toggle)?;
    let closed = inst.render(80, ROWS, heights())?;
    if !panel_of(&closed).is_empty() {
        bail!("the second `Tab` did not close the panel");
    }
    // THE INVITATION HAS TO BE ON SCREEN. It only appears with the search EMPTY: if
    // `Tab` had gone into the field, the buffer would emit what was typed
    // (`Text::Data`) instead of the key, and the tab would sit there unseen.
    if !keys_of(&closed).iter().any(|(k, _)| k == "glyphs-search") {
        bail!(
            "after two `Tab`s the field no longer shows the invitation: the key went\n\
             into the search as a character, and a tab in the field is invisible."
        );
    }
    // AND WITH A SEARCH IN PROGRESS it does not lose it: toggling the panel is a gesture over
    // the query, not a restart of it.
    let only_folder = query(&mut inst, "folder")?.total_filtered;
    inst.event(&Input::Toggle)?;
    inst.event(&Input::Toggle)?;
    let after = read(&inst.render(80, ROWS, heights())?)?.total_filtered;
    if after != only_folder {
        bail!(
            "two `Tab`s over the search `folder` changed the result from {only_folder} to {after}"
        );
    }
    println!("filter    tab           -> opens, closes, and does not touch the search");

    // THE LABEL ONE READS FINDS, WHEN TYPED, THE WHOLE SET.
    //
    // They are two representations of the same fact: the set's name lives in the
    // `.ftl` (because it is interface text and gets translated) and in `gen-glyphdb.py`'s
    // `SETS` (because it becomes a search tag). Nothing forces them to
    // agree — and "Clima" in the panel with "weather" in the search would be a screen
    // that teaches a word that does not work.
    for language in ["pt-BR", "en-GB"] {
        let table = ftl(&format!("{i18n}/{language}.ftl"))?;
        for (key, ..) in &options {
            let Some(prefix) = key.strip_prefix(SET_PREFIX_OF) else {
                continue;
            };
            // "all" is no set at all: it REMOVES the filter.
            if prefix == "all" {
                continue;
            }
            let label = table.get(key).context("label checked above")?;
            let set_only = query(&mut inst, &format!("{prefix}:"))?.total_filtered;
            clear(&mut inst)?;
            // PASTING, and not typing: it is one query instead of one per
            // character, and the result is the same (already proved further below).
            inst.event(&Input::Paste(label.clone()))?;
            let by_name = read(&inst.render(80, ROWS, heights())?)?.total_filtered;
            if by_name < set_only {
                bail!(
                    "in {language}, the panel shows {label:?} and searching for that finds {by_name}\n\
                     glyphs — the set `{prefix}` has {set_only}. The name in the `.ftl` and the\n\
                     name in gen-glyphdb.py's `SETS` diverged, and the screen is teaching a\n\
                     word the search does not know."
                );
            }
        }
        println!("filter    {language:8}      -> every label finds its own set");
    }
    clear(&mut inst)?;

    // ---- THE SCALES: one rule per ROLE, and not one size per run ----
    //
    // Three roles, three sizes: the GLYPH (magnifying glass, filter, grid and footer) at
    // icon scale; the INTERFACE (the search field) at the system scale, the
    // same as the minibuffer's; and the ACCESSORY (the counter) smaller than both.
    //
    // Without this each run picks its own size and the screen becomes a patchwork:
    // that is how the counter came out at the same size as what one types and the icon
    // at the same size as the word beside it.
    clear(&mut inst)?;
    let screen = inst.render(80, ROWS, heights())?;
    let icon = |name: &str| -> Option<ronoctua_protocol::Scale> {
        screen.commands.iter().find_map(|c| match c {
            Command::Text {
                text: Text::Icon(n),
                scale,
                ..
            } if n == name => Some(*scale),
            _ => None,
        })
    };
    let magnifier = icon("search").context("the header did not emit the magnifier")?;
    let filter = icon(FILTER_ICON).context("the header did not emit the filter icon")?;
    // The footer's glyph: the `Data` run with the largest row in column zero.
    let footer = screen
        .commands
        .iter()
        .filter_map(|c| match c {
            Command::Text {
                line,
                column: 0,
                text: Text::Data(_),
                scale,
                ..
            } => Some((*line, *scale)),
            _ => None,
        })
        .max_by_key(|(l, _)| *l)
        .map(|(_, e)| e)
        .context("the footer did not emit the focused glyph")?;
    // The search: the only run that comes out as a KEY in the header.
    let search = screen
        .commands
        .iter()
        .find_map(|c| match c {
            Command::Text {
                text: Text::Key(k),
                scale,
                ..
            } if k == "glyphs-search" => Some(*scale),
            _ => None,
        })
        .context("the header did not emit the search invitation")?;
    let counter_scale = screen
        .commands
        .iter()
        .filter_map(|c| match c {
            Command::Text {
                line: 0,
                column,
                text: Text::Data(_),
                scale,
                ..
            } => Some((*column, *scale)),
            _ => None,
        })
        .max_by_key(|(c, _)| *c)
        .map(|(_, e)| e)
        .context("the header did not emit the counter")?;

    if magnifier != filter || filter != footer {
        bail!(
            "the magnifier came out at {magnifier:?}, the filter at {filter:?} and the footer's glyph at \
             {footer:?} — the three are the SAME thing on screen and have to have the same body"
        );
    }
    if order(magnifier) <= order(search) {
        bail!(
            "the icon came out at {magnifier:?} and the text at {search:?}: the icon has to be BIGGER"
        );
    }
    if search != ronoctua_protocol::Scale::Base {
        bail!(
            "the search field came out at {search:?}. It is the same thing as the minibuffer,\n\
             and its body is the SYSTEM's — not the buffer's content's."
        );
    }
    if order(counter_scale) >= order(search) {
        bail!(
            "the counter came out at {counter_scale:?} and the search at {search:?}: the counter\n\
             is read at a glance, and has to be SMALLER than what is read with attention."
        );
    }
    println!("scales    icon/text     -> {magnifier:?} / {search:?} / {counter_scale:?}");

    // ---- THE SPACING: ADJACENT BLOCKS, and no blank line ----
    //
    // THE DEFECT THIS CATCHES, and which showed up on the owner's screen: I was adding one
    // empty cell between every two blocks, without noticing that the line box ALREADY
    // carries the air (`1.45 at` of box against `1.25 at` of font). The result
    // was a blank line above and below the header, a blank line
    // between the icon rows, and a third of the glyphs off screen.
    //
    // The rule is now the inverse, and it is what gets asserted: the blocks are
    // ADJACENT, and the only white left over is what does not fit in a row.
    let a = heights();
    let rule = screen
        .commands
        .iter()
        .find_map(|c| match c {
            Command::Rule { line, .. } => Some(*line),
            _ => None,
        })
        .context("the screen emitted no rule at all")?;
    if rule != a.large {
        bail!(
            "the header takes {} cells and the rule came out on line {rule}: that is {} blank \
             lines between the two",
            a.large,
            rule.saturating_sub(a.large)
        );
    }
    let vision = read(&screen)?;
    let (first_row, _) = *vision
        .positions
        .first()
        .context("the grid drew no glyph at all")?;
    if first_row != rule + 1 {
        bail!(
            "the rule is on line {rule} and the grid only starts on {first_row}: \
             a blank line between the two"
        );
    }
    // THE ROWS ARE ADJACENT: two consecutive rows are exactly the glyph's box
    // apart. It was the blank line between them that the owner saw.
    let distinct_lines: Vec<u16> = {
        let mut l: Vec<u16> = vision.positions.iter().map(|(line, _)| *line).collect();
        l.dedup();
        l
    };
    if let Some(&second) = distinct_lines.get(1)
        && second - first_row != a.large
    {
        bail!(
            "two consecutive rows are {} cells apart and the glyph's box is {}: there is a blank \
             line between them",
            second - first_row,
            a.large
        );
    }
    let (last_row, _) = *vision
        .positions
        .last()
        .context("the grid drew no glyph at all")?;
    let footer_line = screen
        .commands
        .iter()
        .filter_map(|c| match c {
            Command::Text { line, .. } => Some(*line),
            _ => None,
        })
        .max()
        .context("the screen emitted no text at all")?;
    // ANCHORED TO THE BOTTOM: the footer block ends exactly where the window
    // ends. A row left over AFTER it is the empty strip the owner saw.
    if footer_line + a.large != ROWS {
        bail!(
            "the footer starts on line {footer_line} and takes {} cells in a window of \
             {ROWS}: {} empty lines are left AFTER it",
            a.large,
            ROWS.saturating_sub(footer_line + a.large)
        );
    }
    // AND THE REMAINDER ABOVE IT IS SMALLER THAN ONE ROW. If another row of icons
    // fitted there, it had to have been used — it was by not using it that the screen
    // ended up with a useless gap above the footer.
    let left_over = footer_line.saturating_sub(last_row + a.large);
    if left_over >= a.large {
        bail!(
            "{left_over} cells are left between the last row and the footer, and one row takes \
             {}: another row of icons would have fitted there",
            a.large
        );
    }
    println!(
        "spacing   rule {rule} · grid {first_row}..{last_row} · footer \
         {footer_line} de {ROWS} (sobra {left_over})"
    );

    // ---- the search finds by TAG, in both languages and with or without accents ----
    //
    // This is the request, literally: `cod-location` has to show up under
    // "location" as well as under "map", "mapa" and "localizacao" — with and without accents.
    // Without this, searching in Portuguese only found what happened to have a name
    // resembling the English word.
    // The query goes PREFIXED BY THE SET (`cod:mapa`), and not loose.
    //
    // It is not to make it easier: `mapa` on its own matches thousands of glyphs, and the list
    // is VIRTUALISED — `cod-location` exists in the result and simply
    // is not in the emitted part. The test would end up measuring the ordering instead
    // of the tag matching. With the set, the result fits on screen and the
    // answer is about what one wants to know.
    let finds = |inst: &mut ronoctua_runtime::Instance, q: &str| -> Result<bool> {
        query(inst, &format!("cod:{q}"))?;
        let names = names_while_stepping(inst, 40)?;
        Ok(names.iter().any(|n| n == "cod-location"))
    };
    for q in ["location", "map", "mapa", "localização", "localizacao"] {
        if !finds(&mut inst, q)? {
            bail!(
                "searching `{q}` does not find `cod-location`.\n\
                 The bilingual tags exist for this: name, English synonym,\n\
                 the Portuguese translation and the accent-free form."
            );
        }
        println!("search    {q:14} -> cod-location");
    }

    // And the converse, because a search that finds EVERYTHING finds nothing: a term
    // with no relation at all must not bring the glyph.
    let nothing = query(&mut inst, "xyzzyquux")?;
    if nothing.total_filtered != 0 {
        bail!(
            "a nonsense query gave back {} glyphs",
            nothing.total_filtered
        );
    }

    // ---- the footer shows the focused glyph's TAGS ----
    clear(&mut inst)?;
    let with_tags = read(&inst.render(80, ROWS, heights())?)?;
    let tags = with_tags
        .tags
        .context("the footer did not emit the focused glyph's tags")?;
    if tags.trim().is_empty() {
        bail!("the tags run came out empty");
    }
    println!("tags      in the footer -> {tags}");

    // ---- and in a NARROW window they disappear, without taking the name with them ----
    //
    // The name never gives up space: whoever is searching by name is reading the name. Without
    // this guard, the row would become an elided stub taking up space to say nothing.
    // Here `read` CANNOT be used: with 24 columns the counter also disappears
    // (it gives the line to the search), and `read` requires the counter. What
    // matters is the LAST row, and it is read straight from the display list.
    let list = inst.render(24, ROWS, heights())?;
    let last_one = list
        .commands
        .iter()
        .filter_map(|c| match c {
            Command::Text {
                line,
                column,
                text: Text::Data(s),
                ..
            } => Some((*line, *column, s.clone())),
            _ => None,
        })
        .max_by_key(|(l, _, _)| *l)
        .map(|(l, _, _)| l)
        .context("the 24-column render emitted no text at all")?;
    let on_the_last: Vec<_> = list
        .commands
        .iter()
        .filter_map(|c| match c {
            Command::Text {
                line,
                text: Text::Data(s),
                ..
            } if *line == last_one => Some(s.clone()),
            _ => None,
        })
        .collect();
    // TWO: the focused glyph and its name. It is the tags that disappear — and the glyph
    // stays, because it is a single character and answers "which one is it" on its own.
    if on_the_last.len() != 2 {
        bail!(
            "in a 24-column window the last line has {} runs, expected 2 \
             (the glyph and the name): {:?}",
            on_the_last.len(),
            on_the_last
        );
    }
    if on_the_last[1].trim().is_empty() {
        bail!("in a narrow window the glyph's NAME went away along with the tags");
    }
    println!("tags      window 24    -> omitted, and the name stayed");

    // ---- ENTER OPENS THE SHEET, and it REPLACES the content ----
    //
    // "Replaces" is the request, and it is what gets asserted: the grid disappears. A test that
    // only looked for the glyph's name would pass with the sheet drawn ON TOP
    // of the grid, which is another thing.
    clear(&mut inst)?;
    query(&mut inst, "cod-folder")?;
    let before = read(&inst.render(80, ROWS, heights())?)?;
    let chosen = before
        .focus
        .clone()
        .context("no glyph in focus before the sheet")?;

    if inst.event(&Input::Confirm)?.is_empty() {
        bail!("Enter on the grid asked for no effect — the sheet did not open");
    }
    let sheet = texts_of(&inst.render(80, ROWS, heights())?);
    // The grid emitted the counter `N / M` on row 0. On the sheet it does not exist.
    if sheet.iter().any(|(_, _, t)| t.contains(" / ")) {
        bail!("the sheet still has the grid's counter — it did not REPLACE the content");
    }
    if !sheet.iter().any(|(_, _, t)| t == &chosen) {
        bail!("the sheet does not carry the chosen glyph's name (`{chosen}`)");
    }
    println!("sheet     opened        -> {chosen}");

    // ---- and it brings what was asked for: identity, use and the nine formats ----
    let everything: String = sheet
        .iter()
        .map(|(_, _, t)| t.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    for expected in ["U+", "codicons"] {
        if !everything.contains(expected) {
            bail!("the sheet does not show `{expected}`:\n{everything}");
        }
    }
    // The VALUES of each format, and not only the labels: a label with no value
    // would be a list of promises.
    let target = inst
        .render(80, ROWS, heights())?
        .commands
        .iter()
        .find_map(|c| match c {
            Command::Text {
                line: 0,
                text: Text::Data(s),
                ..
            } => s.chars().next(),
            _ => None,
        })
        .context("the sheet did not emit the big glyph on line 0")?;
    for expected in [
        format!("U+{:04X}", target as u32),
        format!("nf nf-{chosen}"),
        format!("\\u{{{:x}}}", target as u32),
    ] {
        if !everything.contains(&expected) {
            bail!("the sheet does not carry the value `{expected}`:\n{everything}");
        }
    }
    println!("sheet     values        -> codepoint, CSS class and escape");

    // ---- down + Enter copies the SECOND option, and not the first ----
    //
    // Without this, a sheet that ignored the selection and always copied the glyph
    // would pass — and the nine options would be decorative.
    inst.event(&Input::Down)?;
    let effects = inst.event(&Input::Confirm)?;
    let copied = effects
        .iter()
        .find_map(|e| match e {
            ronoctua_protocol::Effect::Copy(s) => Some(s.clone()),
            _ => None,
        })
        .context("Enter on the sheet did not ask for Copy")?;
    if copied != target.to_string() {
        bail!("the second option should copy the GLYPH (`{target}`), and it copied `{copied}`");
    }
    println!("sheet     down+enter    -> copied the glyph, and not the name");

    // ---- the arrows switch GLYPH without leaving the sheet ----
    inst.event(&Input::Right)?;
    let neighbour = texts_of(&inst.render(80, ROWS, heights())?);
    if neighbour.iter().any(|(_, _, t)| t == &chosen) {
        bail!("`->` did not change the sheet's glyph");
    }
    if neighbour.iter().any(|(_, _, t)| t.contains(" / ")) {
        bail!("`->` left the sheet and went back to the grid");
    }
    inst.event(&Input::Left)?;
    println!("sheet     arrows        -> changes glyph without leaving");

    // ---- Esc goes back, and the search and the cursor stay where they were ----
    inst.event(&Input::Cancel)?;
    let after = read(&inst.render(80, ROWS, heights())?)?;
    if after.focus != before.focus {
        bail!(
            "going back changed the focused glyph: {:?} -> {:?}",
            before.focus,
            after.focus
        );
    }
    if after.total_filtered != before.total_filtered {
        bail!(
            "going back changed the search: {} -> {}",
            before.total_filtered,
            after.total_filtered
        );
    }
    println!("sheet     esc           -> came back with the search and cursor intact");

    // ---- THE GLYPH FITS THE RESERVED SPACE, and slack is left over ----
    //
    // THE DEFECT THIS CATCHES, and which showed up on the owner's screen: the sheet
    // reserved FOUR cells for the `huge` glyph, and its line box
    // asks for SIX (116 px of font with 1.45 line height against a 29 px cell).
    // It came out clipped at the bottom, touching the rule.
    //
    // The buffer has no way of knowing that on its own — font size and
    // line height belong to the host (invariant 11) — and that is why each scale's
    // heights arrive in the `context_of`. This test checks that it USES them: between
    // the glyph and whatever comes below there has to be at least the scale's height,
    // plus one cell of breathing room.
    inst.event(&Input::Cancel)?;
    clear(&mut inst)?;
    inst.event(&Input::Confirm)?;
    let a = heights();
    let sheet_list = inst.render(80, ROWS, a)?;
    let glyph = sheet_list
        .commands
        .iter()
        .find_map(|c| match c {
            Command::Text {
                line,
                scale: ronoctua_protocol::Scale::Huge,
                ..
            } => Some(*line),
            _ => None,
        })
        .context("the sheet did not emit the glyph at the Huge scale")?;
    // The FIRST thing drawn below the glyph — a rule or text.
    let below = sheet_list
        .commands
        .iter()
        .filter_map(|c| match c {
            Command::Text {
                line, column: 0, ..
            }
            | Command::Rule { line, .. }
                if *line > glyph =>
            {
                Some(*line)
            }
            _ => None,
        })
        .min()
        .context("the sheet drew nothing below the glyph")?;
    let gap = below - glyph;
    // THE BOX, and not one cell beyond it.
    //
    // The box already carries the air: `1.45 at` of box against `0.60 at` of ink
    // leave `0.85 at` split above and below. Reserving the box avoids the
    // clipping; reserving one cell MORE put a gap almost twice as large below the
    // glyph as above it, and that is what the owner saw as disproportion.
    if gap < a.huge {
        bail!(
            "the glyph starts on line {glyph} and the next thing comes on {below}: {gap} cells.\n\
             The `huge` scale's box takes {} — it will come out CUT underneath.",
            a.huge
        );
    }
    if gap > a.huge {
        bail!(
            "the glyph takes {} cells and the next thing only comes {gap} later: blank space is left\n\
             below it. The air between blocks already comes in the line's box.",
            a.huge
        );
    }
    println!(
        "sheet     glyph air     -> {gap} cells for a scale of {}",
        a.huge
    );

    // ---- in a SHORT window the glyph does not use the Huge scale ----
    inst.event(&Input::Confirm)?;
    let low = inst.render(80, 8, heights())?;
    let huge = low
        .commands
        .iter()
        .filter(|c| {
            matches!(
                c,
                Command::Text {
                    scale: ronoctua_protocol::Scale::Huge,
                    ..
                }
            )
        })
        .count();
    if huge != 0 {
        bail!("in an 8-line window the glyph still asked for the Huge scale");
    }
    inst.event(&Input::Cancel)?;
    println!("sheet     short window  -> the glyph gives up its scale");

    // ---- EVERY SHEET LABEL FITS THE BUDGET, in both languages ----
    //
    // THE DEFECT THIS CATCHES: "Token do design system" came out as
    // "Design system ..." at ANY window size, because the cut came
    // from the run's budget and not from the space on screen.
    //
    // The buffer has no way of defending itself: it sends a KEY, and what
    // translates is the host (invariant 11) — the sentence's length changes with the
    // language and it never sees it. So what checks is this test, which has
    // both sides: the budget the buffer declared and the actual `.ftl`.
    inst.event(&Input::Cancel)?;
    clear(&mut inst)?;
    inst.event(&Input::Confirm)?;
    let keys = keys_of(&inst.render(80, ROWS, heights())?);
    if keys.is_empty() {
        bail!("the sheet emitted no text key — invariant 8 requires the labels to be keys");
    }
    for language in ["pt-BR", "en-GB"] {
        let table = ftl(&format!("{i18n}/{language}.ftl"))?;
        for (key, budget) in &keys {
            let Some(text) = table.get(key) else {
                bail!("the sheet's key `{key}` does not exist in {language}.ftl");
            };
            let fits = usize::from(*budget);
            if text.chars().count() > fits {
                bail!(
                    "in {language}, the label `{key}` = {text:?} is {} cells and the buffer\n\
                     reserved {fits}. It will come out CUT in any window.",
                    text.chars().count()
                );
            }
        }
        println!("labels    {language:8}      -> the {} fit", keys.len());
    }

    // ---- COPYING GIVES FEEDBACK ON SCREEN ----
    //
    // Without this, pressing Enter on an option did not change a pixel: there was no way
    // to tell "it copied" from "the key never even arrived".
    let before_the_notice = texts_of(&inst.render(80, ROWS, heights())?).len();
    inst.event(&Input::Confirm)?;
    // The host answers with the verdict; here we simulate both.
    if inst.event(&Input::Copied(true))?.is_empty() {
        bail!("the copy's verdict asked for no redraw — the sheet shows nothing");
    }
    let with_notice = keys_of(&inst.render(80, ROWS, heights())?);
    if !with_notice.iter().any(|(k, _)| k == "glyphs-sheet-copied") {
        bail!("after copying, the sheet does not show the confirmation");
    }
    // AND THE NOTICE GOES AWAY on the next key: a "copied" that stays becomes decoration, and
    // on the next copy nobody knows whether it is new or old.
    inst.event(&Input::Down)?;
    let without_notice = keys_of(&inst.render(80, ROWS, heights())?);
    if without_notice
        .iter()
        .any(|(k, _)| k == "glyphs-sheet-copied")
    {
        bail!("the copy notice stayed on screen after the next key");
    }
    let _ = before_the_notice;
    println!("sheet     feedback      -> confirms, and goes on the next key");

    // AND THE FAILURE SHOWS UP, with a different face. Copying silently and not copying
    // silently are indistinguishable — that is how the copy went months without
    // working.
    inst.event(&Input::Copied(false))?;
    let failed = keys_of(&inst.render(80, ROWS, heights())?);
    if !failed.iter().any(|(k, _)| k == "glyphs-sheet-copy-failed") {
        bail!("the sheet shows nothing when the copy FAILS");
    }
    println!("sheet     failure       -> shows, and differs from success");

    // ---- PASTING GOES INTO THE SEARCH ----
    inst.event(&Input::Cancel)?;
    clear(&mut inst)?;
    if inst.event(&Input::Paste("folder".into()))?.is_empty() {
        bail!("pasting asked for no redraw — the text did not reach the search");
    }
    let pasted = read(&inst.render(80, ROWS, heights())?)?;
    let only_folder = query(&mut inst, "folder")?;
    if pasted.total_filtered != only_folder.total_filtered {
        bail!(
            "pasting `folder` filtered {} and typing `folder` filtered {}",
            pasted.total_filtered,
            only_folder.total_filtered
        );
    }
    println!("paste     `folder`      -> the same search as typing it");

    // ---- PASTING THE GLYPH ITSELF FINDS THE GLYPH ----
    //
    // The browser's most natural gesture, and the one that did not work: copying an
    // icon and pasting it back to find out WHICH it is. The search looked at name,
    // set and tag — and the character is none of the three, so the screen
    // answered "no results" to the most obvious question it has.
    //
    // The whole route, and not just the matching: it picks a glyph, copies the
    // CHARACTER the way the user would (the sheet), and pastes what came out.
    clear(&mut inst)?;
    query(&mut inst, "cod-location")?;
    let expected = read(&inst.render(80, ROWS, heights())?)?
        .focus
        .context("no glyph in focus to copy")?;
    inst.event(&Input::Confirm)?; // opens the sheet
    inst.event(&Input::Down)?; // the second option is the GLYPH
    let copied = inst
        .event(&Input::Confirm)?
        .iter()
        .find_map(|e| match e {
            ronoctua_protocol::Effect::Copy(s) => Some(s.clone()),
            _ => None,
        })
        .context("copying the glyph did not ask for Copy")?;
    inst.event(&Input::Cancel)?;

    clear(&mut inst)?;
    inst.event(&Input::Paste(copied.clone()))?;
    let found_at = read(&inst.render(80, ROWS, heights())?)?;
    if found_at.total_filtered == 0 {
        bail!("pasting the glyph `{copied}` found nothing");
    }
    let focus = found_at
        .focus
        .context("pasting the glyph left nothing in focus")?;
    if focus != expected {
        bail!("pasting the glyph `{copied}` found `{focus}`, and it is the `{expected}`");
    }
    println!(
        "paste     the glyph     -> {focus} (of {} results)",
        found_at.total_filtered
    );

    // AND IT COMES FIRST, even pasted in the middle of other text: whoever pastes
    // drags whitespace along, and the question is still the glyph.
    clear(&mut inst)?;
    inst.event(&Input::Paste(format!(" {copied} ")))?;
    let dirty = read(&inst.render(80, ROWS, heights())?)?;
    if dirty.focus.as_deref() != Some(expected.as_str()) {
        bail!(
            "pasting the glyph with spaces around it found {:?}",
            dirty.focus
        );
    }
    println!("paste     with spaces   -> {expected}");

    println!("OK: the five filters of section 6.5 narrow, the state persists,");
    println!("    the click lands, search by tag works in both languages,");
    println!(
        "    the sheet opens, copies, warns, navigates and returns, and pasting the glyph finds it"
    );
    Ok(())
}

/// The text keys emitted, with the budget the buffer reserved.
fn keys_of(list: &ronoctua_protocol::DisplayList) -> Vec<(String, u16)> {
    list.commands
        .iter()
        .filter_map(|c| match c {
            Command::Text {
                text: Text::Key(k),
                max_width,
                ..
            } => Some((k.clone(), *max_width)),
            _ => None,
        })
        .collect()
}

/// The scales' size order, so one can say "larger" and "smaller".
///
/// `Scale` is an enum of ROLE and not of value — how many pixels each is worth
/// belongs to the host (invariant 11). What this test asserts is the relation between them,
/// which is what the screen shows.
const fn order(e: ronoctua_protocol::Scale) -> u8 {
    match e {
        ronoctua_protocol::Scale::Small => 0,
        ronoctua_protocol::Scale::Base => 1,
        ronoctua_protocol::Scale::Normal => 2,
        ronoctua_protocol::Scale::Large => 3,
        ronoctua_protocol::Scale::Huge => 4,
    }
}

/// The filter panel's options: key, row, column and budget.
///
/// Empty when the panel is closed — that is how the test knows whether it
/// opened, without repeating its geometry here.
fn panel_of(list: &ronoctua_protocol::DisplayList) -> Vec<(String, u16, u16, u16)> {
    list.commands
        .iter()
        .filter_map(|c| match c {
            Command::Text {
                line,
                column,
                text: Text::Key(k),
                max_width,
                ..
            } if k.starts_with(SET_PREFIX_OF) => Some((k.clone(), *line, *column, *max_width)),
            _ => None,
        })
        .collect()
}

/// Where the FOCUSED glyph was drawn in the grid.
///
/// The focus changes COLOUR and gains no box (invariant 15), so finding it is
/// finding the accented run at icon scale. It has to be `Data`: the FILTER icon
/// also lights up, and is also a large glyph — only it is an `Icon`,
/// because what knows its codepoint is the host.
fn drawn_focus(list: &ronoctua_protocol::DisplayList) -> Option<(u16, u16)> {
    list.commands.iter().find_map(|c| match c {
        Command::Text {
            line,
            column,
            color,
            text: Text::Data(_),
            scale: ronoctua_protocol::Scale::Large,
            ..
        } if color.as_str() == "state-accent" => Some((*line, *column)),
        _ => None,
    })
}

/// A `.ftl` as a `key -> text` table.
///
/// A one-line parser, and that is enough: the Fluent format we use is
/// `key = text`, with no selector and no attribute. Bringing a Fluent parser into
/// a test would be bringing a dependency to answer what a `split_once`
/// answers.
fn ftl(path: &str) -> Result<std::collections::HashMap<String, String>> {
    let raw = std::fs::read_to_string(path).with_context(|| format!("could not read {path}"))?;
    Ok(raw
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .filter_map(|l| l.split_once(" = "))
        .map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
        .collect())
}

/// Every text emitted, with where it ended up.
fn texts_of(list: &ronoctua_protocol::DisplayList) -> Vec<(u16, u16, String)> {
    list.commands
        .iter()
        .filter_map(|c| match c {
            Command::Text {
                line,
                column,
                text: Text::Data(s),
                ..
            } => Some((*line, *column, s.clone())),
            _ => None,
        })
        .collect()
}
