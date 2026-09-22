//! The one boundary with the keyboards — §7.4, one file later.
//!
//! # What this is for
//!
//! The compositor has ONE `input { keyboard { … } }` for every keyboard
//! attached to the machine: one layout, one repeat rate, no way to name a
//! device. That is the compositor's shape, not an oversight, and invariant 3 keeps it
//! out of reach. So "this keyboard, not that one" exists in exactly one place
//! on a Linux desktop: `keyd`, which sits below the compositor and matches
//! devices by `vendor:product`.
//!
//! This crate is the whole of that: which keyboards the kernel sees, what the
//! person decided about each one, and the files keyd reads.
//!
//! # THE ONE RULE THAT MATTERS MORE THAN THE FEATURE
//!
//! keyd's config says a device id may appear in ONE file. The distro's own
//! file claims `*`, so a keyboard with a rule of its own has to be taken OUT
//! of the catch-all — and a keyboard outside the catch-all has no rules at
//! all unless its own file repeats them. One of those rules is the leader
//! ([`LEADER`]), which is the key that opens everything.
//!
//! So every file this renders carries the leader, and [`render`] is written so
//! that forgetting is not possible: the rule goes in from one place, for every
//! file, and a test reads them all back.
//!
//! # And nothing here is privileged
//!
//! The directory is the machine's (`nix/modules/keyboard.nix` gives it to the
//! session's user through `systemd-tmpfiles`), the validation is `keyd check`
//! on a file we own, and the restart goes through the polkit rule that already
//! exists for `keyboard-restore` — one verb, one unit, one user.

use std::collections::BTreeMap;

use anyhow::{Context, Result};

// The vocabulary, GENERATED from `assets/keyboards.nix`. It defines `LEADER`,
// `KEYS` and `CHOICES` — see `nix/keyboards.nix`.
include!(concat!(env!("OUT_DIR"), "/keyboards.rs"));

/// The file the catch-all lives in — every keyboard nobody has touched.
pub const EVERYONE: &str = "default.conf";

/// Where the rendered files go, when nothing says otherwise.
///
/// A path and not a discovery: the module declares it, `systemd-tmpfiles`
/// makes it, and `/etc/keyd` is a symlink to it. Whoever runs this outside a
/// built system passes their own.
pub const DIRECTORY: &str = "/var/lib/ronoctua/keyd";

/// The `keyd` binary, by path — a unit's `PATH` is minimal.
const KEYD: Option<&str> = option_env!("RONOCTUA_KEYD");

/// How the kernel reports what is attached.
const DEVICES: &str = "/proc/bus/input/devices";

/// Three ordinary letters, by their kernel codes — `input-event-codes.h`.
const KEY_Q: u32 = 16;
const KEY_A: u32 = 30;
const KEY_SPACE: u32 = 57;

/// Does this `B: KEY=` bitmap carry that key?
///
/// The kernel prints the bitmap as hex words separated by spaces, most
/// significant FIRST — so the last word holds bits 0 to 63, the one before it
/// 64 to 127, and so on.
fn has_key(bitmap: &str, code: u32) -> bool {
    let words: Vec<&str> = bitmap.split_whitespace().collect();
    let word = usize::try_from(code / 64).unwrap_or(usize::MAX);
    let bit = code % 64;
    let Some(index) = words.len().checked_sub(word + 1) else {
        return false;
    };
    u64::from_str_radix(words[index], 16).is_ok_and(|w| w >> bit & 1 == 1)
}

/// One keyboard, as the kernel sees it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Keyboard {
    /// `vendor:product`, four hex digits each — what keyd matches on.
    ///
    /// NOT unique: two of the same model report the same pair, and keyd has
    /// no finer grain. A person with two identical keyboards configures them
    /// together, and the screen says so rather than pretending.
    pub id: String,
    /// What the kernel calls it.
    pub name: String,
    /// Is it built into this machine?
    ///
    /// `BUS_I8042` and `BUS_HOST` are the laptop's own; everything else
    /// arrived by cable or by radio. It is drawn because a list of two
    /// keyboards called "AT Translated Set 2 keyboard" and "Bluetooth
    /// Keyboard" is a list where only one of them is obvious.
    pub built_in: bool,
}

/// The `vendor:product` inside a BlueZ `Modalias`, which is what keyd matches.
///
/// BlueZ reports `usb:v3554pF605d0100`; the kernel's input device reports
/// `Vendor=3554 Product=f605`. They are the same two numbers in two spellings,
/// and this is the bridge — it is what lets a row on a Bluetooth device's page
/// open the keyboards ON that keyboard.
#[must_use]
pub fn id_of_modalias(modalias: &str) -> Option<String> {
    let rest = modalias.split_once(':')?.1;
    let (vendor, rest) = rest.strip_prefix('v')?.split_at_checked(4)?;
    let (product, _) = rest.strip_prefix('p')?.split_at_checked(4)?;
    let ok = |s: &str| s.chars().all(|c| c.is_ascii_hexdigit());
    (ok(vendor) && ok(product))
        .then(|| format!("{}:{}", vendor.to_lowercase(), product.to_lowercase()))
}

/// Every keyboard the kernel is reporting.
///
/// # Errors
///
/// `/proc/bus/input/devices` could not be read, which on Linux means `/proc`
/// is not mounted.
pub fn seen() -> Result<Vec<Keyboard>> {
    let text = std::fs::read_to_string(DEVICES).with_context(|| format!("reading {DEVICES}"))?;
    Ok(seen_in(&text))
}

/// The keyboards in what `/proc/bus/input/devices` printed.
///
/// A free function so the parsing is testable without a kernel: the fixture
/// beside it is this machine's own output, trimmed to the lines this reads and
/// with the addresses invented (invariant 13).
#[must_use]
pub fn seen_in(text: &str) -> Vec<Keyboard> {
    let mut out: Vec<Keyboard> = Vec::new();
    for block in text.split("\n\n") {
        let mut id = None;
        let mut name = None;
        let mut bus = None;
        let mut keys = false;
        let mut handler = false;
        for line in block.lines() {
            if let Some(rest) = line.strip_prefix("I: ") {
                let mut vendor = None;
                let mut product = None;
                for word in rest.split_whitespace() {
                    match word.split_once('=') {
                        Some(("Bus", v)) => bus = u16::from_str_radix(v, 16).ok(),
                        Some(("Vendor", v)) => vendor = Some(v.to_lowercase()),
                        Some(("Product", v)) => product = Some(v.to_lowercase()),
                        _ => {}
                    }
                }
                if let (Some(v), Some(p)) = (vendor, product) {
                    id = Some(format!("{v}:{p}"));
                }
            } else if let Some(rest) = line.strip_prefix("N: Name=") {
                name = Some(rest.trim_matches('"').to_owned());
            } else if let Some(rest) = line.strip_prefix("H: Handlers=") {
                handler = rest.split_whitespace().any(|h| h == "kbd");
            } else if let Some(rest) = line.strip_prefix("B: KEY=") {
                // THE `kbd` HANDLER IS NOT ENOUGH, and this machine says so.
                //
                // A power button, a lid switch, the video bus, the PC
                // speaker and the headphones' remote all carry `kbd`. Measured
                // on 2026-09-08: nine devices here carried the handler and TWO
                // were keyboards — a list of nine with two real ones is a list
                // nobody can use. keyd asks the same question the same way:
                // does it have the ordinary letters?
                //
                // The bitmap is hex words, the LAST one holding bits 0 to 63.
                keys = [KEY_Q, KEY_A, KEY_SPACE].iter().all(|c| has_key(rest, *c));
            }
        }
        let (Some(id), Some(name), true) = (id, name, keys && handler) else {
            continue;
        };
        // A DEVICE THAT REPORTS NO VENDOR IS THE KERNEL'S OWN — the lid
        // switch, the power button, the video bus. keyd cannot address them
        // and nobody wants to remap a lid.
        if id.starts_with("0000:") {
            continue;
        }
        let built_in = matches!(bus, Some(0x0011 | 0x0019 | 0x0006));
        let board = Keyboard { id, name, built_in };
        if !out.iter().any(|k| k.id == board.id) {
            out.push(board);
        }
    }
    out.sort();
    out
}

/// What one keyboard has been told to do.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Board {
    /// keyd's key name, to keyd's action. An empty map is a keyboard nobody
    /// has touched, and [`Preferences::set`] removes it rather than keeping it.
    #[serde(default)]
    pub keys: BTreeMap<String, String>,
}

/// What the person decided, for every keyboard they have touched.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Preferences {
    /// By `vendor:product`.
    #[serde(default)]
    pub boards: BTreeMap<String, Board>,
}

/// The file this lives in, under `$XDG_CONFIG_HOME/ronoctua`.
pub const FILE: &str = "keyboards.json";

impl Preferences {
    /// What was decided about this keyboard — nothing, by default.
    #[must_use]
    pub fn of(&self, id: &str) -> Board {
        self.boards.get(id).cloned().unwrap_or_default()
    }

    /// Remembers what a key becomes, and FORGETS a keyboard that says nothing.
    ///
    /// An empty action is "leave it alone", which is the absence of a rule and
    /// not a rule that says nothing — written down it would be a line keyd has
    /// to parse for no reason. And a keyboard with no rules left leaves the
    /// file: a map that only grows becomes a list of every keyboard that ever
    /// touched the machine, each one saying nothing. It is the rule
    /// `display::Preferences::set` follows, for the same reason.
    pub fn set(&mut self, id: &str, key: &str, becomes: &str) {
        let board = self.boards.entry(id.to_owned()).or_default();
        if becomes.is_empty() {
            board.keys.remove(key);
        } else {
            board.keys.insert(key.to_owned(), becomes.to_owned());
        }
        if board.keys.is_empty() {
            self.boards.remove(id);
        }
    }
}

/// The store this lives in.
#[must_use]
pub fn store() -> ronoctua_store::Store<Preferences> {
    ronoctua_store::Store::at(ronoctua_store::config_dir(), FILE)
}

/// The files keyd should be reading, given what the person decided.
///
/// The first is always the catch-all, and every file carries [`LEADER`] —
/// see this module's header for why that is the rule and not a detail.
#[must_use]
pub fn render(prefs: &Preferences) -> Vec<(String, String)> {
    let (key, action) = LEADER;
    let leader = format!("{key} = {action}\n");
    let mine: Vec<&String> = prefs.boards.keys().collect();

    // THE CATCH-ALL, LESS WHOEVER LEFT IT. keyd reads `*` as "every keyboard"
    // and `-vvvv:pppp` as "except this one"; a device named in two files is
    // what its manual forbids.
    let mut everyone = String::from("[ids]\n*\n");
    for id in &mine {
        everyone.push_str(&format!("-{id}\n"));
    }
    everyone.push_str("\n[main]\n");
    everyone.push_str(&leader);

    let mut out = vec![(EVERYONE.to_owned(), everyone)];
    for id in mine {
        let mut text = format!("[ids]\n{id}\n\n[main]\n");
        text.push_str(&leader);
        for (from, to) in &prefs.boards[id].keys {
            // THE LEADER'S OWN KEY IS NOT OVERWRITTEN by a remap of it: the
            // line above already claims it, and keyd takes the last one. A
            // person who asked for `leftmeta = esc` gets it, and loses the
            // leader on that keyboard — which is their decision and is why the
            // screen says so before it happens.
            text.push_str(&format!("{from} = {to}\n"));
        }
        out.push((format!("{}.conf", id.replace(':', "-")), text));
    }
    out
}

/// Is there anywhere to put the rules on this machine?
///
/// # Why the screen asks BEFORE anybody presses a key
///
/// The directory is made by `systemd-tmpfiles`, on the installed system. In
/// the nested loop (`nix run .#dev`) the host is somebody else's distro and
/// there is no such place — and without this the first arrow would come back
/// saying *keyd would not take these rules*, which blames the daemon for a
/// directory nobody created. Ninety per cent of the work on this system
/// happens in that loop.
#[must_use]
pub fn somewhere_to_write(directory: &str) -> bool {
    std::path::Path::new(directory).is_dir()
}

/// Writes the files, after asking keyd whether it can read them.
///
/// # Why it validates first, into a directory of its own
///
/// Because installing a file keyd refuses leaves the daemon dead, and a dead
/// keyd is a keyboard with no leader on a machine somebody is holding. `keyd
/// check` is the daemon's own parser; the files go in only when it says yes.
///
/// # Errors
///
/// The directory could not be written, or keyd refused what was rendered — in
/// which case NOTHING is installed and the message carries what it said.
pub fn install(directory: &str, files: &[(String, String)]) -> Result<()> {
    let scratch = format!("{directory}/.new");
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).with_context(|| format!("making {scratch}"))?;
    for (name, text) in files {
        std::fs::write(format!("{scratch}/{name}"), text)
            .with_context(|| format!("writing {name}"))?;
    }
    let written: Vec<String> = files
        .iter()
        .map(|(name, _)| format!("{scratch}/{name}"))
        .collect();
    check(&written).inspect_err(|_| {
        let _ = std::fs::remove_dir_all(&scratch);
    })?;

    for (name, text) in files {
        std::fs::write(format!("{directory}/{name}"), text)
            .with_context(|| format!("installing {name}"))?;
    }
    // AND WHATEVER IS LEFT OVER GOES. A keyboard that stopped having rules
    // left `Preferences`, so its file is no longer rendered — and a file keyd
    // still reads is a rule nobody can see and nobody asked for.
    let wanted: Vec<&str> = files.iter().map(|(n, _)| n.as_str()).collect();
    if let Ok(entries) = std::fs::read_dir(directory) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.ends_with(".conf") && !wanted.contains(&name.as_str()) {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
    let _ = std::fs::remove_dir_all(&scratch);
    Ok(())
}

/// Asks keyd whether it can read these files.
///
/// # It takes FILES, and that is not a detail
///
/// `keyd check` accepts a list of config FILES; with no argument it checks
/// whatever is in `/etc/keyd`, which is the installed configuration and not
/// the one about to be installed. Measured against the binary, because the
/// three plausible spellings do three different things:
///
/// ```text
/// keyd check good.conf bad.conf   exit 255   what this uses
/// keyd check -c <directory>       exit 255   `-c` is read as a filename
/// keyd check <directory>          exit   0   a directory parses as nothing
/// ```
///
/// The middle one refuses everything and the last one refuses nothing. The
/// first version of this used the middle one, which would have made the whole
/// feature a screen that never wrote anything; the third is worse, because it
/// is a guard that says yes to a file that would kill the daemon.
///
/// # Errors
///
/// keyd is not in this build, did not run, or refused — and then the message
/// is its own.
pub fn check(files: &[String]) -> Result<()> {
    let program = KEYD.context("this build has no keyd")?;
    if files.is_empty() {
        anyhow::bail!("nothing to check: keyd with no file would read the installed one");
    }
    let out = std::process::Command::new(program)
        .arg("check")
        .args(files)
        .output()
        .context("running keyd check")?;
    if !out.status.success() {
        let said = String::from_utf8_lossy(&out.stderr);
        let said = if said.trim().is_empty() {
            String::from_utf8_lossy(&out.stdout).trim().to_owned()
        } else {
            said.trim().to_owned()
        };
        anyhow::bail!("keyd refused the configuration: {said}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// This machine's own `/proc/bus/input/devices`, trimmed to the lines the
    /// parser reads. The Bluetooth keyboard's address is invented (invariant
    /// 13); the vendor and product pairs are the manufacturers'.
    const DEVICES_FIXTURE: &str = "\
I: Bus=0019 Vendor=0000 Product=0001 Version=0000
N: Name=\"Power Button\"
H: Handlers=kbd event1
B: KEY=10000000000000 0

I: Bus=0011 Vendor=0001 Product=0001 Version=ab83
N: Name=\"AT Translated Set 2 keyboard\"
H: Handlers=sysrq kbd leds event3
B: KEY=402000007 ff803078f800d001 feffffdfffcfffff fffffffffffffffe

I: Bus=0005 Vendor=3554 Product=f605 Version=0100
N: Name=\"Bluetooth Keyboard\"
U: Uniq=aa:bb:cc:dd:ee:ff
H: Handlers=sysrq kbd leds event4
B: KEY=e080ffdf01cfffff fffffffffffffffe

I: Bus=0010 Vendor=001f Product=0001 Version=0100
N: Name=\"PC Speaker\"
H: Handlers=kbd event6
B: KEY=40000000000000 0

I: Bus=0005 Vendor=054c Product=0d58 Version=0001
N: Name=\"WH-1000XM4 (AVRCP)\"
H: Handlers=kbd event7
B: KEY=2fc800 145200000000 0 10300 49e800000c00 e16800000000f f810000010000ffc

I: Bus=0003 Vendor=046d Product=b036 Version=0111
N: Name=\"Pebble M350s Mouse\"
H: Handlers=mouse0 event5
B: KEY=1f0000 0 0 0
";

    #[test]
    fn a_modalias_and_a_kernel_id_are_the_same_two_numbers() {
        // BlueZ says this about the owner's keyboard; the kernel says
        // `Vendor=3554 Product=f605` about the same unit. A row on the
        // device's page opens the keyboards on it through this.
        assert_eq!(
            id_of_modalias("usb:v3554pF605d0100").as_deref(),
            Some("3554:f605")
        );
        assert_eq!(
            id_of_modalias("bluetooth:v05D6p000Ad0240").as_deref(),
            Some("05d6:000a")
        );
        // AND ANYTHING ELSE IS NOT ONE. A device that reports no modalias, or
        // one shaped differently, opens the LIST — which is a screen, and not
        // a wrong keyboard under the cursor.
        assert_eq!(id_of_modalias(""), None);
        assert_eq!(id_of_modalias("usb:vZZZZpF605d0100"), None);
        assert_eq!(id_of_modalias("nonsense"), None);
    }

    #[test]
    fn the_two_keyboards_are_seen_and_the_lid_is_not() {
        let seen = seen_in(DEVICES_FIXTURE);
        let ids: Vec<&str> = seen.iter().map(|k| k.id.as_str()).collect();
        assert_eq!(ids, vec!["0001:0001", "3554:f605"]);
        // FOUR OF THE SIX CARRY `kbd` AND ARE NOT KEYBOARDS, and every one of
        // them is on this machine: the power button (no vendor at all), the PC
        // speaker (one key, and it is a bell), the headphones' remote (media
        // keys, no letters) and the mouse (no `kbd` handler). A list of six
        // with two real ones is a list nobody can use.
        assert!(!ids.contains(&"0000:0001"), "the power button");
        assert!(!ids.contains(&"001f:0001"), "the PC speaker");
        assert!(!ids.contains(&"054c:0d58"), "the headphones' remote");
        assert!(!ids.iter().any(|i| i.starts_with("046d")), "the mouse");
    }

    #[test]
    #[ignore = "reads this machine's kernel, for looking at with --nocapture"]
    fn look_at_this_machine() {
        for board in seen().expect("/proc is mounted") {
            let where_from = if board.built_in {
                "built in"
            } else {
                "attached"
            };
            println!("  {:<12} {:<9} {}", board.id, where_from, board.name);
        }
    }

    #[test]
    fn the_built_in_one_says_so() {
        let seen = seen_in(DEVICES_FIXTURE);
        let laptop = seen.iter().find(|k| k.id == "0001:0001").unwrap();
        assert!(laptop.built_in, "the i8042 keyboard is the machine's own");
        let bluetooth = seen.iter().find(|k| k.id == "3554:f605").unwrap();
        assert!(!bluetooth.built_in);
        assert_eq!(bluetooth.name, "Bluetooth Keyboard");
    }

    #[test]
    fn nothing_decided_renders_one_file_with_the_leader() {
        let files = render(&Preferences::default());
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, EVERYONE);
        assert!(files[0].1.contains("[ids]\n*\n"));
        assert!(files[0].1.contains(&format!("{} = {}", LEADER.0, LEADER.1)));
    }

    #[test]
    fn every_rendered_file_carries_the_leader() {
        // THE RULE THIS CRATE EXISTS TO NOT BREAK. A keyboard with a rule of
        // its own leaves the catch-all, so its own file has to carry the key
        // that opens everything — or configuring a keyboard is how you lose
        // the leader on it.
        let mut prefs = Preferences::default();
        prefs.set("3554:f605", "capslock", "overload(control, esc)");
        prefs.set("0001:0001", "capslock", "esc");
        let files = render(&prefs);
        assert_eq!(files.len(), 3, "the catch-all and one file per keyboard");
        for (name, text) in &files {
            assert!(
                text.contains(&format!("{} = {}", LEADER.0, LEADER.1)),
                "`{name}` has no leader"
            );
            assert!(
                text.starts_with("[ids]\n"),
                "`{name}` does not open with ids"
            );
        }
    }

    #[test]
    fn a_configured_keyboard_leaves_the_catch_all() {
        // keyd's manual: a device id may be listed in a single config file.
        // Two files claiming one keyboard is undefined behaviour in a daemon
        // that owns the keyboard.
        let mut prefs = Preferences::default();
        prefs.set("3554:f605", "capslock", "esc");
        let files = render(&prefs);
        let everyone = &files[0].1;
        assert!(everyone.contains("-3554:f605\n"), "still in the catch-all");
        assert!(files[1].0 == "3554-f605.conf");
        assert!(files[1].1.contains("capslock = esc\n"));
    }

    #[test]
    fn leaving_a_key_alone_takes_the_keyboard_out_of_the_file() {
        let mut prefs = Preferences::default();
        prefs.set("3554:f605", "capslock", "esc");
        assert_eq!(prefs.boards.len(), 1);
        // THE WAY BACK. An empty action is the absence of a rule, and a
        // keyboard with no rules left is not a keyboard worth remembering —
        // otherwise the file becomes a list of every keyboard that ever
        // touched the machine, each one saying nothing.
        prefs.set("3554:f605", "capslock", "");
        assert!(prefs.boards.is_empty());
        assert_eq!(render(&prefs).len(), 1);
    }

    #[test]
    fn the_vocabulary_offers_a_way_back_first() {
        assert_eq!(
            CHOICES[0].0, "",
            "the first choice has to be `leave it alone`"
        );
        assert!(CHOICES.len() > 3);
        assert!(!KEYS.is_empty());
        // AND THE LEADER'S KEY IS AMONG THE ONES OFFERED, because a person
        // who wants it somewhere else has to be able to say so.
        assert!(KEYS.iter().any(|(k, _)| *k == LEADER.0));
    }
}
