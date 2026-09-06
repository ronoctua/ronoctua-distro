//! Exercises the formatting buffer — `LEADER s f`.
//!
//! # Why a binary, and not a `#[test]` in the buffer
//!
//! The component compiles to `wasm32-wasip2` and lives in a workspace of its
//! own (invariant 12): a `cargo test` of it would test the logic OUTSIDE the
//! component, which is precisely what proves nothing. Here it is loaded the way
//! the host loads it, pushed a rack the way the host pushes one, and read
//! through `render`.
//!
//! # What only this can prove
//!
//! The transfer window proved that a cursor which is an ID survives the list
//! reordering underneath it. This one proves the thing that mattered enough to
//! be worth a screen of its own: **that the two refusals are refusals, and not
//! comments**.
//!
//! - A device the running system needs is DRAWN, and `Enter` on it goes
//!   nowhere. Not greyed and then accepted; not accepted and then refused three
//!   screens later by the host.
//! - `Enter` on the confirmation screen, pressed by somebody who has been
//!   pressing `Enter` to get here, goes BACK. The row that erases a disk costs
//!   one deliberate keystroke more than the rhythm gives away for free.
//!
//! Neither is visible from the host: the host sees a plan or no plan, and "no
//! plan" is also what a buffer that crashed produces.
//!
//! Usage: `ronoctua-test-format <component.wasm> <i18n-dir>`

use anyhow::{Context as _, Result, anyhow, bail};
use ronoctua_protocol::{Command, Device, Effect, Input, Outcome, Rack, Recipe, Text};

/// The labels this buffer cuts ON PURPOSE, and why.
///
/// `(key prefix, the reason)`. The reason is not decoration: it is what tells
/// the next reader whether an entry is a decision or a leftover — the same
/// bargain `generator-flags-agree` makes with `--reflink`.
const ON_PURPOSE: &[(&str, &str)] = &[];

/// The scale heights the production host sends.
fn heights() -> ronoctua_protocol::Heights {
    ronoctua_protocol::Heights::from_sizes(15.0, 18.0, 20.0, 40.0, 116.0, 1.45)
}

const ROWS: u16 = 30;
const COLUMNS: u16 = 110;
const GIB: u64 = 1024 * 1024 * 1024;

/// The sentence the host worded for a device the running system needs.
///
/// A LITERAL, and not a key: by the time it reaches the buffer it is already
/// text, because a component has no bundle to look one up in (invariant 11).
/// Checking that it is drawn AS DATA is checking that the split held.
const NEEDED: &str = "the system is mounted at /";

fn device(id: &str, name: &str, node: &str, kind: &str, whole: bool, needed: &str) -> Device {
    Device {
        id: id.to_owned(),
        name: name.to_owned(),
        node: node.to_owned(),
        kind: kind.to_owned(),
        bytes: 32 * GIB,
        mounted: false,
        removable: true,
        needed: needed.to_owned(),
        whole,
    }
}

/// Three rows: a whole stick, one partition of it, and the machine's root.
fn rack() -> Rack {
    Rack {
        devices: vec![
            device("/block/sdb", "SanDisk Cruzer", "/dev/sdb", "", true, ""),
            device("/block/sdb1", "FOTOS", "/dev/sdb1", "vfat", false, ""),
            device("/block/dm-0", "nixos", "/dev/dm-0", "btrfs", false, NEEDED),
        ],
    }
}

/// Everything the component drew, in one bucket per kind.
struct Screen {
    data: Vec<String>,
    keys: Vec<String>,
    selections: usize,
}

impl Screen {
    fn of(list: &ronoctua_protocol::DisplayList) -> Self {
        let mut me = Self {
            data: Vec::new(),
            keys: Vec::new(),
            selections: 0,
        };
        for c in &list.commands {
            match c {
                Command::Text { text, .. } => match text {
                    Text::Data(t) => me.data.push(t.clone()),
                    Text::Key(k) => me.keys.push(k.clone()),
                    Text::Icon(_) => {}
                },
                Command::Selection { .. } => me.selections += 1,
                _ => {}
            }
        }
        me
    }

    fn shows(&self, what: &str) -> bool {
        self.data.iter().any(|d| d.contains(what))
    }

    fn names(&self, key: &str) -> bool {
        self.keys.iter().any(|k| k == key)
    }
}

/// Draws, reads the screen, and refuses a frame that draws on top of itself.
///
/// Here on EVERY reading, and not only in the width walk below: the walk goes
/// over one screen, and the three this buffer has do not lay out alike.
fn look(inst: &mut ronoctua_runtime::Instance) -> Result<Screen> {
    let list = inst.render(COLUMNS, ROWS, heights())?;
    ronoctua_runtime::driven::note(&list);
    nothing_touches(&list, COLUMNS)?;
    Ok(Screen::of(&list))
}

/// Nothing on the frame touches anything else, in either direction.
///
/// # The two defects this exists for, both found on a SCREENSHOT
///
/// SIDEWAYS: the detail line has two halves, a sentence on the left and an
/// exact byte count against the right edge. The first version gave the sentence
/// the whole window's width and then drew the number on top of it — on screen,
/// `…dos até 2 §1BO88p4 B`. Every test passed, because every test compared
/// STATE, and the state was right: both fields existed and both said the right
/// thing.
///
/// DOWNWARDS: the title asked for `Scale::Large`, whose line box is 58 px
/// against the grid's 29, so it covered the cell under it — and the rule drawn
/// there came out as an underline through the descenders. Same shape of
/// mistake: the display list named two different rows, and nothing multiplied a
/// row by the height its scale is worth.
///
/// Both live in `ronoctua-protocol`, next to the display list they read, so
/// every driver asks the same question and no two of them can disagree about
/// what a command occupies.
fn nothing_touches(list: &ronoctua_protocol::DisplayList, columns: u16) -> Result<()> {
    list.nothing_overlaps(heights(), columns)
        .map_err(|e| anyhow!(e))?;
    list.nothing_is_covered(heights(), columns)
        .map_err(|e| anyhow!(e))?;
    Ok(())
}

/// The state the buffer last reported, if this input changed it.
fn moved_to(effects: &[Effect]) -> Option<&str> {
    effects.iter().find_map(|e| match e {
        Effect::StateChanged(id) => Some(id.as_str()),
        _ => None,
    })
}

/// The plan, if one left.
fn plan(effects: &[Effect]) -> Option<&ronoctua_protocol::Plan> {
    effects.iter().find_map(|e| match e {
        Effect::Format(p) => Some(p),
        _ => None,
    })
}

fn main() -> Result<()> {
    let path = std::env::args()
        .nth(1)
        .context("usage: ronoctua-test-format <component.wasm> <i18n-dir>")?;
    let i18n = std::env::args()
        .nth(2)
        .context("usage: ronoctua-test-format <component.wasm> <i18n-dir>")?;
    let wasm = std::fs::read(&path).with_context(|| format!("could not read {path}"))?;

    let rt = ronoctua_runtime::Runtime::fresh()?;
    let cwasm = rt.precompile(&wasm)?;
    let component = rt.load(&cwasm)?;
    let mut inst = rt.open(&component)?;

    // ---- THE DESCRIPTOR ----
    let d = inst.init()?;

    // THE PHRASES, exactly as the host hands them over at load. Without this a
    // buffer that holds interface text would be driven against its fallback,
    // and the screen this test reads would not be the screen anybody sees.
    // en-GB is the canonical one; the pt-BR round trip is checked separately.
    ronoctua_runtime::driven::hand_phrases(&mut inst, &d, &i18n, "en-GB")?;
    if d.mode != "format" {
        bail!("the buffer's mode is `{}`; the modeline shows that", d.mode);
    }
    if !d
        .capabilities
        .contains(&ronoctua_protocol::Capability::Disks)
    {
        bail!("the buffer does not declare `disks`: the host would never subscribe it to udisks");
    }
    // AND IT DECLARES NOTHING ELSE. `disks` is the heaviest of the five, and a
    // buffer that also held `system` could end processes and sweep the store on
    // top of writing filesystems — which is why they were kept apart.
    for extra in [
        ronoctua_protocol::Capability::System,
        ronoctua_protocol::Capability::Transfers,
    ] {
        if d.capabilities.contains(&extra) {
            bail!(
                "the buffer asks for `{extra:?}`, which it does not use: that would be powers for free"
            );
        }
    }
    if let Err(e) = d.validate() {
        bail!("the descriptor does not pass the section 7.9 guard: {e}");
    }
    // THREE STATES, AND THERE IS DELIBERATELY NO `working`. Every state a
    // buffer declares is offered under `SPC w s`, so a fourth one would put
    // "writing to the disk" on the modeline with nothing being written. The
    // wait happens on the list, where the row is about to change.
    let states: Vec<&str> = d.states.iter().map(|s| s.id.as_str()).collect();
    if states != ["normal", "choose", "confirm"] {
        bail!("the buffer's states are {states:?}, outside what was agreed");
    }
    for language in ronoctua_runtime::driven::languages(&i18n)? {
        let table = ftl(&format!("{i18n}/{language}.ftl"))?;
        for s in &d.states {
            let key = format!("state-{}", s.id);
            if !table.contains(&key) {
                bail!("`{key}` is missing from {language}.ftl — the modeline would show [{key}]");
            }
        }
    }
    println!(
        "descriptor: mode `{}`, {} states, and only the capability it uses",
        d.mode,
        d.states.len()
    );

    // ---- BEFORE THE RACK: nothing, and it says so ----
    let empty = look(&mut inst)?;
    if !empty.names("format-empty") {
        bail!("the empty screen does not say there is no disk at all");
    }
    if empty.shows("/dev/") {
        bail!("the buffer drew a device before taking any reading at all");
    }

    // ---- THE RACK CROSSES, FIELD BY FIELD ----
    inst.event(&Input::Rack(rack()))?;
    let full = look(&mut inst)?;
    for what in ["SanDisk Cruzer", "/dev/sdb1", "vfat", "32.0 GiB"] {
        if !full.shows(what) {
            bail!("`{what}` did not show up in the list: {:?}", full.data);
        }
    }
    // AN EMPTY `kind` IS A SENTENCE. The whole stick has nothing written on it,
    // and a gap where the other rows have a word would read as a row that
    // failed to load rather than as a disk that is empty.
    if !full.names("format-blank") {
        bail!("the row with no filesystem did not say it is empty");
    }
    // AND `whole` IS ON THE ROW. It decides which question comes next — a whole
    // disk gets a fresh partition table and a partition cannot — so discovering
    // it one screen later would be discovering it too late.
    if !full.names("format-whole") || !full.names("format-part") {
        bail!("the rows do not say which is the whole disk and which is a part");
    }
    // THE REFUSAL ARRIVED AS TEXT AND IS DRAWN AS TEXT. Had the buffer been
    // given a key instead, this is where the split would have failed.
    if !full.shows(NEEDED) {
        bail!(
            "the system's row did not carry the written reason: {:?}",
            full.data
        );
    }
    println!("the reading crossed over: name, node, kind, size, what each row is, and the reason");

    // THE LIST HAS THE SECOND REGISTER TOO. The row rounds to `32.0 GiB`, which
    // is what somebody READS; the line under the cursor gives the exact count,
    // which is what somebody COMPARES — against the vendor's number, against
    // `lsblk`, against the disk they meant to grab.
    if !full.shows(&format!("{} B", 32 * GIB)) {
        bail!(
            "the list does not give the exact size under the cursor: {:?}",
            full.data
        );
    }

    // ---- AND THE MOTIVE SURVIVES A NARROW WINDOW ----
    //
    // THIS IS THE DEFECT THAT WAS FOUND ON A SCREEN AND NOT HERE. The first
    // layout put five columns at fixed offsets, and in a window that could not
    // hold them the reason fell off the right edge — leaving a grey glyph as
    // the only sign that a disk could not be written to. A colour alone is a
    // rule somebody has to already know, which is what this screen exists not
    // to require.
    let narrow = Screen::of(&inst.render(46, ROWS, heights())?);
    if !narrow.shows(NEEDED) {
        bail!(
            "in a 46-column window the reason fell outside: {:?}",
            narrow.data
        );
    }
    println!("and the reason stays on screen in a 46-column window");

    // ---- AND NOTHING TOUCHES ANYTHING, AT ANY WIDTH ----
    //
    // Walked over a range and not at one size, because the fault this catches
    // appears at the width where two fields happen to meet — and that width
    // depends on how long a sentence came out in whichever language is loaded.
    for columns in [46, 60, 80, 110, 160] {
        nothing_touches(&inst.render(columns, ROWS, heights())?, columns)?;
    }
    println!("and nothing touches anything, from 46 to 160 columns");

    // ---- THE CURSOR STARTS SOMEWHERE USABLE ----
    //
    // udisks sorts by device node, so on an ordinary machine the first row is
    // the internal disk the system booted from — the one row that goes
    // nowhere. A cursor that started there would make the first key everybody
    // presses do nothing.
    if full.selections != 1 {
        bail!(
            "the list highlighted {} rows on taking the reading",
            full.selections
        );
    }
    let opened = inst.event(&Input::Confirm)?;
    if moved_to(&opened) != Some("choose") {
        bail!("the cursor was born on a disk that does not advance: {opened:?}");
    }
    inst.event(&Input::Cancel)?;

    // ---- THE FIRST REFUSAL: `Enter` ON WHAT THE SYSTEM NEEDS GOES NOWHERE ----
    //
    // Walked to, and not jumped to: this is the path a person's hand takes.
    // The root is the last of the three rows.
    inst.event(&Input::Up)?;
    let refused = inst.event(&Input::Confirm)?;
    if moved_to(&refused).is_some() {
        bail!("the buffer advanced from the disk the system needs");
    }
    if plan(&refused).is_some() {
        bail!("the buffer ASKED to format the disk the system needs");
    }
    let still = look(&mut inst)?;
    if !still.names("format-title") {
        bail!("the screen changed after an Enter that should do nothing");
    }
    println!("refusal 1: Enter on the system's disk does not advance, and asks nothing");

    // ---- THE ORDINARY PATH ----
    //
    // One up from the root lands on the partition, which is the row this whole
    // walk is aiming at: a `whole_disk` that came out `true` from it would wipe
    // the stick's partition table when somebody asked to reformat one part.
    inst.event(&Input::Up)?;
    let chose = inst.event(&Input::Confirm)?;
    if moved_to(&chose) != Some("choose") {
        bail!("Enter on a free disk did not lead to the choice: {chose:?}");
    }
    let choosing = look(&mut inst)?;
    // The chosen disk is NAMED on the screen that asks about it: somebody who
    // got here by pressing `Enter` twice can still see what they chose.
    if !choosing.shows("/dev/sdb1") {
        bail!(
            "the choice screen does not say which disk it is about: {:?}",
            choosing.data
        );
    }
    if !choosing.names("format-recipe-universal") || !choosing.names("format-recipe-modern") {
        bail!("the filesystem list did not come out whole");
    }
    // XFS IS ON THE LIST, saying why it is not on offer. Leaving it out would
    // be somebody searching a screen for something that is not there.
    if !choosing.names("format-recipe-missing") {
        bail!("XFS went away from the list instead of saying why it is not there");
    }

    // ---- THE SAME SCREEN, ANSWERING A SECOND PERSON ----
    //
    // THE LABEL IS FOR SOMEBODY WHO DOES NOT KNOW WHAT FAT32 IS; the line under
    // the cursor is for somebody who does. Neither replaces the other, and what
    // is asserted here is that they are the SAME row — the technical line has
    // to follow the cursor, or it is a caption that happens to be near a list.
    if !choosing.names("format-tech-universal") {
        bail!(
            "the technical line did not follow the cursor on the first recipe: {:?}",
            choosing.keys
        );
    }
    for other in ["format-tech-windows", "format-tech-modern"] {
        if choosing.names(other) {
            bail!(
                "the screen showed `{other}` with the cursor on another row: these are captions, not detail"
            );
        }
    }
    // AND IT MOVES. One row down is exFAT, and the line has to say so.
    inst.event(&Input::Down)?;
    let moved = look(&mut inst)?;
    if !moved.names("format-tech-large") || moved.names("format-tech-universal") {
        bail!(
            "the technical line did not change with the arrow: {:?}",
            moved.keys
        );
    }
    inst.event(&Input::Up)?;

    // ---- THE THIRD REFUSAL: THE ROW THAT NAMES WHAT THIS BUILD CANNOT WRITE
    //
    // XFS is on the list and the cursor REACHES it — it was a fixed caption
    // under the five at first, which made it the one thing on any of these
    // screens that explains itself only to whoever happened to read it. One up
    // from the first row wraps onto it.
    inst.event(&Input::Up)?;
    let on_xfs = look(&mut inst)?;
    if !on_xfs.names("format-tech-missing") {
        bail!("the cursor does not reach the XFS row: {:?}", on_xfs.keys);
    }
    let refused_xfs = inst.event(&Input::Confirm)?;
    if moved_to(&refused_xfs).is_some() || plan(&refused_xfs).is_some() {
        bail!("Enter on XFS advanced, and this distro does not have its mkfs: {refused_xfs:?}");
    }
    inst.event(&Input::Down)?;
    println!("refusal 3: XFS is in the list, the cursor reaches it, and Enter on it does not move");

    // The name, typed. It is the one place on any of the three screens where a
    // key means a letter rather than a choice.
    for c in "fotos".chars() {
        inst.event(&Input::Char(c))?;
    }
    let named = look(&mut inst)?;
    if !named.shows("FOTOS") {
        bail!("the typed name did not show up: {:?}", named.data);
    }

    // A second filesystem, so the plan below cannot pass by accident on the
    // default.
    inst.event(&Input::Down)?;

    // ---- THE SECOND REFUSAL: THE FIRST `Enter` ON `confirm` GOES BACK ----
    let asked = inst.event(&Input::Confirm)?;
    if moved_to(&asked) != Some("confirm") {
        bail!("Enter on the choice did not lead to the confirmation: {asked:?}");
    }
    let back = inst.event(&Input::Confirm)?;
    if plan(&back).is_some() {
        bail!("the fourth Enter in a row formatted the disk: `no` was not the first row");
    }
    if moved_to(&back) != Some("choose") {
        bail!("Enter on `no` did not go back a screen: {back:?}");
    }
    println!("refusal 2: an Enter in the rhythm goes back, instead of erasing the disk");

    // ---- AND THE DELIBERATE ONE GOES THROUGH ----
    inst.event(&Input::Confirm)?;
    inst.event(&Input::Down)?;

    // AND THE LAST SCREEN SPELLS OUT THE WHOLE GESTURE: which filesystem, onto
    // which node, under which name. It is the closest thing to the command
    // somebody would have typed, on the screen where being sure is the whole
    // question — and it is the one place all three facts appear together.
    let sure = look(&mut inst)?;
    if !sure.names("format-tech-large") {
        bail!(
            "the confirmation does not say which filesystem will be written: {:?}",
            sure.keys
        );
    }
    if !sure.shows("/dev/sdb1") || !sure.shows("FOTOS") {
        bail!(
            "the confirmation says neither where nor under what name: {:?}",
            sure.data
        );
    }

    let did = inst.event(&Input::Confirm)?;
    let Some(p) = plan(&did) else {
        bail!("`yes` chosen on purpose asked for nothing: {did:?}");
    };
    // THE ID IS THE ONE THAT WAS SENT, byte for byte. A buffer that built one
    // would be a buffer with an opinion about what a udisks object path is.
    if p.device != "/block/sdb1" {
        bail!(
            "the plan names `{}`, which is not the chosen disk",
            p.device
        );
    }
    if p.recipe != Recipe::LargeFiles {
        bail!(
            "the plan carries `{:?}`, and not the row that was under the cursor",
            p.recipe
        );
    }
    if p.label != "FOTOS" {
        bail!("the plan carries the name `{}`", p.label);
    }
    // AND `whole_disk` COMES OFF THE ROW. `/dev/sdb1` is one partition, so a
    // `true` here would wipe the partition table of the whole stick when
    // somebody asked to reformat one part of it.
    if p.whole_disk {
        bail!("the plan asked to erase the whole disk starting from a partition");
    }
    // AND IT WENT BACK TO THE LIST, which is where the row being written is.
    if moved_to(&did) != Some("normal") {
        bail!("the buffer did not go back to the list after asking: {did:?}");
    }
    let waiting = look(&mut inst)?;
    if !waiting.names("format-working") {
        bail!("nothing on screen says a disk is being written");
    }
    println!("the plan came out: id taken, filesystem chosen, name typed, right reach");

    // ---- WHAT CAME BACK ----
    let trouble = "could not format PHOTOS: in use".to_owned();
    inst.event(&Input::Formatted(Outcome {
        done: false,
        trouble: trouble.clone(),
    }))?;
    let told = look(&mut inst)?;
    if !told.shows(&trouble) {
        bail!(
            "the host's refusal did not reach the screen: {:?}",
            told.data
        );
    }
    if !told.names("format-title") {
        bail!("the buffer did not go back to the list after the refusal");
    }

    // ---- AND THE CURSOR IS AN ID, not a row ----
    //
    // A stick pulled out reorders everything below it. The cursor was on
    // `/block/sdb1`, which is row 1 here and row 2 after the reorder; an index
    // would now be pointing at the machine's root, one `Enter` away from a
    // screen asking whether to erase it.
    let mut shuffled = rack();
    shuffled.devices.reverse();
    inst.event(&Input::Rack(shuffled))?;
    let after = look(&mut inst)?;
    if after.selections != 1 {
        bail!(
            "the list highlighted {} rows; it should highlight exactly the cursor's",
            after.selections
        );
    }
    let followed = inst.event(&Input::Confirm)?;
    if moved_to(&followed) != Some("choose") {
        bail!("after the reordering the cursor landed on a disk that does not advance");
    }
    let where_now = look(&mut inst)?;
    if !where_now.shows("/dev/sdb1") {
        bail!(
            "the cursor did not follow the id through the reordering: {:?}",
            where_now.data
        );
    }

    // ---- AND EVERY LABEL FITS THE BUDGET, IN EVERY LANGUAGE ----
    //
    // A buffer cannot measure a sentence it does not have: it emits a KEY and
    // the host translates (invariant 11), so what it declares is what it
    // RESERVED. This measures every screen this run looked at, against both
    // `.ftl` files, with the ruler the host CUTS with.
    let measured = ronoctua_runtime::driven::every_label_fits(&i18n, ON_PURPOSE, COLUMNS)?;
    println!(
        "labels    {measured} measured in {} languages, all inside their budget",
        ronoctua_runtime::driven::languages(&i18n)?.len()
    );

    println!(
        "OK: the buffer refuses what the system needs, and an Enter in the rhythm erases nothing"
    );
    Ok(())
}

/// The keys of an `.ftl`, as one string.
fn ftl(path: &str) -> Result<String> {
    std::fs::read_to_string(path).with_context(|| format!("could not read {path}"))
}
