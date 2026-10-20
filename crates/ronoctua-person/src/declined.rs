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
//!
//! # One rule, here
//!
//! Every writer changes the list through [`Declined::decline`],
//! [`Declined::undecline`], [`Declined::bring_back_everywhere`] and
//! [`Declined::join`] — the software centre, the installer's choice signed
//! into the repository (`crate::choice`, `ronoctua-belongings plain
//! --decline`) — and every merge of two copies through [`Declining::merged`]
//! (`crate::merge::merge_declined`), so a decline means the same wherever it
//! is made. A decline only ever widens: declined here after elsewhere is
//! declined on both, and declined everywhere stays so. Bringing a program
//! back is per machine: from a list, this machine leaves it and the others
//! keep it out (the installer declines for "this machine only"); an entry
//! for every machine goes whole, since there is no list of the others to
//! keep. Two copies meet machine by machine: what either side added or
//! took away holds. And a stamp never goes back: an edit made earlier
//! elsewhere joins in without moving "last changed".

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

impl Declined {
    /// Decline `id` on `machines` (`None`: everywhere), edited at `at` on
    /// the machine `on`. A decline already there takes the new machines too,
    /// and one of every machine stays so; a host named twice counts once,
    /// and an empty list declines nothing. Whether anything changed (the
    /// stamps move only with a change).
    pub fn decline(
        &mut self,
        id: &str,
        machines: Option<Vec<String>>,
        at: &Stamp,
        on: &str,
    ) -> bool {
        let machines = match machines {
            None => None,
            Some(hosts) => {
                let mut once: Vec<String> = Vec::with_capacity(hosts.len());
                for host in hosts {
                    if !once.contains(&host) {
                        once.push(host);
                    }
                }
                if once.is_empty() {
                    return false;
                }
                Some(once)
            }
        };
        match self.apps.iter_mut().find(|d| d.id == id) {
            Some(d) => {
                let joined = match (&d.machines, machines) {
                    (None, _) | (_, None) => None,
                    (Some(were), Some(more)) => {
                        let mut all = were.clone();
                        for host in more {
                            if !all.contains(&host) {
                                all.push(host);
                            }
                        }
                        Some(all)
                    }
                };
                if joined == d.machines {
                    return false;
                }
                d.machines = joined;
                d.restamp(at, on);
                true
            }
            None => {
                self.apps.push(Declining {
                    id: id.to_owned(),
                    machines,
                    edited_at: at.clone(),
                    edited_on: on.to_owned(),
                });
                true
            }
        }
    }

    /// Bring `id` back on `machine`, edited at `at`: from a list of
    /// machines, the host leaves it and the entry goes when the list is
    /// empty; an entry for every machine (`null`) goes whole (there is no
    /// list of "the others" to keep). A decline that does not name `machine`
    /// is left as it is. Whether anything changed.
    pub fn undecline(&mut self, id: &str, machine: &str, at: &Stamp) -> bool {
        let Some(i) = self.apps.iter().position(|d| d.id == id) else {
            return false;
        };
        let d = &mut self.apps[i];
        let Some(hosts) = d.machines.as_mut() else {
            self.apps.remove(i);
            return true;
        };
        let before = hosts.len();
        hosts.retain(|h| h != machine);
        if hosts.len() == before {
            return false;
        }
        if hosts.is_empty() {
            self.apps.remove(i);
        } else {
            d.restamp(at, machine);
        }
        true
    }

    /// Bring `id` back on every machine: the decline goes whole, whatever
    /// machines it names — for one of machines that no longer exist, which
    /// nobody could bring back otherwise. Whether there was one.
    pub fn bring_back_everywhere(&mut self, id: &str) -> bool {
        let before = self.apps.len();
        self.apps.retain(|d| d.id != id);
        self.apps.len() != before
    }

    /// Every decline of `more` joined in, each with its own stamps
    /// ([`Self::decline`]): idempotent, it never narrows `null`, and an
    /// older stamp of `more`'s never moves one of ours back. Whether
    /// anything changed.
    pub fn join(&mut self, more: &Self) -> bool {
        let mut changed = false;
        for d in &more.apps {
            changed |= self.decline(&d.id, d.machines.clone(), &d.edited_at, &d.edited_on);
        }
        changed
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
    /// Takes the edit stamped `at` on the machine `on`, unless the one it
    /// carries is later: "last changed" says the latest edit and the machine
    /// that made it, whichever arrived last.
    fn restamp(&mut self, at: &Stamp, on: &str) {
        if *at >= self.edited_at {
            self.edited_at = at.clone();
            on.clone_into(&mut self.edited_on);
        }
    }

    /// One decline as two copies of it meet, from what both came from: the
    /// machines either side added join the base, the machines either side
    /// took away leave it — so two machines bringing a program back at
    /// once both hold, and two declining it at once both hold. A decline of
    /// every machine (`null`) that a side made absorbs the rest; one the
    /// base had keeps only what both sides kept of it. Nothing left: `None`.
    /// The stamps are the latest edit's among the sides that changed it.
    #[must_use]
    pub fn merged(base: Option<&Self>, ours: Option<&Self>, theirs: Option<&Self>) -> Option<Self> {
        // WHERE EACH COPY DECLINES IT: every machine (`None`), or these — an
        // absent decline is the empty list.
        let hosts = |d: Option<&Self>| d.map_or_else(|| Some(Vec::new()), |d| d.machines.clone());
        let (b, o, t) = (hosts(base), hosts(ours), hosts(theirs));
        let machines = match (b, o, t) {
            (None, None, None) => None,
            (None, None, Some(kept)) | (None, Some(kept), None) => Some(kept),
            (None, Some(o), Some(t)) => Some(o.into_iter().filter(|h| t.contains(h)).collect()),
            (Some(_), None, _) | (Some(_), _, None) => None,
            (Some(b), Some(o), Some(t)) => {
                let mut all = b.clone();
                for host in o.iter().chain(&t) {
                    if !all.contains(host) {
                        all.push(host.clone());
                    }
                }
                all.retain(|h| !b.contains(h) || (o.contains(h) && t.contains(h)));
                Some(all)
            }
        };
        if machines.as_ref().is_some_and(Vec::is_empty) {
            return None;
        }
        // THE LATEST EDIT among the sides that changed it, ours on a tie;
        // with none changed, what they agree on.
        let latest = [ours, theirs]
            .into_iter()
            .flatten()
            .filter(|d| Some(*d) != base)
            .fold(None::<&Self>, |best, d| match best {
                Some(b) if b.edited_at >= d.edited_at => Some(b),
                _ => Some(d),
            })
            .or(ours)
            .or(theirs)
            .or(base)?;
        Some(Self {
            id: latest.id.clone(),
            machines,
            edited_at: latest.edited_at.clone(),
            edited_on: latest.edited_on.clone(),
        })
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: i64) -> Stamp {
        Stamp::from_unix_s(1_791_201_600 + s)
    }

    fn hosts(names: &[&str]) -> Option<Vec<String>> {
        Some(names.iter().map(|&h| h.to_owned()).collect())
    }

    fn one(doc: &Declined, id: &str) -> Option<Declining> {
        doc.apps.iter().find(|d| d.id == id).cloned()
    }

    /// FAILS IF: a decline here after one elsewhere does not join the two, a
    /// decline everywhere does not stay everywhere (nor absorb a list), an
    /// unchanged decline counts as a change or moves the stamps, a host named
    /// twice is kept twice, or an empty list declines anything.
    #[test]
    fn a_decline_only_widens() {
        let mut doc = Declined::default();
        assert!(doc.decline("gimp", hosts(&["laptop"]), &at(0), "laptop"));
        assert!(doc.decline("gimp", hosts(&["noctua", "noctua"]), &at(1), "noctua"));
        let gimp = one(&doc, "gimp").unwrap();
        assert_eq!(gimp.machines, hosts(&["laptop", "noctua"]));
        assert_eq!((gimp.edited_at, gimp.edited_on.as_str()), (at(1), "noctua"));
        assert!(!doc.decline("gimp", hosts(&["noctua"]), &at(2), "noctua"));
        assert_eq!(
            one(&doc, "gimp").unwrap().edited_at,
            at(1),
            "no change moved the stamp"
        );
        assert!(doc.decline("gimp", None, &at(3), "noctua"));
        assert_eq!(one(&doc, "gimp").unwrap().machines, None);
        assert!(!doc.decline("gimp", hosts(&["elsewhere"]), &at(4), "elsewhere"));
        assert_eq!(
            one(&doc, "gimp").unwrap().machines,
            None,
            "a list narrowed null"
        );
        assert!(!doc.decline("vlc", Some(Vec::new()), &at(5), "noctua"));
        assert!(
            one(&doc, "vlc").is_none(),
            "an empty list declined something"
        );
        assert!(doc.validate().is_ok());
    }

    /// FAILS IF: bringing a program back on one machine brings it back on
    /// the others of its list, leaves an entry with no machine, keeps a
    /// decline of every machine, touches a decline that does not name the
    /// machine, or narrows without saying who and when.
    #[test]
    fn bringing_back_is_per_machine() {
        let mut doc = Declined::default();
        doc.decline("gimp", hosts(&["laptop", "noctua"]), &at(0), "laptop");
        doc.decline("vlc", None, &at(0), "laptop");
        doc.decline("htop", hosts(&["laptop"]), &at(0), "laptop");
        assert!(doc.undecline("gimp", "noctua", &at(9)));
        let gimp = one(&doc, "gimp").unwrap();
        assert_eq!(
            gimp.machines,
            hosts(&["laptop"]),
            "the others came back too"
        );
        assert_eq!((gimp.edited_at, gimp.edited_on.as_str()), (at(9), "noctua"));
        assert!(
            !doc.undecline("gimp", "noctua", &at(10)),
            "brought back twice"
        );
        assert!(
            !doc.undecline("htop", "noctua", &at(10)),
            "another machine's decline"
        );
        assert_eq!(one(&doc, "htop").unwrap().machines, hosts(&["laptop"]));
        assert!(doc.undecline("gimp", "laptop", &at(11)));
        assert!(
            one(&doc, "gimp").is_none(),
            "an entry with no machine is left"
        );
        assert!(doc.undecline("vlc", "noctua", &at(12)));
        assert!(
            one(&doc, "vlc").is_none(),
            "a decline of every machine stayed"
        );
        assert!(!doc.undecline("nothing", "noctua", &at(12)));
        assert!(doc.validate().is_ok());
    }

    /// FAILS IF: joining another list loses one of its declines or one of
    /// ours, narrows a decline of every machine, changes anything a second
    /// time, or takes a stamp of its own instead of the decline's.
    #[test]
    fn joining_is_idempotent_and_never_narrows() {
        let mut ours = Declined::default();
        ours.decline("gimp", hosts(&["laptop"]), &at(0), "laptop");
        ours.decline("vlc", None, &at(0), "laptop");
        let mut more = Declined::default();
        more.decline("gimp", hosts(&["new-host"]), &at(5), "new-host");
        more.decline("vlc", hosts(&["new-host"]), &at(5), "new-host");
        more.decline("libreoffice", hosts(&["new-host"]), &at(5), "new-host");
        assert!(ours.join(&more));
        assert_eq!(
            one(&ours, "gimp").unwrap().machines,
            hosts(&["laptop", "new-host"])
        );
        assert_eq!(one(&ours, "vlc").unwrap().machines, None);
        assert_eq!(one(&ours, "vlc").unwrap().edited_at, at(0));
        let libre = one(&ours, "libreoffice").unwrap();
        assert_eq!(libre.machines, hosts(&["new-host"]));
        assert_eq!(
            (libre.edited_at, libre.edited_on.as_str()),
            (at(5), "new-host")
        );
        let once = ours.clone();
        assert!(!ours.join(&more), "a second join changed something");
        assert!(!ours.join(&Declined::default()));
        assert_eq!(ours, once);
        assert!(ours.validate().is_ok());
    }

    /// FAILS IF: joining an edit stamped earlier — a choice made before an
    /// edit here — moves "last changed" back, or names the machine of the
    /// older edit; or a later one does not move it forward.
    #[test]
    fn a_stamp_never_goes_back() {
        let mut ours = Declined::default();
        ours.decline("gimp", hosts(&["alpha"]), &at(100), "alpha");
        let mut older = Declined::default();
        older.decline("gimp", hosts(&["new-host"]), &at(-1000), "new-host");
        assert!(ours.join(&older));
        let gimp = one(&ours, "gimp").unwrap();
        assert_eq!(gimp.machines, hosts(&["alpha", "new-host"]));
        assert_eq!(
            (gimp.edited_at, gimp.edited_on.as_str()),
            (at(100), "alpha")
        );
        assert!(ours.undecline("gimp", "new-host", &at(50)));
        assert_eq!(one(&ours, "gimp").unwrap().edited_at, at(100));
        let mut later = Declined::default();
        later.decline("gimp", hosts(&["beta"]), &at(200), "beta");
        assert!(ours.join(&later));
        let gimp = one(&ours, "gimp").unwrap();
        assert_eq!((gimp.edited_at, gimp.edited_on.as_str()), (at(200), "beta"));
    }

    /// FAILS IF: bringing a program back on every machine leaves any of its
    /// decline — one of machines that no longer exist included — or says
    /// it changed something that was not there.
    #[test]
    fn bringing_back_everywhere_takes_the_whole_decline() {
        let mut doc = Declined::default();
        doc.decline(
            "gimp",
            hosts(&["gone-laptop", "old-desk"]),
            &at(0),
            "gone-laptop",
        );
        doc.decline("vlc", None, &at(0), "noctua");
        assert!(doc.bring_back_everywhere("gimp"));
        assert!(one(&doc, "gimp").is_none());
        assert!(!doc.bring_back_everywhere("gimp"));
        assert!(doc.bring_back_everywhere("vlc"));
        assert!(doc.apps.is_empty());
    }
}
