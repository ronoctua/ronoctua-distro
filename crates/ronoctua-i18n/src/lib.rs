//! Interface text — invariant 8, section 7.6.
//!
//! **Zero string literals in UI.** Every sentence comes from a `.ftl` file, through
//! a key. What this crate does is resolve a key into a sentence in the active
//! language.
//!
//! # How the invariant stops depending on discipline
//!
//! Two guards, neither of them a test:
//!
//! 1. **Build.** `nix/scripts/gen-i18n.py` requires `pt-BR` and `en-GB` to have
//!    exactly the same set of keys, and none of them empty. A missing translation
//!    takes the build down before any test runs.
//! 2. **Compilation.** Every key is one constant in [`key`]. Whoever writes
//!    `key::CMD_BUFFER_CLOSE` cannot get it wrong; whoever writes the string by
//!    hand loses the guarantee, and that is why the project's code uses the
//!    constants.
//!
//! # The names are compiled, the sentences are data
//!
//! Every crate that speaks owns its sentences (`crates/<crate>/i18n/<lang>.ftl`)
//! and has a `key` module of its own, written at Nix EVALUATION by
//! `nix/text-keys.nix` from the key NAMES of its fragment alone — no value
//! reaches it, not even as a doc comment — which also re-exports the `key` of
//! every crate it stands on that speaks. This crate's own [`key`] holds the few
//! it says itself; [`LANGUAGES`] comes from `assets/i18n/languages.nix`, at
//! evaluation too. The sentences are read at RUN TIME from the directory
//! [`ENV`] names — one merged `<language>.ftl` per declared language, the
//! `texts` derivation of `nix/i18n.nix`, which every wrapper sets. So editing a
//! sentence rebuilds no crate; adding a key rebuilds the crate that says it.
//!
//! There is NO embedded copy to fall back to. A program whose texts are
//! missing, unreadable, or written by another build (a key it was compiled
//! with is not in them) is refused at its start by [`texts`] /
//! [`Texts::checked`] and [`refuse`]: one English line naming [`ENV`], and
//! [`EXIT_MISSING`]. A fallback would be a screen of `[key]` brackets, or the
//! wrong build's words, with nothing saying why. WHICH keys a program was
//! compiled with only the program knows — this crate is below every crate
//! that speaks — so it hands them in: [`Keys::of`] over its own `key::sets`.
//!
//! # Why Fluent, and not gettext
//!
//! Plural, gender and argument order belong to the LANGUAGE, not to the program. In
//! gettext that becomes `ngettext` with a numeric rule in the code; in Fluent the
//! rule lives in the `.ftl`, which is where the translator works. A language with a different
//! plural rule does not require touching Rust.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::ffi::OsString;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use fluent_bundle::{FluentBundle, FluentResource};
use unic_langid::LanguageIdentifier;

pub mod key;

// The language list, written at evaluation by `nix/text-keys.nix` out of
// `assets/i18n/languages.nix` (see the crate's header).
include!(env!("RONOCTUA_LANGUAGES_RS"));

/// Default language when the environment says nothing useful.
///
/// THE BASE, which is the first of [`LANGUAGES`] and today is `en-GB`: a system
/// that fell back probably is not on a Brazilian machine, and English is the
/// least surprising choice. The distro owner's preference comes in through
/// `LANG`, as with everything else.
///
/// Derived and not written, so that "the language every other is compared
/// against" and "the language we fall back to" cannot come to disagree — they
/// are the same decision, and `assets/i18n/languages.nix` is where it is made.
pub const DEFAULT: &str = LANGUAGES[0];

/// The variable naming the directory the sentences are in: the `texts`
/// derivation (`nix/i18n.nix`), one `<language>.ftl` per declared language,
/// which the wrappers set.
pub const ENV: &str = "RONOCTUA_TEXTS";

/// What a program exits with when its texts cannot be had: `EX_CONFIG` of
/// `sysexits.h`, "something was found in an unconfigured or misconfigured
/// state" — the environment, not the person and not a bug.
pub const EXIT_MISSING: u8 = 78;

/// Why the texts could not be had. Each one says so naming [`ENV`], which is
/// the one thing a person can fix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Missing {
    /// The variable is not set, or is empty.
    Unset,
    /// A language's file could not be read.
    Unreadable {
        /// The file asked for.
        file: PathBuf,
        /// What the system answered.
        why: String,
    },
    /// A language's file is not Fluent this program can load: a syntax error,
    /// or a message declared twice.
    Unparsable {
        /// The file read.
        file: PathBuf,
        /// What is wrong with it, as the parser said it.
        why: String,
    },
    /// The file lacks keys this program was compiled with: the program and its
    /// texts come from different builds.
    Keys {
        /// The file read.
        file: PathBuf,
        /// The keys it lacks, in the order of the program's [`Keys`].
        keys: Vec<&'static str>,
        /// How many keys the program was compiled with.
        of: usize,
    },
}

/// How many of the lacking keys a refusal names before it only counts them.
const NAMED: usize = 5;

impl fmt::Display for Missing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let dir = |file: &Path| {
            file.parent()
                .map_or_else(String::new, |d| d.display().to_string())
        };
        match self {
            Self::Unset => write!(
                f,
                "{ENV} is not set: the interface's sentences are data the build writes \
                 beside the program (one `<language>.ftl` in the texts' generated \
                 directory), and nothing here can say a word without them — run the \
                 program through its wrapper or the session, or inside `nix develop`"
            ),
            Self::Unreadable { file, why } => write!(
                f,
                "{ENV} points at {}, and {} cannot be read: {why}",
                dir(file),
                file.display()
            ),
            Self::Unparsable { file, why } => write!(
                f,
                "{ENV} points at {}, and {} is not Fluent this program can load: {why}",
                dir(file),
                file.display()
            ),
            Self::Keys { file, keys, of } => {
                let named: Vec<&str> = keys.iter().take(NAMED).copied().collect();
                let more = keys.len().saturating_sub(NAMED);
                write!(
                    f,
                    "{ENV} points at {}, and {} lacks {} of the {} keys this program was \
                     compiled with ({}{}): the program and its texts come from different builds",
                    dir(file),
                    file.display(),
                    keys.len(),
                    of,
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

/// One crate's keys, as its generated `key::sets` hands them in: the crate's
/// name and its `key::ALL`.
pub type Set = (&'static str, &'static [&'static str]);

/// Every key a program was compiled with: the keys of each crate it links
/// that speaks, collected by the program's own `key::sets` (written by
/// `nix/text-keys.nix`), each crate once.
///
/// # Why the program hands them in
///
/// Because this crate is BELOW every crate that speaks: each owns its keys
/// (`crates/<crate>/i18n/`), and only the program above them all knows which
/// it links. A list kept here would make every crate's new key an input of
/// this crate — and with it of every crate that speaks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keys {
    sets: Vec<Set>,
}

impl Keys {
    /// The keys `collect` gathers: a program's `key::sets`.
    #[must_use]
    pub fn of(collect: fn(&mut Vec<Set>)) -> Self {
        let mut sets = Vec::new();
        collect(&mut sets);
        Self { sets }
    }

    /// Every key, crate by crate.
    pub fn iter(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.sets.iter().flat_map(|(_, keys)| keys.iter().copied())
    }

    /// How many keys.
    #[must_use]
    pub fn len(&self) -> usize {
        self.sets.iter().map(|(_, keys)| keys.len()).sum()
    }

    /// No key at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The crates that said them, in the order they were collected.
    pub fn crates(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.sets.iter().map(|(by, _)| *by)
    }
}

/// Each declared language's file, parsed — read the first time anything asks
/// for that language, and never again: the directory is a store path, and
/// what it holds cannot change under a running process. A refusal is kept
/// too, so every later ask says the same thing.
static PARSED: [OnceLock<Result<Parsed, Missing>>; LANGUAGES.len()] =
    [const { OnceLock::new() }; LANGUAGES.len()];

/// A language's file, and what it parsed into.
#[derive(Clone)]
struct Parsed {
    file: PathBuf,
    resource: Arc<FluentResource>,
}

/// The file of `language` in the directory `dir` names.
fn file_in(dir: Option<OsString>, language: &str) -> Result<PathBuf, Missing> {
    let dir = PathBuf::from(dir.filter(|d| !d.is_empty()).ok_or(Missing::Unset)?);
    Ok(dir.join(format!("{language}.ftl")))
}

/// One language's file in the directory `dir` names, parsed. Pure in what it
/// is handed: the tests point it at files they planted.
fn read(dir: Option<OsString>, language: &str) -> Result<Parsed, Missing> {
    let file = file_in(dir, language)?;
    let source = std::fs::read_to_string(&file).map_err(|e| Missing::Unreadable {
        file: file.clone(),
        why: e.to_string(),
    })?;
    match FluentResource::try_new(source) {
        Ok(resource) => Ok(Parsed {
            file,
            resource: Arc::new(resource),
        }),
        // EVERY ERROR REFUSES, as the embedded bundle's did (`ok()?`): a
        // resource with junk in it loads the rest silently, and the junk is
        // a message that comes out as `[key]`.
        Err((resource, errors)) => {
            let first = errors.first();
            let line = first.map_or(0, |e| {
                resource.source()[..e.pos.start.min(resource.source().len())]
                    .matches('\n')
                    .count()
                    + 1
            });
            Err(Missing::Unparsable {
                file,
                why: format!(
                    "line {line}: {}",
                    first.map_or_else(String::new, ToString::to_string)
                ),
            })
        }
    }
}

/// The parsed file of a declared language, from [`ENV`]; `None` for a
/// language that is not declared.
fn parsed(language: &str) -> Option<Result<Parsed, Missing>> {
    let at = LANGUAGES.iter().position(|l| *l == language)?;
    Some(
        PARSED[at]
            .get_or_init(|| read(std::env::var_os(ENV), language))
            .clone(),
    )
}

/// The language the environment names — `LC_ALL`, `LC_MESSAGES` and `LANG`,
/// in that order, as POSIX does — or [`DEFAULT`]. Always a declared one.
fn environment_language() -> String {
    for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        let Ok(v) = std::env::var(var) else { continue };
        // `pt_BR.UTF-8` -> `pt-BR`.
        let base = v.split('.').next().unwrap_or("").replace('_', "-");
        if LANGUAGES.contains(&base.as_str()) {
            return base;
        }
    }
    DEFAULT.to_owned()
}

/// The texts of the environment's language ([`Texts::from_env`]'s choice),
/// with every key this program was compiled with in them (`keys`, the
/// program's [`Keys::of`] over its own `key::sets`).
///
/// THE START GUARD of a program that speaks: called before its first word,
/// so a missing directory, an unreadable file or another build's texts are
/// said at once ([`refuse`]) rather than as a `[key]` on a screen or a panic
/// in the middle of a door.
///
/// # Errors
///
/// [`Missing`], which says why naming [`ENV`].
pub fn texts(keys: &Keys) -> Result<Texts, Missing> {
    let language = environment_language();
    Texts::checked(&language, keys)
        .map(|texts| texts.unwrap_or_else(|| unreachable!("`{language}` is a declared language")))
}

/// Says why the texts could not be had — one English line on stderr, the
/// program's name first — and exits with [`EXIT_MISSING`].
///
/// English and not a key: the line exists precisely because there are no
/// keys to say it with.
pub fn refuse(missing: &Missing) -> ! {
    let program = std::env::args_os()
        .next()
        .map(PathBuf::from)
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_default();
    eprintln!("{program}: {missing}");
    std::process::exit(i32::from(EXIT_MISSING))
}

/// One language's texts.
pub struct Texts {
    bundle: FluentBundle<Arc<FluentResource>>,
    language: String,
    /// The file they were read from, for a refusal to name.
    file: PathBuf,
}

impl std::fmt::Debug for Texts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Texts")
            .field("language", &self.language)
            .finish()
    }
}

/// One argument of a text, when the value is not always a string.
///
/// See [`Texts::t_args`] for why a number has to arrive as a number.
#[derive(Debug, Clone, Copy)]
pub enum Arg<'a> {
    /// A piece of data: a name, a path, a key notation.
    Text(&'a str),
    /// A count. This is the one that makes a plural pick the right branch.
    Number(i64),
    /// A measurement, with a fraction.
    ///
    /// # Why it is not simply `Text`
    ///
    /// Because the character between the whole part and the fraction is the
    /// LANGUAGE's, and a string carries whatever the code that formatted it
    /// happened to write — a full stop, in Rust, in every language. A volume
    /// of 15,6 cm³ reading `15.6` in pt-BR is the kind of wrong that nobody
    /// files a report about and everybody notices.
    ///
    /// Separate from [`Arg::Number`] and not folded into it: that one exists
    /// to make a plural pick its branch, and `1.0` is not the same question as
    /// `1` when a selector asks.
    Fraction(f64),
}

impl Texts {
    /// Loads a language by name (`pt-BR`, `en-GB`).
    ///
    /// Returns `None` for a language that does not exist — the caller decides whether to fall back
    /// to the default or complain. [`Texts::from_env`] falls back to the default.
    ///
    /// The file is read and parsed once per process (see [`ENV`]); every later
    /// call only builds a bundle over it.
    ///
    /// # Panics
    ///
    /// When a DECLARED language's texts cannot be had ([`Missing`], with a
    /// sentence naming [`ENV`]). A program's start guard ([`texts`],
    /// [`Texts::checked`]) has said so already, with [`EXIT_MISSING`], before
    /// anything got here.
    #[must_use]
    pub fn fresh(language: &str) -> Option<Self> {
        Self::loaded(language).unwrap_or_else(|missing| panic!("{missing}"))
    }

    /// [`Texts::fresh`], and every key this program was compiled with (`keys`)
    /// is in it — the start guard's question, asked of the language the
    /// program is about to speak. `Ok(None)` for a language that is not
    /// declared.
    ///
    /// # Errors
    ///
    /// [`Missing`]: the texts cannot be read, do not parse, or lack keys of
    /// `keys` (another build's).
    pub fn checked(language: &str, keys: &Keys) -> Result<Option<Self>, Missing> {
        let Some(texts) = Self::loaded(language)? else {
            return Ok(None);
        };
        let lacking = texts.lacking(keys);
        if lacking.is_empty() {
            return Ok(Some(texts));
        }
        Err(Missing::Keys {
            file: texts.file,
            keys: lacking,
            of: keys.len(),
        })
    }

    /// The keys of `keys` these texts have no sentence for.
    fn lacking(&self, keys: &Keys) -> Vec<&'static str> {
        keys.iter().filter(|k| !self.has(k)).collect()
    }

    /// [`Texts::fresh`], saying why instead of panicking.
    fn loaded(language: &str) -> Result<Option<Self>, Missing> {
        let Some(parsed) = parsed(language) else {
            return Ok(None);
        };
        Self::over(language, parsed?)
    }

    /// A bundle of `language` over an already parsed file.
    fn over(language: &str, parsed: Parsed) -> Result<Option<Self>, Missing> {
        let Ok(id) = language.parse::<LanguageIdentifier>() else {
            return Ok(None);
        };
        let mut bundle = FluentBundle::new(vec![id]);
        // ISOLATION MARKS STAY ON, and `format` is what takes them off.
        //
        // Fluent puts a bidirectional isolation mark (U+2068/U+2069) around
        // every interpolated argument. They are CORRECT in a renderer that
        // implements the bidi algorithm; ours draws cell by cell, and left in
        // they would show up as an error glyph.
        //
        // Two things used to remove them — this switch and the strip in
        // `format` — which means one of the two was dead code, and a reader
        // could not tell which. Now there is one, and it EARNS its keep: the
        // mark is the only thing that says where an argument ended up in the
        // sentence, and `t_marked` hands that boundary to a caller that needs
        // to draw the argument in a different colour from the prose around it.
        // Asking Rust to find the value back inside the finished sentence
        // would be a second copy of it, and it would go wrong the day a
        // language put the same word somewhere else in the line.
        bundle.set_use_isolating(true);
        // A MESSAGE DECLARED TWICE is refused like a syntax error: the bundle
        // keeps the first and drops the second, and which sentence a key says
        // would be an accident of the file's order.
        if let Err(errors) = bundle.add_resource(parsed.resource) {
            return Err(Missing::Unparsable {
                file: parsed.file,
                why: errors.first().map_or_else(String::new, ToString::to_string),
            });
        }
        Ok(Some(Self {
            bundle,
            language: language.to_owned(),
            file: parsed.file,
        }))
    }

    /// Loads the environment's language, falling back to [`DEFAULT`].
    ///
    /// It reads `LC_ALL`, `LC_MESSAGES` and `LANG`, in that order — the same as POSIX.
    ///
    /// # Panics
    ///
    /// As [`Texts::fresh`]: when the texts cannot be had.
    #[must_use]
    pub fn from_env() -> Self {
        Self::fresh(&environment_language()).expect("the default language has to exist")
    }

    /// The loaded language.
    #[must_use]
    pub fn language(&self) -> &str {
        &self.language
    }

    /// Is there a sentence under this key at all?
    ///
    /// # Why anything needs to ask
    ///
    /// Because [`Texts::t`] never fails, and what it returns for a key nobody
    /// declared is `[the-key]` — a marker meant for a screenshot, not for a
    /// sentence somebody is supposed to read. Every caller in this tree knows
    /// its key exists, because a build guard says so, and so none of them ever
    /// had to ask.
    ///
    /// One does now. The hint line explains the shortcut under the pointer,
    /// and the long form of a shortcut lives under `cmd-<id>` for the ones that
    /// are commands of the registry and under `about-<id>` for the ones that
    /// are a view's own keys. Which of the two a given id has is a fact about
    /// the catalogue, and this is the only honest way to read it: sniffing
    /// `t()`'s answer for a leading bracket would make a translator who wrote
    /// a sentence in square brackets into a bug.
    #[must_use]
    pub fn has(&self, key: &str) -> bool {
        self.bundle
            .get_message(key)
            .is_some_and(|m| m.value().is_some())
    }

    /// Resolves a key.
    ///
    /// **It never fails.** An unknown key becomes `[key]`, visible on screen and
    /// obvious in a screenshot. Taking the shell down over a missing text
    /// would trade a wrong sentence for no interface at all — and the two build
    /// guards, with the start guard over the texts read at run time
    /// ([`Texts::checked`]), already make that case unreachable from the
    /// project's code.
    #[must_use]
    pub fn t(&self, key: &str) -> String {
        self.t_with(key, &[])
    }

    /// Resolves a key with arguments.
    ///
    /// # Why arguments, and not concatenation
    ///
    /// "`SPC g x` is not bound to anything" has a different ORDER in each language,
    /// and concatenating `t(prefix) + key + t(suffix)` imposes Portuguese's
    /// order on all of them. In Fluent the position of `{ $key }` belongs to the translation
    /// file, which is where it belongs — the same reason the project uses
    /// Fluent and not gettext.
    #[must_use]
    pub fn t_with(&self, key: &str, args: &[(&str, &str)]) -> String {
        let mut fluent = fluent_bundle::FluentArgs::new();
        for (k, v) in args {
            fluent.set(*k, *v);
        }
        self.format(key, fluent, args.is_empty())
    }

    /// The same, with arguments that may be NUMBERS.
    ///
    /// # Why a number cannot travel as a string
    ///
    /// A plural is a decision of the LANGUAGE — English has two forms, Polish
    /// three — and Fluent expresses it as `{ $n -> [one] ... *[other] ... }`.
    /// Choosing the branch means asking the plural rules what CATEGORY the
    /// number falls into, and that only happens for a numeric value: given the
    /// string `"1"`, Fluent looks for a variant literally named `1` and falls
    /// through to the default. The symptom is a screen that says "1 items"
    /// while every set check is green.
    ///
    /// It is a second method and not a wider `t_with` so that the twenty
    /// existing call sites, which have no numbers, keep their shape.
    #[must_use]
    pub fn t_args(&self, key: &str, args: &[(&str, Arg<'_>)]) -> String {
        let mut fluent = fluent_bundle::FluentArgs::new();
        for (k, v) in args {
            match v {
                Arg::Text(s) => fluent.set(*k, *s),
                Arg::Number(n) => fluent.set(*k, *n),
                // FORMATTED HERE, WHERE THE LANGUAGE IS. `fluent-bundle` is
                // built without a number formatter — measured, not assumed:
                // `225154` comes out ungrouped and `15.6` keeps its full stop
                // in pt-BR — so handing it a float would have been a decimal
                // separator chosen by Rust for every language on earth.
                //
                // A `Number` is deliberately NOT treated this way: Fluent
                // selects a plural branch on it, and a string has no branch.
                Arg::Fraction(n) => fluent.set(*k, self.decimal(*n)),
            }
        }
        self.format(key, fluent, args.is_empty())
    }

    /// One number with the separator this language writes.
    ///
    /// Two languages, two separators, and a table of two is the honest size of
    /// the problem: the build declares `en-GB` and `pt-BR` and an undeclared
    /// language never loads, so a third would fail long before it got here.
    /// One decimal place, which is what every measurement this is used for
    /// wants — a volume to a tenth of a cubic centimetre, a side to a tenth of
    /// a millimetre.
    fn decimal(&self, n: f64) -> String {
        let plain = format!("{n:.1}");
        if self.decimal_mark() == ',' {
            return plain.replace('.', ",");
        }
        plain
    }

    /// What this language puts between a whole number and its tenths.
    ///
    /// # Why a CHARACTER and not a formatted string
    ///
    /// Because two places assemble a size by hand, and both are hot: the file
    /// listing writes two hundred of them per frame and the network monitor
    /// rewrites its totals every second. Going through the bundle for each
    /// would be a Fluent resolution per row.
    ///
    /// They used to write `','` INTO the format string, so `44,2 GB` and
    /// `5,0 MiB` were on an English screen too — which reads as forty-four
    /// thousand two hundred gigabytes. The doc comment on one of them already
    /// described the arrangement this makes real: *the language only decides
    /// the separator, which the caller passes in*.
    #[must_use]
    ///
    /// # Why it comes out of the bundle and not out of the language's NAME
    ///
    /// Because a separator is a property of the language, and the languages
    /// are declared in `assets/i18n/languages.nix` and nowhere else
    /// (invariant 16). `self.language.starts_with("pt")` named one in Rust —
    /// it worked, and it was the second place in this system that knew what
    /// languages there are. The value is a text key like every other, and a
    /// language added tomorrow brings its own.
    pub fn decimal_mark(&self) -> char {
        self.t(key::NUMBER_DECIMAL_MARK)
            .chars()
            .next()
            .unwrap_or('.')
    }

    /// The same as [`Texts::t_with`], and it also says WHERE each argument
    /// landed in the sentence.
    ///
    /// # Why a caller would want that
    ///
    /// Because a line like "type to filter · C-m opens the action menu" is one
    /// sentence with two registers in it: the KEY, which is what the eye
    /// hunts for, and the prose that explains it. Drawing the two in
    /// different colours means knowing where one ends, and the only thing
    /// that honestly knows is the LOCALISER — it is the localiser that
    /// decided where in the line the argument goes, and a different language
    /// puts it somewhere else.
    ///
    /// The alternative was searching the finished sentence for `"C-m"`. That
    /// is a second copy of the key, and it picks the wrong occurrence the day
    /// a translation happens to contain the same two characters as prose.
    #[must_use]
    pub fn t_marked(&self, key: &str, args: &[(&str, &str)]) -> Marked {
        let mut fluent = fluent_bundle::FluentArgs::new();
        for (k, v) in args {
            fluent.set(*k, *v);
        }
        self.marked(key, fluent, args.is_empty())
    }

    /// The half both public forms share.
    fn format(&self, key: &str, fluent: fluent_bundle::FluentArgs<'_>, bare: bool) -> String {
        self.marked(key, fluent, bare).text
    }

    /// The one that resolves, and the one that takes the marks off.
    fn marked(&self, key: &str, fluent: fluent_bundle::FluentArgs<'_>, bare: bool) -> Marked {
        let Some(msg) = self.bundle.get_message(key) else {
            return Marked::plain(format!("[{key}]"));
        };
        let Some(default_of) = msg.value() else {
            return Marked::plain(format!("[{key}]"));
        };
        let args = (!bare).then_some(&fluent);
        let mut errors = Vec::new();
        let s = self.bundle.format_pattern(default_of, args, &mut errors);
        if !errors.is_empty() {
            tracing::warn!(key, ?errors, "text with a formatting error");
        }
        // THE MARKS COME OFF HERE, and their positions are kept.
        //
        // An isolation mark is invisible in a terminal, but it counts as a
        // column in our integer layout and throws the line out of alignment,
        // so no sentence may leave this crate carrying one. There is no RTL
        // language here (section 2 fixes pt-BR and en-GB), so nothing is lost
        // by dropping them.
        //
        // The DEPTH counter is what makes a range mean an argument and not a
        // fragment of one: Fluent nests marks when a placeable resolves to
        // something that itself carries one, and only the outermost pair is a
        // boundary anybody asked about. A closing mark with nothing open is
        // dropped rather than trusted — a range built from it would start
        // wherever the last one did.
        let mut text = String::with_capacity(s.len());
        let mut at = Vec::new();
        let mut depth = 0_usize;
        let mut from = 0_usize;
        for c in s.chars() {
            match c {
                '\u{2068}' => {
                    if depth == 0 {
                        from = text.len();
                    }
                    depth += 1;
                }
                '\u{2069}' if depth > 0 => {
                    depth -= 1;
                    if depth == 0 {
                        at.push(from..text.len());
                    }
                }
                '\u{2069}' => {}
                _ => text.push(c),
            }
        }
        Marked { text, at }
    }
}

/// A sentence, and where its arguments ended up inside it.
///
/// See [`Texts::t_marked`]. The ranges are byte ranges into [`Marked::text`],
/// which is the sentence with the marks already gone — so they can be sliced
/// straight, without the caller ever meeting an isolation mark.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Marked {
    /// The sentence.
    pub text: String,
    /// One range per interpolated argument, in the order they READ, which is
    /// not the order they were passed: the language decides.
    pub at: Vec<core::ops::Range<usize>>,
}

impl Marked {
    /// A sentence with no arguments in it.
    fn plain(text: String) -> Self {
        Self {
            text,
            at: Vec::new(),
        }
    }

    /// The sentence cut into its pieces, saying which ones are arguments.
    ///
    /// The pieces come out in reading order and concatenate back to
    /// [`Marked::text`] exactly — a caller that draws them one after another
    /// draws the sentence, and nothing else has to know where the seams are.
    #[must_use]
    pub fn pieces(&self) -> Vec<(&str, bool)> {
        let mut out = Vec::with_capacity(self.at.len() * 2 + 1);
        let mut cut = 0;
        for r in &self.at {
            if r.start > cut {
                out.push((&self.text[cut..r.start], false));
            }
            out.push((&self.text[r.clone()], true));
            cut = r.end;
        }
        if cut < self.text.len() {
            out.push((&self.text[cut..], false));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every message id of a language's file, in file order: what the merged
    /// texts hold, read off their own source — the census needs no list of
    /// keys from the crates above this one.
    fn ids(language: &str) -> Vec<String> {
        let parsed = parsed(language)
            .unwrap_or_else(|| panic!("{language} is a declared language"))
            .unwrap_or_else(|missing| panic!("{missing}"));
        parsed
            .resource
            .source()
            .lines()
            .filter_map(|line| {
                let (id, _) = line.split_once('=')?;
                let id = id.trim_end();
                (id.starts_with(|c: char| c.is_ascii_lowercase())
                    && id
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'))
                .then(|| id.to_owned())
            })
            .collect()
    }

    /// A language's texts over a file planted with `text`, for the tests that
    /// need sentences of their own (the real ones belong to the crates above).
    fn planted_texts(name: &str, language: &str, text: &str) -> Texts {
        let dir = planted(name, &[(&format!("{language}.ftl"), text)]);
        let parsed = read(Some(dir.clone().into_os_string()), language).expect("it parses");
        let _ = std::fs::remove_dir_all(&dir);
        Texts::over(language, parsed)
            .expect("it loads")
            .expect("a declared language")
    }

    #[test]
    fn both_languages_load_and_have_the_same_keys() {
        // The generator already guarantees this at build time, over every
        // fragment. This guarantees it of what is READ AT RUN TIME
        // (`RONOCTUA_TEXTS`, the merged files): every language has the base's
        // keys and no other, and each one says something.
        let base = ids(DEFAULT);
        assert!(base.len() > 1000, "only {} keys in {DEFAULT}", base.len());
        let mut sorted = base.clone();
        sorted.sort();
        for language in LANGUAGES {
            let t = Texts::fresh(language).unwrap_or_else(|| panic!("{language} did not load"));
            let mut mine = ids(language);
            mine.sort();
            assert_eq!(mine, sorted, "{language} has other keys than {DEFAULT}");
            for key in &base {
                let s = t.t(key);
                assert!(!s.starts_with('['), "{language}: `{key}` did not resolve");
                assert!(!s.is_empty(), "{language}: `{key}` resolved to empty");
            }
        }
    }

    #[test]
    fn both_languages_say_different_things() {
        // It catches the mistake of copying one language's file over the other,
        // which passes every set check.
        let en = Texts::fresh("en-GB").unwrap();
        let pt = Texts::fresh("pt-BR").unwrap();
        let every = ids(DEFAULT);
        let different = every.iter().filter(|c| en.t(c) != pt.t(c)).count();
        assert!(
            different >= every.len() / 2,
            "only {different} of {} keys differ between the languages",
            every.len()
        );
    }

    #[test]
    fn an_unknown_key_does_not_bring_it_down() {
        let t = Texts::fresh("en-GB").unwrap();
        assert_eq!(t.t("does-not-exist"), "[does-not-exist]");
    }

    #[test]
    fn the_environment_chooses_the_language() {
        // `pt_BR.UTF-8` is the form `LANG` actually has; without the
        // normalisation (`_` -> `-`, cut at the `.`) no language would match and
        // everything would fall back to the default in silence.
        assert!(Texts::fresh("pt-BR").is_some());
        assert!(Texts::fresh("pt_BR.UTF-8").is_none());
    }

    /// A hint with three keys in it, which the two languages place differently
    /// — the shape of the theme chooser's.
    const HINT_EN: &str = "hint = { $apply } applies · { $left } { $right } turn\n";
    const HINT_PT: &str = "hint = girar com { $left } { $right } · { $apply } aplica\n";

    #[test]
    fn a_marked_sentence_says_where_its_arguments_are() {
        // The whole point of `t_marked`: the pieces concatenate back to the
        // sentence, and the ones flagged `true` are exactly what was passed in.
        let t = planted_texts("marked", "en-GB", HINT_EN);
        let arg = [("apply", "C-a"), ("left", "C-h"), ("right", "C-l")];
        let m = t.t_marked("hint", &arg);
        assert_eq!(m.text, t.t_with("hint", &arg));
        let pieces = m.pieces();
        let rebuilt: String = pieces.iter().map(|(s, _)| *s).collect();
        assert_eq!(rebuilt, m.text, "the pieces have to be the sentence");
        let keys: Vec<&str> = pieces.iter().filter(|(_, k)| *k).map(|(s, _)| *s).collect();
        assert_eq!(keys, vec!["C-a", "C-h", "C-l"]);
    }

    #[test]
    fn an_argument_at_either_end_still_has_a_range() {
        // `{ $label }: { $value }` — one argument opens the sentence and one
        // closes it. An off-by-one in the strip shows up here and nowhere else,
        // because every other value has prose on both sides.
        let t = planted_texts("ends", "en-GB", "says = { $label }: { $value }\n");
        let m = t.t_marked("says", &[("label", "path"), ("value", "/tmp")]);
        assert_eq!(m.text, "path: /tmp");
        assert_eq!(m.at, vec![0..4, 6..10]);
        assert_eq!(
            m.pieces(),
            vec![("path", true), (": ", false), ("/tmp", true)]
        );
    }

    #[test]
    fn a_sentence_with_no_arguments_has_no_ranges() {
        let t = planted_texts("bare", "en-GB", "up = up\n");
        let m = t.t_marked("up", &[]);
        assert!(m.at.is_empty());
        assert_eq!(m.pieces(), vec![(m.text.as_str(), false)]);
    }

    #[test]
    fn the_language_decides_where_an_argument_reads() {
        // The reason the boundary comes from the localiser and not from a
        // search: the two languages put the same argument at different byte
        // offsets, and both are right.
        let en = planted_texts("order-en", "en-GB", HINT_EN);
        let pt = planted_texts("order-pt", "pt-BR", HINT_PT);
        let arg = [("apply", "C-a"), ("left", "C-h"), ("right", "C-l")];
        let a = en.t_marked("hint", &arg);
        let b = pt.t_marked("hint", &arg);
        assert_ne!(a.at, b.at, "the two languages cannot agree on every offset");
        for m in [&a, &b] {
            assert_eq!(m.at.len(), 3);
            for r in &m.at {
                assert!(m.text.get(r.clone()).is_some_and(|s| s.starts_with("C-")));
            }
        }
    }

    #[test]
    fn an_unknown_key_is_marked_too() {
        let t = Texts::fresh("en-GB").unwrap();
        let m = t.t_marked("does-not-exist", &[("key", "C-m")]);
        assert_eq!(m.text, "[does-not-exist]");
        assert!(m.at.is_empty());
    }

    #[test]
    fn there_is_no_bidi_isolation_mark() {
        // U+2068/U+2069 are correct in a bidi renderer; in ours they would show up
        // as an error glyph in the middle of the sentence.
        //
        // The bundle PRODUCES them now — they are what `t_marked` reads the
        // argument boundaries out of — so this is the guard that says the
        // strip is the only door out of this crate. The call WITH arguments is
        // the one that matters: a sentence resolved with none never had a mark
        // to lose.
        let planted = planted_texts("bidi", "pt-BR", "says = { $label }: { $value }\n");
        let with_args = planted.t_with("says", &[("label", "path"), ("value", "/tmp")]);
        assert_eq!(with_args, "path: /tmp");
        let t = Texts::fresh("pt-BR").unwrap();
        for key in ids("pt-BR") {
            let s = t.t(&key);
            assert!(
                !s.contains('\u{2068}') && !s.contains('\u{2069}'),
                "`{key}` came with an isolation mark"
            );
        }
    }
    #[test]
    // THE DEFECT THIS CATCHES: believing `fluent-bundle` localises a number.
    // It does not — measured here, in both languages — so a measurement handed
    // over as a float would have carried Rust's full stop into pt-BR, where the
    // separator is a comma. Nothing else in the bundle would have said so.
    fn a_measurement_wears_the_separator_of_its_language() {
        let volume = "volume = { $n } cm³\n";
        let en = planted_texts(
            "measure-en",
            "en-GB",
            &format!("{volume}number-decimal-mark = .\n"),
        );
        let pt = planted_texts(
            "measure-pt",
            "pt-BR",
            &format!("{volume}number-decimal-mark = ,\n"),
        );
        assert!(
            en.t_args("volume", &[("n", Arg::Fraction(15.6))])
                .contains("15.6"),
            "en-GB should keep the full stop"
        );
        assert!(
            pt.t_args("volume", &[("n", Arg::Fraction(15.6))])
                .contains("15,6"),
            "pt-BR should use a comma: {}",
            pt.t_args("volume", &[("n", Arg::Fraction(15.6))])
        );
    }

    #[test]
    fn the_real_separators_are_the_languages() {
        // The planted files above say what the arithmetic does; this says the
        // real texts carry the separator each language writes.
        assert_eq!(Texts::fresh("en-GB").unwrap().decimal_mark(), '.');
        assert_eq!(Texts::fresh("pt-BR").unwrap().decimal_mark(), ',');
    }

    /// A directory of its own under the system's temporary one, holding
    /// `files` and nothing else. Removed by the caller.
    fn planted(name: &str, files: &[(&str, &str)]) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-i18n-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a temporary directory");
        for (file, text) in files {
            std::fs::write(dir.join(file), text).expect("a planted file");
        }
        dir
    }

    #[test]
    fn no_variable_is_refused_naming_it() {
        // Unset and empty are the same absence: an empty value would
        // otherwise be read as the current directory.
        for dir in [None, Some(OsString::new())] {
            let refused = read(dir, DEFAULT).err().expect("refused");
            assert_eq!(refused, Missing::Unset);
            assert!(
                refused.to_string().starts_with("RONOCTUA_TEXTS is not set"),
                "{refused}"
            );
        }
    }

    #[test]
    fn a_missing_file_is_refused_naming_the_variable_and_the_file() {
        let dir = planted("missing", &[]);
        let refused = read(Some(dir.clone().into_os_string()), "pt-BR")
            .err()
            .expect("refused");
        let said = refused.to_string();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(matches!(refused, Missing::Unreadable { .. }), "{refused:?}");
        assert!(said.contains(ENV) && said.contains("pt-BR.ftl"), "{said}");
    }

    #[test]
    fn junk_is_refused_with_its_line() {
        // The embedded bundle refused a resource with junk in it (`ok()?`);
        // the file read at run time is refused the same way, and says where.
        let dir = planted(
            "junk",
            &[("en-GB.ftl", "hello = Hello\n\nthis is not fluent\n")],
        );
        let refused = read(Some(dir.clone().into_os_string()), "en-GB")
            .err()
            .expect("refused");
        let _ = std::fs::remove_dir_all(&dir);
        let Missing::Unparsable { why, .. } = &refused else {
            panic!("{refused:?}")
        };
        assert!(why.starts_with("line 3:"), "{why}");
        assert!(refused.to_string().contains(ENV), "{refused}");
    }

    #[test]
    fn a_message_declared_twice_is_refused() {
        let dir = planted("twice", &[("en-GB.ftl", "hello = Hello\nhello = Again\n")]);
        let parsed = read(Some(dir.clone().into_os_string()), "en-GB").expect("it parses");
        let _ = std::fs::remove_dir_all(&dir);
        let refused = Texts::over("en-GB", parsed).expect_err("refused");
        assert!(matches!(refused, Missing::Unparsable { .. }), "{refused:?}");
        assert!(refused.to_string().contains("\"hello\""), "{refused}");
    }

    /// A program's keys, as a crate's generated `key::sets` would hand them in:
    /// two crates, one of them reached twice and collected once.
    static FIRST: [&str; 2] = ["files-up", "first-two"];
    static SECOND: [&str; 7] = ["s-1", "s-2", "s-3", "s-4", "s-5", "s-6", "s-7"];
    fn first(into: &mut Vec<Set>) {
        if into.iter().any(|(by, _)| *by == "first") {
            return;
        }
        into.push(("first", &FIRST));
    }
    fn program(into: &mut Vec<Set>) {
        if into.iter().any(|(by, _)| *by == "program") {
            return;
        }
        into.push(("program", &SECOND));
        first(into);
        first(into);
    }

    #[test]
    fn a_programs_keys_are_its_crates_each_once() {
        let keys = Keys::of(program);
        assert_eq!(keys.crates().collect::<Vec<_>>(), vec!["program", "first"]);
        assert_eq!(keys.len(), FIRST.len() + SECOND.len());
        assert!(keys.iter().any(|k| k == "files-up"));
        assert!(!Keys::of(|_| {}).iter().any(|_| true));
        assert!(Keys::of(|_| {}).is_empty());
    }

    #[test]
    fn another_builds_texts_are_refused_saying_which_keys() {
        // One message stands for texts written by another build: every key
        // this program was compiled with but that one is missing from them,
        // and the refusal names the first few and counts the rest.
        let texts = planted_texts("other-build", "en-GB", "files-up = Up\n");
        let keys = Keys::of(program);
        let lacking = texts.lacking(&keys);
        assert_eq!(lacking.len(), keys.len() - 1);
        assert!(!lacking.contains(&"files-up"));
        let said = Missing::Keys {
            file: texts.file.clone(),
            keys: lacking.clone(),
            of: keys.len(),
        }
        .to_string();
        assert!(said.contains(ENV), "{said}");
        assert!(said.contains("different builds"), "{said}");
        assert!(said.contains(lacking[0]), "{said}");
        assert!(
            said.contains(&format!("of the {} keys", keys.len())),
            "{said}"
        );
        assert!(
            said.contains(&format!(" and {} more", lacking.len() - NAMED)),
            "{said}"
        );
    }

    #[test]
    fn the_start_guard_passes_on_the_real_texts() {
        // `RONOCTUA_TEXTS` of the tests' environment is the build's own
        // directory: the guard every program runs first lets it through, over
        // this crate's own keys.
        let keys = Keys::of(key::sets);
        assert!(keys.iter().any(|k| k == key::NUMBER_DECIMAL_MARK));
        let texts = texts(&keys).unwrap_or_else(|missing| panic!("{missing}"));
        assert!(LANGUAGES.contains(&texts.language()));
    }

    #[test]
    fn an_undeclared_language_is_none_and_reads_nothing() {
        // `None` is still the answer for a language that does not exist: it
        // is asked before any file is, so no directory can turn it into a
        // refusal.
        assert!(Texts::fresh("xx-XX").is_none());
        assert!(matches!(
            Texts::checked("xx-XX", &Keys::of(key::sets)),
            Ok(None)
        ));
    }
}
