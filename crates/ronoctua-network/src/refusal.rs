//! Why NetworkManager — or this crate — refused, by NAME.
//!
//! # Why a name and never the words
//!
//! Because the words are English. NetworkManager's error messages, `nmcli`'s
//! stderr and this crate's own `.context` (invariant 18) all are, and a pt-BR
//! machine put them on the message line beside a translated sentence — half a
//! sentence in a language the person did not choose (invariant 8). Every way a
//! refusal reaches the line has a name here, and the door picks the sentence
//! by it; the words go to the log, where whoever reads a log reads English.

/// Why something could not be done.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// polkit or the bus said no.
    NotAuthorized,
    /// NetworkManager is not on the bus, or the thing asked about is gone.
    NotAvailable,
    /// A setting NetworkManager would not take.
    Invalid,
    /// No password was given where one is needed, or it was refused.
    NoSecrets,
    /// NetworkManager tried and could not — an activation that failed.
    Failed,
    /// No answer in time.
    TimedOut,
    /// The system bus could not be reached.
    NoBus,
    /// This machine has no Wi-Fi radio.
    NoRadio,
    /// The network is no longer in range.
    OutOfRange,
    /// No device of that kind.
    NoDevice,
    /// No saved profile by that name.
    NoProfile,
    /// `nmcli` would not import the file.
    ImportRefused,
    /// The TPM would not seal or unseal the password.
    Vault,
    /// Anything else. Its words go to the log.
    Other,
}

/// A refusal this crate names itself — the `.context` on an `Option` that
/// had no error under it. Carried inside the `anyhow::Error`, so
/// [`refusal`] finds it in the chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Named(pub Refusal);

impl std::fmt::Display for Named {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self.0 {
            Refusal::NoRadio => "this machine has no Wi-Fi radio",
            Refusal::OutOfRange => "that network is no longer in range",
            Refusal::NoDevice => "no device of that kind",
            Refusal::NoProfile => "there is no saved profile by that name",
            Refusal::ImportRefused => "nmcli would not import the file",
            Refusal::Vault => "the TPM refused",
            _ => "refused",
        })
    }
}

impl std::error::Error for Named {}

/// The refusal a D-Bus error NAME stands for.
fn named(name: &str) -> Refusal {
    match name {
        "org.freedesktop.NetworkManager.PermissionDenied"
        | "org.freedesktop.NetworkManager.Settings.PermissionDenied"
        | "org.freedesktop.NetworkManager.AgentManager.PermissionDenied"
        | "org.freedesktop.DBus.Error.AccessDenied"
        | "org.freedesktop.DBus.Error.AuthFailed"
        | "org.freedesktop.DBus.Error.InteractiveAuthorizationRequired" => Refusal::NotAuthorized,
        "org.freedesktop.NetworkManager.UnknownConnection"
        | "org.freedesktop.NetworkManager.UnknownDevice"
        | "org.freedesktop.NetworkManager.ConnectionNotAvailable"
        | "org.freedesktop.NetworkManager.ConnectionNotActive"
        | "org.freedesktop.NetworkManager.Device.NotActive"
        | "org.freedesktop.NetworkManager.Device.NotSoftware"
        | "org.freedesktop.DBus.Error.ServiceUnknown"
        | "org.freedesktop.DBus.Error.NameHasNoOwner"
        | "org.freedesktop.DBus.Error.UnknownObject"
        | "org.freedesktop.DBus.Error.UnknownMethod" => Refusal::NotAvailable,
        "org.freedesktop.NetworkManager.Settings.InvalidConnection"
        | "org.freedesktop.NetworkManager.Settings.InvalidArguments"
        | "org.freedesktop.NetworkManager.Settings.InvalidSetting"
        | "org.freedesktop.NetworkManager.InvalidArguments"
        | "org.freedesktop.NetworkManager.Device.InvalidConnection"
        | "org.freedesktop.NetworkManager.Device.InvalidArgument"
        | "org.freedesktop.DBus.Error.InvalidArgs" => Refusal::Invalid,
        "org.freedesktop.NetworkManager.AgentManager.NoSecrets"
        | "org.freedesktop.NetworkManager.SecretAgent.NoSecrets"
        | "org.freedesktop.NetworkManager.SecretAgent.UserCanceled"
        | "org.freedesktop.NetworkManager.SecretAgent.AgentCanceled" => Refusal::NoSecrets,
        "org.freedesktop.DBus.Error.NoReply"
        | "org.freedesktop.DBus.Error.Timeout"
        | "org.freedesktop.DBus.Error.TimedOut" => Refusal::TimedOut,
        "org.freedesktop.DBus.Error.NoServer"
        | "org.freedesktop.DBus.Error.Disconnected"
        | "org.freedesktop.DBus.Error.NoNetwork" => Refusal::NoBus,
        // EVERY OTHER NetworkManager NAME ENDING IN `Failed` is one activation
        // or one creation that did not happen: one sentence covers them.
        n if n.starts_with("org.freedesktop.NetworkManager.") && n.ends_with("Failed") => {
            Refusal::Failed
        }
        "org.freedesktop.DBus.Error.Failed" => Refusal::Failed,
        _ => Refusal::Other,
    }
}

/// Reads the refusal out of an error, whatever it is wrapped in.
#[must_use]
pub fn refusal(e: &anyhow::Error) -> Refusal {
    for cause in e.chain() {
        if let Some(Named(r)) = cause.downcast_ref::<Named>() {
            return *r;
        }
        if let Some(zbus::Error::MethodError(name, _, _)) = cause.downcast_ref::<zbus::Error>() {
            return named(name.as_str());
        }
        if let Some(
            zbus::Error::InputOutput(_) | zbus::Error::Address(_) | zbus::Error::Handshake(_),
        ) = cause.downcast_ref::<zbus::Error>()
        {
            return Refusal::NoBus;
        }
        if let Some(f) = cause.downcast_ref::<zbus::fdo::Error>() {
            use zbus::fdo::Error as F;
            return match f {
                F::ZBus(zbus::Error::MethodError(name, _, _)) => named(name.as_str()),
                F::AccessDenied(_) | F::AuthFailed(_) | F::InteractiveAuthorizationRequired(_) => {
                    Refusal::NotAuthorized
                }
                F::ServiceUnknown(_)
                | F::NameHasNoOwner(_)
                | F::UnknownObject(_)
                | F::UnknownMethod(_) => Refusal::NotAvailable,
                F::InvalidArgs(_) => Refusal::Invalid,
                F::NoReply(_) | F::Timeout(_) | F::TimedOut(_) => Refusal::TimedOut,
                F::NoServer(_) | F::Disconnected(_) | F::NoNetwork(_) => Refusal::NoBus,
                F::Failed(_) => Refusal::Failed,
                _ => Refusal::Other,
            };
        }
    }
    Refusal::Other
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wrapped(name: &str) -> anyhow::Error {
        let e = zbus::Error::MethodError(
            zbus::names::OwnedErrorName::try_from(name.to_owned()).expect("a name"),
            Some("the daemon's English".to_owned()),
            zbus::message::Message::method_call("/", "Ping")
                .expect("a builder")
                .build(&())
                .expect("a message"),
        );
        anyhow::Error::from(e).context("asking NetworkManager")
    }

    /// FAILS IF: a refusal that has a name is read as the nameless one, or
    /// the crate's own `.context` is lost under a wrapper.
    #[test]
    fn every_refusal_is_read_by_its_name() {
        for (name, want) in [
            (
                "org.freedesktop.NetworkManager.PermissionDenied",
                Refusal::NotAuthorized,
            ),
            (
                "org.freedesktop.NetworkManager.UnknownConnection",
                Refusal::NotAvailable,
            ),
            (
                "org.freedesktop.NetworkManager.Settings.InvalidConnection",
                Refusal::Invalid,
            ),
            (
                "org.freedesktop.NetworkManager.AgentManager.NoSecrets",
                Refusal::NoSecrets,
            ),
            (
                "org.freedesktop.NetworkManager.Device.ActivationFailed",
                Refusal::Failed,
            ),
            ("org.freedesktop.DBus.Error.NoReply", Refusal::TimedOut),
            ("org.example.Unheard", Refusal::Other),
        ] {
            assert_eq!(refusal(&wrapped(name)), want, "{name}");
        }
        let ours = anyhow::Error::from(Named(Refusal::OutOfRange)).context("connecting");
        assert_eq!(refusal(&ours), Refusal::OutOfRange);
        assert_eq!(refusal(&anyhow::anyhow!("odd")), Refusal::Other);
    }
}
