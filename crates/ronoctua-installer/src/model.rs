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
//! [`Question::skipped_one`]. Whoever goes by cable answers fifteen screens;
//! whoever chooses a network answers sixteen.
//!
//! # And the list of programs is one too
//!
//! [`Question::LeftOut`] only appears for a `full` installation: the
//! programs it lists are none of them on a `minimal` machine, so there is
//! nothing to leave out of one. A `full` installation answers one screen more.
//!
//! # And the network is a GATE now
//!
//! The person's repository became mandatory (2026-10-05): the installation
//! applies their declarations and restores their belongings, and asks the
//! release which installer to use. So leaving the network question requires a
//! way out (`crate::gate`), and the three questions about the repository come
//! after it — each checked against the network before the next one appears.

use crate::key;

/// The questions. **Eighteen, and the test counts them.**
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
    /// Which of the two installations this machine is.
    ///
    /// `full` brings everything and is the recommended one; `minimal` leaves out
    /// the applications — Android, the toolchains, the browser, the editor,
    /// media and the extra font variants — and installs in a fraction of the
    /// time, because there is that much less to download (what no public
    /// cache serves is either on the ISO or in the distro's binary cache).
    ///
    /// It is asked and not guessed because it is the one answer here that is
    /// nobody else's to infer: it says what the machine is FOR. And it is not
    /// final — the answer becomes a line in `host.nix`, and a group comes back
    /// with that line changed and the machine applied again (the update, or
    /// `ronoctua-machine apply`).
    Profile,
    /// Which of the heavy programs a `full` installation leaves out of this
    /// machine (`OFFERED`, the ledger's `offered`).
    ///
    /// # Why it is asked, and why its answer is not a line of `host.nix`
    ///
    /// The same reason as the installation's own: it says what the machine
    /// is FOR, and some of these programs weigh gigabytes a person may never
    /// open. But a program left out is the PERSON'S decision about a program,
    /// not a fact about the machine — so it is a decline in their own
    /// `declined.json`, scoped to this hostname, which every update honours
    /// and the software centre undoes, and never a switch in `host.nix`.
    ///
    /// The answer is the unmarked ids joined by commas, or [`LEFT_OUT_NONE`]
    /// — never empty, so it crosses the answers' file like any other.
    LeftOut,
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
    /// The private repository the person's belongings are kept in.
    ///
    /// # Why it is asked here, and why it is not optional any more
    ///
    /// Because only the person knows it, and it is what turns a new machine
    /// into THEIR machine: their declarations (`plain/person/`) are applied
    /// to the machine's flake before it is built, and their belongings are
    /// restored into the home before anybody logs in (`crate::access`).
    ///
    /// The address alone, never a credential (`user:pass@` is refused, and so
    /// are `http://` and a local path). It is written into the PERSON'S home
    /// (`~/.config/ronoctua/belongings.json`, owned by them), never into
    /// `host.nix`: it is theirs, and `host.nix` describes the machine.
    BelongingsAddress,
    /// How this machine reaches the repository.
    ///
    /// For an SSH address it is not typed: the installer makes a deploy key
    /// in RAM, shows it (text, QR, the host's page) and probes the repository
    /// until the key reads and writes. For an HTTPS address it is a token,
    /// typed and probed the same way. The terminal-less mode names a FILE
    /// holding the one or the other — never its content.
    BelongingsAccess,
    /// The passphrase that opens the belongings: proved by opening the
    /// repository's `key.age`, or — the repository being empty — typed twice,
    /// and the installation makes the repository with it.
    BelongingsPassphrase,
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
    pub const ALL_KEYS: [Self; 18] = [
        Self::Language,
        Self::Disk,
        // WHICH installation, right after WHERE it goes. It comes before the
        // keyboard trio because it is the answer that changes how long
        // everything after it takes, and the person deserves that in front of
        // them early rather than at the end of a form.
        Self::Profile,
        // AND WHAT OF IT, right after: the list only exists for `full`, and
        // it changes the download as much as the installation does.
        Self::LeftOut,
        Self::Keyboard,
        Self::KeyboardVariant,
        Self::ComposeKey,
        Self::SystemLocale,
        Self::Timezone,
        Self::Hostname,
        Self::Password,
        Self::Passphrase,
        // THE NETWORK comes after the language so the network list already
        // appears in the chosen language — and it is a GATE: what follows is
        // checked against the network.
        Self::Wifi,
        Self::WifiPassword,
        // THE BELONGINGS LAST OF WHAT IS TYPED: the answers about the person
        // rather than the machine, each proved before the next one — the
        // address, the access that reads and writes it, the passphrase that
        // opens it.
        Self::BelongingsAddress,
        Self::BelongingsAccess,
        Self::BelongingsPassphrase,
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
            Self::Profile => key::INST_PROFILE,
            Self::LeftOut => key::INST_LEFT_OUT,
            Self::ResumeOffset => key::INST_RESUME_OFFSET,
            Self::Wifi => key::INST_WIFI,
            Self::WifiPassword => key::INST_WIFI_PASSWORD,
            // `inst-belongings` and not `-address`: the label names the
            // terminal-less mode's variable, and `RONOCTUA_INST_BELONGINGS`
            // is what the tests and the VM already set.
            Self::BelongingsAddress => key::INST_BELONGINGS,
            Self::BelongingsAccess => key::INST_BELONGINGS_ACCESS,
            Self::BelongingsPassphrase => key::INST_BELONGINGS_PASSPHRASE,
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
            Self::Profile => key::INST_PROFILE_HELP,
            Self::LeftOut => key::INST_LEFT_OUT_HELP,
            Self::ResumeOffset => key::INST_RESUME_OFFSET_HELP,
            // THE NETWORK'S HELP NAMES THE HOSTS IT WILL REACH — answering
            // the question is the consent to reach them (`$hosts`).
            Self::Wifi => key::INST_NETWORK_HELP,
            // The password's own line: that it does not enter the
            // repository matters more here.
            Self::WifiPassword => key::INST_WIFI_HELP,
            Self::BelongingsAddress => key::INST_BELONGINGS_HELP,
            Self::BelongingsAccess => key::INST_BELONGINGS_ACCESS_HELP,
            Self::BelongingsPassphrase => key::INST_BELONGINGS_PASSPHRASE_HELP,
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
        // The ACCESS too: for an HTTPS address what is typed there is a
        // token, which opens the repository as surely as a password.
        matches!(
            self,
            Self::Password
                | Self::Passphrase
                | Self::WifiPassword
                | Self::BelongingsAccess
                | Self::BelongingsPassphrase
        )
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
            Self::Profile => Some(DEFAULT_PROFILE),
            // The ledger's unmarked rows: what pressing Enter through the
            // list leaves out.
            Self::LeftOut => Some(DEFAULT_LEFT_OUT),
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
        //
        // The belongings are NOT here any more: the repository is mandatory
        // (owner, 2026-10-05), and none of its three answers may be blank.
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
    ///
    /// The list of programs is skipped for any installation but `full`: none
    /// of them is on a `minimal` machine (`offered-programs-are-the-registrys`
    /// proves it of the evaluated systems), so a decline there would decline
    /// nothing.
    #[must_use]
    pub fn skipped_one(self, r: &Answers) -> bool {
        (self == Self::WifiPassword && r.wifi_ssid.is_empty())
            || (self == Self::LeftOut && (r.profile != FULL_PROFILE || OFFERED.is_empty()))
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
    /// A profile that is neither of the two.
    UnknownProfile,
    /// A repository address the belongings may not use.
    Belongings(ronoctua_belongings::prefs::Invalid),
    /// The access named is not a file's absolute path (the terminal-less
    /// mode names the deploy key's or the token's file, never its content).
    AccessNotAFile,
    /// A program to leave out that the list does not offer, or one named
    /// twice, or `none` beside a program.
    NotOffered,
}

impl Invalid {
    /// The sentence the TUI shows for it (invariant 8). The English of
    /// [`Display`](std::fmt::Display) is the terminal-less mode's — a command
    /// line's error, which invariant 18 keeps in English.
    #[must_use]
    pub const fn key(&self) -> &'static str {
        match self {
            Self::Blank => key::INST_INVALID_BLANK,
            Self::HostnameMalformed => key::INST_INVALID_HOSTNAME,
            Self::HostnameWithoutPrefix => key::INST_INVALID_HOSTNAME_PREFIX,
            Self::PassphraseTooShort => key::INST_INVALID_PASSPHRASE_SHORT,
            Self::InvalidDisk => key::INST_INVALID_DISK,
            Self::UnknownLanguage => key::INST_INVALID_LANGUAGE,
            Self::UnknownLayout => key::INST_INVALID_LAYOUT,
            Self::WifiPasswordTooShort => key::INST_WIFI_PASSWORD_SHORT,
            Self::UnknownProfile => key::INST_INVALID_PROFILE,
            Self::Belongings(why) => crate::access::invalid_key(*why),
            Self::AccessNotAFile => key::INST_INVALID_ACCESS,
            Self::NotOffered => key::INST_INVALID_LEFT_OUT,
        }
    }
}

impl Invalid {
    /// The arguments its sentence takes: the hostname's prefix is the
    /// code's ([`HOSTNAME_PREFIX`]), never spelled in a `.ftl`.
    #[must_use]
    pub fn args(&self) -> Vec<(&'static str, String)> {
        match self {
            Self::HostnameWithoutPrefix => vec![("prefix", HOSTNAME_PREFIX.to_owned())],
            _ => Vec::new(),
        }
    }
}

/// The sentence of a text key the installer shows, with the arguments the
/// key takes — the one place a refusal's arguments are given, for the TUI
/// and the terminal-less mode alike.
#[must_use]
pub fn sentence(texts: &ronoctua_i18n::Texts, k: &str) -> String {
    if k == key::INST_INVALID_HOSTNAME_PREFIX {
        let args = Invalid::HostnameWithoutPrefix.args();
        let args: Vec<(&str, &str)> = args.iter().map(|(n, v)| (*n, v.as_str())).collect();
        texts.t_with(k, &args)
    } else {
        texts.t(k)
    }
}

impl std::fmt::Display for Invalid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Blank => "it cannot be left empty",
            Self::HostnameMalformed => {
                "lowercase letters, digits and hyphens only; it may not start or end with a hyphen"
            }
            // THE PREFIX IS THE CODE'S, never spelled a second time.
            Self::HostnameWithoutPrefix => {
                return write!(f, "the machine's name starts with `{HOSTNAME_PREFIX}`");
            }
            Self::PassphraseTooShort => "the disk's passphrase needs at least 12 characters",
            Self::InvalidDisk => "that path is not a disk",
            Self::UnknownProfile => "the installation is `full` or `minimal`",
            Self::UnknownLanguage => "the language is not available",
            Self::UnknownLayout => "the keyboard layout does not exist in xkeyboard-config",
            Self::WifiPasswordTooShort => "a WPA network password is at least 8 characters",
            Self::Belongings(why) => {
                use ronoctua_belongings::prefs::Invalid as Why;
                match why {
                    Why::Empty => "it cannot be left empty",
                    Why::CredentialsInAddress => {
                        "the address carries a user and password; give the address alone, and \
                         the key or token later, from the belongings view"
                    }
                    Why::PlainHttp => {
                        "an http:// address sends the access in the clear; use https:// or ssh"
                    }
                    Why::NotRemote => "a local path or file:// is not a remote repository",
                    Why::UnnamedHost => {
                        "the host must be a name or an IPv4 address: an IPv6 address or a name \
                         with `_` cannot be allowed"
                    }
                    Why::Unknown => "the address is git@host:path, ssh://… or https://…",
                }
            }
            Self::AccessNotAFile => {
                "name the file that holds the deploy key or the token, by its absolute path"
            }
            Self::NotOffered => {
                "the programs left out are ids of the installer's list, separated by commas and \
                 each named once, or `none`"
            }
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
pub const WIFI_PASSWORD_MIN: usize = 8;

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

/// The five XML entities a `base.lst` description can carry, as the
/// characters they stand for; anything else is left as it is.
fn unescaped(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
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
        // `base.lst` is generated from XML, and a few descriptions keep its
        // entities (`The "&lt; &gt;" key`): a person reads the characters.
        let description = unescaped(rest.trim());
        if keep(name, &description) {
            out.push(Choice {
                name: name.to_owned(),
                description,
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
/// Caps Lock: the largest key on the home row, under the left little finger,
/// and what it does by default — shouting — is something nobody asked for. It
/// does not stop existing (the two Shifts together toggle it, and that comes
/// from the system rather than from this answer), and AltGr goes back to being
/// AltGr with the level-3 symbols it always had.
///
/// It has to be the same value as `nix/keyboard-xkb.nix`'s `compose`, and
/// `installer-defaults-match-the-keyboard` is what says so: this is the default
/// of a QUESTION, and that is the default of the SYSTEM — a machine installed
/// by pressing Enter and one built by `nix build` would otherwise type
/// differently.
pub const DEFAULT_COMPOSE: &str = "compose:caps";

/// What the system adds to the Compose answer, whatever it is: the two
/// Shifts together toggle Caps Lock, which is where Caps Lock went when the
/// Compose key took its key. The same value as `nix/keyboard-xkb.nix`'s
/// `always`, and `installer-defaults-match-the-keyboard` says so.
pub const CAPS_TOGGLE: &str = "shift:both_capslock";

// THE KEYS THE COMPOSE QUESTION'S HELP NAMES, by the xkb option that puts
// them to work — the code's, never a sentence's (`keys-are-never-text`): the
// help said "Caps Lock" and "the two Shifts" in both languages, and the
// default had already moved once (it was AltGr). A default with no name here
// does not compile ([`key_of`]).

/// Caps Lock, by name — and the lock the toggle turns on and off.
pub const KEY_CAPS_LOCK: &str = "Caps Lock";
/// AltGr, by name.
pub const KEY_ALTGR: &str = "AltGr";
/// The two Shifts pressed together, by name.
pub const KEY_BOTH_SHIFTS: &str = "Shift+Shift";

/// Each xkb option the help may name, and the key it puts to work.
pub const OPTION_KEYS: [(&str, &str); 3] = [
    ("compose:caps", KEY_CAPS_LOCK),
    ("compose:ralt", KEY_ALTGR),
    ("shift:both_capslock", KEY_BOTH_SHIFTS),
];

/// The key an xkb option of [`OPTION_KEYS`] puts to work. Evaluated while
/// compiling: an option with no name stops the build.
const fn key_of(option: &str) -> &'static str {
    let mut i = 0;
    while i < OPTION_KEYS.len() {
        let (named, key) = OPTION_KEYS[i];
        if named.len() == option.len() {
            let (a, b) = (named.as_bytes(), option.as_bytes());
            let mut j = 0;
            while j < a.len() && a[j] == b[j] {
                j += 1;
            }
            if j == a.len() {
                return key;
            }
        }
        i += 1;
    }
    panic!("an xkb option the Compose help names has no key in OPTION_KEYS")
}

/// What the Compose question's help is handed: the default Compose key, the
/// chord that toggles the lock, and the lock — by the options the system
/// sets, never by words.
pub const COMPOSE_HELP_KEYS: [(&str, &str); 3] = [
    ("compose", key_of(DEFAULT_COMPOSE)),
    ("toggle", key_of(CAPS_TOGGLE)),
    ("capslock", KEY_CAPS_LOCK),
];

/// The keyboard layout when nobody chooses another.
pub const DEFAULT_LAYOUT: &str = "gb";

/// The interface language when nobody chooses another.
pub const DEFAULT_LANGUAGE: &str = ronoctua_i18n::DEFAULT;

/// The system's locale when nobody chooses another.
///
/// British English, and metric by consequence: `en_GB` is the only widely
/// available English locale whose `LC_MEASUREMENT` is metric. Picking `en_US`
/// here would bring inches and Fahrenheit along with the language.
pub const DEFAULT_LOCALE: &str = "en_GB.UTF-8";

/// The complete installation's name — the one that offers the list of
/// programs ([`Question::LeftOut`]).
pub const FULL_PROFILE: &str = "full";

/// The two installations, in the order they appear on screen.
///
/// The names are the ones `ronoctua.profile` takes in `host.nix`, and they are
/// written ONCE: the answer goes into the generated file verbatim, so a second
/// spelling here would be a machine that fails to evaluate on the target.
pub const PROFILES: [&str; 2] = [FULL_PROFILE, "minimal"];

/// The complete installation, and it is the recommended one.
///
/// Recommended because it is the one that needs no second decision afterwards:
/// everything is there. The reduced one is for whoever knows what they are
/// leaving out — and it is what a machine used to WORK ON this distro wants,
/// since installing it takes minutes instead of the best part of an afternoon.
pub const DEFAULT_PROFILE: &str = FULL_PROFILE;

/// One program a `full` installation lists ([`Question::LeftOut`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Offered {
    /// The registry's id (`ronoctua.apps.<id>`): what a decline names.
    pub id: &'static str,
    /// The program's own name, the same in every language — data, like a
    /// launcher entry's `Name=`, and never a text key (invariant 8 frames it).
    pub name: &'static str,
    /// Whether it comes marked: installed unless the person unmarks it.
    pub checked: bool,
}

// `OFFERED` and `DEFAULT_LEFT_OUT`, written from the ledger's `offered` by
// `nix/offered.nix`, which refuses a row the screen could not draw.
include!(env!("RONOCTUA_OFFERED_RS"));

/// The answer that leaves nothing out. No offered id may be it
/// (`nix/offered.nix` refuses one).
pub const LEFT_OUT_NONE: &str = "none";

/// The ids an answer names, as written (trimmed), `none` naming none. Pure.
fn named(answer: &str) -> Vec<&str> {
    if answer.trim() == LEFT_OUT_NONE || answer.trim().is_empty() {
        return Vec::new();
    }
    answer.split(',').map(str::trim).collect()
}

/// Whether `answer` leaves `id` out. Pure.
#[must_use]
pub fn is_left_out(answer: &str, id: &str) -> bool {
    named(answer).contains(&id)
}

/// The ids `answer` names that the list does not offer — what an older
/// installer's answer carries for a program this one no longer lists. Pure.
#[must_use]
pub fn not_offered(answer: &str) -> Vec<String> {
    named(answer)
        .into_iter()
        .filter(|id| !OFFERED.iter().any(|o| o.id == *id))
        .map(ToOwned::to_owned)
        .collect()
}

/// `answer` written as this list writes it: its offered ids in the list's
/// order, or [`LEFT_OUT_NONE`]. Pure.
#[must_use]
pub fn canonical(answer: &str) -> String {
    let out: Vec<&str> = OFFERED
        .iter()
        .map(|o| o.id)
        .filter(|id| is_left_out(answer, id))
        .collect();
    if out.is_empty() {
        LEFT_OUT_NONE.to_owned()
    } else {
        out.join(",")
    }
}

/// `answer` with `id` marked if it was left out, and left out if it was
/// marked — written in the list's order, or [`LEFT_OUT_NONE`] when nothing
/// is left out. An id the list does not offer changes nothing. Pure.
#[must_use]
pub fn toggle(answer: &str, id: &str) -> String {
    let flips = OFFERED.iter().any(|o| o.id == id);
    let out: Vec<&str> = OFFERED
        .iter()
        .map(|o| o.id)
        .filter(|o| is_left_out(answer, o) != (flips && *o == id))
        .collect();
    canonical(&out.join(","))
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
        // THE PROFILE IS CHECKED, unlike the four above it.
        //
        // Those are lists read from the same file that would validate them, so
        // the screen can only offer valid answers. This one is a list of two
        // written here — and the terminal-less mode reads it from the
        // environment, where a typo would reach `host.nix` and fail on the
        // TARGET, during `nixos-install`, with the disk already erased.
        Question::Profile => {
            if !PROFILES.contains(&answer) {
                return Err(Invalid::UnknownProfile);
            }
        }
        // THE LIST'S OWN IDS, each once, or `none` alone: the terminal-less
        // mode reads it from the environment, and a typo there would decline
        // nothing — and install a program the person meant to leave out.
        Question::LeftOut => {
            if answer.trim() != LEFT_OUT_NONE {
                let ids = named(answer);
                let mut seen: Vec<&str> = Vec::new();
                for id in ids {
                    if !OFFERED.iter().any(|o| o.id == id) || seen.contains(&id) {
                        return Err(Invalid::NotOffered);
                    }
                    seen.push(id);
                }
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
            if answer.chars().count() < WIFI_PASSWORD_MIN {
                return Err(Invalid::WifiPasswordTooShort);
            }
        }
        // THE ADDRESS ALONE: a credential in it would sit in a plain file and
        // in git's config; `http://` sends the access in the clear.
        Question::BelongingsAddress => {
            ronoctua_belongings::prefs::repository_is_plain(answer).map_err(Invalid::Belongings)?;
        }
        // A FILE'S PATH, never the key or the token itself: the terminal-less
        // mode reads it from the environment, and a secret there would be in
        // every child's. Whether it reads and writes is the probe's question,
        // asked before the first step.
        Question::BelongingsAccess => {
            if !answer.starts_with('/') {
                return Err(Invalid::AccessNotAFile);
            }
        }
        // Whatever the person chose: it opens the repository, or — an empty
        // one — it is the one the repository will be made with, and THAT is
        // where its length is asked (`MINIMUM_PASSPHRASE`, the TUI's second
        // typing and the terminal-less mode's preflight).
        Question::BelongingsPassphrase
        | Question::Password
        | Question::ResumeOffset
        | Question::Wifi => {}
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
    /// Which of the two installations: `full` or `minimal`.
    pub profile: String,
    /// The programs left out of this machine: offered ids joined by `,`, or
    /// `none`. Read only for a `full` installation ([`Answers::left_out_ids`]).
    pub left_out: String,
    /// When they were left out — ONE stamp per run, set by
    /// [`run_command`](crate::executor::run_command) (or the dry run), so
    /// every copy of the declines it writes is byte for byte the same.
    /// Computed; never asked.
    pub left_out_at: Option<ronoctua_person::Stamp>,
    /// The swap's offset, computed once the disk exists.
    pub resume_offset: Option<u64>,
    /// The chosen Wi-Fi network. Empty = going by cable.
    pub wifi_ssid: String,
    /// Its password. It **never** enters the `host.nix` (section 9).
    pub wifi_password: String,
    /// The belongings' repository. It never enters `host.nix`: it is written
    /// into the person's home.
    pub belongings: String,
    /// The file holding this machine's access to it: the deploy key the
    /// installer made, or the token typed (or, terminal-less, the file
    /// named).
    pub belongings_access: String,
    /// The belongings' passphrase. Never written anywhere but the `0600`
    /// file a restore reads and deletes.
    pub belongings_passphrase: String,
    /// What the probe found: the repository has nothing yet, so the
    /// installation MAKES it (`init`) instead of restoring from it.
    /// Computed, like the offset.
    pub belongings_empty: Option<bool>,
    /// The swapfile, measured once the disk is known. Computed; `None`
    /// leaves the hardware report's default.
    pub swap: Option<crate::swap::Swap>,
    /// The machine started from BIOS: detected (`crate::platform`), never
    /// asked — the person only chose to go on with it — and it is GRUB the
    /// machine boots (`ronoctua.firmware`, `host.nix`).
    pub bios: bool,
    /// Whether the machine is built from — and keeps — the installer's own
    /// distro tree (`crate::plan::Origin::aimed`). Computed when the
    /// installation runs; the dry run leaves it out, as it leaves the swap.
    pub aimed: bool,
}

/// The variable of the answers' file that names where to resume — the label
/// of the first question the release's installer asks.
pub const RESUME: &str = "RONOCTUA_INST_RESUME";

/// What every answer's variable starts with ([`Question::variable`],
/// [`RESUME`]): the name a child must never inherit.
pub const ANSWER_PREFIX: &str = "RONOCTUA_INST_";

/// Every answer's variable by name: each question's, the resume's, and any
/// other this process's environment holds under [`ANSWER_PREFIX`].
#[must_use]
pub fn answer_variables() -> Vec<String> {
    let mut names: Vec<String> = Question::ALL_KEYS
        .iter()
        .map(|q| q.variable())
        .chain(std::iter::once(RESUME.to_owned()))
        .collect();
    for (name, _) in std::env::vars_os() {
        if let Some(name) = name.to_str()
            && name.starts_with(ANSWER_PREFIX)
            && !names.iter().any(|n| n == name)
        {
            names.push(name.to_owned());
        }
    }
    names
}

impl Answers {
    /// The answers as the release's installer reads them back
    /// (`--answers`): one `NAME=value` per answered question, named by
    /// [`Question::variable`], and the question to resume at. It carries
    /// SECRETS: whoever writes it writes it `0600`, and whoever reads it
    /// deletes it.
    #[must_use]
    pub fn lines(&self, resume_at: Question) -> String {
        let mut out = String::new();
        for q in Question::ALL_KEYS {
            let v = self.get(q);
            if q.computed() || v.is_empty() || v.contains('\n') {
                continue;
            }
            out.push_str(&q.variable());
            out.push('=');
            out.push_str(v);
            out.push('\n');
        }
        // BIOS, CHOSEN: by the time the answers are handed on, a machine
        // started from BIOS got this far only by the person's choice — the
        // TUI's worse way, or the variable itself —, and the release's
        // installer without a terminal would stop again without it.
        if self.bios {
            out.push_str(crate::platform::FIRMWARE);
            out.push('=');
            out.push_str(crate::platform::CHOOSES_BIOS);
            out.push('\n');
        }
        out.push_str(RESUME);
        out.push('=');
        out.push_str(resume_at.label());
        out.push('\n');
        out
    }

    /// Reads [`Answers::lines`] back: the answers, and the question to
    /// resume at. A line this installer does not know is skipped (a newer
    /// one may have more), and a value is taken as it is — the questions
    /// validate again when they are shown or run.
    #[must_use]
    pub fn from_lines(text: &str) -> (Self, Option<Question>) {
        let mut r = Self::default();
        let mut resume = None;
        for line in text.lines() {
            let Some((name, value)) = line.split_once('=') else {
                continue;
            };
            if name == RESUME {
                resume = Question::ALL_KEYS.into_iter().find(|q| q.label() == value);
                continue;
            }
            // BIOS, CHOSEN before the hop: the TUI does not ask it again.
            if name == crate::platform::FIRMWARE {
                r.bios = value == crate::platform::CHOOSES_BIOS;
                continue;
            }
            if let Some(q) = Question::ALL_KEYS
                .into_iter()
                .find(|q| !q.computed() && q.variable() == name)
            {
                r.set(q, value.to_owned());
            }
        }
        (r, resume)
    }

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
            Question::Profile => &self.profile,
            Question::LeftOut => &self.left_out,
            Question::Wifi => &self.wifi_ssid,
            Question::WifiPassword => &self.wifi_password,
            Question::BelongingsAddress => &self.belongings,
            Question::BelongingsAccess => &self.belongings_access,
            Question::BelongingsPassphrase => &self.belongings_passphrase,
            Question::ResumeOffset => "",
        }
    }

    /// The ids this machine leaves out, in the list's order: none unless the
    /// installation is `full` and the answer names some.
    #[must_use]
    pub fn left_out_ids(&self) -> Vec<&'static str> {
        if self.profile != FULL_PROFILE {
            return Vec::new();
        }
        OFFERED
            .iter()
            .filter(|o| is_left_out(&self.left_out, o.id))
            .map(|o| o.id)
            .collect()
    }

    /// The offered ids this machine KEEPS — the ones ticked — in the list's
    /// order: none unless the installation is `full`. With
    /// [`Answers::left_out_ids`], the whole answer: a reinstall under the
    /// same name brings back what an earlier one left out.
    #[must_use]
    pub fn kept_ids(&self) -> Vec<&'static str> {
        if self.profile != FULL_PROFILE {
            return Vec::new();
        }
        OFFERED
            .iter()
            .filter(|o| !is_left_out(&self.left_out, o.id))
            .map(|o| o.id)
            .collect()
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
            Question::Profile => self.profile = v,
            Question::LeftOut => self.left_out = v,
            Question::Wifi => self.wifi_ssid = v,
            Question::WifiPassword => self.wifi_password = v,
            Question::BelongingsAddress => self.belongings = v,
            Question::BelongingsAccess => self.belongings_access = v,
            Question::BelongingsPassphrase => self.belongings_passphrase = v,
            Question::ResumeOffset => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // THE REAL LIST, not a fixture that shadows it. A hand-written pair here
    // would keep passing while the system spoke three.
    const LANGUAGES: &[&str] = &ronoctua_i18n::LANGUAGES;
    /// Enough to tell an accepted layout from a refused one.
    const LAYOUTS: &[&str] = &["br", "us", "gb"];

    /// FAILS IF: a `base.lst` description reaches the screen with its XML
    /// entities (`The "&lt; &gt;" key`) instead of the characters, or an
    /// ampersand is decoded twice.
    #[test]
    fn a_base_lst_description_is_read_as_characters() {
        assert_eq!(unescaped("The \"&lt; &gt;\" key"), "The \"< >\" key");
        assert_eq!(unescaped("A &amp;lt; B"), "A &lt; B");
        assert_eq!(unescaped("&quot;x&quot; &apos;y&apos;"), "\"x\" 'y'");
        assert_eq!(unescaped("plain"), "plain");
    }

    #[test]
    fn there_are_exactly_eighteen_questions() {
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
        //
        // THE BELONGINGS' REPOSITORY is three, and mandatory (owner,
        // 2026-10-05): where the person keeps their configuration, how this
        // machine reaches it and what opens it are theirs alone to know, and
        // they are what makes the new machine theirs before its first boot.
        //
        // THE PROGRAMS LEFT OUT (owner, 2026-10-20) are the eighteenth: which
        // heavy programs a `full` machine is for is the person's to say, like
        // the installation itself, and nothing on the hardware tells it. It
        // is only asked of a `full` installation, which alone carries them.
        assert_eq!(Question::ALL_KEYS.len(), 18);
        let mut views: Vec<_> = Question::ALL_KEYS.to_vec();
        views.sort();
        views.dedup();
        assert_eq!(views.len(), 18, "there is a repeated question in the list");
    }

    /// FAILS IF: the repository's questions come before the network they
    /// are proved against, or out of their order — the access before the
    /// address it is for, the passphrase before the access that reads the
    /// repository.
    #[test]
    fn the_repository_is_asked_after_the_network_in_its_own_order() {
        let at = |q: Question| Question::ALL_KEYS.iter().position(|x| *x == q).unwrap();
        assert!(at(Question::WifiPassword) < at(Question::BelongingsAddress));
        assert!(at(Question::BelongingsAddress) < at(Question::BelongingsAccess));
        assert!(at(Question::BelongingsAccess) < at(Question::BelongingsPassphrase));
        for q in [
            Question::BelongingsAddress,
            Question::BelongingsAccess,
            Question::BelongingsPassphrase,
        ] {
            assert!(!q.optional(), "{q:?} became optional");
            assert_eq!(
                validate_it(q, "", LANGUAGES, LAYOUTS),
                Err(Invalid::Blank),
                "{q:?}"
            );
        }
        assert!(Question::BelongingsPassphrase.secret_one());
        assert!(Question::BelongingsAccess.secret_one());
        assert_eq!(
            validate_it(Question::BelongingsAccess, "deploy", LANGUAGES, LAYOUTS),
            Err(Invalid::AccessNotAFile)
        );
        assert!(
            validate_it(
                Question::BelongingsAccess,
                "/run/keys/deploy",
                LANGUAGES,
                LAYOUTS
            )
            .is_ok()
        );
        assert_eq!(
            Question::BelongingsAccess.variable(),
            "RONOCTUA_INST_BELONGINGS_ACCESS"
        );
        assert_eq!(
            Question::BelongingsPassphrase.variable(),
            "RONOCTUA_INST_BELONGINGS_PASSPHRASE"
        );
    }

    /// FAILS IF: the answers the release's installer reads back lose one,
    /// gain a computed one, resume anywhere but where they were left, or
    /// lose the choice of BIOS.
    #[test]
    fn the_answers_cross_to_the_release_installer_whole() {
        let mut r = Answers::default();
        r.set(Question::Language, "pt-BR".into());
        r.set(Question::Disk, "/dev/nvme0n1".into());
        r.set(Question::Password, "Zq7 with = and spaces".into());
        r.set(Question::Wifi, "home-5g".into());
        r.set(Question::WifiPassword, "Zq7-radio".into());
        r.set(Question::Profile, FULL_PROFILE.into());
        // THE PROGRAMS LEFT OUT cross too, commas and all: a hop that lost
        // them would install what the person unmarked.
        r.set(Question::LeftOut, "blender,libreoffice".into());
        r.resume_offset = Some(1);
        let text = r.lines(Question::BelongingsAddress);
        assert!(
            !text.contains("RESUME_OFFSET"),
            "a computed answer travelled"
        );
        let (back, resume) = Answers::from_lines(&text);
        assert_eq!(resume, Some(Question::BelongingsAddress));
        for q in Question::ALL_KEYS {
            assert_eq!(back.get(q), r.get(q), "{q:?}");
        }
        assert_eq!(back.resume_offset, None);
        assert_eq!(back.left_out_ids(), ["blender", "libreoffice"]);
        // BIOS, chosen, crosses as the variable that chooses it.
        let choice = format!(
            "{}={}\n",
            crate::platform::FIRMWARE,
            crate::platform::CHOOSES_BIOS
        );
        assert!(!text.contains(&choice), "a UEFI machine chose BIOS");
        r.bios = true;
        let text = r.lines(Question::BelongingsAddress);
        assert!(text.contains(&choice));
        assert!(Answers::from_lines(&text).0.bios, "the choice is read back");
        // A line from a newer installer, and nonsense, are skipped.
        let (_, none) = Answers::from_lines("RONOCTUA_INST_FROM_THE_FUTURE=x\nnonsense\n");
        assert_eq!(none, None);
    }

    /// FAILS IF: an answer the TUI refuses has no sentence of its own, or
    /// two refusals say the same thing.
    #[test]
    fn every_refused_answer_has_its_own_sentence() {
        use ronoctua_belongings::prefs::Invalid as Why;
        let all = [
            Invalid::Blank,
            Invalid::HostnameMalformed,
            Invalid::HostnameWithoutPrefix,
            Invalid::PassphraseTooShort,
            Invalid::InvalidDisk,
            Invalid::UnknownLanguage,
            Invalid::UnknownLayout,
            Invalid::WifiPasswordTooShort,
            Invalid::UnknownProfile,
            Invalid::Belongings(Why::PlainHttp),
            Invalid::AccessNotAFile,
            Invalid::NotOffered,
        ];
        let mut keys: Vec<&str> = all.iter().map(Invalid::key).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), all.len());
        for language in ronoctua_i18n::LANGUAGES {
            let t = ronoctua_i18n::Texts::fresh(language).unwrap();
            for i in &all {
                let said = sentence(&t, i.key());
                assert!(!said.starts_with('['), "{language}: {i:?}");
                assert!(
                    !said.contains('{'),
                    "{language}: {i:?} lost an argument: {said}"
                );
            }
            // THE PREFIX IS THE CODE'S: the sentence says the constant, and
            // a `.ftl` that spelled it would not follow a rename.
            assert!(
                sentence(&t, Invalid::HostnameWithoutPrefix.key()).contains(HOSTNAME_PREFIX),
                "{language}"
            );
        }
        assert!(
            Invalid::HostnameWithoutPrefix
                .to_string()
                .contains(HOSTNAME_PREFIX)
        );
    }

    /// FAILS IF: an answer's variable does not start with the prefix a
    /// child is cleaned of — it would reach every child the installer
    /// starts.
    #[test]
    fn every_answer_is_named_under_the_prefix() {
        let names = answer_variables();
        for q in Question::ALL_KEYS {
            assert!(q.variable().starts_with(ANSWER_PREFIX), "{q:?}");
            assert!(names.contains(&q.variable()), "{q:?}");
        }
        assert!(RESUME.starts_with(ANSWER_PREFIX));
    }

    #[test]
    fn whoever_goes_by_wire_does_not_see_the_password_screen() {
        // Without this rule, whoever chooses "no Wi-Fi" lands on a field asking for the
        // password of nothing — with no way out other than typing
        // something, because empty would be an error.
        let mut r = Answers::default();
        assert!(Question::WifiPassword.skipped_one(&r));
        r.wifi_ssid = "some-network".into();
        assert!(!Question::WifiPassword.skipped_one(&r));
        // The others are never skipped for any answer — but the list of
        // programs, which is the installation's (see below).
        r.profile = FULL_PROFILE.into();
        for q in Question::ALL_KEYS {
            if q != Question::WifiPassword {
                assert!(!q.skipped_one(&r), "{q:?} was skipped");
            }
        }
    }

    /// FAILS IF: the list of programs is asked of an installation that does
    /// not carry them (a decline there declines nothing), is not asked of a
    /// `full` one, or comes before the installation it depends on.
    #[test]
    fn the_checklist_is_asked_only_of_the_full_installation() {
        let at = |q: Question| Question::ALL_KEYS.iter().position(|x| *x == q).unwrap();
        assert_eq!(at(Question::LeftOut), at(Question::Profile) + 1);
        let mut r = Answers::default();
        for profile in PROFILES {
            r.profile = profile.into();
            assert_eq!(
                Question::LeftOut.skipped_one(&r),
                profile != FULL_PROFILE,
                "{profile}"
            );
        }
        // Not even the default: a blank profile is not `full`.
        r.profile.clear();
        assert!(Question::LeftOut.skipped_one(&r));
        assert!(!OFFERED.is_empty(), "the ledger offers nothing");
        // AND WHAT A `minimal` ANSWER LEFT OUT IS NOTHING, whatever it says.
        r.profile = "minimal".into();
        r.left_out = "blender".into();
        assert!(r.left_out_ids().is_empty());
        r.profile = FULL_PROFILE.into();
        assert_eq!(r.left_out_ids(), ["blender"]);
        // THE REST IS KEPT, and the two halves are the whole list.
        let kept = r.kept_ids();
        assert!(!kept.contains(&"blender"));
        assert_eq!(kept.len() + 1, OFFERED.len());
        r.profile = "minimal".into();
        assert!(r.kept_ids().is_empty());
        assert_eq!(Question::LeftOut.variable(), "RONOCTUA_INST_LEFT_OUT");
        assert!(!Question::LeftOut.secret_one() && !Question::LeftOut.optional());
    }

    /// FAILS IF: an answer names a program the list does not offer, names
    /// one twice or `none` beside one and is taken — or `none` and the
    /// default are refused; `toggle` does not mark and unmark in the list's
    /// order; `none` could be a program's id; or the default is not the
    /// ledger's unmarked rows.
    #[test]
    fn the_left_out_answer_is_offered_ids_or_none() {
        let check = |v: &str| validate_it(Question::LeftOut, v, LANGUAGES, LAYOUTS);
        for bad in [
            "zsh",
            "blender,blender",
            "none,blender",
            "blender,none",
            "blender,",
            "Blender",
        ] {
            assert_eq!(check(bad), Err(Invalid::NotOffered), "{bad}");
        }
        assert_eq!(check(""), Err(Invalid::Blank));
        for good in [
            LEFT_OUT_NONE,
            DEFAULT_LEFT_OUT,
            "blender, vlc",
            "vlc,blender",
        ] {
            assert_eq!(check(good), Ok(()), "{good}");
        }
        for o in OFFERED {
            assert_eq!(check(o.id), Ok(()), "{}", o.id);
        }
        assert!(!OFFERED.iter().any(|o| o.id == LEFT_OUT_NONE));
        let unmarked: Vec<&str> = OFFERED
            .iter()
            .filter(|o| !o.checked)
            .map(|o| o.id)
            .collect();
        assert_eq!(
            DEFAULT_LEFT_OUT,
            if unmarked.is_empty() {
                LEFT_OUT_NONE.to_owned()
            } else {
                unmarked.join(",")
            }
        );
        assert_eq!(Question::LeftOut.default_answer(), Some(DEFAULT_LEFT_OUT));
        // TOGGLE, both ways, in the list's order — whatever order it came in.
        let first = OFFERED[0].id;
        let last = OFFERED[OFFERED.len() - 1].id;
        let both = toggle(&toggle(LEFT_OUT_NONE, last), first);
        assert_eq!(both, format!("{first},{last}"));
        assert!(is_left_out(&both, first) && is_left_out(&both, last));
        assert_eq!(toggle(&toggle(&both, first), last), LEFT_OUT_NONE);
        assert_eq!(
            toggle(&toggle(DEFAULT_LEFT_OUT, first), first),
            DEFAULT_LEFT_OUT
        );
        assert_eq!(toggle("vlc,blender", "zsh"), "blender,vlc");
        // AN OLDER LIST'S IDS are named, and dropped when written again.
        assert_eq!(
            not_offered("vlc,no-such-program, blender"),
            ["no-such-program"]
        );
        assert!(not_offered(LEFT_OUT_NONE).is_empty());
        assert_eq!(canonical("no-such-program,vlc"), "vlc");
        assert_eq!(canonical("no-such-program"), LEFT_OUT_NONE);
        assert!(!is_left_out(LEFT_OUT_NONE, LEFT_OUT_NONE));
        for o in OFFERED {
            assert_eq!(check(&toggle(LEFT_OUT_NONE, o.id)), Ok(()), "{}", o.id);
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
                    | Question::BelongingsAddress
                    | Question::BelongingsAccess
                    | Question::BelongingsPassphrase
            );
            assert_eq!(
                q.default_answer().is_some(),
                guessable,
                "{q:?} disagrees about having a default answer"
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
            validate_it(Question::Hostname, "machine", LANGUAGES, LAYOUTS),
            Err(Invalid::HostnameWithoutPrefix)
        );
        assert!(validate_it(Question::Hostname, "noctua-portatil", LANGUAGES, LAYOUTS).is_ok());
    }

    #[test]
    fn the_hostname_follows_rfc_1123() {
        for bad in [
            "noctua-",
            "noctua-MAIUSCULA",
            "noctua-with space",
            "noctua-x_y",
        ] {
            assert!(
                validate_it(Question::Hostname, bad, LANGUAGES, LAYOUTS).is_err(),
                "`{bad}` should have been refused"
            );
        }
    }

    #[test]
    fn the_passphrase_counts_characters_and_not_bytes() {
        // Twelve accented characters exceed 12 bytes. Refusing by bytes
        // would punish whoever writes the phrase in their own language.
        let twelve_accented = "ãçãoçãoçãoç";
        assert_eq!(twelve_accented.chars().count(), 11);
        assert!(validate_it(Question::Passphrase, twelve_accented, LANGUAGES, LAYOUTS).is_err());

        let thirteen = "ãçãoçãoçãoçã";
        assert_eq!(thirteen.chars().count(), 12);
        assert!(
            thirteen.len() > 12,
            "the test only counts if there is an accent"
        );
        assert!(validate_it(Question::Passphrase, thirteen, LANGUAGES, LAYOUTS).is_ok());
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

    /// FAILS IF: an address the belongings may use is refused, or one with a
    /// credential, `http://` or a local path is taken — or empty is taken.
    #[test]
    fn the_belongings_take_a_plain_remote_and_nothing_else() {
        use ronoctua_belongings::prefs::Invalid as Why;
        assert!(!Question::BelongingsAddress.optional());
        assert!(!Question::BelongingsAddress.secret_one());
        assert_eq!(
            validate_it(Question::BelongingsAddress, "", LANGUAGES, LAYOUTS),
            Err(Invalid::Blank)
        );
        for good in [
            "git@example.org:someone/belongings.git",
            "ssh://git@example.org/someone/belongings.git",
            "https://example.org/someone/belongings.git",
        ] {
            assert!(
                validate_it(Question::BelongingsAddress, good, LANGUAGES, LAYOUTS).is_ok(),
                "{good}"
            );
        }
        for (bad, why) in [
            (
                "https://someone:hunter2@example.org/b.git",
                Why::CredentialsInAddress,
            ),
            ("http://example.org/b.git", Why::PlainHttp),
            ("file:///srv/b.git", Why::NotRemote),
            ("/srv/b.git", Why::NotRemote),
            ("example.org", Why::Unknown),
        ] {
            assert_eq!(
                validate_it(Question::BelongingsAddress, bad, LANGUAGES, LAYOUTS),
                Err(Invalid::Belongings(why)),
                "{bad}"
            );
            assert!(!Invalid::Belongings(why).to_string().is_empty());
        }
        assert_eq!(
            Question::BelongingsAddress.variable(),
            "RONOCTUA_INST_BELONGINGS",
            "the terminal-less mode's variable is derived from the label"
        );
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
