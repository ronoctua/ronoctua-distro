//! What the installer DOES with the answers — section 7.1.
//!
//! No Nix logic is reimplemented here (invariant 5). The installer writes the
//! machine's own flake from the distro's template, a `host.nix` with what
//! describes the MACHINE, and calls `nix`, `disko`, `nixos-install` and the
//! person's two programs (`ronoctua-person`, `ronoctua-belongings`); what
//! knows how to lock, partition, mount, build and restore is theirs.
//!
//! This module's role is that of a form: turning answers into a
//! file and a sequence of commands, both inspectable before running.
//!
//! # The sequence, and why in this order
//!
//! NOTHING HERE ASKS THE NETWORK. Every `nix` runs `--offline` with no
//! substituter and no registry ([`OFFLINE_NIX_CONFIG`]): the ISO carries what
//! the machine builds (`nix/installer/offline.nix`), so a network that is up
//! changes nothing and one that is down stops nothing.
//!
//! 1. The disk's passphrase for `disko`, and the machine's flake made in a
//!    working directory: the TEMPLATE (`templates/machine`), aimed at the
//!    ISO's own tree, `host.nix`, the hardware report, the lock, and the name
//!    of the system it builds — asked of the flake itself, never spelled
//!    here.
//! 2. The programs left out, as root's staged declines applied to the
//!    working flake (below).
//! 3. The system BUILT, before the disk is touched: what the machine needs
//!    and the ISO does not carry fails with the disk intact.
//! 4. `disko`, the flake to the target, the password, the Wi-Fi, and
//!    `nixos-install`, which copies what step 3 built; then the parts the ISO
//!    carries, seeded into the machine's store (`ronoctua-parts seed`).
//! 5. The person's home: the access to the repository (the deploy key or the
//!    token), the consent for its host and `belongings.json` — and what the
//!    first boot finishes (`crate::first_boot`): the record, and the
//!    passphrase sealed in the TPM. Copying the plain part out, pinning the
//!    person's key, applying their declarations and restoring their home ask
//!    the repository's server, so the machine does them once it is up.
//!
//! # The programs left out
//!
//! A `full` installation's unmarked programs ([`Answers::left_out_ids`]) are
//! the person's DECLINES, for this hostname alone (`software/declined.json`,
//! [`left_out_declined`]). A machine with no key pinned takes the
//! declarations as staged: the declines are staged root's (in a folder of
//! their own, which nothing else fills) and applied before the disk, so
//! `nixos-install` leaves the programs out. A step right after the apply
//! READS the working flake's `declined.json` and stops before the disk unless
//! it says the choice ([`LEFT_OUT_APPLIED`]).
//!
//! The repository hears the choice on the first boot: a repository with
//! something in it has the copy of its plain part sign it in (`plain
//! --decline`, a `ronoctua_person::choice::Choice`: each program left out
//! declined on this machine, each one ticked brought back on it), and an
//! empty one has the declines in the person's mirror before `init`, whose
//! first sync signs them with the new key.
//!
//! A run starts from nothing an earlier one left ([`leftovers`]): a retry
//! would otherwise apply that attempt's declines.

use std::fmt::Write as _;

use crate::key;

use crate::model::Answers;

/// Where the generated `host.nix` is written, on the target system.
///
/// Gitignored in the repository: it contains the hostname and the machine's disk, which
/// are installation data. Password and passphrase do **not** enter it — they go
/// to `sops`//persist, per section 9.
pub const HOST_NIX_PATH: &str = concat!("/mnt", env!("RONOCTUA_MACHINE_DIR"), "/host.nix");

/// The flake's directory on the target system. It is what `nixos-install` receives.
///
/// The target's root is mounted at `/mnt`, and under it the machine directory
/// by its one name (`nix/machine.nix`, baked by `nix/rust.nix` as
/// `RONOCTUA_MACHINE_DIR`; `machine-dir-has-one-name`): the place the
/// installed system's rescue, update and units read is the place this writes.
pub const TARGET_DIRECTORY: &str = concat!("/mnt", env!("RONOCTUA_MACHINE_DIR"));

/// Where the flake is prepared BEFORE the disk exists.
///
/// # Why there is a working directory
///
/// `disko` needs the `host.nix` — it is where `ronoctua.disk` comes from, the
/// disk to erase. But the `host.nix` lives inside the machine's flake, and at
/// that moment the two places that flake could be are impossible:
///
///   - the target's machine directory does not exist yet: it is disko that is
///     going to mount it.
///   - the ISO's own is a store link, READ-ONLY (the same trap as
///     `/etc/fonts/conf.d` in AGENTS.md).
///
/// So the machine's flake is made here from the distro's template, the
/// `host.nix` is written here, and it is from here that `disko` evaluates.
/// After mounting, all of this goes to the target.
pub const WORK_DIRECTORY: &str = "/tmp/ronoctua-flake";

/// The working directory's `host.nix` — the one `disko` reads.
pub const HOST_NIX_WORK_PATH: &str = "/tmp/ronoctua-flake/host.nix";

/// Where the installation takes its distro from, and how the machine's
/// flake points at it while it installs.
///
/// # The distro's tree
///
/// The ISO carries the tree it was built from (`nix/installer/iso.nix`, the
/// variable [`crate::release::FLAKE_DIR`]); the release's installer, after a
/// hop, is handed the release's tree (`--distro`). The TEMPLATE comes from
/// it — never a template this binary carries, which would be a second copy.
///
/// # Aimed, and kept
///
/// The machine's flake follows the release (`templates/machine/flake.nix`).
/// When what is installed is NOT the release — a development build, one
/// ahead of the release, or a release that did not answer
/// ([`crate::release::Stay::aims_at_own`]) — the working flake is AIMED at
/// this tree before it is locked, so it installs what it carries and never
/// fetches what it cannot. And the target KEEPS it aimed: its `flake.nix`
/// and its `flake.lock` are the pair the system was built from, copied
/// together after the lock and never written again.
///
/// Putting the template's `flake.nix` back after `nixos-install` would leave
/// it disagreeing with a lock made for the aimed one, and every evaluation
/// that may not move the lock (`ronoctua-person verify`, the machine's
/// `apply`, both `--no-update-lock-file`) would refuse the machine until an
/// update re-locked it. Locking the template against the release instead
/// would make the folder describe a system that does not run — the very
/// thing the machine's `apply` puts back on a failure. So an aimed machine
/// stays on the tree it was installed from; the tree is kept alive by the
/// system itself (`host_nix`, [`Answers::aimed`]), so no collection can
/// leave the flake naming a store path that is gone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Origin {
    /// The distro's tree.
    pub distro: String,
    /// Whether the machine's flake is aimed at it while it installs.
    pub aimed: bool,
    /// The `nix` the person's verification runs, by path
    /// (`ronoctua-person verify` refuses a bare name).
    pub nix: String,
}

impl Origin {
    /// The template's folder, under the distro's tree.
    #[must_use]
    pub fn template(&self) -> String {
        format!("{}/{}", self.distro, crate::release::TEMPLATE)
    }

    /// The template's `flake.nix`.
    #[must_use]
    pub fn template_flake(&self) -> String {
        format!("{}/{}", self.template(), crate::release::TEMPLATE_FLAKE)
    }
}

/// The installer's scratch folder, in RAM: the scratch home the repository
/// is asked from (its settings, consent and the access made for this
/// machine), the plain part copied out, root's lock for the person's
/// applier, the restore's runtime folder and the answers a hop hands on.
///
/// `/run`, a tmpfs: the deploy key and the passphrase's file never touch a
/// disk, and the whole folder goes with the installer's last step.
pub const SCRATCH: &str = "/run/ronoctua-install";

/// The scratch folder's layout — ONE layout, which the plan reads at
/// [`SCRATCH`] and a test reads in a folder of its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scratch {
    /// Its root.
    pub root: String,
}

impl Scratch {
    /// The installer's own, at [`SCRATCH`].
    #[must_use]
    pub fn installer() -> Self {
        Self {
            root: SCRATCH.to_owned(),
        }
    }

    /// The scratch home (`HOME` of every belongings' child run as root).
    #[must_use]
    pub fn home(&self) -> String {
        format!("{}/home", self.root)
    }

    /// The scratch home's configuration root (`~/.config/ronoctua`).
    #[must_use]
    pub fn config(&self) -> String {
        format!("{}/.config/{}", self.home(), ronoctua_xdg::NAME)
    }

    /// The scratch home's access folder — what `ronoctua-belongings` reads
    /// the deploy key or the token from.
    #[must_use]
    pub fn keys(&self) -> String {
        format!("{}/{}", self.config(), ronoctua_belongings::places::KEYS)
    }

    /// The runtime folder of the belongings' children run as root (the
    /// passphrase a verification reads, a probe's scratch repository).
    #[must_use]
    pub fn runtime(&self) -> String {
        format!("{}/run", self.root)
    }

    /// Root's lock for the person's applier and verifier while they work
    /// on the working flake.
    #[must_use]
    pub fn lock(&self) -> String {
        format!("{}/person.lock", self.root)
    }

    /// The staged tree of an EMPTY repository's declines, in the plain
    /// part's shape — a folder of their own, so root's applier never reads
    /// a copy of the plain part an earlier attempt left.
    #[must_use]
    pub fn staged(&self) -> String {
        format!("{}/staged-declines", self.root)
    }
}

/// What every `nix` of the installation runs with (`NIX_CONFIG`): no
/// substituter and no flake registry. The ISO carries what the machine
/// builds (`nix/installer/offline.nix`), and a network that happens to be up
/// is never asked — an installation gives the same result with one and
/// without.
pub const OFFLINE_NIX_CONFIG: &str = "substituters =\nflake-registry =\n";

/// [`OFFLINE_NIX_CONFIG`], as a step's variables.
fn offline_env() -> Vec<(String, String)> {
    vec![("NIX_CONFIG".to_owned(), OFFLINE_NIX_CONFIG.to_owned())]
}

/// The parts' program, by name: the ISO's `PATH` holds it.
pub const PARTS: &str = "ronoctua-parts";

/// An internal command: the belongings' passphrase sealed in the TPM, into
/// the first boot's folder on the target ([`crate::first_boot`]). Like
/// [`SEAL`], the secret reaches `systemd-creds` through stdin.
pub const SEAL_PASSPHRASE: &str = "seal-passphrase";

/// The installation's marker in the working flake: what nobody may
/// redistribute waits for the first network.
#[must_use]
pub fn deferred_marker() -> String {
    format!(
        "{WORK_DIRECTORY}/{}",
        ronoctua_person::names::DEFERRED_HARDWARE
    )
}

/// The first boot's folder, on the target.
#[must_use]
pub fn first_boot_directory() -> String {
    format!("/mnt{}", crate::first_boot::DIRECTORY)
}

/// The first boot's record, on the target.
#[must_use]
pub fn first_boot_record() -> String {
    format!("{}/{}", first_boot_directory(), crate::first_boot::RECORD)
}

/// [`Scratch::home`] of the installer's.
#[must_use]
pub fn scratch_home() -> String {
    Scratch::installer().home()
}

/// [`Scratch::keys`] of the installer's.
#[must_use]
pub fn scratch_keys() -> String {
    Scratch::installer().keys()
}

/// [`Scratch::lock`] of the installer's.
#[must_use]
pub fn person_lock() -> String {
    Scratch::installer().lock()
}

/// [`Scratch::staged`] of the installer's.
#[must_use]
pub fn staged_dir() -> String {
    Scratch::installer().staged()
}

/// Where an EMPTY repository's declines are staged for root's applier: the
/// plain part's own shape (`person/software/declined.json`).
#[must_use]
pub fn staged_declined() -> String {
    format!(
        "{}/{}/{}",
        staged_dir(),
        ronoctua_person::tree::PERSON,
        ronoctua_person::names::DECLINED
    )
}

/// The declines the working flake's `person/` holds once root's applier
/// took the person's tree — what [`LEFT_OUT_APPLIED`] reads.
#[must_use]
pub fn applied_declined() -> String {
    format!(
        "{WORK_DIRECTORY}/{}/{}",
        ronoctua_person::names::MACHINE_PERSON,
        ronoctua_person::names::DECLINED
    )
}

/// An internal command: the working flake's declines are READ, and the
/// installation stops before the disk unless they say the machine's choice:
/// every program left out declined there for this machine, and no program
/// ticked declined for it — but by a decline of every machine, the person's
/// own, which stays and is said.
pub const LEFT_OUT_APPLIED: &str = "left-out-applied";

/// What a run clears before its first step: the working flake and the
/// staged declines an earlier attempt left — the template is copied over
/// the working flake, and a decline of that attempt would be applied as
/// this one's.
#[must_use]
pub fn leftovers() -> [String; 2] {
    [WORK_DIRECTORY.to_owned(), staged_dir()]
}

/// The machine's choice of the offered programs, made `at` here: the ones
/// left out and the ones ticked (`ronoctua_person::choice`). `None` unless
/// the installation is `full` — the one that was asked. Pure.
#[must_use]
pub fn machine_choice(
    r: &Answers,
    at: &ronoctua_person::Stamp,
) -> Option<ronoctua_person::choice::Choice> {
    let owned = |ids: Vec<&str>| ids.into_iter().map(ToOwned::to_owned).collect();
    (r.profile == crate::model::FULL_PROFILE && !crate::model::OFFERED.is_empty()).then(|| {
        ronoctua_person::choice::Choice::new(
            &r.hostname,
            at.clone(),
            owned(r.left_out_ids()),
            owned(r.kept_ids()),
        )
    })
}

/// The programs left out of this machine as the person's declines: each
/// left-out id on this hostname alone, edited `at` here. `None` when
/// nothing is left out. Pure.
#[must_use]
pub fn left_out_declined(
    r: &Answers,
    at: &ronoctua_person::Stamp,
) -> Option<ronoctua_person::declined::Declined> {
    let ids = r.left_out_ids();
    (!ids.is_empty()).then(|| ronoctua_person::declined::Declined {
        format: ronoctua_person::FORMAT,
        apps: ids
            .into_iter()
            .map(|id| ronoctua_person::declined::Declining {
                id: id.to_owned(),
                machines: Some(vec![r.hostname.clone()]),
                edited_at: at.clone(),
                edited_on: r.hostname.clone(),
            })
            .collect(),
    })
}

/// The working flake's `flake.nix` — written only when aimed, and only
/// before the lock.
#[must_use]
pub fn aimed_flake() -> String {
    format!("{WORK_DIRECTORY}/{}", crate::release::TEMPLATE_FLAKE)
}

/// The lock file's name, beside a flake's `flake.nix`.
pub const LOCK_FILE: &str = "flake.lock";

/// The mark in a command where the attribute the machine's flake names goes
/// — asked of the flake ([`Capture::Attribute`]) and filled in by the
/// executor. The plan never spells the person's name as a system's.
pub const ATTRIBUTE: &str = "{attribute}";

/// The mark in `nixos-install`'s command where the system built for the
/// disk goes ([`Capture::Toplevel`]): it is copied, never evaluated again.
pub const TOPLEVEL: &str = "{toplevel}";

/// Where the machine's chosen zone is seeded, on the target.
///
/// THE FILE `systemd-timedated` ITSELF OWNS. NixOS only puts `/etc/localtime`
/// in the store when `time.timeZone` is a string — and the distro leaves it
/// `null` so that `LEADER s l` can change the zone (see `ronoctua.time.zone`).
/// What the installer asked for therefore has to be written as this symlink,
/// exactly as `timedatectl set-timezone` writes it.
pub const LOCALTIME_PATH: &str = "/mnt/etc/localtime";

/// And what it points at, which is `/etc/zoneinfo` on the target — NixOS
/// declares that one unconditionally, so the link is valid at the first boot
/// and stays valid across a rebuild.
pub const ZONEINFO_ON_TARGET: &str = "/etc/zoneinfo";

/// Where the passphrase is left for `disko` to read.
///
/// `nix/disko/default.nix` expects exactly this path in
/// `passwordFile` — it is not an interactive prompt on purpose, and the comment over
/// there explains why. **Outside the target disk**, and deleted at the end: the
/// passphrase exists only in the head of whoever installed (no TPM step, see
/// the end of [`steps`]).
pub const LUKS_KEY_PATH: &str = "/tmp/disko-luks.key";

/// The swapfile `disko` creates, on the target system.
///
/// **`disko`'s and not ours.** `nix/disko/default.nix` declares
/// `swap.swapfile` under the `@swap` subvolume, and that attribute name is the
/// file name — it is the path that reaches `swapDevices`, and therefore the only
/// one the kernel ever resumes from.
///
/// The installer used to create a SECOND file beside it, measure the
/// `resume_offset` from that one, and write it into `host.nix`: two 16 GiB files
/// on the disk, and an offset into the one nothing uses. The failure shows up
/// the first time somebody hibernates, which is far from here.
pub const SWAPFILE_PATH: &str = "/mnt/swap/swapfile";

/// Where the user's password hash is written, on the target system.
///
/// `nix/modules/base.nix` points `hashedPasswordFile` at
/// `/persist/etc/shadow/<user>` and **does not create the file** — it could not, section
/// 9 forbids the hash in any config. What creates it is this step; without
/// it the installed system comes up with `mutableUsers = false` and no valid
/// password, which is a machine nobody can get into.
pub const PASSWORD_HASH_PATH: &str = "/mnt/persist/etc/shadow/noctua";

/// Where the Wi-Fi network's profile is written, on the target system.
///
/// `/persist` because `nix/modules/persist.nix` already lists
/// `/etc/NetworkManager/system-connections` among what survives the wipe —
/// writing straight to `/etc` would give a network that works until the first boot
/// and disappears on the second.
///
/// A FIXED NAME, and not the SSID's: NetworkManager reads the network's name from INSIDE
/// the file, not from its name. A path that varies with the answer could
/// not be matched in `write_of`, and the step would become an `write_bytes` that
/// writes nothing.
pub const WIFI_PROFILE_PATH: &str =
    "/mnt/persist/etc/NetworkManager/system-connections/ronoctua-wifi.nmconnection";

/// Where the Wi-Fi password is sealed, on the target system.
///
/// `/mnt` because the target is mounted there during the installation. Sealing into
/// `/persist` without the prefix would write into the ISO, which is volatile — and the installed
/// system would be born with no password at all, with nothing to flag it.
pub const WIFI_SECRET_DIRECTORY: &str = "/mnt/persist/etc/ronoctua/wifi";

/// The person's user name on the target: the last component of
/// [`PASSWORD_HASH_PATH`], the one place the installer names them — a second
/// spelling here would be a second definition (invariant 16).
#[must_use]
pub fn target_user() -> &'static str {
    PASSWORD_HASH_PATH.rsplit('/').next().unwrap_or_default()
}

/// Where the target's users are, for the person's uid and gid: the file the
/// activation of `nixos-install` wrote.
pub const TARGET_PASSWD: &str = "/mnt/etc/passwd";

/// Where the fifteenth answer is written: the belongings' settings, in the
/// PERSON'S home on the target (`~/.config/ronoctua/belongings.json`), owned
/// by them — never `host.nix`, which describes the machine and not whoever
/// uses it.
#[must_use]
pub fn belongings_path() -> String {
    format!(
        "/mnt/home/{}/.config/{}/{}",
        target_user(),
        ronoctua_xdg::NAME,
        ronoctua_belongings::prefs::FILE
    )
}

/// The person's home on the target.
#[must_use]
pub fn target_home() -> String {
    format!("/mnt/home/{}", target_user())
}

/// The person's configuration root on the target (`~/.config/ronoctua`).
#[must_use]
pub fn target_config() -> String {
    format!("{}/.config/{}", target_home(), ronoctua_xdg::NAME)
}

/// The person's access folder on the target: where the restore — and
/// every sync after it — reads the deploy key or the token from.
#[must_use]
pub fn target_keys() -> String {
    format!("{}/{}", target_config(), ronoctua_belongings::places::KEYS)
}

/// An internal command: the access made in the scratch home goes to the
/// person's, theirs (`0700` folder, `0600` files) — the one copy of the
/// deploy key that outlives the installer, and the one that never travels
/// (`config/belongings-keys` is never kept).
pub const INSTALL_ACCESS: &str = "install-access";

/// An internal command: the repository's host let in the person's
/// `consent.json` (`ronoctua-consent`), theirs — typing the address WAS the
/// consent, and every fetch of the restore and of every sync asks it.
pub const LET_HOST: &str = "let-host";

/// An internal command: seal the Wi-Fi password into the TPM.
///
/// It is not [`WRITE`] because it is not writing content the installer
/// computes — it is running `systemd-creds` against the machine's TPM and storing
/// what it returns. `write_of` would refuse a path it does not know, and
/// that is why this command has a name of its own.
pub const SEAL: &str = "seal-wifi";

/// The network profile, in NetworkManager's keyfile format.
///
/// **It contains the password**, and that is why it is written by the installer (through
/// [`WRITE`]) and never goes through `argv`. The file is born with mode `0600`;
/// NetworkManager REFUSES a profile more permissive than that, which
/// happily turns a permission error into a network that does not connect instead
/// of a secret readable by everyone.
#[must_use]
pub fn wifi_profile(r: &Answers) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "[connection]");
    let _ = writeln!(s, "id={}", r.wifi_ssid);
    let _ = writeln!(s, "type=wifi");
    let _ = writeln!(s, "autoconnect=true\n");
    let _ = writeln!(s, "[wifi]");
    let _ = writeln!(s, "mode=infrastructure");
    let _ = writeln!(s, "ssid={}\n", r.wifi_ssid);
    let _ = writeln!(s, "[wifi-security]");
    // `wpa-psk` covers WPA2 and WPA3-transition, which is what exists at home.
    let _ = writeln!(s, "key-mgmt=wpa-psk");
    // THE PASSWORD DOES NOT GO IN HERE. `psk-flags=1` (`AGENT_OWNED`) tells
    // NetworkManager that it lives outside the file, and that it should ask for it
    // over D-Bus from whoever knows — the `ronoctua-network-agent`, which opens it from
    // the TPM. The world's default is `psk=<password>` here, and it is precisely that
    // which makes any backup of /persist carry the password along.
    let _ = writeln!(s, "psk-flags=1\n");
    let _ = writeln!(s, "[ipv4]");
    let _ = writeln!(s, "method=auto\n");
    let _ = writeln!(s, "[ipv6]");
    let _ = writeln!(s, "method=auto");
    s
}

/// An internal command: the installer writes the file, instead of running something.
///
/// It exists for the three files whose CONTENT cannot go through `argv` —
/// `/proc/<pid>/cmdline` is readable by any process, and the passphrase and the
/// password would be there for as long as the command ran. The plan shows the path and
/// never the content.
pub const WRITE: &str = "write";

/// The resume offset `host.nix` writes: the measured one, unless the swap
/// is smaller than the RAM and holds no hibernation image. Pure.
#[must_use]
pub fn resume_offset_of(r: &Answers) -> Option<u64> {
    r.resume_offset
        .filter(|_| r.swap.is_none_or(|swap| swap.hibernates))
}

/// Generates the target system's `host.nix`.
///
/// **It contains no secret.** Password and passphrase are left out on purpose: the
/// file is versionable in theory and ends up in a backup in practice, and section 9
/// forbids personal data in a repository file. What goes in is what
/// describes the MACHINE.
#[must_use]
pub fn host_nix(r: &Answers) -> String {
    let mut s = String::new();
    let _ = writeln!(
        s,
        "# GENERATED by the installer. It describes this machine, and only it.\n\
         #\n\
         # There is NO secret here: the password and the passphrase live in /persist, by\n\
         # section 9. This file can go to a backup with no risk.\n\
         {{ ... }}:\n\
         {{"
    );
    let _ = writeln!(s, "  networking.hostName = \"{}\";", r.hostname);
    let _ = writeln!(s, "  ronoctua.disk = \"{}\";", r.disk);
    // A MACHINE STARTED FROM BIOS boots GRUB, and its disk gets GRUB's own
    // partition (`nix/modules/boot-loader.nix`, `nix/disko/default.nix`).
    // Written only then: UEFI is the module's default.
    if r.bios {
        let _ = writeln!(s, "  ronoctua.firmware = \"bios\";");
    }
    let _ = writeln!(s, "  ronoctua.language = \"{}\";", r.language);
    // WHICH OF THE TWO INSTALLATIONS, and it is written on both answers.
    //
    // `full` is the module's own default, so the line is redundant there — and
    // it goes in anyway. This file is where somebody looks to find out what
    // their machine is, and an answer that only appears when it is not the
    // default is an answer they have to already know the default to read.
    //
    // It is also the line to edit to change one's mind: `minimal` to `full` plus
    // a `nixos-rebuild switch` is the whole upgrade, with no reinstall.
    let _ = writeln!(s, "  ronoctua.profile = \"{}\";", r.profile);
    // THE THEME THE MACHINE COMES UP IN, written for the reason the profile
    // is: it is the module's own default today, and the line is what tells
    // whoever reads this file that the default exists and where to change it.
    // There is no question for it — `LEADER s t` changes it at run time in a
    // keystroke, and a question here has thirteen derivatives to keep.
    let _ = writeln!(
        s,
        "  ronoctua.theme = \"{}\";",
        ronoctua_tokens::theme::default_one().id()
    );
    // THE LOCALE IS ITS OWN LINE, and not derived from the language: the two
    // answer different questions, and the module stopped tying them.
    let _ = writeln!(s, "  ronoctua.locale = \"{}\";", r.system_locale);
    // THE TIME ZONE IS NOT WRITTEN HERE ANY MORE, and that is the point.
    //
    // It was `time.timeZone = "…"`, which pins it in the flake — and NixOS
    // then starts `systemd-timedated` with `NIXOS_STATIC_TIMEZONE=1`, so
    // `LEADER s l` could never change the machine's zone on any machine this
    // installer had touched. The owner read exactly that: the settings changed
    // what the interface draws and nothing about the computer.
    //
    // `ronoctua.time.zone` stays `null` (its default), and the answer is
    // SEEDED on the target as `/etc/localtime` — the file timedated itself
    // owns and rewrites. A value born ON the machine, like `host.nix` and
    // `~/.shellrc.local`, rather than one the flake carries.
    //
    // Somebody who wants it declared writes the option by hand; the row in
    // `LEADER s l` then says where it comes from.
    // THE LAYOUT, overriding `nix/keyboard-xkb.nix`'s default. Without this line
    // the machine comes up on the layout of the computer this was written on.
    let _ = writeln!(s, "  ronoctua.keyboard.layout = \"{}\";", r.keyboard);
    // THE VARIANT AND THE COMPOSE KEY are written even when EMPTY, and that is
    // deliberate: empty is an answer here — `gb` wants no variant, and a layout
    // that already carries the accents wants no Compose key. Writing the line
    // makes the file say what was chosen; omitting it would leave the module's
    // default in force and make the answer invisible.
    let _ = writeln!(
        s,
        "  ronoctua.keyboard.variant = \"{}\";",
        r.keyboard_variant
    );
    // `compose` AND NOT `options`: the second is the whole xkb string, and the
    // module builds it from this answer plus what the system adds regardless
    // (today, the two Shifts that toggle Caps Lock). Writing `options` here —
    // which is what this line used to do — meant that answering the accent
    // question replaced the whole string, and whoever chose a Compose key other
    // than the default silently lost a key they were never asked about.
    let _ = writeln!(s, "  ronoctua.keyboard.compose = \"{}\";", r.compose_key);
    // THE HARDWARE REPORT, by RELATIVE path.
    //
    // The `nixos-facter` step writes it next to this file, and both end up in
    // `/etc/nixos` on the installed system. An absolute `/mnt/...` would be the
    // INSTALLER's path, which stops existing the moment the machine reboots —
    // and the failure would come at the first rebuild, far from here.
    //
    // Without this line the report is generated and then ignored: the video
    // driver, the microcode and the touchpad go back to defaults, silently.
    let _ = writeln!(s, "  ronoctua.facterReport = ./facter.json;");
    // THE SWAPFILE'S SIZE, when it was measured (`crate::swap`): the RAM, or
    // what the disk can spare. Not measured — a dry run away from the disk —
    // writes nothing, and the hardware report's default stands.
    if let Some(swap) = r.swap {
        let _ = writeln!(s, "  ronoctua.swapSize = \"{}\";", swap.size());
    }
    match resume_offset_of(r) {
        // Without the offset there is NO hibernation. Writing `0` would be worse than omitting it:
        // the kernel would accept it and hibernation would fail in use, not at boot.
        Some(off) => {
            let _ = writeln!(s, "  boot.kernelParams = [ \"resume_offset={off}\" ];");
        }
        // AND IT IS SWITCHED OFF, said as an option and not as a comment:
        // without this line the machine would still name a resume device,
        // and UPower would still hibernate at its last percent — into a
        // swapfile smaller than the RAM, or one whose offset nobody knows.
        None => {
            let _ = writeln!(
                s,
                "  # No resume offset (a swapfile smaller than the RAM, or one not measured):\n  \
                 # this machine does not hibernate.\n  \
                 ronoctua.hibernation = false;"
            );
        }
    }
    // THE TREE AN AIMED MACHINE IS BUILT FROM, kept by every system built
    // from it: its flake names that tree by its store path, and a
    // collection that took it would leave `/etc/nixos` naming nothing.
    // Written when the installation runs (`Answers::aimed`); a dry run
    // leaves it out, as it leaves the swap.
    if r.aimed {
        let _ = writeln!(
            s,
            "  # This machine was installed from its installer's own distro and keeps it:\n  \
             # the tree its flake names stays in the store with every system built from it.\n  \
             imports = [ ({{ inputs, ... }}: {{ system.extraDependencies = [ inputs.self.outPath ]; }}) ];"
        );
    }
    let _ = writeln!(s, "}}");
    s
}

/// Who runs a step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Runner {
    /// Root — the installer itself.
    Root,
    /// The person, on the target (`setpriv` with the ids the target's
    /// `/etc/passwd` gives them): what they own is born theirs, and a
    /// restore cannot write anywhere they could not.
    Person,
}

/// What the executor keeps of a step's output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capture {
    /// The system the machine's flake builds: its one
    /// `nixosConfigurations` name, which fills [`ATTRIBUTE`].
    Attribute,
    /// The system built, by its store path, which fills [`TOPLEVEL`].
    Toplevel,
}

/// One installation step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    /// What to tell the person. A text **key** (invariant 8).
    pub label: &'static str,
    /// The command, with its arguments already.
    pub command: Vec<String>,
    /// If this fails, is the disk left in an intermediate state?
    ///
    /// It marks the steps from which there is no way back. The TUI warns before the
    /// first of them, and not at every step — warning always is not warning.
    pub destructive: bool,
    /// Who runs it.
    pub runs_as: Runner,
    /// Variables it runs with, beside the installer's own — never a secret
    /// (they are shown in the plan).
    pub env: Vec<(String, String)>,
    /// It runs only when this path exists, and is skipped — said — when not:
    /// a repository with no declarations has nothing to apply.
    pub only_if: Option<String>,
    /// What the executor keeps of its output.
    pub captures: Option<Capture>,
}

impl Step {
    /// A step root runs, with nothing around it.
    fn root(label: &'static str, command: Vec<String>, destructive: bool) -> Self {
        Self {
            label,
            command,
            destructive,
            runs_as: Runner::Root,
            env: Vec::new(),
            only_if: None,
            captures: None,
        }
    }

    /// The step as the plan prints it: who runs it when it is not root, its
    /// variables, the command, and the condition it waits on.
    #[must_use]
    pub fn shown(&self) -> String {
        let mut out = String::new();
        if self.runs_as == Runner::Person {
            let _ = write!(out, "[{}] ", target_user());
        }
        for (name, value) in &self.env {
            let _ = write!(out, "{name}={value} ");
        }
        out.push_str(&self.command.join(" "));
        if let Some(path) = &self.only_if {
            let _ = write!(out, " [if {path}]");
        }
        out
    }
}

/// The variables a belongings' child runs with in `home` — [`home_env`],
/// for the executor's own children of the belongings (`binary-cache-token`).
#[must_use]
pub fn home_env_of(home: &str) -> Vec<(String, String)> {
    home_env(home)
}

/// The variables a belongings' child runs with in `home`: its home and its
/// roots, so `consent.json`, the settings and the access are THAT home's.
fn home_env(home: &str) -> Vec<(String, String)> {
    vec![
        ("HOME".to_owned(), home.to_owned()),
        ("XDG_CONFIG_HOME".to_owned(), format!("{home}/.config")),
        ("XDG_STATE_HOME".to_owned(), format!("{home}/.local/state")),
        ("XDG_CACHE_HOME".to_owned(), format!("{home}/.cache")),
    ]
}

/// The person's programs, by name: the ISO's `PATH` (and the release's
/// installer app's) holds the ones built with this installer.
pub const BELONGINGS: &str = "ronoctua-belongings";
/// See [`BELONGINGS`].
pub const PERSON: &str = "ronoctua-person";

/// The installation's sequence.
///
/// Returned as DATA, and not executed here: that way the TUI can show everything
/// that will run before running it, and the test can check the order without touching
/// any disk.
#[must_use]
pub fn steps(r: &Answers, origin: &Origin) -> Vec<Step> {
    let work_flake = format!("path:{WORK_DIRECTORY}");
    // WHETHER THIS MACHINE LEAVES PROGRAMS OUT — see the module's header.
    let leaving = !r.left_out_ids().is_empty();
    let offline = |label, command: Vec<String>, destructive| Step {
        env: offline_env(),
        ..Step::root(label, command, destructive)
    };
    let mut sequence = vec![Step::root(
        // BEFORE partitioning, and not alongside: `disko` reads the file in the
        // first second, and a step that fails for a missing file is
        // better discovered before the disk has been touched.
        key::INST_STEP_KEY,
        vec![WRITE.into(), LUKS_KEY_PATH.into()],
        false,
    )];
    // THE MACHINE'S OWN FLAKE, from the template — to a WRITABLE place, before
    // anything else. See `WORK_DIRECTORY`.
    sequence.push(Step::root(
        key::INST_STEP_TEMPLATE,
        vec![
            "cp".into(),
            // `-T`: without it, an existing destination becomes `<destination>/machine`.
            //
            // `-L`: AND THIS ONE IS NOT COSMETIC. The distro's tree on the ISO
            // is a SYMLINK into the store (`environment.etc`), and `cp -r`
            // does not follow a symlink given on the command line — it
            // copies the LINK. The working directory then WAS the store
            // path, and writing the `host.nix` into it failed with
            // "Read-only file system".
            "-rTL".into(),
            // The template comes from the store, read-only. Copied with the
            // source's mode, writing the `host.nix` beside it would fail with
            // "Permission denied".
            "--no-preserve=mode".into(),
            origin.template(),
            WORK_DIRECTORY.into(),
        ],
        false,
    ));
    if origin.aimed {
        // AIMED AT THIS INSTALLER'S TREE while it installs — see `Origin`.
        sequence.push(Step::root(
            key::INST_STEP_AIM,
            vec![WRITE.into(), aimed_flake()],
            false,
        ));
    }
    sequence.push(Step::root(
        // The `host.nix` DISKO reads: `ronoctua.disk` comes from it. Without
        // this step `disko` does not know which disk to erase — and the value
        // CANNOT come as a command-line argument, because the following
        // `nixos-install` needs the same value and does not accept one.
        key::INST_STEP_HOST_NIX,
        vec![WRITE.into(), HOST_NIX_WORK_PATH.into()],
        false,
    ));
    // WHAT NOBODY MAY REDISTRIBUTE WAITS FOR THE FIRST NETWORK: the ISO
    // carries none of it, so the machine is installed with those hardware
    // branches off (`ronoctua.hardware.deferUndistributable`), and its first
    // apply with a network brings them (`ronoctua-machine`).
    sequence.push(Step::root(
        key::INST_STEP_DEFER,
        vec![WRITE.into(), deferred_marker()],
        false,
    ));
    sequence.push(Step::root(
        // THE HARDWARE REPORT COMES BEFORE `disko`, and it has to: the
        // generated `host.nix` says `facterReport = ./facter.json`, and every
        // evaluation of the flake below reads it. Into the WORKING directory,
        // which a later step copies whole to the target.
        key::INST_STEP_FACTER,
        vec![
            "nixos-facter".into(),
            "-o".into(),
            format!("{WORK_DIRECTORY}/facter.json"),
        ],
        // It reads hardware and writes one file outside the target: nothing
        // it does is destructive.
        false,
    ));
    // THE LOCK: the ISO's tree the machine is built from, and every input of
    // the distro's exactly as its lock pinned them — every one in the ISO's
    // store (`nix/installer/offline.nix`), so nothing is fetched.
    sequence.push(offline(
        key::INST_STEP_LOCK,
        vec![
            "nix".into(),
            "flake".into(),
            "lock".into(),
            "--offline".into(),
            work_flake.clone(),
        ],
        false,
    ));
    // THE SYSTEM'S NAME, asked of the flake: `nixosConfigurations` has the
    // one the machine builds (`ronoctua.machine.attr`), and spelling it here
    // would be the person's name written a second time.
    sequence.push(Step {
        captures: Some(Capture::Attribute),
        ..offline(
            key::INST_STEP_ATTRIBUTE,
            vec![
                "nix".into(),
                "eval".into(),
                "--offline".into(),
                "--json".into(),
                format!("{work_flake}#nixosConfigurations"),
                "--apply".into(),
                "builtins.attrNames".into(),
            ],
            false,
        )
    });
    if leaving {
        // THE DECLINES, staged root's in the plain part's shape and applied
        // to the working flake by root's applier — which, with no key pinned,
        // takes them as staged — so `nixos-install` already leaves the
        // programs out. The repository hears the choice on the first boot.
        sequence.push(Step::root(
            key::INST_STEP_LEFT_OUT,
            vec![WRITE.into(), staged_declined()],
            false,
        ));
        sequence.push(Step {
            only_if: Some(format!(
                "{}/{}",
                staged_dir(),
                ronoctua_person::tree::PERSON
            )),
            ..Step::root(
                key::INST_STEP_PERSON,
                vec![
                    PERSON.into(),
                    "apply".into(),
                    "--staged".into(),
                    staged_dir(),
                    "--machine".into(),
                    WORK_DIRECTORY.into(),
                    "--owner".into(),
                    "root".into(),
                    "--lock".into(),
                    person_lock(),
                ],
                false,
            )
        });
        sequence.push(applied_check());
    }
    // THE SYSTEM BUILT BEFORE THE DISK IS TOUCHED: what this machine needs
    // and the ISO does not carry — hardware no probe of
    // `nix/installer/offline.nix` covers — fails HERE, with the disk intact,
    // and `nixos-install` below only copies what this built.
    sequence.push(offline(
        key::INST_STEP_BUILD_FIRST,
        vec![
            "nix".into(),
            "build".into(),
            "--offline".into(),
            "--no-link".into(),
            format!("{work_flake}#nixosConfigurations.{ATTRIBUTE}.config.system.build.toplevel"),
        ],
        false,
    ));
    sequence.push(offline(
        key::INST_STEP_PARTITION,
        // `disko` does everything: it partitions, creates the LUKS, formats,
        // mounts. It evaluates the WORKING directory: `/mnt` only comes into
        // existence after this command.
        //
        // `--yes-wipe-all-disks` BECAUSE THE INSTALLER HAS ALREADY ASKED: the
        // TUI's confirmation screen, or `--execute` in the environment mode.
        // Without it `disko` asks again on a `stdin` nobody types into, reads
        // nothing and aborts with the disk untouched.
        vec![
            "disko".into(),
            "--mode".into(),
            "destroy,format,mount".into(),
            "--flake".into(),
            format!("{WORK_DIRECTORY}#{ATTRIBUTE}"),
            "--yes-wipe-all-disks".into(),
        ],
        true,
    ));
    // THERE IS NO SWAPFILE STEP: `disko` creates `/swap/swapfile` from
    // `nix/disko/default.nix`, at the size `host.nix` says, and the offset
    // is taken from that file right after `disko` (`executor::run_command`).
    sequence.push(Step::root(
        // Now with the `resume_offset`, which only exists after the swap.
        key::INST_STEP_HOST_NIX,
        vec![WRITE.into(), HOST_NIX_WORK_PATH.into()],
        true,
    ));
    sequence.push(Step::root(
        // THE TARGET'S MACHINE DIRECTORY IS MADE FIRST: `disko` mounts the
        // root at `/mnt` and nothing under it, and `cp -rT` makes the last
        // folder of its destination but never the ones above it.
        key::INST_STEP_TARGET,
        vec!["mkdir".into(), "-p".into(), TARGET_DIRECTORY.into()],
        true,
    ));
    sequence.push(Step::root(
        // The whole flake to the target: `host.nix`, the report, the lock,
        // root's staged declines in `person/` — and its `flake.nix` with the
        // lock made for it, aimed: the pair agrees.
        key::INST_STEP_FLAKE,
        vec![
            "cp".into(),
            "-rT".into(),
            "--no-preserve=mode".into(),
            WORK_DIRECTORY.into(),
            TARGET_DIRECTORY.into(),
        ],
        true,
    ));
    sequence.push(Step::root(
        // BEFORE `nixos-install`: `mutableUsers = false` makes the activator
        // read the `hashedPasswordFile` during the installation.
        key::INST_STEP_PASSWORD,
        vec![WRITE.into(), PASSWORD_HASH_PATH.into()],
        true,
    ));
    if !r.wifi_ssid.is_empty() {
        // THE NETWORK STEPS ONLY EXIST WHEN THERE IS A NETWORK: with none the
        // profile would carry an empty `id=`, which NetworkManager accepts
        // and never connects with.
        sequence.push(Step::root(
            key::INST_STEP_WIFI,
            vec![WRITE.into(), WIFI_PROFILE_PATH.into()],
            true,
        ));
        sequence.push(Step::root(
            // THE SEALING, right after the profile. The SSID goes in the
            // command and the PASSWORD does not: it reaches `systemd-creds`
            // through stdin, inside the executor.
            key::INST_STEP_SEAL_WIFI,
            vec![
                SEAL.into(),
                WIFI_SECRET_DIRECTORY.into(),
                r.wifi_ssid.clone(),
            ],
            true,
        ));
    }
    // THE SYSTEM THE DISK GETS, built from the target's flake — the same
    // as the one built before the disk, but for the `resume_offset` the
    // swapfile only now has: the few derivations the kernel's command line
    // reaches, from inputs already here. Built in the ISO's store, where
    // the flake's sources are: `nixos-install --flake` evaluates against
    // the TARGET's store, which has none of them, and would fetch them.
    sequence.push(Step {
        captures: Some(Capture::Toplevel),
        ..offline(
            key::INST_STEP_BUILD,
            vec![
                "nix".into(),
                "build".into(),
                "--offline".into(),
                "--no-link".into(),
                "--print-out-paths".into(),
                format!(
                    "path:{TARGET_DIRECTORY}#nixosConfigurations.{ATTRIBUTE}.config.system.build.toplevel"
                ),
            ],
            true,
        )
    });
    sequence.push(offline(
        key::INST_STEP_INSTALL,
        vec![
            "nixos-install".into(),
            // THE SYSTEM JUST BUILT, copied and activated: nothing is
            // evaluated or built against the target's store.
            "--system".into(),
            TOPLEVEL.into(),
            "--no-root-password".into(),
            // No channel: the machine is a flake, and copying the ISO's
            // channel would be the one thing here with a download in it.
            "--no-channel-copy".into(),
        ],
        true,
    ));
    // THE PARTS THE ISO CARRIES, into the machine's store: a part the plan
    // wants and this medium holds arrives on the first boot with no
    // network (`ronoctua-parts seed`). It never fails the installation —
    // a part that is not here downloads later, like any other.
    sequence.push(Step {
        // The `nix` it copies with, by path: the installer's own.
        env: vec![("RONOCTUA_NIX".to_owned(), origin.nix.clone())],
        ..Step::root(
            key::INST_STEP_SEED_PARTS,
            vec![PARTS.into(), "seed".into(), "--root".into(), "/mnt".into()],
            true,
        )
    });
    sequence.push(Step::root(
        // THE ZONE THAT WAS ASKED FOR, seeded as the file timedated owns —
        // AFTER `nixos-install`, because that is what creates `/mnt/etc`; a
        // SYMLINK and not a copy, because that is what `timedatectl
        // set-timezone` writes and what the daemon reads back.
        key::INST_STEP_ZONE,
        vec![
            "ln".into(),
            "-sfn".into(),
            format!("{ZONEINFO_ON_TARGET}/{}", r.timezone),
            LOCALTIME_PATH.into(),
        ],
        true,
    ));
    // NO STEP POINTS THE FLAKE BACK at the release: the machine's first
    // update does (`flake-update`, which un-aims it) — see `Origin`.
    // THE PERSON'S HOME, after `nixos-install` made them (and so the uid
    // and gid every file is given, read from the target's `/etc/passwd`):
    // what the belongings need that no network gives.
    sequence.push(Step::root(
        key::INST_STEP_ACCESS,
        vec![INSTALL_ACCESS.into(), scratch_keys(), target_keys()],
        true,
    ));
    sequence.push(Step::root(
        key::INST_STEP_CONSENT,
        vec![LET_HOST.into(), target_config()],
        true,
    ));
    sequence.push(Step::root(
        // THE SETTINGS: the address alone and how it is reached; no key, no
        // token, no passphrase.
        key::INST_STEP_BELONGINGS,
        vec![WRITE.into(), belongings_path()],
        true,
    ));
    // AND WHAT THE FIRST BOOT FINISHES (`crate::first_boot`): the record —
    // the address, the machine, its choice of programs — and the passphrase
    // sealed in the TPM beside it, never on the disk in the clear.
    sequence.push(Step::root(
        key::INST_STEP_FIRST_BOOT,
        vec![WRITE.into(), first_boot_record()],
        true,
    ));
    sequence.push(Step::root(
        key::INST_STEP_SEAL_PASSPHRASE,
        vec![SEAL_PASSPHRASE.into(), first_boot_directory()],
        true,
    ));
    // THERE IS NO TPM STEP FOR THE DISK, and its absence is the decision
    // rather than an omission: `systemd-cryptenroll --tpm2-pcrs=7+11+12+15`
    // could not deliver what it named without a measured UKI.
    // `nix/disko/default.nix` carries the whole argument. The Wi-Fi key and
    // the belongings' passphrase ARE sealed in the TPM — see `SEAL`, a
    // different mechanism and a different question.
    sequence
}

/// The step that reads what root's applier made of the declines — see
/// [`LEFT_OUT_APPLIED`]. It waits on nothing: no file is a refusal too.
fn applied_check() -> Step {
    Step::root(
        key::INST_STEP_LEFT_OUT_APPLIED,
        vec![LEFT_OUT_APPLIED.into(), applied_declined()],
        false,
    )
}

/// A disk's LUKS partition, per section 7.2's layout.
///
/// `p2` on NVMe and MMC (which number with a `p`), `2` on the rest. Getting that wrong makes
/// `cryptenroll` act on the wrong device.
#[must_use]
pub fn luks_partition(disk: &str) -> String {
    let last = disk.rsplit('/').next().unwrap_or("");
    if last.starts_with("nvme") || last.starts_with("mmcblk") || last.starts_with("loop") {
        format!("{disk}p2")
    } else {
        format!("{disk}2")
    }
}

/// The installed machine's Wi-Fi, written on THIS machine — run by
/// `wifi-joins-vm` (`tests/wifi-joins-vm.nix`), as root, for the network in
/// `RONOCTUA_TEST_WIFI_SSID` with the password in
/// `RONOCTUA_TEST_WIFI_PASSWORD`: the profile [`wifi_profile`] writes and the
/// blob the seal step makes, where the installed machine reads them — and
/// NetworkManager then joins as at its first boot.
#[cfg(test)]
mod on_the_air {
    use super::*;
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt as _;

    #[test]
    #[ignore = "writes this machine's NetworkManager profile and seals in its TPM: wifi-joins-vm"]
    fn the_installed_wifi_is_written_here() {
        let variable = |name: &str| {
            std::env::var(name).unwrap_or_else(|_| panic!("{name} names the test's network"))
        };
        let r = Answers {
            wifi_ssid: variable("RONOCTUA_TEST_WIFI_SSID"),
            wifi_password: variable("RONOCTUA_TEST_WIFI_PASSWORD"),
            ..Answers::default()
        };
        // The target's path without `/mnt/persist`: on the installed machine
        // `persist.nix` puts it at `/etc`, where this one reads it.
        let path = WIFI_PROFILE_PATH
            .strip_prefix("/mnt/persist")
            .expect("the profile's place on the target");
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
            .and_then(|mut f| f.write_all(wifi_profile(&r).as_bytes()))
            .expect("writing the profile");
        ronoctua_network::secret::seal(&r.wifi_ssid, &r.wifi_password).expect("sealing");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn answers() -> Answers {
        Answers {
            disk: "/dev/nvme0n1".into(),
            hostname: "noctua-portatil".into(),
            // Values that do NOT collide with prose: the generated header talks about
            // "secret" and "passphrase", and a test that looked for those
            // words would flag a leak where there is only an explanation.
            password: "Zq7-test-password".into(),
            wifi_ssid: String::new(),
            wifi_password: String::new(),
            passphrase: "Kx9-passphrase-de-teste".into(),
            language: "pt-BR".into(),
            keyboard: "br".into(),
            keyboard_variant: "abnt2".into(),
            compose_key: String::new(),
            system_locale: "en_GB.UTF-8".into(),
            timezone: "Europe/London".into(),
            // `minimal`, and not the default: a fixture that answers the default
            // proves the field reaches `host.nix` even when it is being dropped.
            profile: "minimal".into(),
            // Left out, and read only by a `full` installation: on this one
            // the answer must change nothing.
            left_out: "blender".into(),
            left_out_at: ronoctua_person::Stamp::parse("2026-10-20T12:00:00Z"),
            resume_offset: Some(123_456),
            belongings: "git@example.org:someone/belongings.git".into(),
            belongings_access: format!("{}/deploy", scratch_keys()),
            belongings_passphrase: "Qw4-belongings-passphrase".into(),
            belongings_empty: Some(false),
            online: None,
            belongings_checked: false,
            swap: None,
            aimed: false,
            bios: false,
        }
    }

    fn origin() -> Origin {
        Origin {
            distro: "/etc/ronoctua/distro".into(),
            aimed: false,
            nix: "/run/current-system/sw/bin/nix".into(),
        }
    }

    /// A `full` installation leaving two programs out, its repository empty
    /// or not.
    fn leaving(empty: bool) -> Answers {
        Answers {
            profile: crate::model::FULL_PROFILE.into(),
            left_out: "blender,libreoffice".into(),
            belongings_empty: Some(empty),
            ..answers()
        }
    }

    /// Every shape of the plan the tests below must hold for: the fixture,
    /// and a `full` one leaving programs out of a repository with something
    /// in it and of an empty one.
    fn every_plan() -> Vec<Vec<Step>> {
        vec![
            steps(&answers(), &origin()),
            steps(&leaving(false), &origin()),
            steps(&leaving(true), &origin()),
        ]
    }

    /// The value after `flag` on a step's command line.
    fn flag(s: &Step, f: &str) -> Option<String> {
        s.command
            .iter()
            .position(|a| a == f)
            .and_then(|i| s.command.get(i + 1))
            .cloned()
    }

    /// FAILS IF: the programs left out are not staged for root's applier
    /// (unpinned: taken as staged) and applied to the working flake before
    /// the system is built and the disk touched — whatever the repository
    /// holds — or what the applier made of them is not read; or the choice
    /// and the declines the first boot hands over are not both halves of the
    /// answer, for this hostname alone.
    #[test]
    fn left_out_programs_are_applied_before_the_disk() {
        for empty in [Some(false), Some(true), None] {
            let r = Answers {
                belongings_empty: empty,
                ..leaving(false)
            };
            let p = steps(&r, &origin());
            let order = [
                key::INST_STEP_ATTRIBUTE,
                key::INST_STEP_LEFT_OUT,
                key::INST_STEP_PERSON,
                key::INST_STEP_LEFT_OUT_APPLIED,
                key::INST_STEP_BUILD_FIRST,
                key::INST_STEP_PARTITION,
                key::INST_STEP_INSTALL,
                key::INST_STEP_FIRST_BOOT,
            ];
            let places: Vec<usize> = order.iter().map(|l| at(&p, l)).collect();
            assert!(
                places.windows(2).all(|w| w[0] < w[1]),
                "out of order: {places:?}"
            );
            assert_eq!(p[places[1]].command, [WRITE.to_owned(), staged_declined()]);
            let applier = &p[places[2]];
            assert_eq!(applier.command[..2], [PERSON, "apply"]);
            assert_eq!(flag(applier, "--staged"), Some(staged_dir()));
            assert_eq!(flag(applier, "--machine").as_deref(), Some(WORK_DIRECTORY));
            assert_eq!(flag(applier, "--owner").as_deref(), Some("root"));
            assert_eq!(
                applier.only_if,
                Some(format!(
                    "{}/{}",
                    staged_dir(),
                    ronoctua_person::tree::PERSON
                ))
            );
            // WHAT THE APPLIER MADE OF THEM IS READ, and waits on nothing: no
            // file is a refusal, never a skip.
            let read = &p[places[3]];
            assert_eq!(
                read.command,
                [LEFT_OUT_APPLIED.to_owned(), applied_declined()]
            );
            assert_eq!(read.only_if, None);
            assert!(!read.destructive);
        }
        assert_eq!(
            staged_declined(),
            format!("{}/person/software/declined.json", staged_dir())
        );
        assert_eq!(
            applied_declined(),
            format!("{WORK_DIRECTORY}/person/software/declined.json")
        );
        // THE CHOICE: both halves of the answer, for this machine alone.
        let r = leaving(false);
        let choice = machine_choice(&r, r.left_out_at.as_ref().unwrap()).unwrap();
        assert_eq!(choice.machine, "noctua-portatil");
        assert_eq!(choice.declined, ["blender", "libreoffice"]);
        assert_eq!(choice.kept, r.kept_ids());
        assert!(
            !choice.kept.is_empty() && !choice.kept.iter().any(|k| choice.declined.contains(k))
        );
        assert_eq!(choice.kept.len() + 2, crate::model::OFFERED.len());
        // AND THE DECLINES: each on this hostname.
        let declined = left_out_declined(&r, r.left_out_at.as_ref().unwrap()).unwrap();
        let ids: Vec<&str> = declined.apps.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, ["blender", "libreoffice"]);
        for d in &declined.apps {
            assert_eq!(
                d.machines.as_deref(),
                Some(&["noctua-portatil".to_owned()][..])
            );
            assert_eq!(d.edited_on, "noctua-portatil");
            assert_eq!(d.edited_at, r.left_out_at.clone().unwrap());
        }
    }

    /// FAILS IF: a `full` installation that leaves nothing out does not hand
    /// its choice to the first boot — every program ticked, so an earlier
    /// attempt's declines of this machine come back — or a `minimal` one
    /// hands one over.
    #[test]
    fn a_full_choice_is_handed_over_even_when_nothing_is_left_out() {
        let r = Answers {
            left_out: crate::model::LEFT_OUT_NONE.into(),
            ..leaving(false)
        };
        let record = crate::first_boot::Record::of(&r, r.left_out_at.as_ref().unwrap());
        let choice = record
            .choice
            .expect("a full installation hands its choice over");
        assert!(choice.declined.is_empty());
        assert_eq!(choice.kept.len(), crate::model::OFFERED.len());
        assert!(record.declined.is_none());
        // A `minimal` one was never asked: no choice.
        assert!(machine_choice(&answers(), answers().left_out_at.as_ref().unwrap()).is_none());
    }

    /// FAILS IF: a `minimal` installation, or a `full` one that leaves
    /// nothing out, stages declines or applies anything.
    #[test]
    fn nothing_is_declined_when_nothing_is_left_out() {
        let none = Answers {
            left_out: crate::model::LEFT_OUT_NONE.into(),
            ..leaving(true)
        };
        for r in [answers(), none] {
            let p = steps(&r, &origin());
            for s in &p {
                assert!(
                    ![
                        key::INST_STEP_LEFT_OUT,
                        key::INST_STEP_PERSON,
                        key::INST_STEP_LEFT_OUT_APPLIED,
                    ]
                    .contains(&s.label),
                    "{} for {:?}/{:?}",
                    s.label,
                    r.profile,
                    r.left_out
                );
            }
            assert!(left_out_declined(&r, r.left_out_at.as_ref().unwrap()).is_none());
        }
    }

    /// FAILS IF: a run does not start from nothing an earlier attempt left —
    /// the working flake the template is copied over, the copy of the plain
    /// part, or the staged declines — or would clear anything else.
    #[test]
    fn a_run_clears_what_an_earlier_attempt_left() {
        let cleared = leftovers();
        assert_eq!(cleared, [WORK_DIRECTORY.to_owned(), staged_dir()]);
        for path in [
            staged_declined(),
            applied_declined(),
            aimed_flake(),
            HOST_NIX_WORK_PATH.to_owned(),
        ] {
            assert!(
                cleared.iter().any(|c| path.starts_with(&format!("{c}/"))),
                "{path}"
            );
        }
        for kept in [scratch_keys(), scratch_home(), person_lock()] {
            assert!(
                !cleared.iter().any(|c| kept.starts_with(c.as_str())),
                "{kept} would be cleared"
            );
        }
    }

    /// FAILS IF: anything of the installation can reach the network: a
    /// `nix` step without the offline configuration (no substituter, no
    /// registry), a `nix` command without `--offline`, or a step that asks
    /// the person's repository (`plain`, `restore`, `init`, `probe`,
    /// `verify`) — those are the first boot's.
    #[test]
    fn nothing_reaches_the_network() {
        for p in every_plan() {
            for s in &p {
                let program = s.command.first().map(String::as_str).unwrap_or_default();
                if ["nix", "disko", "nixos-install"].contains(&program) {
                    assert!(
                        s.env
                            .iter()
                            .any(|(n, v)| n == "NIX_CONFIG" && v == OFFLINE_NIX_CONFIG),
                        "`{}` runs with the substituters and the registry",
                        s.label
                    );
                }
                if program == "nix" {
                    assert!(
                        s.command.iter().any(|a| a == "--offline"),
                        "`{}` is not offline",
                        s.label
                    );
                }
                assert_ne!(
                    program, BELONGINGS,
                    "`{}` asks the repository while installing",
                    s.label
                );
                assert!(
                    !s.command
                        .iter()
                        .any(|a| a.contains("substituters") || a.contains("netrc")),
                    "{:?}",
                    s.command
                );
            }
        }
        assert_eq!(OFFLINE_NIX_CONFIG, "substituters =\nflake-registry =\n");
    }

    fn at(p: &[Step], label: &str) -> usize {
        p.iter()
            .position(|s| s.label == label)
            .unwrap_or_else(|| panic!("no step `{label}`"))
    }

    /// FAILS IF: the belongings' address reaches `host.nix`, the settings
    /// are written anywhere but the person's home or before `nixos-install`
    /// made them, or the first boot's record and the sealed passphrase come
    /// before the access, the consent and the settings it reads are in place.
    #[test]
    fn the_belongings_go_into_the_home_after_the_install_and_never_host_nix() {
        let r = answers();
        let path = belongings_path();
        assert_eq!(path, "/mnt/home/noctua/.config/ronoctua/belongings.json");
        let plan = steps(&r, &origin());
        let written = plan
            .iter()
            .position(|s| s.command.get(1) == Some(&path))
            .expect("no step writes the belongings");
        let installed = at(&plan, key::INST_STEP_INSTALL);
        assert!(written > installed, "written before the user exists");
        assert_eq!(plan[written].label, key::INST_STEP_BELONGINGS);
        let record = at(&plan, key::INST_STEP_FIRST_BOOT);
        for before in [
            key::INST_STEP_ACCESS,
            key::INST_STEP_CONSENT,
            key::INST_STEP_BELONGINGS,
        ] {
            let b = at(&plan, before);
            assert!(installed < b && b < record, "`{before}` out of order");
        }
        assert_eq!(
            plan[record].command,
            [WRITE.to_owned(), first_boot_record()]
        );
        let sealed = at(&plan, key::INST_STEP_SEAL_PASSPHRASE);
        assert!(record < sealed);
        assert_eq!(
            plan[sealed].command,
            [SEAL_PASSPHRASE.to_owned(), first_boot_directory()]
        );
        assert_eq!(
            first_boot_record(),
            "/mnt/persist/etc/ronoctua/first-boot/record.json"
        );
        assert!(
            !host_nix(&r).contains("example.org"),
            "the address reached host.nix"
        );
    }

    /// FAILS IF: the system is not built before the disk is touched — out of
    /// its order, from anything but the working flake's system, or not
    /// offline — or anything destructive comes first; or the parts the ISO
    /// carries are not seeded after `nixos-install`, with its `nix` by path.
    #[test]
    fn the_system_is_built_before_the_disk_is_touched() {
        let p = steps(&answers(), &origin());
        let order = [
            key::INST_STEP_TEMPLATE,
            key::INST_STEP_HOST_NIX,
            key::INST_STEP_FACTER,
            key::INST_STEP_LOCK,
            key::INST_STEP_ATTRIBUTE,
            key::INST_STEP_BUILD_FIRST,
            key::INST_STEP_PARTITION,
            key::INST_STEP_INSTALL,
            key::INST_STEP_SEED_PARTS,
        ];
        let places: Vec<usize> = order.iter().map(|l| at(&p, l)).collect();
        assert!(
            places.windows(2).all(|w| w[0] < w[1]),
            "out of order: {places:?}"
        );
        assert!(!p[..places[6]].iter().any(|s| s.destructive));
        // THE MARKER, before anything evaluates the flake.
        let defer = at(&p, key::INST_STEP_DEFER);
        assert!(defer < places[3], "the marker comes after the lock");
        assert_eq!(p[defer].command, [WRITE.to_owned(), deferred_marker()]);
        assert_eq!(
            deferred_marker(),
            "/tmp/ronoctua-flake/hardware-deferred.json"
        );
        let built = &p[places[5]];
        assert_eq!(
            built.command[..4],
            ["nix", "build", "--offline", "--no-link"]
        );
        assert_eq!(
            built.command[4],
            format!(
                "path:{WORK_DIRECTORY}#nixosConfigurations.{ATTRIBUTE}.config.system.build.toplevel"
            )
        );
        let seeded = &p[places[8]];
        assert_eq!(seeded.command, [PARTS, "seed", "--root", "/mnt"]);
        assert!(
            seeded
                .env
                .iter()
                .any(|(n, v)| n == "RONOCTUA_NIX" && v.starts_with('/')),
            "the seed's nix is not a path"
        );
        // THE DISK'S SYSTEM, built from the target's flake after the swap
        // was measured, and installed by its path: nothing is evaluated
        // against the target's store.
        let build = at(&p, key::INST_STEP_BUILD);
        let install = &p[places[7]];
        assert!(at(&p, key::INST_STEP_FLAKE) < build && build < places[7]);
        assert_eq!(p[build].captures, Some(Capture::Toplevel));
        assert!(p[build].command.iter().any(|a| a == "--print-out-paths"));
        assert!(
            p[build]
                .command
                .iter()
                .any(|a| a.starts_with(&format!("path:{TARGET_DIRECTORY}#")))
        );
        assert_eq!(
            install.command[..3],
            ["nixos-install", "--system", TOPLEVEL]
        );
        assert!(install.command.iter().any(|a| a == "--no-channel-copy"));
        assert!(!install.command.iter().any(|a| a == "--flake"));
    }

    /// FAILS IF: the target's `flake.nix` and `flake.lock` can disagree —
    /// a step writes either (on the target or in the working flake) after
    /// the lock, the working flake is aimed after it was locked, something
    /// locks or updates it again, or the target gets anything but the
    /// working pair, whole, after the lock. A lock made for another
    /// `flake.nix` makes every `--no-update-lock-file` evaluation of the
    /// machine (`ronoctua-person verify`, the machine's `apply`) refuse it.
    #[test]
    fn the_targets_flake_and_lock_agree() {
        let pair = |dir: &str| {
            [
                format!("{dir}/{}", crate::release::TEMPLATE_FLAKE),
                format!("{dir}/{LOCK_FILE}"),
            ]
        };
        let touched: Vec<String> = pair(WORK_DIRECTORY)
            .into_iter()
            .chain(pair(TARGET_DIRECTORY))
            .collect();
        for aimed in [false, true] {
            let p = steps(&answers(), &Origin { aimed, ..origin() });
            let lock = at(&p, key::INST_STEP_LOCK);
            assert_eq!(
                p[lock].command,
                [
                    "nix",
                    "flake",
                    "lock",
                    "--offline",
                    &format!("path:{WORK_DIRECTORY}")
                ]
            );
            // AIMED BEFORE THE LOCK, and only then.
            match p.iter().position(|s| s.label == key::INST_STEP_AIM) {
                Some(aim) => {
                    assert!(aimed, "a release's install was aimed");
                    assert!(at(&p, key::INST_STEP_TEMPLATE) < aim && aim < lock);
                    assert_eq!(p[aim].command, [WRITE.to_owned(), aimed_flake()]);
                }
                None => assert!(!aimed, "an aimed install was not aimed"),
            }
            // NOTHING AFTER THE LOCK touches either file, nor locks again.
            for s in &p[lock + 1..] {
                for a in &s.command {
                    assert!(
                        !touched.iter().any(|t| a == t),
                        "`{}` writes {a} after the lock (aimed: {aimed})",
                        s.label
                    );
                }
                let words: Vec<&str> = s.command.iter().map(String::as_str).collect();
                assert!(
                    !words
                        .windows(2)
                        .any(|w| w[0] == "flake" && (w[1] == "lock" || w[1] == "update")),
                    "`{}` locks again (aimed: {aimed})",
                    s.label
                );
            }
            // THE TARGET GETS THE WORKING PAIR, WHOLE, after the lock.
            let copied = at(&p, key::INST_STEP_FLAKE);
            assert!(copied > lock);
            assert_eq!(
                p[copied].command[p[copied].command.len() - 2..],
                [WORK_DIRECTORY.to_owned(), TARGET_DIRECTORY.to_owned()]
            );
        }
    }

    /// FAILS IF: an aimed machine does not keep the tree its flake names
    /// (a collection would leave `/etc/nixos` naming a store path that is
    /// gone), or a release's machine keeps one it does not need.
    #[test]
    fn an_aimed_machine_keeps_its_tree() {
        let mut r = answers();
        assert!(!host_nix(&r).contains("extraDependencies"));
        r.aimed = true;
        let kept = host_nix(&r);
        assert!(
            kept.contains("system.extraDependencies = [ inputs.self.outPath ];"),
            "{kept}"
        );
        // INSIDE the file's attribute set, as a module of its own: the
        // file's header does not change.
        assert!(kept.contains("{ ... }:\n{\n"));
        assert!(kept.trim_end().ends_with('}'));
    }

    /// FAILS IF: the plan spells the system's name instead of asking the
    /// machine's flake for it, or uses the mark anywhere but where the two
    /// builds and `disko` take the flake.
    #[test]
    fn the_system_is_asked_of_the_machines_flake() {
        let p = steps(&answers(), &origin());
        let ask = &p[at(&p, key::INST_STEP_ATTRIBUTE)];
        assert_eq!(ask.captures, Some(Capture::Attribute));
        assert!(
            ask.command
                .iter()
                .any(|a| a.ends_with("#nixosConfigurations"))
        );
        let marked: Vec<&'static str> = p
            .iter()
            .filter(|s| s.command.iter().any(|a| a.contains(ATTRIBUTE)))
            .map(|s| s.label)
            .collect();
        assert_eq!(
            marked,
            [
                key::INST_STEP_BUILD_FIRST,
                key::INST_STEP_PARTITION,
                key::INST_STEP_BUILD
            ]
        );
        assert!(at(&p, key::INST_STEP_ATTRIBUTE) < at(&p, key::INST_STEP_PARTITION));
        for s in &p {
            for a in &s.command {
                assert!(
                    !a.contains(&format!("#{}", target_user())),
                    "the system's name spelled: {a}"
                );
            }
        }
    }

    /// FAILS IF: a machine started from BIOS is written without the line
    /// that makes it boot GRUB — it would get systemd-boot, an EFI program
    /// its firmware cannot run — or a UEFI machine gets the line.
    #[test]
    fn a_bios_machine_is_written_so() {
        let mut r = answers();
        assert!(!host_nix(&r).contains("ronoctua.firmware"));
        r.bios = true;
        assert!(host_nix(&r).contains("  ronoctua.firmware = \"bios\";\n"));
    }

    #[test]
    fn the_host_nix_names_the_machines_theme() {
        // The default, spelled out: the file is where somebody looks to find
        // out what their machine is. The id comes from the generated table,
        // never from a literal.
        let s = host_nix(&answers());
        let line = format!(
            "  ronoctua.theme = \"{}\";",
            ronoctua_tokens::theme::default_one().id()
        );
        assert!(s.contains(&line), "the theme line is missing:\n{s}");
    }

    /// FAILS IF: the measured swap does not reach `host.nix`, one is
    /// written when nothing was measured, or a swap capped below the RAM
    /// keeps a resume offset.
    #[test]
    fn the_swap_reaches_host_nix_only_when_measured() {
        let mut r = answers();
        assert!(!host_nix(&r).contains("swapSize"));
        r.swap = Some(crate::swap::Swap {
            gib: 16,
            hibernates: true,
        });
        assert!(host_nix(&r).contains("  ronoctua.swapSize = \"16G\";"));
        assert!(host_nix(&r).contains("resume_offset=123456"));
        r.swap = Some(crate::swap::Swap {
            gib: 8,
            hibernates: false,
        });
        let capped = host_nix(&r);
        assert!(capped.contains("  ronoctua.swapSize = \"8G\";"));
        assert!(!capped.contains("resume_offset="), "{capped}");
        // AND HIBERNATION IS SWITCHED OFF, as an option the machine reads —
        // not a comment over a resume device that is still there.
        assert!(
            capped.contains("  ronoctua.hibernation = false;"),
            "{capped}"
        );
        // A swap that holds the RAM, with its offset: hibernation is on (the
        // option's default), and nothing switches it off.
        r.swap = Some(crate::swap::Swap {
            gib: 16,
            hibernates: true,
        });
        assert!(!host_nix(&r).contains("ronoctua.hibernation"));
    }

    #[test]
    fn the_host_nix_carries_no_secret() {
        // Section 9 forbids personal data in a file. This one goes into backups.
        let r = answers();
        let s = host_nix(&r);
        for secret in [&r.password, &r.passphrase, &r.belongings_passphrase] {
            assert!(!s.contains(secret.as_str()), "a secret leaked:\n{s}");
        }
        assert!(s.contains("noctua-portatil"));
        assert!(s.contains("/dev/nvme0n1"));
    }

    /// FAILS IF: a secret — any of the four passwords — reaches a command
    /// line or a step's variables: `/proc/<pid>/cmdline` and `environ`
    /// outlive the step that leaked them in the installer's own log.
    #[test]
    fn no_secret_goes_through_argv_or_the_environment() {
        let mut r = answers();
        r.wifi_ssid = "home-5g".into();
        r.wifi_password = "Zq7-radio-password".into();
        let every: Vec<Step> = [
            r.clone(),
            Answers {
                wifi_ssid: r.wifi_ssid.clone(),
                wifi_password: r.wifi_password.clone(),
                ..leaving(false)
            },
            Answers {
                wifi_ssid: r.wifi_ssid.clone(),
                wifi_password: r.wifi_password.clone(),
                ..leaving(true)
            },
        ]
        .iter()
        .flat_map(|a| steps(a, &origin()))
        .collect();
        for p in every {
            let shown = p.shown();
            for secret in [
                &r.password,
                &r.passphrase,
                &r.wifi_password,
                &r.belongings_passphrase,
            ] {
                assert!(!shown.contains(secret.as_str()), "a secret in {shown}");
            }
        }
        assert!(!host_nix(&r).contains("Zq7-radio-password"));
        assert!(
            !host_nix(&r).contains("home-5g"),
            "the SSID leaked into host.nix"
        );
        // AND IT IS NOT IN THE PROFILE — `psk-flags=1` leaves it to the agent,
        // which opens it from the TPM.
        let profile = wifi_profile(&r);
        assert!(!profile.contains(&r.wifi_password), "{profile}");
        assert!(profile.contains("psk-flags=1"), "{profile}");
        assert!(profile.contains(&format!("ssid={}", r.wifi_ssid)));
    }

    #[test]
    fn with_no_network_chosen_the_profile_step_does_not_exist() {
        // Without this the step would write a profile with an empty `id=` — which
        // NetworkManager accepts and never uses.
        let r = answers();
        assert!(r.wifi_ssid.is_empty());
        let none = steps(&r, &origin());
        assert!(
            !none
                .iter()
                .any(|p| p.command.get(1).map(String::as_str) == Some(WIFI_PROFILE_PATH)),
            "it would write a profile for no network at all"
        );
        assert!(
            !none
                .iter()
                .any(|x| x.command.first().map(String::as_str) == Some(SEAL))
        );
        let mut with_it = answers();
        with_it.wifi_ssid = "home-5g".into();
        with_it.wifi_password = "Zq7-radio-password".into();
        let p = steps(&with_it, &origin());
        let profile = p
            .iter()
            .position(|x| x.command.get(1).map(String::as_str) == Some(WIFI_PROFILE_PATH))
            .expect("a network was chosen and the profile would not be written");
        let sealing = p
            .iter()
            .position(|x| x.command.first().map(String::as_str) == Some(SEAL))
            .expect("the profile would be written with no password stored at all");
        assert_eq!(sealing, profile + 1);
        assert_eq!(
            p[sealing].command.get(2).map(String::as_str),
            Some("home-5g")
        );
    }

    #[test]
    fn with_no_offset_hibernation_is_left_out() {
        // Writing `resume_offset=0` would be worse: the kernel accepts it and
        // hibernation fails in use, not at boot.
        let mut r = answers();
        r.resume_offset = None;
        let s = host_nix(&r);
        assert!(!s.contains("resume_offset="), "{s}");
        assert!(s.contains("  ronoctua.hibernation = false;"), "{s}");
    }

    #[test]
    fn the_luks_partition_knows_about_nvme() {
        assert_eq!(luks_partition("/dev/nvme0n1"), "/dev/nvme0n1p2");
        assert_eq!(luks_partition("/dev/mmcblk0"), "/dev/mmcblk0p2");
        assert_eq!(luks_partition("/dev/sda"), "/dev/sda2");
        assert_eq!(luks_partition("/dev/vda"), "/dev/vda2");
    }

    /// THE ZONE IS SEEDED ON THE MACHINE, and never written into the flake:
    /// `time.timeZone` in `host.nix` starts timedated with
    /// `NIXOS_STATIC_TIMEZONE=1`, and `LEADER s l` could not change it.
    #[test]
    fn the_zone_is_seeded_on_the_machine_and_not_pinned_in_the_flake() {
        let r = answers();
        let written = host_nix(&r);
        assert!(
            !written.contains("time.timeZone"),
            "the zone was pinned in the flake:\n{written}"
        );
        let p = steps(&r, &origin());
        let seeding = at(&p, key::INST_STEP_ZONE);
        assert!(p[seeding].command.contains(&LOCALTIME_PATH.to_owned()));
        assert!(
            p[seeding]
                .command
                .iter()
                .any(|a| a == &format!("{ZONEINFO_ON_TARGET}/{}", r.timezone))
        );
        // AFTER `nixos-install`, which is what creates `/mnt/etc`.
        assert!(seeding > at(&p, key::INST_STEP_INSTALL));
    }

    /// NOTHING ENROLS THE TPM: without a measured UKI those PCRs are never
    /// extended, so the sealing held for any volume at all.
    #[test]
    fn nothing_enrols_the_tpm() {
        let p = steps(&answers(), &origin());
        assert!(
            !p.iter()
                .any(|x| x.command.iter().any(|a| a.contains("cryptenroll"))),
            "a step enrols the TPM"
        );
    }

    #[test]
    fn disko_reads_a_host_nix_that_already_exists() {
        // `disko` needs `ronoctua.disk`, which comes from the `host.nix`, and
        // the `host.nix` cannot be written in `/mnt` (it does not exist yet)
        // nor in the ISO's distro (a store link, read-only).
        let p = steps(&answers(), &origin());
        let idx = |prog: &str| {
            p.iter()
                .position(|x| x.command.first().map(String::as_str) == Some(prog))
                .unwrap_or_else(|| panic!("no step runs `{prog}`"))
        };
        let host_work = p
            .iter()
            .position(|x| x.command.contains(&HOST_NIX_WORK_PATH.to_owned()))
            .expect("nobody writes the working host.nix");
        let working_copy = at(&p, key::INST_STEP_TEMPLATE);
        assert_eq!(
            p[working_copy].command.get(3),
            Some(&origin().template()),
            "the working flake is not the template"
        );
        assert!(working_copy < host_work, "host.nix before the flake");
        assert!(host_work < idx("disko"), "disko before host.nix");
        // And disko evaluates the WORKING directory, never `/mnt`.
        let disko_target = p[idx("disko")]
            .command
            .iter()
            .find(|a| a.ends_with(&format!("#{ATTRIBUTE}")))
            .expect("disko takes no flake");
        assert!(disko_target.starts_with(WORK_DIRECTORY), "{disko_target}");
        assert!(
            p[idx("disko")]
                .command
                .contains(&"--yes-wipe-all-disks".to_owned()),
            "disko asks its own question, and nobody can answer it"
        );
    }

    #[test]
    fn the_flake_goes_to_the_target_before_nixos_install() {
        let p = steps(&answers(), &origin());
        let disko = at(&p, key::INST_STEP_PARTITION);
        let made = at(&p, key::INST_STEP_TARGET);
        let copied = at(&p, key::INST_STEP_FLAKE);
        let installs = at(&p, key::INST_STEP_INSTALL);
        assert!(disko < made && made < copied && copied < installs);
        assert!(p[copied].command.contains(&TARGET_DIRECTORY.to_owned()));
        for cmd in p
            .iter()
            .filter(|x| x.command.first().map(String::as_str) == Some("cp"))
        {
            // `--no-preserve=mode`: the template comes from the store,
            // read-only. `-T`: an existing destination gains no subfolder.
            assert!(cmd.command.contains(&"--no-preserve=mode".to_owned()));
            assert!(
                cmd.command
                    .iter()
                    .any(|a| a.starts_with('-') && a.contains('T')),
                "{:?}",
                cmd.command
            );
        }
    }

    #[test]
    fn partitioning_is_the_first_destructive_step() {
        // The TUI warns before the FIRST destructive one. Warning at every step is
        // not warning.
        for p in every_plan() {
            let first = p.iter().position(|x| x.destructive).unwrap();
            assert_eq!(p[first].label, key::INST_STEP_PARTITION);
        }
    }
}
