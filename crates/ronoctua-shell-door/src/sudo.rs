//! A ROOT GESTURE, from the asking side: who asks (`Asker`), what for
//! (`Ask`, `Deed`), and the `Gate` a door makes its gestures through — the
//! footer's, a window's over the footer's socket, or none.
//!
//! # Why a module of its own
//!
//! The bar that asks the person (`ronoctua_shell_footer::sudo_bar`) is the footer's and the
//! sudo door's one caller (`sudo-door-is-one`). Every screen that needs root —
//! the security centre, the file manager, the stick, the belongings, the
//! update — only HANDS a gesture to it, and used to name the bar's module to
//! do so. The handing is this; the asking, and the door, stay with the bar,
//! which gives the footer's gate its gestures (`sudo_bar::gate`).

use crate::key;
use ronoctua_i18n::Arg;
use ronoctua_security::Why;
use ronoctua_security::verb::{Fingers, RootVerb, Said};
use serde::{Deserialize, Serialize};
use std::io::Write as _;
use std::os::unix::net::UnixStream;
use std::sync::Arc;
#[cfg(any(test, feature = "test-support"))]
use std::sync::{Mutex, PoisonError};

/// The word a window's deed starts with on the footer's control socket.
pub const ORDER: &str = "root-gesture";

/// The screen that asks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Asker {
    /// The security centre (`LEADER s e`).
    Security,
    /// The file manager.
    Files,
    /// The installer stick (`LEADER s z`).
    Stick,
    /// The belongings (`LEADER s v`).
    Belongings,
    /// The update's binary cache page (`LEADER s u c`): root's netrc for the
    /// person's private cache.
    Update,
    /// The build switch (`LEADER / b`, the modeline's glyph): a build of the
    /// distro checkout's working tree.
    Build,
}

impl Asker {
    /// Every one — for the test that each has its words in every language.
    #[cfg(any(test, feature = "test-support"))]
    pub const ALL: [Self; 6] = [
        Self::Security,
        Self::Files,
        Self::Stick,
        Self::Belongings,
        Self::Update,
        Self::Build,
    ];

    /// Its name as the bar says it: the screen, or the screen's window.
    pub const fn key(self, window: bool) -> &'static str {
        match (self, window) {
            (Self::Security, false) => key::SUDO_BAR_WHO_SECURITY,
            (Self::Security, true) => key::SUDO_BAR_WHO_SECURITY_WINDOW,
            (Self::Files, false) => key::SUDO_BAR_WHO_FILES,
            (Self::Files, true) => key::SUDO_BAR_WHO_FILES_WINDOW,
            (Self::Stick, false) => key::SUDO_BAR_WHO_STICK,
            (Self::Stick, true) => key::SUDO_BAR_WHO_STICK_WINDOW,
            (Self::Belongings, false) => key::SUDO_BAR_WHO_BELONGINGS,
            (Self::Belongings, true) => key::SUDO_BAR_WHO_BELONGINGS_WINDOW,
            (Self::Update, false) => key::SUDO_BAR_WHO_UPDATE,
            (Self::Update, true) => key::SUDO_BAR_WHO_UPDATE_WINDOW,
            (Self::Build, false) => key::SUDO_BAR_WHO_BUILD,
            (Self::Build, true) => key::SUDO_BAR_WHO_BUILD_WINDOW,
        }
    }
}

/// What a gesture runs: the helper's verb, and — for the `/private`
/// vault's unit, whose verb names no operation — the operation staged, so
/// the bar says which.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ask {
    /// The helper's verb.
    pub verb: RootVerb,
    /// The vault's operation, for [`RootVerb::PrivateRoot`].
    pub vault: Option<ronoctua_private::Op>,
}

impl From<RootVerb> for Ask {
    fn from(verb: RootVerb) -> Self {
        Self { verb, vault: None }
    }
}

/// A root gesture as the bar says it — and as a window writes it on the
/// footer's socket.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Deed {
    /// The screen that asks.
    pub asker: Asker,
    /// It asks from a window — never on the wire: the footer says every deed
    /// that came down its socket is a window's ([`serve`]).
    #[serde(skip)]
    pub window: bool,
    /// The helper's verb.
    pub verb: RootVerb,
    /// The vault's operation, for [`RootVerb::PrivateRoot`].
    #[serde(default)]
    pub vault: Option<ronoctua_private::Op>,
}

impl Deed {
    /// The deed of `ask`, asked by `asker` (from a window when `window`).
    #[must_use]
    pub fn of(asker: Asker, window: bool, ask: Ask) -> Self {
        Self {
            asker,
            window,
            verb: ask.verb,
            vault: ask.vault,
        }
    }

    /// Whether it is one the helper would take: its verb reads back as
    /// itself through the helper's own parser (`RootVerb::parse` checks the
    /// words a verb carries), and a vault's operation rides only on the
    /// vault's verb. Pure.
    #[must_use]
    pub fn valid(&self) -> bool {
        RootVerb::parse(&self.verb.arguments()).as_ref() == Some(&self.verb)
            && (self.vault.is_none() || self.verb == RootVerb::PrivateRoot)
    }

    /// "The security centre asks to open the passwordless window for 15
    /// minutes" — who asks, and the gesture, as one sentence.
    #[must_use]
    pub fn sentence(&self, texts: &ronoctua_i18n::Texts) -> String {
        texts.t_with(
            key::SUDO_BAR_ASKS,
            &[
                ("who", &texts.t(self.asker.key(self.window))),
                ("deed", &self.what(texts)),
            ],
        )
    }

    /// The gesture alone, as the words after "asks to".
    fn what(&self, texts: &ronoctua_i18n::Texts) -> String {
        let number = |n: usize| Arg::Number(i64::try_from(n).unwrap_or(i64::MAX));
        match &self.verb {
            RootVerb::Relax { span } => match span.minutes() {
                None => texts.t(key::SUDO_BAR_DEED_RELAX_NO_LIMIT),
                Some(minutes) if minutes >= 60 => texts.t_args(
                    key::SUDO_BAR_DEED_RELAX_HOURS,
                    &[("hours", Arg::Number(i64::from(minutes / 60)))],
                ),
                Some(minutes) => texts.t_args(
                    key::SUDO_BAR_DEED_RELAX_MINUTES,
                    &[("minutes", Arg::Number(i64::from(minutes)))],
                ),
            },
            RootVerb::RelaxEnd => texts.t(key::SUDO_BAR_DEED_RELAX_END),
            RootVerb::FirewallApply => texts.t(key::SUDO_BAR_DEED_FIREWALL),
            RootVerb::FingerEnroll { finger } => texts.t_with(
                key::SUDO_BAR_DEED_FINGER_ENROL,
                &[("finger", &texts.t(crate::security_words::finger(*finger)))],
            ),
            RootVerb::FingerDelete {
                which: Fingers::One(finger),
            } => texts.t_with(
                key::SUDO_BAR_DEED_FINGER_DELETE,
                &[("finger", &texts.t(crate::security_words::finger(*finger)))],
            ),
            RootVerb::FingerDelete {
                which: Fingers::All,
            } => texts.t(key::SUDO_BAR_DEED_FINGER_DELETE_ALL),
            RootVerb::FingerSudo { on: true } => texts.t(key::SUDO_BAR_DEED_FINGER_SUDO_ON),
            RootVerb::FingerSudo { on: false } => texts.t(key::SUDO_BAR_DEED_FINGER_SUDO_OFF),
            RootVerb::FilesRoot => texts.t(key::SUDO_BAR_DEED_FILES_ROOT),
            RootVerb::PrivateRoot => self.vault_what(texts),
            RootVerb::StickWrite => texts.t(key::SUDO_BAR_DEED_STICK),
            RootVerb::PersonModuleApprove { id, .. } => {
                texts.t_with(key::SUDO_BAR_DEED_APPROVE, &[("id", id)])
            }
            RootVerb::U2fKeys { credentials } if credentials.is_empty() => {
                texts.t(key::SUDO_BAR_DEED_KEYS_OFF)
            }
            RootVerb::U2fKeys { credentials } => texts.t_args(
                key::SUDO_BAR_DEED_KEYS,
                &[("count", number(credentials.len()))],
            ),
            RootVerb::PersonSignerPin { .. } => texts.t(key::SUDO_BAR_DEED_SIGNER_PIN),
            RootVerb::BinaryCacheNetrc { name } => {
                texts.t_with(key::SUDO_BAR_DEED_CACHE_NETRC, &[("name", name)])
            }
            RootVerb::BinaryCacheNetrcRemove => texts.t(key::SUDO_BAR_DEED_CACHE_NETRC_REMOVE),
            RootVerb::BuildSwitchReference => texts.t(key::SUDO_BAR_DEED_BUILD_REFERENCE),
        }
    }

    /// The vault's operation, said — any of them when none was given.
    fn vault_what(&self, texts: &ronoctua_i18n::Texts) -> String {
        use ronoctua_private::Op;
        let size = |bytes: u64| crate::sizes::readable_bytes(bytes, texts.decimal_mark());
        match self.vault {
            None => texts.t(key::SUDO_BAR_DEED_PRIVATE),
            Some(Op::Create { cap_bytes }) => texts.t_with(
                key::SUDO_BAR_DEED_PRIVATE_CREATE,
                &[("size", &size(cap_bytes))],
            ),
            Some(Op::Open) => texts.t(key::SUDO_BAR_DEED_PRIVATE_OPEN),
            Some(Op::Close) => texts.t(key::SUDO_BAR_DEED_PRIVATE_LOCK),
            Some(Op::EnrollFido2) => texts.t(key::SUDO_BAR_DEED_PRIVATE_ENROL),
            Some(Op::ForgetFido2) => texts.t(key::SUDO_BAR_DEED_PRIVATE_FORGET),
            Some(Op::Passphrase) => texts.t(key::SUDO_BAR_DEED_PRIVATE_PASSPHRASE),
            Some(Op::Grow { cap_bytes }) => texts.t_with(
                key::SUDO_BAR_DEED_PRIVATE_GROW,
                &[("size", &size(cap_bytes))],
            ),
        }
    }
}

/// A root gesture of a person's: the verbs it runs go to the sudo door
/// through the bar. Blocking — a door's thread, never a loop.
pub trait Root: Send + std::fmt::Debug {
    /// Runs `ask` as root, asking the person in the sudo bar when sudo wants
    /// them; every progress word of the helper's goes to `step` as it comes.
    ///
    /// # Errors
    ///
    /// The door's and the helper's refusals; [`Why::Cancelled`] when the
    /// person cancelled; [`Why::NoFooter`] from a window whose footer is not
    /// there.
    fn run(&mut self, ask: Ask, step: &mut dyn FnMut(&str)) -> Result<(), Why>;
}

/// One root gesture, boxed — what a view's door hands its thread.
pub type Gesture = Box<dyn Root>;

/// What makes a door's gestures: the footer's, a window's, or none.
#[derive(Clone)]
pub struct Gate(GateKind);

#[derive(Clone)]
enum GateKind {
    /// The footer: conversations on the view's thread, the bar on its loop —
    /// each gesture made by the bar's own module (`sudo_bar::gate`), the
    /// one caller of the sudo door.
    Footer(Arc<dyn Fn(Asker) -> Gesture + Send + Sync>),
    /// A window: the footer's control socket.
    Window,
    /// No bar to ask on (a photograph's footer, with no loop): every verb is
    /// refused, said.
    Shut,
    /// A test's: every verb recorded, and answered with this.
    #[cfg(any(test, feature = "test-support"))]
    Fake(Arc<Mutex<Vec<(Asker, RootVerb)>>>, Result<(), Why>),
}

impl std::fmt::Debug for Gate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self.0 {
            GateKind::Footer(_) => "Gate::Footer",
            GateKind::Window => "Gate::Window",
            GateKind::Shut => "Gate::Shut",
            #[cfg(any(test, feature = "test-support"))]
            GateKind::Fake(..) => "Gate::Fake",
        })
    }
}

impl Gate {
    /// The footer's: `make` makes each gesture, which the bar hears through
    /// the footer's loop (`sudo_bar::gate`).
    #[must_use]
    pub fn footer(make: impl Fn(Asker) -> Gesture + Send + Sync + 'static) -> Self {
        Self(GateKind::Footer(Arc::new(make)))
    }

    /// A window's: every deed goes to the footer's bar over its socket.
    #[must_use]
    pub const fn window() -> Self {
        Self(GateKind::Window)
    }

    /// None: every verb is refused ([`Why::Unstartable`]).
    #[must_use]
    pub const fn shut() -> Self {
        Self(GateKind::Shut)
    }

    /// A test's gate, recording every verb into `log` and answering `with`.
    #[cfg(any(test, feature = "test-support"))]
    #[must_use]
    pub fn fake(log: Arc<Mutex<Vec<(Asker, RootVerb)>>>, with: Result<(), Why>) -> Self {
        Self(GateKind::Fake(log, with))
    }

    /// A gesture of `asker`'s.
    #[must_use]
    pub fn gesture(&self, asker: Asker) -> Gesture {
        match &self.0 {
            GateKind::Footer(make) => make(asker),
            GateKind::Window => Box::new(FromWindow { asker, link: None }),
            GateKind::Shut => Box::new(Shut),
            #[cfg(any(test, feature = "test-support"))]
            GateKind::Fake(log, with) => Box::new(Fake {
                asker,
                log: Arc::clone(log),
                with: *with,
            }),
        }
    }
}

/// A window's gesture: one connection to the footer, made at the first verb
/// and closed with the gesture.
#[derive(Debug)]
struct FromWindow {
    asker: Asker,
    link: Option<std::io::BufReader<UnixStream>>,
}

impl Root for FromWindow {
    fn run(&mut self, ask: Ask, step: &mut dyn FnMut(&str)) -> Result<(), Why> {
        let deed = Deed::of(self.asker, true, ask);
        if !deed.valid() {
            return Err(Why::Invalid);
        }
        if self.link.is_none() {
            let path = crate::ipc::socket_path().map_err(|_| Why::NoFooter)?;
            let stream = UnixStream::connect(&path).map_err(|e| {
                tracing::warn!(error = %e, "the footer's socket did not answer a root gesture");
                Why::NoFooter
            })?;
            self.link = Some(std::io::BufReader::new(stream));
        }
        let Some(link) = self.link.as_mut() else {
            return Err(Why::NoFooter);
        };
        let line = order_line(&deed).ok_or(Why::Invalid)?;
        // ONE LINE, ONE WRITE: a reader that waits a moment never gets half.
        if link.get_mut().write_all(line.as_bytes()).is_err() {
            self.link = None;
            return Err(Why::NoFooter);
        }
        let verdict = hear_the_verdict(link, step);
        if verdict == Err(Why::NoFooter) {
            self.link = None;
        }
        verdict
    }
}

/// A window's deed as the line it writes: [`ORDER`], a space, the deed in
/// JSON, a line break. `None` for a deed that does not serialise.
pub fn order_line(deed: &Deed) -> Option<String> {
    let json = serde_json::to_string(deed).ok()?;
    Some(format!("{ORDER} {json}\n"))
}

/// Reads the footer's lines for one verb: its progress words to `step`, and
/// its verdict. A footer that went away before the verdict is
/// [`Why::NoFooter`]; a line in no grammar of the helper's is passed over.
pub(crate) fn hear_the_verdict(
    link: &mut impl std::io::BufRead,
    step: &mut dyn FnMut(&str),
) -> Result<(), Why> {
    let mut line = String::new();
    loop {
        line.clear();
        match link.read_line(&mut line) {
            Ok(0) | Err(_) => return Err(Why::NoFooter),
            Ok(_) => {}
        }
        match Said::parse(&line) {
            Some(Said::Step(word)) => step(&word),
            Some(Said::Done) => return Ok(()),
            Some(Said::Refused(why)) => return Err(why),
            None => {}
        }
    }
}

/// The gate with nowhere to ask.
#[derive(Debug)]
struct Shut;

impl Root for Shut {
    fn run(&mut self, _: Ask, _: &mut dyn FnMut(&str)) -> Result<(), Why> {
        Err(Why::Unstartable)
    }
}

/// A test's gesture.
#[cfg(any(test, feature = "test-support"))]
#[derive(Debug)]
struct Fake {
    asker: Asker,
    log: Arc<Mutex<Vec<(Asker, RootVerb)>>>,
    with: Result<(), Why>,
}

#[cfg(any(test, feature = "test-support"))]
impl Root for Fake {
    fn run(&mut self, ask: Ask, _: &mut dyn FnMut(&str)) -> Result<(), Why> {
        self.log
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((self.asker, ask.verb));
        self.with
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: the footer's lines are not read in the helper's grammar —
    /// a progress word lost, a refusal read as another, a footer that went
    /// away before its verdict taken for anything but `NoFooter`, or a line
    /// of no grammar taken for a verdict.
    #[test]
    fn a_window_hears_the_verdict_in_the_helpers_words() {
        let mut steps = Vec::new();
        let mut said = std::io::Cursor::new(
            "noise\nstep enroll-stage-passed\nrefused wrong-password\n".as_bytes(),
        );
        assert_eq!(
            hear_the_verdict(&mut said, &mut |w| steps.push(w.to_owned())),
            Err(Why::WrongPassword)
        );
        assert_eq!(steps, ["enroll-stage-passed"]);
        let mut done = std::io::Cursor::new("done\n".as_bytes());
        assert_eq!(hear_the_verdict(&mut done, &mut |_| {}), Ok(()));
        let mut gone = std::io::Cursor::new("step x\n".as_bytes());
        assert_eq!(hear_the_verdict(&mut gone, &mut |_| {}), Err(Why::NoFooter));
    }
}
