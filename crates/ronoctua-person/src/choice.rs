//! The installer's choice of the distro's programs for ONE machine — what
//! the person ticked and unticked on its list, handed to `ronoctua-belongings
//! plain --decline <file>`.
//!
//! ```json
//! {"format": 1, "machine": "new-host", "at": "2026-10-20T10:00:00Z",
//!  "declined": ["libreoffice"], "kept": ["blender", "vlc"]}
//! ```
//!
//! # A choice, not a list of declines
//!
//! The installer asks again at every attempt, and a reinstall under the same
//! hostname asks again: a program unticked once and ticked now must come
//! back. So the file says both halves of the answer, for one machine, and
//! [`Choice::apply`] makes the person's `declined.json` say it through the
//! one rule of `crate::declined`: each `declined` id is declined on this
//! machine alone, each `kept` id is brought back on this machine alone. A
//! decline of every machine (`null`) is the person's own "everywhere", made
//! on another day: the installer's tick does not undo it, and
//! [`Applied::everywhere`] says which stayed.
//!
//! By construction it names one machine and never `null`: what the
//! installer cannot decide for the person's other computers, it cannot write.

use serde::{Deserialize, Serialize};

use crate::declined::Declined;
use crate::{Document, Refusal, Stamp, Why, check};

/// The file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Choice {
    /// Always [`crate::FORMAT`].
    pub format: u32,
    /// The machine being installed: its hostname.
    pub machine: String,
    /// When the person chose — the stamp each decline it writes carries.
    pub at: Stamp,
    /// The registry ids left out of this machine.
    pub declined: Vec<String>,
    /// The registry ids kept on this machine: a decline of this machine
    /// they had goes.
    pub kept: Vec<String>,
}

/// What a choice did to a `declined.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Applied {
    /// Whether the document changed.
    pub changed: bool,
    /// The kept ids the person declined on every machine: they stay
    /// declined, and the installer says so.
    pub everywhere: Vec<String>,
}

impl Choice {
    /// The choice of `machine` at `at`: `declined` left out, `kept` kept.
    #[must_use]
    pub fn new(machine: &str, at: Stamp, declined: Vec<String>, kept: Vec<String>) -> Self {
        Self {
            format: crate::FORMAT,
            machine: machine.to_owned(),
            at,
            declined,
            kept,
        }
    }

    /// The choice said in `doc`: each `declined` id declined on this
    /// machine ([`Declined::decline`]), each `kept` id brought back on it
    /// ([`Declined::undecline`]) — but a decline of every machine, which
    /// stays and is named in [`Applied::everywhere`].
    pub fn apply(&self, doc: &mut Declined) -> Applied {
        let mut applied = Applied::default();
        for id in &self.declined {
            applied.changed |= doc.decline(
                id,
                Some(vec![self.machine.clone()]),
                &self.at,
                &self.machine,
            );
        }
        for id in &self.kept {
            if doc.apps.iter().any(|d| d.id == *id && d.machines.is_none()) {
                applied.everywhere.push(id.clone());
                continue;
            }
            applied.changed |= doc.undecline(id, &self.machine, &self.at);
        }
        applied
    }
}

impl Document for Choice {
    fn format(&self) -> u32 {
        self.format
    }

    fn validate(&self) -> Result<(), Refusal> {
        if !check::hostname(&self.machine) {
            return Err(Refusal::new(
                Why::Invalid,
                "/machine",
                self.machine.as_str(),
            ));
        }
        let mut seen = std::collections::BTreeSet::new();
        for (list, ids) in [("declined", &self.declined), ("kept", &self.kept)] {
            for (i, id) in ids.iter().enumerate() {
                let at = format!("/{list}/{i}");
                if !check::slug(id) {
                    return Err(Refusal::new(Why::Invalid, at, id.as_str()));
                }
                // ONE ANSWER PER PROGRAM: twice in a list, or in both.
                if !seen.insert(id.as_str()) {
                    return Err(Refusal::new(Why::Duplicate, at, id.as_str()));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: i64) -> Stamp {
        Stamp::from_unix_s(1_791_201_600 + s)
    }

    fn ids(names: &[&str]) -> Vec<String> {
        names.iter().map(|&n| n.to_owned()).collect()
    }

    fn machines(doc: &Declined, id: &str) -> Option<Option<Vec<String>>> {
        doc.apps
            .iter()
            .find(|d| d.id == id)
            .map(|d| d.machines.clone())
    }

    /// FAILS IF: a choice reads with no machine or a machine that is not a
    /// hostname, an id that is not one, an id twice — in one list or in
    /// both — or writes back other than it read.
    #[test]
    fn a_choice_names_one_machine_and_each_program_once() {
        let doc = |machine: &str, declined: &str, kept: &str| {
            format!(
                r#"{{"format": 1, "machine": {machine}, "at": "2026-10-20T10:00:00Z", "declined": {declined}, "kept": {kept}}}"#
            )
        };
        let good = doc(
            r#""new-host""#,
            r#"["libreoffice"]"#,
            r#"["blender", "vlc"]"#,
        );
        let choice: Choice = crate::parse(good.as_bytes()).unwrap();
        assert_eq!(
            choice,
            Choice::new(
                "new-host",
                Stamp::parse("2026-10-20T10:00:00Z").unwrap(),
                ids(&["libreoffice"]),
                ids(&["blender", "vlc"]),
            )
        );
        assert_eq!(
            crate::parse::<Choice>(crate::to_json(&choice).as_bytes()).unwrap(),
            choice
        );
        let host = r#""new-host""#;
        for (machine, declined, kept, why, at) in [
            (r#""Not A Host""#, "[]", "[]", Why::Invalid, "/machine"),
            (host, r#"["Bad Id"]"#, "[]", Why::Invalid, "/declined/0"),
            (
                host,
                r#"["vlc", "vlc"]"#,
                "[]",
                Why::Duplicate,
                "/declined/1",
            ),
            (
                host,
                r#"["vlc"]"#,
                r#"["blender", "vlc"]"#,
                Why::Duplicate,
                "/kept/1",
            ),
        ] {
            let refused =
                crate::parse::<Choice>(doc(machine, declined, kept).as_bytes()).unwrap_err();
            assert_eq!(
                (refused.why, refused.at.as_str()),
                (why, at),
                "{declined} {kept}"
            );
        }
        for not in [
            r#"{"format": 1, "machine": "new-host", "at": "2026-10-20T10:00:00Z", "declined": [], "kept": [], "machines": null}"#,
            r#"{"format": 1, "at": "2026-10-20T10:00:00Z", "declined": [], "kept": []}"#,
        ] {
            assert_eq!(
                crate::parse::<Choice>(not.as_bytes()).unwrap_err().why,
                Why::Shape,
                "{not}"
            );
        }
    }

    /// FAILS IF: a choice declines anywhere but its machine, keeps a
    /// program out that is ticked now (a failed attempt's, an old install's
    /// under the same hostname), takes away another machine's decline,
    /// undoes the person's decline of every machine without saying so, or
    /// says it changed what it did not.
    #[test]
    fn a_choice_declines_and_brings_back_on_its_machine_alone() {
        let mut doc = Declined::default();
        doc.decline(
            "blender",
            Some(ids(&["new-host", "laptop"])),
            &at(0),
            "laptop",
        );
        doc.decline("vlc", None, &at(0), "laptop");
        doc.decline("gimp", Some(ids(&["laptop"])), &at(0), "laptop");
        let choice = Choice::new(
            "new-host",
            at(10),
            ids(&["libreoffice", "gimp"]),
            ids(&["blender", "vlc", "onlyoffice"]),
        );
        let applied = choice.apply(&mut doc);
        assert!(applied.changed);
        assert_eq!(applied.everywhere, ids(&["vlc"]));
        assert_eq!(
            machines(&doc, "libreoffice"),
            Some(Some(ids(&["new-host"])))
        );
        assert_eq!(
            machines(&doc, "gimp"),
            Some(Some(ids(&["laptop", "new-host"])))
        );
        assert_eq!(machines(&doc, "blender"), Some(Some(ids(&["laptop"]))));
        assert_eq!(machines(&doc, "vlc"), Some(None), "the person's everywhere");
        assert_eq!(machines(&doc, "onlyoffice"), None);
        let again = choice.apply(&mut doc);
        assert!(!again.changed, "the same choice twice changed something");
        assert!(doc.validate().is_ok());
        // TICKED NOW what an earlier attempt left out: it comes back.
        let now = Choice::new("new-host", at(20), Vec::new(), ids(&["libreoffice"]));
        assert!(now.apply(&mut doc).changed);
        assert_eq!(machines(&doc, "libreoffice"), None);
    }
}
