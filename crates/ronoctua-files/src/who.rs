//! Numbers into names: `1000` into `noctua`, `100` into `users`.
//!
//! # Why it parses the files instead of calling the C library
//!
//! `getpwuid_r` is the correct answer in general and the wrong one here for two
//! reasons. It needs `unsafe`, which this crate forbids; and it can BLOCK — NSS
//! will happily go to LDAP or to `systemd-userdbd` over a socket, and a
//! directory of two hundred files would make two hundred of those calls inside
//! the frame that draws it.
//!
//! # What that costs, said out loud
//!
//! A user that exists only through an NSS module is not in these files, and the
//! listing shows the number. That is the honest failure: a number is visibly a
//! number, and nobody mistakes it for a name. On this system users come from
//! the flake and land in `/etc/passwd`, so the case is theoretical here — but
//! it is the reason the fallback shows the id rather than inventing a label.

use std::collections::BTreeMap;

/// Where the two files live on a running system.
///
/// A parameter and not a constant, so the whole grammar can be tested against a
/// fake `/etc` — the arrangement `ronoctua_memory::Places` already uses.
#[derive(Debug, Clone)]
pub struct Files {
    /// `/etc/passwd`.
    pub passwd: std::path::PathBuf,
    /// `/etc/group`.
    pub group: std::path::PathBuf,
}

impl Default for Files {
    fn default() -> Self {
        Self {
            passwd: std::path::PathBuf::from("/etc/passwd"),
            group: std::path::PathBuf::from("/etc/group"),
        }
    }
}

/// Everyone the machine can name.
#[derive(Debug, Clone, Default)]
pub struct Roster {
    people: BTreeMap<u32, String>,
    groups: BTreeMap<u32, String>,
    /// group name → the members listed in the fourth field of `/etc/group`.
    ///
    /// It is NOT the whole truth about membership — a user's PRIMARY group is
    /// in `passwd` and never repeated here — which is why [`Roster::mine`] asks
    /// the kernel instead of reading this.
    members: BTreeMap<String, Vec<String>>,
}

impl Roster {
    /// Read both files. Missing or unreadable is not an error.
    ///
    /// An empty roster means every column shows a number, which is a degraded
    /// listing and not a broken one. Refusing to open the manager because
    /// `/etc/group` is unreadable would be the tool failing at exactly the
    /// moment it is most useful.
    #[must_use]
    pub fn read_from(files: &Files) -> Self {
        let passwd = std::fs::read_to_string(&files.passwd).unwrap_or_default();
        let group = std::fs::read_to_string(&files.group).unwrap_or_default();
        Self::parse(&passwd, &group)
    }

    /// The pure half: two file bodies in, a roster out.
    #[must_use]
    pub fn parse(passwd: &str, group: &str) -> Self {
        let mut people = BTreeMap::new();
        for line in passwd.lines() {
            // `name:x:uid:gid:gecos:home:shell`. A short line is skipped
            // rather than guessed at: `/etc/passwd` is written by other
            // programs and a malformed line is theirs to explain.
            let mut f = line.split(':');
            let (Some(name), Some(_), Some(uid)) = (f.next(), f.next(), f.next()) else {
                continue;
            };
            if let Ok(uid) = uid.parse::<u32>() {
                people.insert(uid, name.to_owned());
            }
        }
        let mut groups = BTreeMap::new();
        let mut members = BTreeMap::new();
        for line in group.lines() {
            // `name:x:gid:member,member`.
            let mut f = line.split(':');
            let (Some(name), Some(_), Some(gid)) = (f.next(), f.next(), f.next()) else {
                continue;
            };
            if let Ok(gid) = gid.parse::<u32>() {
                groups.insert(gid, name.to_owned());
            }
            let listed: Vec<String> = f
                .next()
                .unwrap_or_default()
                .split(',')
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned)
                .collect();
            members.insert(name.to_owned(), listed);
        }
        Self {
            people,
            groups,
            members,
        }
    }

    /// The owner's name, when there is one.
    #[must_use]
    pub fn person(&self, uid: u32) -> Option<&str> {
        self.people.get(&uid).map(String::as_str)
    }

    /// The group's name, when there is one.
    #[must_use]
    pub fn group(&self, gid: u32) -> Option<&str> {
        self.groups.get(&gid).map(String::as_str)
    }

    /// Everyone, by id — what the owner picker offers.
    pub fn people(&self) -> impl Iterator<Item = (u32, &str)> {
        self.people.iter().map(|(id, n)| (*id, n.as_str()))
    }

    /// Every group, by id.
    pub fn groups(&self) -> impl Iterator<Item = (u32, &str)> {
        self.groups.iter().map(|(id, n)| (*id, n.as_str()))
    }

    /// Whoever is listed in a group's fourth field.
    #[must_use]
    pub fn listed_in(&self, group: &str) -> &[String] {
        self.members.get(group).map_or(&[], Vec::as_slice)
    }

    /// The groups THIS process may hand a file to, without asking anyone.
    ///
    /// Asked of the kernel and not of `/etc/group`, because the kernel is the
    /// only one that knows: a user's primary group is in `passwd` and never
    /// appears in the member list, and a session can have supplementary groups
    /// the files do not explain. Offering a group `chgrp` will refuse is worse
    /// than not offering it — the picker would look like it worked and the
    /// file would not change.
    #[must_use]
    pub fn mine(&self) -> Vec<(u32, String)> {
        let mut ids: Vec<u32> = rustix::process::getgroups()
            .map(|g| g.into_iter().map(rustix::process::Gid::as_raw).collect())
            .unwrap_or_default();
        ids.push(rustix::process::getgid().as_raw());
        ids.sort_unstable();
        ids.dedup();
        ids.into_iter()
            .map(|id| {
                let name = self
                    .group(id)
                    .map_or_else(|| id.to_string(), ToOwned::to_owned);
                (id, name)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PASSWD: &str = "\
root:x:0:0:System administrator:/root:/bin/sh
noctua:x:1000:100:Pessoa:/home/noctua:/run/current-system/sw/bin/zsh
linha-quebrada
nobody:x:65534:65534:Nobody:/var/empty:/bin/sh
";

    const GROUP: &str = "\
root:x:0:
users:x:100:noctua
wheel:x:1:noctua,outra
vazio:x:42:
";

    #[test]
    fn a_number_becomes_a_name() {
        let r = Roster::parse(PASSWD, GROUP);
        assert_eq!(r.person(1000), Some("noctua"));
        assert_eq!(r.person(0), Some("root"));
        assert_eq!(r.group(100), Some("users"));
        assert_eq!(r.group(1), Some("wheel"));
    }

    #[test]
    fn a_malformed_line_is_skipped_and_the_rest_survives() {
        // THE DEFECT THIS CATCHES: one broken line taking the whole roster
        // down, so every column in the listing shows a number and nothing says
        // why. These files are written by other programs.
        let r = Roster::parse(PASSWD, GROUP);
        assert_eq!(
            r.person(65534),
            Some("nobody"),
            "a linha depois da quebrada sumiu"
        );
    }

    #[test]
    fn an_unknown_id_has_no_name_and_does_not_invent_one() {
        let r = Roster::parse(PASSWD, GROUP);
        assert_eq!(r.person(4242), None);
        assert_eq!(r.group(4242), None);
    }

    #[test]
    fn the_member_list_is_read_and_an_empty_one_is_empty() {
        let r = Roster::parse(PASSWD, GROUP);
        assert_eq!(r.listed_in("wheel"), ["noctua", "outra"]);
        assert!(r.listed_in("vazio").is_empty());
        assert!(r.listed_in("nao-existe").is_empty());
    }

    #[test]
    fn an_empty_etc_gives_an_empty_roster_and_not_a_panic() {
        let r = Roster::parse("", "");
        assert_eq!(r.person(0), None);
        assert_eq!(r.people().count(), 0);
    }
}
