//! THE BRAIN'S MODULES AS DATA — what the footer draws of every module, read
//! at run time from `brain-catalogue.json` (the 2026-10-12 wave, B2).
//!
//! # What it is
//!
//! For each module the build compiled — the distro's and the person's — its
//! marks (each with the TOKEN of its glyph and the key of its hint), its
//! notices, the actions a person may ask of it by name (the command, the
//! label's key, whether it acts on a row, the question it asks first), the
//! events it may tell (each with the notification it raises, as the
//! notification book reads one — `ronoctua_notify` merges those itself), its
//! reasons, its text prefixes and every text key it needs; and the GLYPHS
//! its marks are drawn with, measured against the real face (the codepoint,
//! the size beside the text, the ink across). Nix writes it
//! (`nix/brain-catalogue.nix`), out of the same folders the daemon compiles;
//! the wrappers point [`ENV`] at it.
//!
//! # Why data, and not the compiled tables it was
//!
//! The glyphs lived in the icon table every crate that draws compiles, the
//! notifications in the table `ronoctua_notify` compiles, the keys in the
//! registry every crate that speaks compiles, and the actions in this crate:
//! a module of the person's rebuilt the whole footer on every machine of
//! theirs. Read here at run time, a person's module moves this file, the
//! texts and the daemon, and no crate that draws — and the footer registers
//! the person's actions as it registers the distro's.
//!
//! # Refused loudly, never half-read
//!
//! A program that draws a mark reads it once ([`load`], kept for the
//! process: the file is a store path). Unset, unreadable, not this format,
//! or lacking keys its texts must have, it is REFUSED — at the footer's
//! start, one English line naming [`ENV`] and exit [`EXIT_MISSING`]
//! ([`checked`], [`refuse`]); anywhere else, a panic naming it
//! ([`catalogue`]). A footer that went on without it would register none of
//! the modules' actions and draw every mark with the brain's own glyph, and
//! nothing would say why.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::module::{Level, Manifest};

/// The variable naming the catalogue's file, which the wrappers set.
pub const ENV: &str = "RONOCTUA_BRAIN_CATALOGUE";

/// The file's format: the one `gen-brain-catalogue.py` writes. A file of
/// another is another build's, and refused.
pub const VERSION: u32 = 1;

/// What a program exits with when its catalogue cannot be had: `EX_CONFIG`
/// of `sysexits.h`, as for the texts — the environment, not the person and
/// not a bug.
pub const EXIT_MISSING: u8 = 78;

/// Every module the build compiled, as the footer reads them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Catalogue {
    /// The format, [`VERSION`].
    pub version: u32,
    /// By id, the order the daemon's registry holds them in.
    pub modules: Vec<Module>,
    /// Every glyph a module declared, measured, by token.
    pub glyphs: Vec<Glyph>,
}

/// Whose a module is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Origin {
    /// One of `crates/ronoctua-brain/modules/`.
    Distro,
    /// One the person wrote (`my-…`).
    Person,
}

/// One module, as the footer reads it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Module {
    /// The folder's name.
    pub id: String,
    /// Whose it is.
    pub origin: Origin,
    /// `brain-task-<id>`: its name on screen.
    pub name_key: String,
    /// `brain-task-<id>-does`: the line under the cursor.
    pub does_key: String,
    /// What its text keys may begin with, its id first.
    pub prefixes: Vec<String>,
    /// The marks it may put on the modeline, by name.
    pub marks: Vec<Mark>,
    /// The notices it may raise, by id.
    pub notices: Vec<Notice>,
    /// What a person may ask of it by name, by id.
    pub actions: Vec<Action>,
    /// What it may tell the footer, by name.
    pub events: Vec<Event>,
    /// The words it may give as a reason, in its manifest's order.
    pub whys: Vec<Why>,
    /// Every text key it needs — the keys of all of the above, its
    /// notifications' included — sorted: what the footer's start asks of the
    /// texts it is about to speak.
    pub keys: Vec<String>,
}

/// A mark a module may put on the modeline, as declared.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Mark {
    /// Its name inside the module.
    pub name: String,
    /// Its glyph's token, `<module>-<glyph>` — one of [`Catalogue::glyphs`].
    pub glyph: String,
    /// The key of the line that explains it under the pointer.
    pub hint: String,
    /// How many of it the module may show at once; `0` is a mark that is one.
    pub instances: u8,
    /// Whether it may be drawn at rest.
    pub resting: bool,
}

/// A notice a module may raise, as declared.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Notice {
    /// Its id inside the module.
    pub id: String,
    /// How loud.
    pub level: Level,
    /// Whether a person may dismiss it.
    pub dismiss: bool,
    /// The snoozes offered, in minutes.
    pub snooze: Vec<u16>,
}

/// Something a person may ask of a module by name, as declared.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Action {
    /// Its id inside the module.
    pub id: String,
    /// The registry's command, `<module>-<action>`.
    pub command: String,
    /// The command's label, `cmd-<module>-<action>`.
    pub label: String,
    /// Whether it acts on a row of the module's screen.
    pub row: bool,
    /// The key of the question it asks first, if it asks one.
    pub asks: Option<String>,
}

/// Something a module may tell the footer, as declared.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Event {
    /// Its name inside the module.
    pub name: String,
    /// The tag it travels under, `<module>-<name>`.
    pub tag: String,
    /// The notification it raises, as an entry of the notification book —
    /// `ronoctua_notify`'s own shape, which that crate reads (`brain`); this
    /// one only carries it.
    pub notification: Option<serde_json::Value>,
}

/// A word a module may give as a reason, and its key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Why {
    /// The word.
    pub word: String,
    /// `<module>-why-<word>`.
    pub key: String,
}

/// A module's glyph, as the glyph generator measured it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Glyph {
    /// `<module>-<name>`: what a mark names (invariant 10 — never a codepoint).
    pub token: String,
    /// The font's name for it (`md-<name>`).
    pub glyph: String,
    /// Where the face holds it.
    pub codepoint: u32,
    /// The size it is drawn at beside text of the base size — measured
    /// against the digit, as every chrome glyph's is
    /// (`ronoctua_tokens::icon::chrome_size`).
    pub size: f32,
    /// Its ink's height against the digit's; `None` for a glyph with no ink.
    pub ink: Option<f32>,
    /// Where its ink begins and ends across, in ems, from the pen's origin.
    pub across: Option<[f32; 2]>,
}

impl Glyph {
    /// The glyph as a character, or `None` for a codepoint that is not one.
    #[must_use]
    pub fn char(&self) -> Option<char> {
        char::from_u32(self.codepoint)
    }
}

impl Catalogue {
    /// A catalogue out of its file's text — this format, and whole.
    ///
    /// # Errors
    ///
    /// A sentence saying what is wrong: not JSON of this shape (a field this
    /// program does not know is one), another format, a mark naming a glyph
    /// the file does not measure, a glyph that is no character, or two
    /// modules answering for one command or one tag.
    pub fn of_json(text: &str) -> Result<Self, String> {
        // THE FORMAT FIRST, so a newer file is said as one and not as the
        // first field this program does not know.
        #[derive(Deserialize)]
        struct Format {
            version: u32,
        }
        let format: Format = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if format.version != VERSION {
            return Err(format!(
                "it is format {}, and this program reads format {VERSION}",
                format.version
            ));
        }
        let read: Self = serde_json::from_str(text).map_err(|e| e.to_string())?;
        for g in &read.glyphs {
            if g.char().is_none() {
                return Err(format!(
                    "the glyph `{}` is at {:#x}, which is no character",
                    g.token, g.codepoint
                ));
            }
        }
        let mut commands = std::collections::BTreeSet::new();
        let mut tags = std::collections::BTreeSet::new();
        for m in &read.modules {
            for mark in &m.marks {
                if read.glyph(&mark.glyph).is_none() {
                    return Err(format!(
                        "the mark `{}` of `{}` names the glyph `{}`, which the file does not measure",
                        mark.name, m.id, mark.glyph
                    ));
                }
            }
            for a in &m.actions {
                if !commands.insert(a.command.as_str()) {
                    return Err(format!("two actions are the command `{}`", a.command));
                }
            }
            for e in &m.events {
                if !tags.insert(e.tag.as_str()) {
                    return Err(format!("two events are told under `{}`", e.tag));
                }
            }
        }
        Ok(read)
    }

    /// A module's glyph token, as a character — only a token the file
    /// measured answers.
    #[must_use]
    pub fn glyph(&self, token: &str) -> Option<char> {
        self.glyphs
            .iter()
            .find(|g| g.token == token)
            .and_then(Glyph::char)
    }

    /// The measured glyph drawn as this character, if a module's glyph is it.
    #[must_use]
    pub fn measured(&self, c: char) -> Option<&Glyph> {
        self.glyphs.iter().find(|g| g.codepoint == u32::from(c))
    }

    /// One module, by its id.
    #[must_use]
    pub fn module(&self, id: &str) -> Option<&Module> {
        self.modules.iter().find(|m| m.id == id)
    }

    /// Every action of every module, with its module, in the registry's
    /// order: what the leader registers as a command each.
    pub fn actions(&self) -> impl Iterator<Item = (&Module, &Action)> {
        self.modules
            .iter()
            .flat_map(|m| m.actions.iter().map(move |a| (m, a)))
    }

    /// The action a command names, with its module.
    #[must_use]
    pub fn action(&self, command: &str) -> Option<(&Module, &Action)> {
        self.actions().find(|(_, a)| a.command == command)
    }

    /// Every event of every module, with its module, in the registry's order.
    pub fn events(&self) -> impl Iterator<Item = (&Module, &Event)> {
        self.modules
            .iter()
            .flat_map(|m| m.events.iter().map(move |e| (m, e)))
    }

    /// The keys of every module that `has` does not answer, sorted and once
    /// each.
    #[must_use]
    pub fn lacking(&self, has: impl Fn(&str) -> bool) -> Vec<String> {
        let mut out: Vec<String> = self
            .modules
            .iter()
            .flat_map(|m| m.keys.iter())
            .filter(|k| !has(k))
            .cloned()
            .collect();
        out.sort();
        out.dedup();
        out
    }
}

/// What a module DECLARES in its manifest, in the catalogue's words: the half
/// the daemon that compiled the module and the catalogue the footer reads
/// must agree on. The daemon prints it (`ronoctua-brain modules --catalogue`)
/// and a check compares the two; this crate's test does the same for the
/// distro's manifests it compiles.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Declared {
    /// The id.
    pub id: String,
    /// Its name's key.
    pub name_key: String,
    /// Its line's key.
    pub does_key: String,
    /// Its marks.
    pub marks: Vec<Mark>,
    /// Its notices.
    pub notices: Vec<Notice>,
    /// Its actions.
    pub actions: Vec<Action>,
    /// Its events, by name and tag (the notification each raises is the
    /// manifest's word the daemon never reads).
    pub events: Vec<DeclaredEvent>,
    /// Its reasons.
    pub whys: Vec<Why>,
}

/// An event as a manifest declares it: its name and its tag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct DeclaredEvent {
    /// Its name inside the module.
    pub name: String,
    /// `<module>-<name>`.
    pub tag: String,
}

impl Declared {
    /// What a compiled manifest declares.
    #[must_use]
    pub fn of(m: &Manifest) -> Self {
        Self {
            id: m.id.to_owned(),
            name_key: m.name_key.to_owned(),
            does_key: m.does_key.to_owned(),
            marks: m
                .marks
                .iter()
                .map(|k| Mark {
                    name: k.name.to_string(),
                    glyph: k.glyph.to_string(),
                    hint: k.hint.to_string(),
                    instances: k.instances,
                    resting: k.resting,
                })
                .collect(),
            notices: m
                .notices
                .iter()
                .map(|n| Notice {
                    id: n.id.to_owned(),
                    level: n.level,
                    dismiss: n.dismiss,
                    snooze: n.snooze.to_vec(),
                })
                .collect(),
            actions: m
                .actions
                .iter()
                .map(|a| Action {
                    id: a.id.to_owned(),
                    command: a.command.to_owned(),
                    label: a.label.to_owned(),
                    row: a.row,
                    asks: a.asks.map(str::to_owned),
                })
                .collect(),
            events: m
                .events
                .iter()
                .map(|e| DeclaredEvent {
                    name: e.name.to_owned(),
                    tag: e.tag.to_owned(),
                })
                .collect(),
            whys: m
                .whys
                .iter()
                .map(|w| Why {
                    word: w.word.to_owned(),
                    key: w.key.to_owned(),
                })
                .collect(),
        }
    }
}

impl Module {
    /// What this module's manifest declared, as the catalogue says it.
    #[must_use]
    pub fn declared(&self) -> Declared {
        Declared {
            id: self.id.clone(),
            name_key: self.name_key.clone(),
            does_key: self.does_key.clone(),
            marks: self.marks.clone(),
            notices: self.notices.clone(),
            actions: self.actions.clone(),
            events: self
                .events
                .iter()
                .map(|e| DeclaredEvent {
                    name: e.name.clone(),
                    tag: e.tag.clone(),
                })
                .collect(),
            whys: self.whys.clone(),
        }
    }
}

/// Why the catalogue could not be had. Each says so naming [`ENV`], which is
/// the one thing a person can fix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Missing {
    /// The variable is not set, or is empty.
    Unset,
    /// The file could not be read.
    Unreadable {
        /// The file.
        file: PathBuf,
        /// What the system answered.
        why: String,
    },
    /// The file is not a catalogue this program reads.
    Unparsable {
        /// The file.
        file: PathBuf,
        /// What is wrong with it.
        why: String,
    },
    /// The texts lack keys the catalogue's modules need: the two come from
    /// different builds.
    Keys {
        /// The file.
        file: PathBuf,
        /// The keys lacking, sorted.
        keys: Vec<String>,
    },
}

/// How many of the lacking keys a refusal names before it only counts them.
const NAMED: usize = 5;

impl fmt::Display for Missing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unset => write!(
                f,
                "{ENV} is not set: what the footer draws of the brain's modules — their marks, \
                 the actions it registers, their notifications — is data the build writes \
                 beside the program (`brain-catalogue.json`), and nothing here draws them \
                 without it — run the program through its wrapper or the session, or inside \
                 `nix develop`"
            ),
            Self::Unreadable { file, why } => {
                write!(
                    f,
                    "{ENV} points at {}, which cannot be read: {why}",
                    file.display()
                )
            }
            Self::Unparsable { file, why } => write!(
                f,
                "{ENV} points at {}, which is not a brain catalogue this program reads: {why}",
                file.display()
            ),
            Self::Keys { file, keys } => {
                let named: Vec<&str> = keys.iter().take(NAMED).map(String::as_str).collect();
                let more = keys.len().saturating_sub(NAMED);
                write!(
                    f,
                    "{ENV} points at {}, whose modules need {} keys the texts lack ({}{}): the \
                     catalogue and the texts come from different builds",
                    file.display(),
                    keys.len(),
                    named.join(", "),
                    if more > 0 {
                        format!(" and {more} more")
                    } else {
                        String::new()
                    }
                )
            }
        }
    }
}

impl std::error::Error for Missing {}

/// The catalogue the variable names, read once: the file is a store path,
/// and what it holds cannot change under a running process. A refusal is
/// kept too, so every later ask says the same thing.
static LOADED: OnceLock<Result<Catalogue, Missing>> = OnceLock::new();

/// The catalogue at `file`, read and parsed. Pure in what it is handed: the
/// tests point it at files they planted.
fn read(file: Option<&Path>) -> Result<Catalogue, Missing> {
    let file = file.ok_or(Missing::Unset)?;
    let text = std::fs::read_to_string(file).map_err(|e| Missing::Unreadable {
        file: file.to_owned(),
        why: e.to_string(),
    })?;
    Catalogue::of_json(&text).map_err(|why| Missing::Unparsable {
        file: file.to_owned(),
        why,
    })
}

/// The file [`ENV`] names, or `None` for none (unset or empty).
fn named() -> Option<PathBuf> {
    std::env::var_os(ENV)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// The catalogue [`ENV`] names, read the first time anything asks.
///
/// # Errors
///
/// [`Missing`], which says why naming [`ENV`].
pub fn load() -> Result<&'static Catalogue, Missing> {
    LOADED
        .get_or_init(|| read(named().as_deref()))
        .as_ref()
        .map_err(Clone::clone)
}

/// The catalogue, if this process already read it — never reading it here.
///
/// For what may be asked where no module's glyph can be drawn yet (the size
/// of a glyph, `ronoctua_shell_kit::glyph`): a module's codepoint only ever
/// reaches a painter through [`Catalogue::glyph`], which read the file first.
#[must_use]
pub fn loaded() -> Option<&'static Catalogue> {
    LOADED.get().and_then(|r| r.as_ref().ok())
}

/// The catalogue, for a program that has passed its start guard.
///
/// # Panics
///
/// When it cannot be had, saying why naming [`ENV`]: the footer's start
/// ([`checked`]) has already refused that case, so only a program run
/// without its wrapper — or a test without the variable — meets it.
#[must_use]
pub fn catalogue() -> &'static Catalogue {
    load().unwrap_or_else(|missing| panic!("{missing}"))
}

/// [`load`], and every key its modules need is one `has` answers — the
/// start guard's question, asked of the texts the program is about to speak.
///
/// # Errors
///
/// [`Missing`]: the catalogue cannot be read, is not this format, or needs
/// keys the texts lack (another build's).
pub fn checked(has: impl Fn(&str) -> bool) -> Result<&'static Catalogue, Missing> {
    let catalogue = load()?;
    let keys = catalogue.lacking(has);
    if keys.is_empty() {
        return Ok(catalogue);
    }
    Err(Missing::Keys {
        file: named().unwrap_or_default(),
        keys,
    })
}

/// Says why the catalogue could not be had — one English line on stderr, the
/// program's name first — and exits with [`EXIT_MISSING`].
///
/// English and not a key, as the texts' own refusal: the line exists for
/// when the program's data is not what it was built with.
pub fn refuse(missing: &Missing) -> ! {
    let program = std::env::args_os()
        .next()
        .map(PathBuf::from)
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_default();
    eprintln!("{program}: {missing}");
    std::process::exit(i32::from(EXIT_MISSING))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A catalogue of two made-up modules, written here: the shape the
    /// generator writes, with nothing a real module declares.
    fn sample() -> serde_json::Value {
        serde_json::json!({
            "version": VERSION,
            "modules": [
                {
                    "id": "made-up",
                    "origin": "distro",
                    "name-key": "brain-task-made-up",
                    "does-key": "brain-task-made-up-does",
                    "prefixes": ["made-up"],
                    "marks": [{
                        "name": "full", "glyph": "made-up-full", "hint": "made-up-mark-full",
                        "instances": 0, "resting": false
                    }],
                    "notices": [{ "id": "due", "level": "warn", "dismiss": true, "snooze": [60] }],
                    "actions": [{
                        "id": "clean", "command": "made-up-clean", "label": "cmd-made-up-clean",
                        "row": false, "asks": "made-up-clean-asks"
                    }],
                    "events": [{ "name": "done", "tag": "made-up-done", "notification": { "id": "made-up-done" } }],
                    "whys": [{ "word": "busy", "key": "made-up-why-busy" }],
                    "keys": [
                        "brain-task-made-up", "brain-task-made-up-does", "cmd-made-up-clean",
                        "made-up-clean-asks", "made-up-mark-full", "made-up-why-busy"
                    ]
                },
                {
                    "id": "my-own",
                    "origin": "person",
                    "name-key": "brain-task-my-own",
                    "does-key": "brain-task-my-own-does",
                    "prefixes": ["my-own"],
                    "marks": [],
                    "notices": [],
                    "actions": [{
                        "id": "go", "command": "my-own-go", "label": "cmd-my-own-go",
                        "row": true, "asks": null
                    }],
                    "events": [],
                    "whys": [],
                    "keys": ["brain-task-my-own", "brain-task-my-own-does", "cmd-my-own-go"]
                }
            ],
            "glyphs": [{
                "token": "made-up-full", "glyph": "md-made_up", "codepoint": 0xF0B7F_u32,
                "size": 22.0, "ink": 1.0, "across": [0.05, 0.55]
            }]
        })
    }

    fn of(v: &serde_json::Value) -> Result<Catalogue, String> {
        Catalogue::of_json(&v.to_string())
    }

    /// FAILS IF: a catalogue of this format is not read whole, or reading it
    /// answers a mark's glyph, an action, an event or a key that it does not
    /// hold — the person's module as the distro's.
    #[test]
    fn a_catalogue_is_read_whole_and_answers_what_it_holds() {
        let c = of(&sample()).expect("the sample reads");
        assert_eq!(c.glyph("made-up-full"), char::from_u32(0xF0B7F));
        assert_eq!(c.glyph("made-up-empty"), None);
        assert_eq!(
            c.measured(char::from_u32(0xF0B7F).unwrap()).map(|g| g.size),
            Some(22.0)
        );
        assert!(c.measured('a').is_none());
        let commands: Vec<&str> = c.actions().map(|(_, a)| a.command.as_str()).collect();
        assert_eq!(commands, ["made-up-clean", "my-own-go"]);
        let (m, a) = c.action("my-own-go").expect("the person's action");
        assert_eq!(
            (m.id.as_str(), m.origin, a.row),
            ("my-own", Origin::Person, true)
        );
        assert!(c.action("files-open").is_none());
        let tags: Vec<&str> = c.events().map(|(_, e)| e.tag.as_str()).collect();
        assert_eq!(tags, ["made-up-done"]);
        assert_eq!(
            c.lacking(|k| k != "cmd-my-own-go" && k != "made-up-why-busy"),
            ["cmd-my-own-go", "made-up-why-busy"]
        );
        assert!(c.lacking(|_| true).is_empty());
    }

    /// FAILS IF: a file of another format, a field this program does not
    /// know, a mark naming a glyph the file does not measure, a codepoint
    /// that is no character, or two modules on one command or one tag is
    /// read rather than refused with its sentence.
    #[test]
    fn a_catalogue_that_is_not_this_builds_is_refused_by_its_sentence() {
        let mut newer = sample();
        newer["version"] = serde_json::json!(VERSION + 1);
        assert!(of(&newer).unwrap_err().contains("format"));
        let mut stranger = sample();
        stranger["modules"][0]["colour"] = serde_json::json!("red");
        assert!(of(&stranger).unwrap_err().contains("colour"));
        let mut unmeasured = sample();
        unmeasured["modules"][0]["marks"][0]["glyph"] = serde_json::json!("made-up-empty");
        assert!(of(&unmeasured).unwrap_err().contains("does not measure"));
        let mut no_char = sample();
        no_char["glyphs"][0]["codepoint"] = serde_json::json!(0xD800_u32);
        assert!(of(&no_char).unwrap_err().contains("no character"));
        let mut twice = sample();
        twice["modules"][1]["actions"][0]["command"] = serde_json::json!("made-up-clean");
        assert!(of(&twice).unwrap_err().contains("made-up-clean"));
        let mut told_twice = sample();
        told_twice["modules"][1]["events"] = serde_json::json!([
            { "name": "done", "tag": "made-up-done", "notification": null }
        ]);
        assert!(of(&told_twice).unwrap_err().contains("made-up-done"));
    }

    /// FAILS IF: a refusal does not name the variable — the one thing a
    /// person can fix — or a missing file is not said as one.
    #[test]
    fn every_refusal_names_the_variable() {
        assert_eq!(read(None), Err(Missing::Unset));
        let gone = Path::new("/nonexistent/brain-catalogue.json");
        let unreadable = read(Some(gone)).unwrap_err();
        assert!(matches!(unreadable, Missing::Unreadable { .. }));
        let keys = Missing::Keys {
            file: gone.to_owned(),
            keys: (0..7).map(|n| format!("made-up-{n}")).collect(),
        };
        for said in [Missing::Unset, unreadable, keys].map(|m| m.to_string()) {
            assert!(said.contains(ENV), "{said}");
        }
    }

    /// FAILS IF: the catalogue the build wrote is not what this crate
    /// compiled of the distro's modules — a mark, a notice, an action, an
    /// event or a reason the footer would read and the daemon never
    /// declared, or the other way round — or a distro module is missing
    /// from one side. Read from the build's own file (`RONOCTUA_BRAIN_CATALOGUE`,
    /// in the tests' environment), so a generator that drifts from the
    /// manifests fails here.
    #[test]
    fn the_builds_catalogue_is_what_this_crate_compiled() {
        let c = catalogue();
        let distro: Vec<&Module> = c
            .modules
            .iter()
            .filter(|m| m.origin == Origin::Distro)
            .collect();
        assert_eq!(
            distro.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            crate::compiled::IDS,
            "the catalogue's distro modules are not the compiled ones"
        );
        for m in &distro {
            let manifest = crate::compiled::manifest(&m.id).expect("a compiled manifest");
            assert_eq!(m.declared(), Declared::of(manifest), "`{}` differs", m.id);
        }
    }
}
