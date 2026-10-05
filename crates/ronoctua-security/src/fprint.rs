//! Fingerprints: the reader, the enrolled fingers, and what enrolment says.
//!
//! Listing readers and fingers runs as the person over D-Bus
//! (`net.reactivated.Fprint`); the section exists only when a reader does.
//! Enrolling and deleting go through the sudo door ([`enroll`], [`delete`],
//! run by the root helper: adding a credential must require the existing
//! one). Fingerprints answer sudo only in terminals, and only for a user
//! listed in [`SUDO_LIST`] — the PAM stack's two gate lines before
//! `pam_fprintd` ([`sudo_list_with`] edits the list, [`sudo_allows`] reads
//! it).

use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::verb::Fingers;
use crate::{Refusal, Why, refused};

/// The file whose user names may answer sudo with a finger (root, `0644`).
pub const SUDO_LIST: &str = "/var/lib/ronoctua-security/fingerprint-sudo";
/// fprintd's bus name.
pub const SERVICE: &str = "net.reactivated.Fprint";
/// The manager's object.
pub const MANAGER_PATH: &str = "/net/reactivated/Fprint/Manager";
/// The manager's interface.
pub const MANAGER_INTERFACE: &str = "net.reactivated.Fprint.Manager";
/// A device's interface.
pub const DEVICE_INTERFACE: &str = "net.reactivated.Fprint.Device";
/// How long an enrolment waits for the next touch before it gives up.
pub const TOUCH_WAIT: Duration = Duration::from_secs(60);

/// A finger, by fprintd's name for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Finger {
    /// Left thumb.
    LeftThumb,
    /// Left index finger.
    LeftIndex,
    /// Left middle finger.
    LeftMiddle,
    /// Left ring finger.
    LeftRing,
    /// Left little finger.
    LeftLittle,
    /// Right thumb.
    RightThumb,
    /// Right index finger.
    RightIndex,
    /// Right middle finger.
    RightMiddle,
    /// Right ring finger.
    RightRing,
    /// Right little finger.
    RightLittle,
}

impl Finger {
    /// The ten, left to right.
    pub const ALL: [Self; 10] = [
        Self::LeftLittle,
        Self::LeftRing,
        Self::LeftMiddle,
        Self::LeftIndex,
        Self::LeftThumb,
        Self::RightThumb,
        Self::RightIndex,
        Self::RightMiddle,
        Self::RightRing,
        Self::RightLittle,
    ];

    /// fprintd's name — also the suffix of its key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::LeftThumb => "left-thumb",
            Self::LeftIndex => "left-index-finger",
            Self::LeftMiddle => "left-middle-finger",
            Self::LeftRing => "left-ring-finger",
            Self::LeftLittle => "left-little-finger",
            Self::RightThumb => "right-thumb",
            Self::RightIndex => "right-index-finger",
            Self::RightMiddle => "right-middle-finger",
            Self::RightRing => "right-ring-finger",
            Self::RightLittle => "right-little-finger",
        }
    }

    /// The finger of fprintd's name.
    #[must_use]
    pub fn of_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.id() == id)
    }
}

/// A fingerprint reader.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reader {
    /// Its name, as fprintd reports it — data.
    pub name: String,
    /// How many touches an enrolment takes.
    pub stages: u32,
}

/// What an enrolment step said (fprintd's `EnrollStatus`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Enrolling {
    /// Done.
    Completed,
    /// One more touch taken.
    StagePassed,
    /// Touch again.
    RetryScan,
    /// Too short a swipe.
    SwipeTooShort,
    /// Not centred.
    NotCentred,
    /// Lift the finger and touch again.
    RemoveAndRetry,
    /// Too fast: touch again, slower.
    TooFast,
    /// This print is already enrolled.
    Duplicate,
    /// The reader holds no more fingers.
    DataFull,
    /// It failed.
    Failed,
    /// The reader went away.
    Disconnected,
    /// fprintd said something else.
    Unknown,
}

impl Enrolling {
    /// Every one, for a key per variant.
    pub const ALL: [Self; 12] = [
        Self::Completed,
        Self::StagePassed,
        Self::RetryScan,
        Self::SwipeTooShort,
        Self::NotCentred,
        Self::RemoveAndRetry,
        Self::TooFast,
        Self::Duplicate,
        Self::DataFull,
        Self::Failed,
        Self::Disconnected,
        Self::Unknown,
    ];

    /// fprintd's word for it (what the helper passes on in a `step` line).
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Completed => "enroll-completed",
            Self::StagePassed => "enroll-stage-passed",
            Self::RetryScan => "enroll-retry-scan",
            Self::SwipeTooShort => "enroll-swipe-too-short",
            Self::NotCentred => "enroll-finger-not-centered",
            Self::RemoveAndRetry => "enroll-remove-and-retry",
            Self::TooFast => "enroll-too-fast",
            Self::Duplicate => "enroll-duplicate",
            Self::DataFull => "enroll-data-full",
            Self::Failed => "enroll-failed",
            Self::Disconnected => "enroll-disconnected",
            Self::Unknown => "enroll-unknown-error",
        }
    }

    /// The step fprintd's result string names.
    #[must_use]
    pub fn of(said: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|e| e.word() == said)
            .unwrap_or(Self::Unknown)
    }

    /// The refusal an enrolment that ended here is, or `None` when it ended
    /// well.
    #[must_use]
    pub const fn refusal(self) -> Option<Why> {
        match self {
            Self::Completed => None,
            Self::Duplicate => Some(Why::Invalid),
            Self::Disconnected => Some(Why::NoReader),
            _ => Some(Why::Failed),
        }
    }
}

/// The PAM list with `user` in it (`on`) or out of it. Other names stay as
/// they were, in their order; blank lines go. Pure.
#[must_use]
pub fn sudo_list_with(list: &str, user: &str, on: bool) -> String {
    let mut names: Vec<&str> = list
        .lines()
        .map(str::trim)
        .filter(|n| !n.is_empty() && *n != user)
        .collect();
    if on {
        names.push(user);
    }
    names.iter().map(|n| format!("{n}\n")).collect()
}

/// Whether fingerprints answer sudo for `user` in terminals. Read as the
/// person (`0644`); a missing list is "no".
#[must_use]
pub fn sudo_allows(user: &str) -> bool {
    std::fs::read_to_string(SUDO_LIST).is_ok_and(|text| text.lines().any(|n| n.trim() == user))
}

/// Puts `user` on the PAM list or takes them off (root, through the door).
///
/// # Errors
///
/// [`Why::Invalid`] for a name that is not plain; [`Why::Failed`] when the
/// list cannot be written.
pub fn set_sudo(list: &Path, user: &str, on: bool) -> Result<(), Refusal> {
    if !crate::users::plain(user) {
        return Err(refused(Why::Invalid));
    }
    let was = match std::fs::read_to_string(list) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(_) => return Err(refused(Why::Unreadable)),
    };
    crate::write_whole(list, sudo_list_with(&was, user, on).as_bytes(), 0o644).map_err(|e| {
        eprintln!("<3>{} could not be written: {e}", list.display());
        refused(Why::Failed)
    })
}

/// The refusal a D-Bus error from fprintd is.
fn why_of(error: &zbus::Error) -> Why {
    match error {
        zbus::Error::MethodError(name, _, _) => match name.as_str() {
            "net.reactivated.Fprint.Error.NoSuchDevice" => Why::NoReader,
            "net.reactivated.Fprint.Error.AlreadyInUse" => Why::Busy,
            "net.reactivated.Fprint.Error.PermissionDenied" => Why::NotAllowed,
            "net.reactivated.Fprint.Error.InvalidFingername" => Why::Invalid,
            "org.freedesktop.DBus.Error.ServiceUnknown" => Why::NoReader,
            _ => Why::Failed,
        },
        _ => Why::Unreadable,
    }
}

fn failed(error: &zbus::Error) -> Refusal {
    eprintln!("<3>fprintd: {error}");
    refused(why_of(error))
}

/// Whether an error is fprintd saying there is nothing enrolled.
fn nothing_enrolled(error: &zbus::Error) -> bool {
    matches!(error, zbus::Error::MethodError(name, _, _)
        if name.as_str() == "net.reactivated.Fprint.Error.NoEnrolledPrints")
}

fn proxy<'a>(
    connection: &zbus::blocking::Connection,
    path: &'a str,
    interface: &'a str,
) -> Result<zbus::blocking::Proxy<'a>, zbus::Error> {
    zbus::blocking::proxy::Builder::<zbus::blocking::Proxy<'a>>::new(connection)
        .destination(SERVICE)?
        .path(path)?
        .interface(interface)?
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
}

/// The readers fprintd knows (activating it).
fn devices(
    connection: &zbus::blocking::Connection,
) -> Result<Vec<zbus::zvariant::OwnedObjectPath>, zbus::Error> {
    proxy(connection, MANAGER_PATH, MANAGER_INTERFACE)?.call("GetDevices", &())
}

/// The reader, when there is one (fprintd's `GetDevices`, as the person). A
/// machine with no fprintd has none. Blocks on D-Bus: a thread.
///
/// # Errors
///
/// [`Why::Unreadable`] when the bus cannot be reached or fprintd fails.
pub fn reader() -> Result<Option<Reader>, Refusal> {
    let connection = zbus::blocking::Connection::system().map_err(|e| failed(&e))?;
    let devices = match devices(&connection) {
        Ok(devices) => devices,
        Err(e) if why_of(&e) == Why::NoReader => return Ok(None),
        Err(e) => return Err(failed(&e)),
    };
    let Some(path) = devices.first() else {
        return Ok(None);
    };
    let device = proxy(&connection, path.as_str(), DEVICE_INTERFACE).map_err(|e| failed(&e))?;
    let name: String = device.get_property("name").map_err(|e| failed(&e))?;
    let stages: i32 = device
        .get_property("num-enroll-stages")
        .map_err(|e| failed(&e))?;
    Ok(Some(Reader {
        name,
        stages: u32::try_from(stages).unwrap_or(0),
    }))
}

/// The fingers `user` enrolled, left to right. Blocks on D-Bus: a thread.
///
/// # Errors
///
/// [`Why::NoReader`], [`Why::Unreadable`], [`Why::Invalid`] for a name that
/// is not plain.
pub fn enrolled(user: &str) -> Result<Vec<Finger>, Refusal> {
    if !crate::users::plain(user) {
        return Err(refused(Why::Invalid));
    }
    let connection = zbus::blocking::Connection::system().map_err(|e| failed(&e))?;
    let devices = devices(&connection).map_err(|e| failed(&e))?;
    let path = devices.first().ok_or(refused(Why::NoReader))?;
    let device = proxy(&connection, path.as_str(), DEVICE_INTERFACE).map_err(|e| failed(&e))?;
    let names: Vec<String> = match device.call("ListEnrolledFingers", &(user,)) {
        Ok(names) => names,
        Err(e) if nothing_enrolled(&e) => Vec::new(),
        Err(e) => return Err(failed(&e)),
    };
    Ok(Finger::ALL
        .into_iter()
        .filter(|f| names.iter().any(|n| n == f.id()))
        .collect())
}

/// The default reader, claimed for `user`; released when dropped.
struct Claimed<'a> {
    device: zbus::blocking::Proxy<'a>,
}

impl Drop for Claimed<'_> {
    fn drop(&mut self) {
        let _: Result<(), _> = self.device.call("Release", &());
    }
}

fn claim<'a>(
    connection: &zbus::blocking::Connection,
    user: &str,
    path: &'a str,
) -> Result<Claimed<'a>, Refusal> {
    let device = proxy(connection, path, DEVICE_INTERFACE).map_err(|e| failed(&e))?;
    let (): () = device.call("Claim", &(user,)).map_err(|e| failed(&e))?;
    Ok(Claimed { device })
}

fn default_device(connection: &zbus::blocking::Connection) -> Result<String, Refusal> {
    let manager = proxy(connection, MANAGER_PATH, MANAGER_INTERFACE).map_err(|e| failed(&e))?;
    let path: zbus::zvariant::OwnedObjectPath = manager
        .call("GetDefaultDevice", &())
        .map_err(|e| failed(&e))?;
    Ok(path.as_str().to_owned())
}

/// Enrols `finger` for `user` on the default reader, as root: claim, start,
/// every `EnrollStatus` handed to `on_step` as it comes, stop, release. It
/// gives up after [`TOUCH_WAIT`] with no word from the reader.
///
/// # Errors
///
/// [`Why::NoReader`], [`Why::Busy`] (another program holds the reader),
/// [`Why::Invalid`] (a name that is not plain, or a print already
/// enrolled), [`Why::TimedOut`], [`Why::Failed`].
pub fn enroll(
    user: &str,
    finger: Finger,
    on_step: &mut dyn FnMut(Enrolling),
) -> Result<(), Refusal> {
    if !crate::users::plain(user) {
        return Err(refused(Why::Invalid));
    }
    let connection = zbus::blocking::Connection::system().map_err(|e| failed(&e))?;
    let path = default_device(&connection)?;
    let claimed = claim(&connection, user, &path)?;
    // Subscribed BEFORE the start, so the first touch is not missed.
    let statuses = claimed
        .device
        .receive_signal("EnrollStatus")
        .map_err(|e| failed(&e))?;
    let (sender, heard) = mpsc::channel::<(String, bool)>();
    std::thread::spawn(move || {
        for message in statuses {
            let Ok((result, done)) = message.body().deserialize::<(String, bool)>() else {
                continue;
            };
            if sender.send((result, done)).is_err() || done {
                break;
            }
        }
    });
    let (): () = claimed
        .device
        .call("EnrollStart", &(finger.id(),))
        .map_err(|e| failed(&e))?;
    let ended = loop {
        match heard.recv_timeout(TOUCH_WAIT) {
            Ok((result, done)) => {
                let step = Enrolling::of(&result);
                on_step(step);
                if done {
                    break step.refusal().map_or(Ok(()), |why| Err(refused(why)));
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => break Err(refused(Why::TimedOut)),
            Err(mpsc::RecvTimeoutError::Disconnected) => break Err(refused(Why::Failed)),
        }
    };
    let _: Result<(), _> = claimed.device.call("EnrollStop", &());
    drop(claimed);
    ended
}

/// Deletes `which` of `user`'s enrolled fingers, as root. Deleting what is
/// not there is not an error.
///
/// # Errors
///
/// [`Why::NoReader`], [`Why::Busy`], [`Why::Invalid`], [`Why::Failed`].
pub fn delete(user: &str, which: Fingers) -> Result<(), Refusal> {
    if !crate::users::plain(user) {
        return Err(refused(Why::Invalid));
    }
    let connection = zbus::blocking::Connection::system().map_err(|e| failed(&e))?;
    let path = default_device(&connection)?;
    let claimed = claim(&connection, user, &path)?;
    let deleted: Result<(), zbus::Error> = match which {
        Fingers::One(finger) => claimed.device.call("DeleteEnrolledFinger", &(finger.id(),)),
        Fingers::All => claimed.device.call("DeleteEnrolledFingers2", &()),
    };
    match deleted {
        Ok(()) => Ok(()),
        Err(e) if nothing_enrolled(&e) => Ok(()),
        Err(e) => Err(failed(&e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;

    #[test]
    fn fingers_are_fprintds_names() {
        for finger in Finger::ALL {
            assert_eq!(Finger::of_id(finger.id()), Some(finger));
        }
        for step in Enrolling::ALL {
            assert_eq!(Enrolling::of(step.word()), step);
            assert!(crate::verb::Said::parse(&format!("step {}", step.word())).is_some());
        }
        assert_eq!(Enrolling::of("enroll-stage-passed"), Enrolling::StagePassed);
        assert_eq!(Enrolling::of("what"), Enrolling::Unknown);
        assert_eq!(Enrolling::Completed.refusal(), None);
        assert_eq!(Enrolling::Duplicate.refusal(), Some(Why::Invalid));
        assert_eq!(Enrolling::DataFull.refusal(), Some(Why::Failed));
    }

    /// fprintd on the machine that runs the test, read only — by hand
    /// (`cargo test -- --ignored`): a build has no system bus.
    #[test]
    #[ignore = "needs this machine's fprintd"]
    fn the_machines_reader_is_read() {
        let reader = reader().unwrap();
        eprintln!("reader: {reader:?}");
        if reader.is_some() {
            let user = std::env::var("USER").unwrap();
            eprintln!("enrolled: {:?}", enrolled(&user).unwrap());
        }
    }

    #[test]
    fn the_sudo_list_takes_one_name_in_and_out() {
        assert_eq!(sudo_list_with("", "someone", true), "someone\n");
        assert_eq!(
            sudo_list_with("other\nsomeone\n", "someone", true),
            "other\nsomeone\n"
        );
        assert_eq!(
            sudo_list_with("someone\nother\n\n", "someone", false),
            "other\n"
        );
        assert_eq!(sudo_list_with("someone\n", "someone", false), "");
        let dir = Scratch::new("fprint");
        let list = dir.0.join("fingerprint-sudo");
        set_sudo(&list, "someone", true).unwrap();
        set_sudo(&list, "someone", true).unwrap();
        assert_eq!(std::fs::read_to_string(&list).unwrap(), "someone\n");
        set_sudo(&list, "someone", false).unwrap();
        assert_eq!(std::fs::read_to_string(&list).unwrap(), "");
        assert_eq!(
            set_sudo(&list, "a\nroot", true).unwrap_err().why,
            Why::Invalid
        );
    }
}
