//! `software/declined.json` — the distro's programs this person declined.
//!
//! ```json
//! {"format": 1, "apps": [{"id": "gimp", "machines": ["noctua"],
//!   "edited-at": "2026-10-05T12:00:00Z", "edited-on": "noctua"}]}
//! ```
//!
//! The id is the distro registry's (`ronoctua.apps.<id>`): a declined
//! program leaves `systemPackages` on the machines named (`null` = every
//! one). An essential program cannot be declined; the Nix side reports the
//! refusal in `person-applied.json` and the screen says why.

use serde::{Deserialize, Serialize};

use crate::{Document, Refusal, Stamp, Why, check};

/// The file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Declined {
    /// Always [`crate::FORMAT`].
    pub format: u32,
    /// The declined programs.
    pub apps: Vec<Declining>,
}

impl Default for Declined {
    fn default() -> Self {
        Self {
            format: crate::FORMAT,
            apps: Vec::new(),
        }
    }
}

/// One declined program.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Declining {
    /// The distro registry's id.
    pub id: String,
    /// `None` for every machine, or the hostnames.
    pub machines: Option<Vec<String>>,
    /// When it was last edited.
    pub edited_at: Stamp,
    /// Which machine edited it.
    pub edited_on: String,
}

impl Declining {
    /// Whether it is declined on `host`.
    #[must_use]
    pub fn applies_to(&self, host: &str) -> bool {
        self.machines
            .as_deref()
            .is_none_or(|hosts| hosts.iter().any(|h| h == host))
    }
}

/// `machines`: `None`, or a non-empty list of distinct hostnames.
pub(crate) fn machines_are_valid(machines: Option<&[String]>, at: &str) -> Result<(), Refusal> {
    let Some(hosts) = machines else {
        return Ok(());
    };
    if hosts.is_empty() {
        return Err(Refusal::new(
            Why::Invalid,
            format!("{at}/machines"),
            "an empty list (null is every machine)",
        ));
    }
    let mut seen = std::collections::BTreeSet::new();
    for (i, host) in hosts.iter().enumerate() {
        if !check::hostname(host) {
            return Err(Refusal::new(
                Why::Invalid,
                format!("{at}/machines/{i}"),
                host.as_str(),
            ));
        }
        if !seen.insert(host) {
            return Err(Refusal::new(
                Why::Duplicate,
                format!("{at}/machines/{i}"),
                host.as_str(),
            ));
        }
    }
    Ok(())
}

impl Document for Declined {
    fn format(&self) -> u32 {
        self.format
    }

    fn validate(&self) -> Result<(), Refusal> {
        let mut seen = std::collections::BTreeSet::new();
        for (i, d) in self.apps.iter().enumerate() {
            let at = format!("/apps/{i}");
            if !check::slug(&d.id) {
                return Err(Refusal::new(
                    Why::Invalid,
                    format!("{at}/id"),
                    d.id.as_str(),
                ));
            }
            machines_are_valid(d.machines.as_deref(), &at)?;
            if !check::hostname(&d.edited_on) {
                return Err(Refusal::new(
                    Why::Invalid,
                    format!("{at}/edited-on"),
                    d.edited_on.as_str(),
                ));
            }
            if !seen.insert(d.id.as_str()) {
                return Err(Refusal::new(
                    Why::Duplicate,
                    format!("{at}/id"),
                    d.id.as_str(),
                ));
            }
        }
        Ok(())
    }
}
