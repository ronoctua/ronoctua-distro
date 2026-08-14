//! What the installer ASKS — section 7.1, milestone 8's criterion.
//!
//! The question list is a closed ENUM with a test that counts the cases.
//! Adding a question requires touching the enum, and touching the enum breaks the
//! test — which is how one more question stops getting in by inattention.
//!
//! # What is NOT asked, and why
//!
//! Keyboard, time zone, graphics card, filesystem, partition size,
//! boot manager. Each of those is either detected by `nixos-facter`, or the
//! distro has already decided (section 7.2). Asking would be handing the person
//! a decision the project has already made — and section 7.1 calls that by its name:
//! an installer that asks too much is an installer that did not decide.
//!
//! # The NETWORK left that list, and why it is different
//!
//! Until milestone 8 the network was here, among what is not asked. That was
//! wrong, and for two reasons none of the others have:
//!
//! 1. **There is nothing to detect.** `nixos-facter` finds the network card, but
//!    the Wi-Fi password is neither a property of the hardware nor a decision of the project:
//!    it is a per-person secret only whoever is installing knows.
//! 2. **Without it the installation does not run.** The `inst-step-install` step downloads
//!    the whole system over the network. On a machine with no Ethernet port — and this
//!    project's laptop is one of them — not asking means not
//!    installing.
//!
//! The criterion still holds with the same force: one asks what only the
//! person knows, and nothing beyond. The network came to fit that.
//!
//! # The Wi-Fi password is a CONDITIONAL screen
//!
//! [`Question::WifiPassword`] only appears once a network has been chosen — see
//! [`Question::skipped_one`]. Whoever goes by cable answers six screens, as before;
//! whoever chooses a network answers seven.

use ronoctua_i18n::key;

/// The questions. **Eight, and the test counts them.**
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Question {
    /// Which disk to install on. It erases everything.
    Disk,
    /// The machine's name.
    Hostname,
    /// The user's password.
    Password,
    /// The disk's passphrase (LUKS).
    Passphrase,
    /// The interface language.
    Language,
    /// The keyboard layout, in xkb's name (`br`, `us`, `gb`).
    ///
    /// # Why it is asked, and why it is asked EARLY
    ///
    /// Without it the machine came up on the layout of the computer this distro
    /// was written on. The worst moment to find that out is while typing the
    /// disk's passphrase — it is not echoed, and it cannot be corrected later.
    ///
    /// And xkb does NOT refuse a layout that does not exist: it falls back to a
    /// mute `us`. So the answer is validated against the real list, here, and
    /// not left for the system to notice.
    Keyboard,
    /// The layout's variant, in xkb's name (`abnt2`, `nodeadkeys`, or none).
    ///
    /// A separate question because the variant is what SEPARATES two keyboards
    /// that share a layout: `br` and `br(abnt2)` are the same country and a
    /// different key map. Whoever knows their keyboard knows this; nobody knows
    /// the string that names it, and that is what makes it a list.
    KeyboardVariant,
    /// Which key composes accents.
    ///
    /// # Why this is asked at all
    ///
    /// Because the answer depends on the keyboard AND on the language, and the
    /// installer already knows both by the time it gets here. A British layout
    /// writing Portuguese has no accent key: without Compose there is no `ã` and
    /// no `ç`. A `br(abnt2)` has the accents on the keys and needs no Compose at
    /// all — and forcing one on it would take a key away from someone who does
    /// not need it.
    ComposeKey,
    /// The machine's timezone.
    ///
    /// Asked, and not detected: `nixos-facter` reads hardware, and where a
    /// person is is not a property of the hardware. Guessing it from the network
    /// would mean asking the internet where the machine is, during an install,
    /// before the network is necessarily up.
    Timezone,
    /// The system's locale: date, number, paper and units.
    ///
    /// NOT the same as `Language`, and the difference is the reason there are
    /// two: the interface's language is which of OUR translations to load, and
    /// there are two of those. The locale is glibc's, there are hundreds, and it
    /// decides whether the date reads `31/12` or `12/31`.
    SystemLocale,
    /// The swap file's offset, for hibernation.
    ResumeOffset,
    /// Which Wi-Fi network to use. Empty means "I am going by cable".
    Wifi,
    /// Its password. It only appears when a network has been chosen.
    WifiPassword,
}

impl Question {
    /// All of them, in the order the TUI presents them.
    ///
    /// # THE LANGUAGE COMES FIRST, and that is not a preference
    ///
    /// Every screen after it is drawn in the answer to it. Asking anything else
    /// first means asking it in whatever language the ISO happened to carry,
    /// which for half the people is a screen they cannot read — and the first of
    /// those screens erases a disk.
    ///
    /// This reverses the order that used to be here, where the disk came first
    /// so that nobody would invest five answers before finding out they picked
    /// the wrong one. That reason survives: the disk is still SECOND, so what is
    /// invested before it is one screen, and it is the screen that makes the
    /// rest legible.
    ///
    /// The keyboard trio comes right after the disk, and before the passphrase:
    /// the passphrase is typed blind, is not echoed, and cannot be corrected
    /// later — typing it on the wrong layout is the one mistake here with no
    /// recovery.
    pub const ALL_KEYS: [Self; 13] = [
        Self::Language,
        Self::Disk,
        Self::Keyboard,
        Self::KeyboardVariant,
        Self::ComposeKey,
        Self::SystemLocale,
        Self::Timezone,
        Self::Hostname,
        Self::Password,
        Self::Passphrase,
        // THE NETWORK is the last thing the person types, and it comes after the
        // language so the network list already appears in the chosen language.
        Self::Wifi,
        Self::WifiPassword,
        Self::ResumeOffset,
    ];

    /// The label's text **key** (invariant 8).
    ///
    /// Through the generated CONSTANT, and not through the literal. The difference is not one
    /// of style: a literal here compiles with any text and only shows up
    /// wrong on screen, between brackets, when someone installs. The constant
    /// does not exist if the key is not in both `.ftl` files.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Disk => key::INST_DISK,
            Self::Hostname => key::INST_HOSTNAME,
            Self::Password => key::INST_PASSWORD,
            Self::Passphrase => key::INST_PASSPHRASE,
            Self::Language => key::INST_LANGUAGE,
            Self::Keyboard => key::INST_KEYBOARD,
            Self::KeyboardVariant => key::INST_KEYBOARD_VARIANT,
            Self::ComposeKey => key::INST_COMPOSE_KEY,
            Self::SystemLocale => key::INST_SYSTEM_LOCALE,
            Self::Timezone => key::INST_TIMEZONE,
            Self::ResumeOffset => key::INST_RESUME_OFFSET,
            Self::Wifi => key::INST_WIFI,
            Self::WifiPassword => key::INST_WIFI_PASSWORD,
        }
    }

    /// The **key** of the help line, shown under the field.
    #[must_use]
    pub const fn help(self) -> &'static str {
        match self {
            Self::Disk => key::INST_DISK_HELP,
            Self::Hostname => key::INST_HOSTNAME_HELP,
            Self::Password => key::INST_PASSWORD_HELP,
            Self::Passphrase => key::INST_PASSPHRASE_HELP,
            Self::Language => key::INST_LANGUAGE_HELP,
            Self::Keyboard => key::INST_KEYBOARD_HELP,
            Self::KeyboardVariant => key::INST_KEYBOARD_VARIANT_HELP,
            Self::ComposeKey => key::INST_COMPOSE_KEY_HELP,
            Self::SystemLocale => key::INST_SYSTEM_LOCALE_HELP,
            Self::Timezone => key::INST_TIMEZONE_HELP,
            Self::ResumeOffset => key::INST_RESUME_OFFSET_HELP,
            Self::Wifi => key::INST_WIFI_HELP,
            // The network's help line also serves the password: what it
            // says — that the password does not enter the repository — matters more here.
            Self::WifiPassword => key::INST_WIFI_HELP,
        }
    }

    /// The name of the environment variable that answers this question.
    ///
    /// It exists for the terminal-less mode (the VM test, `nix flake check`). It is
    /// derived from the label on purpose: that way there is no second table to
    /// fall out of sync with the first.
    #[must_use]
    pub fn variable(self) -> String {
        format!("RONOCTUA_{}", self.label().replace('-', "_").to_uppercase())
    }

    /// Whether the answer should be hidden on screen.
    #[must_use]
    pub const fn secret_one(self) -> bool {
        matches!(self, Self::Password | Self::Passphrase | Self::WifiPassword)
    }

    /// Whether the installer computes it instead of asking.
    ///
    /// The `resume_offset` is on section 7.1's list but is NOT typeable: it
    /// comes from the swap file once created. It is on the list because it is
    /// data the installer has to produce, not because the person knows it.
    #[must_use]
    pub const fn computed(self) -> bool {
        matches!(self, Self::ResumeOffset)
    }

    /// The answer that comes already chosen.
    ///
    /// It is the same value in both modes, and that is the point: the TUI opens
    /// the list on it, and the terminal-less mode uses it when the variable is
    /// unset. Two tables would drift, and the drift would show up as
    /// `nix flake check` installing a machine nobody could have got by pressing
    /// Enter through the screens.
    ///
    /// `None` where there is no sensible default: a disk, a hostname and a
    /// password are nobody's to guess.
    #[must_use]
    pub const fn default_answer(self) -> Option<&'static str> {
        match self {
            Self::Language => Some(DEFAULT_LANGUAGE),
            Self::Keyboard => Some(DEFAULT_LAYOUT),
            Self::ComposeKey => Some(DEFAULT_COMPOSE),
            Self::SystemLocale => Some(DEFAULT_LOCALE),
            Self::Timezone => Some(DEFAULT_TIMEZONE),
            // The variant and the network default to empty, which `optional`
            // already accepts — writing it here too would say the same thing twice.
            _ => None,
        }
    }

    /// Whether leaving it blank is a valid answer.
    ///
    /// Only the network: blank there means "I am going by cable", which is a choice
    /// and not an omission. In the others, empty is an error — a blank hostname
    /// means nothing.
    #[must_use]
    pub const fn optional(self) -> bool {
        // The variant and the Compose key join the network here, and for the same
        // reason: empty is a CHOICE. `gb` with no variant is the right answer —
        // both variants it has put a dead accent on level 1 — and a keyboard whose
        // layout already carries the accents wants no Compose key stealing one.
        matches!(self, Self::Wifi | Self::KeyboardVariant | Self::ComposeKey)
    }

    /// Whether this screen should be SKIPPED, given the answers so far.
    ///
    /// # Why this is not just an `if` in the TUI
    ///
    /// Because `go_back` needs it too. With the rule only on the way forward, `Esc`
    /// on the review screen would land on the password screen for a network nobody chose —
    /// a field asking for the password of nothing, with no way out other
    /// than typing something.
    #[must_use]
    pub fn skipped_one(self, r: &Answers) -> bool {
        self == Self::WifiPassword && r.wifi_ssid.is_empty()
    }
}

/// A validation error for an answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Invalid {
    /// Empty.
    Blank,
    /// A hostname outside RFC 1123.
    HostnameMalformed,
    /// A hostname without the prefix section 7.1 requires.
    HostnameWithoutPrefix,
    /// A passphrase that is too short.
    PassphraseTooShort,
    /// A disk that does not exist, or that is not a disk.
    InvalidDisk,
    /// A language that is not in `assets/i18n`.
    UnknownLanguage,
    /// The layout is not in xkeyboard-config.
    UnknownLayout,
    /// A Wi-Fi password too short for WPA.
    WifiPasswordTooShort,
}

impl std::fmt::Display for Invalid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Blank => "nao pode ficar vazio",
            Self::HostnameMalformed => {
                "so letras minusculas, digitos e hifen; nao pode comecar nem terminar com hifen"
            }
            Self::HostnameWithoutPrefix => "o nome da maquina comeca com `noctua-`",
            Self::PassphraseTooShort => {
                "a frase-secreta do disco precisa de ao menos 12 caracteres"
            }
            Self::InvalidDisk => "esse caminho nao e um disco",
            Self::UnknownLanguage => "idioma nao disponivel",
            Self::UnknownLayout => "layout de teclado nao existe no xkeyboard-config",
            Self::WifiPasswordTooShort => "a senha de uma rede WPA tem ao menos 8 caracteres",
        };
        f.write_str(s)
    }
}

/// The smallest accepted size for the disk's passphrase.
///
/// Twelve, and not eight: it protects the whole disk and has **no** attempt
/// limit — there is no lockout after N mistakes, as on an online account.
/// The only defence is argon2's derivation cost times the length.
pub const MINIMUM_PASSPHRASE: usize = 12;

/// The smallest password accepted on a WPA network.
///
/// Eight is the standard's minimum, not a choice of ours: a shorter password is not
/// even attempted by the radio.
pub const MINIMO_SENHA_WIFI: usize = 8;

/// The hostname's mandatory prefix (section 7.1).
pub const HOSTNAME_PREFIX: &str = "noctua-";

/// One entry of `base.lst`: the name xkb wants, and the description a person reads.
///
/// The two travel together because neither serves alone: the name is what goes
/// into the config and means nothing on screen (`gb`, `latam`, `ru(phonetic)`),
/// and the description is what someone recognises and what xkb refuses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    /// What goes into the configuration.
    pub name: String,
    /// What appears on screen.
    pub description: String,
}

impl Choice {
    /// The line the list shows.
    #[must_use]
    pub fn line(&self) -> String {
        if self.name.is_empty() {
            self.description.clone()
        } else {
            format!("{} — {}", self.name, self.description)
        }
    }
}

/// Where `base.lst` is.
///
/// From the store at build time, and from the running system otherwise. It is the
/// same file the `keyboard-layout-exists` check reads: a list written by hand
/// here would go stale when `xkeyboard-config` moves, and the failure would be
/// the installer refusing a layout the system supports.
fn base_lst() -> Option<String> {
    data_file(
        "RONOCTUA_XKB_BASE_LST",
        option_env!("RONOCTUA_XKB_BASE_LST"),
        "/run/current-system/sw/share/X11/xkb/rules/base.lst",
    )
}

/// Reads one of the data files, letting the RUNTIME environment win.
///
/// Three sources, in order: the variable as it is NOW, the one baked in at
/// compile time, and the running system's path.
///
/// The runtime one comes first because the compile-time one is a trap that cost
/// an hour here: `option_env!` is resolved when the crate is built, and cargo
/// does not track environment variables as inputs. A binary compiled before the
/// devshell exported the path goes on believing the file does not exist, however
/// many times it is re-run — and the symptom was an installer refusing every
/// keyboard layout that exists.
///
/// The baked one stays because it is what the ISO has: there is no environment
/// there, and no `/run/current-system` either.
fn data_file(var: &str, baked: Option<&str>, fallback: &str) -> Option<String> {
    std::env::var(var)
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| baked.map(ToOwned::to_owned))
        .or_else(|| Some(fallback.to_owned()))
        .and_then(|p| std::fs::read_to_string(p).ok())
}

/// Reads one section of `base.lst`.
///
/// The file is sections opened by `! layout`, `! variant`, `! option`, each line
/// being `name` then whitespace then the description. `keep` decides which lines
/// of the section survive — it is what filters the variants of one layout out of
/// the two thousand that exist, and the `compose:` options out of all the others.
///
/// With no file — a build without the package — it gives back an empty list, and
/// validation refuses everything. That is deliberate: silently accepting
/// anything is how one ends up typing a passphrase on a mute `us`.
fn section(header: &str, keep: &dyn Fn(&str, &str) -> bool) -> Vec<Choice> {
    let Some(raw_text) = base_lst() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut inside = false;
    for line in raw_text.lines() {
        if line.starts_with('!') {
            inside = line.starts_with(header);
            continue;
        }
        if !inside {
            continue;
        }
        // `trim()` AND NOT `trim_end()`: `base.lst` indents every entry, so
        // splitting the raw line gives an empty first field and drops the whole
        // list. The symptom was the installer refusing every layout that
        // exists, which reads as the file being missing.
        let mut parts = line.trim().splitn(2, char::is_whitespace);
        let (Some(name), Some(rest)) = (parts.next(), parts.next()) else {
            continue;
        };
        if name.is_empty() {
            continue;
        }
        let description = rest.trim();
        if keep(name, description) {
            out.push(Choice {
                name: name.to_owned(),
                description: description.to_owned(),
            });
        }
    }
    out
}

/// The xkb layouts this system actually has.
#[must_use]
pub fn available_layouts() -> Vec<Choice> {
    section("! layout", &|_, _| true)
}

/// Just the names, for whoever only validates.
#[must_use]
pub fn layout_names() -> Vec<String> {
    available_layouts().into_iter().map(|c| c.name).collect()
}

/// The variants of ONE layout, with "no variant" first.
///
/// In `base.lst` a variant line reads `abnt2  br: Brazil ABNT2` — the layout it
/// belongs to is the first word of the description, before the colon. Filtering
/// by it is what keeps the list at the handful that apply instead of the two
/// thousand that exist.
///
/// "No variant" is FIRST, and it is not a courtesy: for `gb` it is the right
/// answer, because both variants it has put a dead accent on level 1 — the key
/// stops writing a backtick until you press space, which on a keyboard that
/// types code all day is worse than the problem it solves.
#[must_use]
pub fn variants_of(layout: &str) -> Vec<Choice> {
    let prefix = format!("{layout}:");
    let mut out = vec![Choice {
        name: String::new(),
        description: "—".to_owned(),
    }];
    out.extend(section("! variant", &|_, d| d.starts_with(&prefix)));
    out
}

/// The xkb options that make a key the Compose key, with "none" first.
///
/// Only the `compose:` family: the section also carries level-3 switches, caps
/// behaviour and terminate-with, and offering those here would be answering a
/// question nobody asked.
#[must_use]
pub fn compose_options() -> Vec<Choice> {
    let mut out = vec![Choice {
        name: String::new(),
        description: "—".to_owned(),
    }];
    out.extend(section("! option", &|n, _| n.starts_with("compose:")));
    out
}

/// The locales this system can generate, with the British one first.
///
/// From glibc's own `SUPPORTED`, whose lines read `pt_BR.UTF-8/UTF-8 \\`. Only the
/// UTF-8 ones: a terminal-first system has no business offering ISO-8859.
///
/// The default sits at the TOP because the top is what someone pressing Enter
/// gets, and with no file the list is the default alone — refusing to install
/// because a package moved would be worse than installing in British English.
#[must_use]
pub fn available_locales() -> Vec<String> {
    let mut out = vec![DEFAULT_LOCALE.to_owned()];
    let Some(raw_text) = data_file(
        "RONOCTUA_LOCALE_SUPPORTED",
        option_env!("RONOCTUA_LOCALE_SUPPORTED"),
        "/run/current-system/sw/share/i18n/SUPPORTED",
    ) else {
        return out;
    };
    for line in raw_text.lines() {
        let name = line.trim().trim_end_matches('\\').trim();
        let Some((locale, charset)) = name.split_once('/') else {
            continue;
        };
        if charset == "UTF-8" && locale.ends_with(".UTF-8") && locale != DEFAULT_LOCALE {
            out.push(locale.to_owned());
        }
    }
    out
}

/// The timezones tzdb knows, with the default first.
///
/// From `zone1970.tab`, whose lines read
/// `GB,GG,IM,JE<TAB>+513030-0000731<TAB>Europe/London`. The third field is the
/// name, and it is the only one that goes anywhere — the coordinates and the
/// country codes are for a map this installer does not draw.
#[must_use]
pub fn available_timezones() -> Vec<String> {
    let mut out = vec![DEFAULT_TIMEZONE.to_owned()];
    let Some(raw_text) = data_file(
        "RONOCTUA_TZ_TAB",
        option_env!("RONOCTUA_TZ_TAB"),
        "/etc/zoneinfo/zone1970.tab",
    ) else {
        return out;
    };
    for line in raw_text.lines() {
        if line.starts_with('#') {
            continue;
        }
        let Some(name) = line.split('\t').nth(2) else {
            continue;
        };
        let name = name.trim();
        if !name.is_empty() && name != DEFAULT_TIMEZONE {
            out.push(name.to_owned());
        }
    }
    out.sort_unstable_by(|a, b| {
        // The default stays on top; everything else alphabetical, because a list
        // of three hundred zones in tzdb's file order is a list nobody can scan.
        (a != DEFAULT_TIMEZONE, a).cmp(&(b != DEFAULT_TIMEZONE, b))
    });
    out
}

/// The timezone when nobody chooses another.
pub const DEFAULT_TIMEZONE: &str = "Europe/London";

/// The Compose key when nobody chooses another.
///
/// AltGr, which is the key a keyboard has and nothing else uses: the right Alt
/// sits under the hand that is not typing the letter, and no layout here needs
/// it for a third level that Compose cannot also reach.
pub const DEFAULT_COMPOSE: &str = "compose:ralt";

/// The keyboard layout when nobody chooses another.
pub const DEFAULT_LAYOUT: &str = "gb";

/// The interface language when nobody chooses another.
pub const DEFAULT_LANGUAGE: &str = "en-GB";

/// The system's locale when nobody chooses another.
///
/// British English, and metric by consequence: `en_GB` is the only widely
/// available English locale whose `LC_MEASUREMENT` is metric. Picking `en_US`
/// here would bring inches and Fahrenheit along with the language.
pub const DEFAULT_LOCALE: &str = "en_GB.UTF-8";

/// Validates an answer.
///
/// # Errors
///
/// The answer will not do; the message says why.
pub fn validate_it(
    q: Question,
    answer: &str,
    languages: &[&str],
    layouts: &[&str],
) -> Result<(), Invalid> {
    if answer.is_empty() && !q.computed() && !q.optional() {
        return Err(Invalid::Blank);
    }
    match q {
        Question::Hostname => {
            if !answer.starts_with(HOSTNAME_PREFIX) {
                return Err(Invalid::HostnameWithoutPrefix);
            }
            let body = answer;
            let valid = body.len() <= 63
                && !body.starts_with('-')
                && !body.ends_with('-')
                && body
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
            if !valid {
                return Err(Invalid::HostnameMalformed);
            }
        }
        Question::Passphrase => {
            // It counts CHARACTERS, not bytes: a Portuguese phrase with accents
            // has more bytes than characters, and refusing it for that would be
            // punishing whoever writes in their own language.
            if answer.chars().count() < MINIMUM_PASSPHRASE {
                return Err(Invalid::PassphraseTooShort);
            }
        }
        Question::Language => {
            if !languages.contains(&answer) {
                return Err(Invalid::UnknownLanguage);
            }
        }
        // THE THREE KEYBOARD ANSWERS AND THE LOCALE need no check here, and that
        // is a property of the screen and not an omission: they are lists built
        // from the same file that would validate them, so the only reachable
        // answers are the valid ones. The layout below is the exception because
        // the terminal-less mode reads it from the environment, where anything
        // can arrive.
        Question::KeyboardVariant
        | Question::ComposeKey
        | Question::SystemLocale
        | Question::Timezone => {}
        // AGAINST THE REAL LIST, and not a shape check: xkb accepts a layout
        // that does not exist and falls back to a mute `us`, so a plausible
        // typo would install a keyboard nobody can type on.
        Question::Keyboard => {
            if !layouts.contains(&answer) {
                return Err(Invalid::UnknownLayout);
            }
        }
        Question::Disk => {
            // Only the shape; existence is checked against the actual disk
            // in `disk::listar`, which needs the system.
            if !answer.starts_with("/dev/") {
                return Err(Invalid::InvalidDisk);
            }
        }
        Question::WifiPassword => {
            // Eight is the WPA standard's MINIMUM — a shorter password is not even
            // attempted by the radio, and the failure would show up as "wrong
            // password" instead of "short password".
            if answer.chars().count() < MINIMO_SENHA_WIFI {
                return Err(Invalid::WifiPasswordTooShort);
            }
        }
        Question::Password | Question::ResumeOffset | Question::Wifi => {}
    }
    Ok(())
}

/// The collected answers.
#[derive(Debug, Clone, Default)]
pub struct Answers {
    /// The disk's path.
    pub disk: String,
    /// The machine's name.
    pub hostname: String,
    /// The user's password.
    pub password: String,
    /// The disk's passphrase.
    pub passphrase: String,
    /// The language.
    pub language: String,
    /// The keyboard layout, in xkb's name.
    pub keyboard: String,
    /// The layout's variant. Empty means the plain layout, which is a valid answer.
    pub keyboard_variant: String,
    /// The xkb option that makes a key the Compose key. Empty means no Compose.
    pub compose_key: String,
    /// The system's locale, glibc's name (`en_GB.UTF-8`).
    pub system_locale: String,
    /// The timezone, in tzdb's name (`Europe/London`).
    pub timezone: String,
    /// The swap's offset, computed once the disk exists.
    pub resume_offset: Option<u64>,
    /// The chosen Wi-Fi network. Empty = going by cable.
    pub wifi_ssid: String,
    /// Its password. It **never** enters the `host.nix` (section 9).
    pub wifi_password: String,
}

impl Answers {
    /// Reads one answer per question.
    #[must_use]
    pub fn get(&self, q: Question) -> &str {
        match q {
            Question::Disk => &self.disk,
            Question::Hostname => &self.hostname,
            Question::Password => &self.password,
            Question::Passphrase => &self.passphrase,
            Question::Language => &self.language,
            Question::Keyboard => &self.keyboard,
            Question::KeyboardVariant => &self.keyboard_variant,
            Question::ComposeKey => &self.compose_key,
            Question::SystemLocale => &self.system_locale,
            Question::Timezone => &self.timezone,
            Question::Wifi => &self.wifi_ssid,
            Question::WifiPassword => &self.wifi_password,
            Question::ResumeOffset => "",
        }
    }

    /// Writes an answer.
    pub fn set(&mut self, q: Question, v: String) {
        match q {
            Question::Disk => self.disk = v,
            Question::Hostname => self.hostname = v,
            Question::Password => self.password = v,
            Question::Passphrase => self.passphrase = v,
            Question::Language => self.language = v,
            Question::Keyboard => self.keyboard = v,
            Question::KeyboardVariant => self.keyboard_variant = v,
            Question::ComposeKey => self.compose_key = v,
            Question::SystemLocale => self.system_locale = v,
            Question::Timezone => self.timezone = v,
            Question::Wifi => self.wifi_ssid = v,
            Question::WifiPassword => self.wifi_password = v,
            Question::ResumeOffset => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LANGUAGES: &[&str] = &["pt-BR", "en-GB"];
    /// Enough to tell an accepted layout from a refused one.
    const LAYOUTS: &[&str] = &["br", "us", "gb"];

    #[test]
    fn there_are_exactly_thirteen_questions() {
        // Milestone 8's CRITERION still holds: one asks what ONLY THE PERSON
        // knows, and nothing beyond. One more question is a decision the project
        // handed back to the person after already making it (section 7.1).
        //
        // There were six; the network added two, and the keyboard a ninth.
        //
        // The network's two: `nixos-facter` has nothing to detect there — a
        // Wi-Fi password is a per-person secret — and NOT asking prevents
        // installing on a machine with no Ethernet port.
        //
        // THE KEYBOARD is the same shape of argument, and it is worth stating
        // because "the keyboard has ONE place" was true and stopped being
        // enough. `nixos-facter` reads the hardware; a layout is not hardware,
        // it is what the person's fingers expect. And xkb does not refuse a
        // layout that does not exist — it falls back to a mute `us`.
        //
        // So not asking meant installing on someone else's layout, and the
        // moment one finds out is while typing the disk's passphrase: not
        // echoed, and not correctable afterwards.
        assert_eq!(Question::ALL_KEYS.len(), 13);
        let mut views: Vec<_> = Question::ALL_KEYS.to_vec();
        views.sort();
        views.dedup();
        assert_eq!(views.len(), 13, "ha pergunta repetida na lista");
    }

    #[test]
    fn whoever_goes_by_wire_does_not_see_the_password_screen() {
        // Without this rule, whoever chooses "no Wi-Fi" lands on a field asking for the
        // password of nothing — with no way out other than typing
        // something, because empty would be an error.
        let mut r = Answers::default();
        assert!(Question::WifiPassword.skipped_one(&r));
        r.wifi_ssid = "alguma-rede".into();
        assert!(!Question::WifiPassword.skipped_one(&r));
        // The others are never skipped for any answer.
        for q in Question::ALL_KEYS {
            if q != Question::WifiPassword {
                assert!(!q.skipped_one(&r), "{q:?} foi pulada");
            }
        }
    }

    #[test]
    fn a_blank_network_is_an_answer_and_not_an_omission() {
        // Blank there means "I am going by cable". Refusing it as empty
        // would force whoever uses a cable to invent a network name.
        assert!(Question::Wifi.optional());
        assert!(validate_it(Question::Wifi, "", LANGUAGES, LAYOUTS).is_ok());
    }

    #[test]
    fn the_wifi_password_respects_the_wpa_minimum() {
        // Eight is the standard's minimum. A shorter password is not even attempted
        // by the radio, and the failure would show up as "wrong password".
        assert_eq!(
            validate_it(Question::WifiPassword, "1234567", LANGUAGES, LAYOUTS),
            Err(Invalid::WifiPasswordTooShort)
        );
        assert!(validate_it(Question::WifiPassword, "12345678", LANGUAGES, LAYOUTS).is_ok());
    }

    #[test]
    fn the_wifi_password_does_not_show_on_screen() {
        assert!(Question::WifiPassword.secret_one());
        assert!(!Question::Wifi.secret_one());
    }

    #[test]
    fn the_language_is_first_and_the_disk_is_second() {
        // THE LANGUAGE FIRST because every screen after it is drawn in the
        // answer to it — including the one that erases a disk. Asking anything
        // before it means asking in whatever language the ISO happened to carry.
        assert_eq!(Question::ALL_KEYS[0], Question::Language);
        // AND THE DISK SECOND, which is what survives of the older rule: it
        // erases data, so what is invested before it has to be one screen.
        assert_eq!(Question::ALL_KEYS[1], Question::Disk);
    }

    #[test]
    fn every_question_that_can_be_defaulted_has_a_default() {
        // The screen opens on an answer that works, and the terminal-less mode
        // uses the same one. What has no default is what nobody can guess.
        for q in Question::ALL_KEYS {
            let guessable = !matches!(
                q,
                Question::Disk
                    | Question::Hostname
                    | Question::Password
                    | Question::Passphrase
                    | Question::WifiPassword
                    | Question::ResumeOffset
                    | Question::Wifi
                    | Question::KeyboardVariant
            );
            assert_eq!(
                q.default_answer().is_some(),
                guessable,
                "{q:?} discorda sobre ter resposta padrao"
            );
        }
        assert_eq!(Question::ComposeKey.default_answer(), Some(DEFAULT_COMPOSE));
        assert_eq!(Question::Timezone.default_answer(), Some(DEFAULT_TIMEZONE));
    }

    #[test]
    fn a_secret_does_not_show_on_screen() {
        assert!(Question::Password.secret_one());
        assert!(Question::Passphrase.secret_one());
        assert!(!Question::Hostname.secret_one());
    }

    #[test]
    fn the_hostname_requires_the_prefix() {
        assert_eq!(
            validate_it(Question::Hostname, "maquina", LANGUAGES, LAYOUTS),
            Err(Invalid::HostnameWithoutPrefix)
        );
        assert!(validate_it(Question::Hostname, "noctua-portatil", LANGUAGES, LAYOUTS).is_ok());
    }

    #[test]
    fn the_hostname_follows_rfc_1123() {
        for bad in [
            "noctua-",
            "noctua-MAIUSCULA",
            "noctua-com espaco",
            "noctua-x_y",
        ] {
            assert!(
                validate_it(Question::Hostname, bad, LANGUAGES, LAYOUTS).is_err(),
                "`{bad}` devia ser recusado"
            );
        }
    }

    #[test]
    fn the_passphrase_counts_characters_and_not_bytes() {
        // Twelve accented characters exceed 12 bytes. Refusing by bytes
        // would punish whoever writes the phrase in their own language.
        let doze_acentuados = "ãçãoçãoçãoç";
        assert_eq!(doze_acentuados.chars().count(), 11);
        assert!(validate_it(Question::Passphrase, doze_acentuados, LANGUAGES, LAYOUTS).is_err());

        let treze = "ãçãoçãoçãoçã";
        assert_eq!(treze.chars().count(), 12);
        assert!(treze.len() > 12, "o teste so vale se houver acento");
        assert!(validate_it(Question::Passphrase, treze, LANGUAGES, LAYOUTS).is_ok());
    }

    #[test]
    fn an_unknown_language_is_refused() {
        assert_eq!(
            validate_it(Question::Language, "fr-FR", LANGUAGES, LAYOUTS),
            Err(Invalid::UnknownLanguage)
        );
    }

    #[test]
    fn the_resume_offset_is_not_typed() {
        // It is on section 7.1's list because the installer has to produce it,
        // not because the person knows it. Empty is not an error.
        assert!(Question::ResumeOffset.computed());
        assert!(validate_it(Question::ResumeOffset, "", LANGUAGES, LAYOUTS).is_ok());
    }

    #[test]
    fn empty_is_an_error_in_what_the_person_types() {
        for q in Question::ALL_KEYS {
            if q.computed() || q.optional() {
                continue;
            }
            assert_eq!(
                validate_it(q, "", LANGUAGES, LAYOUTS),
                Err(Invalid::Blank),
                "{q:?}"
            );
        }
    }
}
