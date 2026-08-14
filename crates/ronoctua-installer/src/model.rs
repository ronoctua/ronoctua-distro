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
    /// A deliberate order: what erases data comes first, so the person does not
    /// invest five answers before finding out they chose the wrong
    /// disk.
    pub const ALL_KEYS: [Self; 9] = [
        Self::Disk,
        Self::Hostname,
        Self::Password,
        Self::Passphrase,
        Self::Language,
        Self::Keyboard,
        // THE NETWORK comes after the language and before the offset: it is the last
        // thing the person types, and it comes after the language so the network list
        // already appears in the chosen language.
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

    /// Whether leaving it blank is a valid answer.
    ///
    /// Only the network: blank there means "I am going by cable", which is a choice
    /// and not an omission. In the others, empty is an error — a blank hostname
    /// means nothing.
    #[must_use]
    pub const fn optional(self) -> bool {
        matches!(self, Self::Wifi)
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
    HostnameMalFormado,
    /// A hostname without the prefix section 7.1 requires.
    HostnameWithoutPrefix,
    /// A passphrase that is too short.
    PassphraseCurta,
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
            Self::HostnameMalFormado => {
                "so letras minusculas, digitos e hifen; nao pode comecar nem terminar com hifen"
            }
            Self::HostnameWithoutPrefix => "o nome da maquina comeca com `noctua-`",
            Self::PassphraseCurta => "a frase-secreta do disco precisa de ao menos 12 caracteres",
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
pub const MINIMO_PASSPHRASE: usize = 12;

/// The smallest password accepted on a WPA network.
///
/// Eight is the standard's minimum, not a choice of ours: a shorter password is not
/// even attempted by the radio.
pub const MINIMO_SENHA_WIFI: usize = 8;

/// The hostname's mandatory prefix (section 7.1).
pub const HOSTNAME_PREFIX: &str = "noctua-";

/// The xkb layouts this system actually has.
///
/// From `base.lst` of `xkeyboard-config`, which is the same file the
/// `keyboard-layout-exists` check reads. A hand-written list here would go
/// stale, and the failure would be the installer refusing a layout the system
/// supports.
///
/// With no file — a build without the package — it gives back an empty list,
/// and validation refuses everything. That is deliberate: silently accepting
/// anything is how one ends up typing a passphrase on a mute `us`.
#[must_use]
pub fn available_layouts() -> Vec<String> {
    let lst = option_env!("RONOCTUA_XKB_BASE_LST")
        .unwrap_or("/run/current-system/sw/share/X11/xkb/rules/base.lst");
    let Ok(raw_text) = std::fs::read_to_string(lst) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut inside = false;
    for line in raw_text.lines() {
        if line.starts_with('!') {
            inside = line.starts_with("! layout");
            continue;
        }
        if inside && let Some(name) = line.split_whitespace().next() {
            out.push(name.to_owned());
        }
    }
    out
}

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
            let valido = body.len() <= 63
                && !body.starts_with('-')
                && !body.ends_with('-')
                && body
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
            if !valido {
                return Err(Invalid::HostnameMalFormado);
            }
        }
        Question::Passphrase => {
            // It counts CHARACTERS, not bytes: a Portuguese phrase with accents
            // has more bytes than characters, and refusing it for that would be
            // punishing whoever writes in their own language.
            if answer.chars().count() < MINIMO_PASSPHRASE {
                return Err(Invalid::PassphraseCurta);
            }
        }
        Question::Language => {
            if !languages.contains(&answer) {
                return Err(Invalid::UnknownLanguage);
            }
        }
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
            Question::Wifi => self.wifi_ssid = v,
            Question::WifiPassword => self.wifi_password = v,
            Question::ResumeOffset => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LANGUAGES: &[&str] = &["pt-BR", "en-US"];
    /// Enough to tell an accepted layout from a refused one.
    const LAYOUTS: &[&str] = &["br", "us", "gb"];

    #[test]
    fn there_are_exactly_nine_questions() {
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
        assert_eq!(Question::ALL_KEYS.len(), 9);
        let mut views: Vec<_> = Question::ALL_KEYS.to_vec();
        views.sort();
        views.dedup();
        assert_eq!(views.len(), 9, "ha pergunta repetida na lista");
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
    fn the_disk_is_the_first_question() {
        // It erases data. Asking last would make the person invest
        // five answers before finding out they chose the wrong disk.
        assert_eq!(Question::ALL_KEYS[0], Question::Disk);
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
