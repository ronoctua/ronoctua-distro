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
//! 1. The disk's passphrase for `disko`, and the machine's flake made in a
//!    working directory: the TEMPLATE (`templates/machine`), `host.nix`, the
//!    hardware report, the lock, and the name of the system it builds — asked
//!    of the flake itself, never spelled here.
//! 2. The person's declarations, BEFORE the disk is touched: the plain part
//!    of their repository copied out (`ronoctua-belongings plain`, with the
//!    passphrase: it opens `key.age`, holds the copy to the person's signing
//!    key and leaves that key for the next step), the key PINNED in the
//!    working flake's `local/` (`ronoctua-person pin` — the key the
//!    passphrase opened, never the repository's copy), the copy applied into
//!    its `person/` by the same root applier the machine runs
//!    (`ronoctua-person apply`, which takes only what that key signed), and
//!    each entry tried (`ronoctua-person verify`, which holds what does not
//!    build). A declaration that cannot be fetched is found here, with the
//!    disk intact.
//! 3. `disko`, the flake to the target, the password, the Wi-Fi, and
//!    `nixos-install` — the machine is built WITH the person's programs.
//! 4. The person's home: the access to the repository (the deploy key or the
//!    token), the consent for its host, `belongings.json`, and the restore
//!    (or, the repository being empty, its making), run AS the person. The
//!    repository made here has a new signing key, and the machine pins it
//!    from what `init` left (`ronoctua-person pin --owner`).
//!
//! # The person's binary cache
//!
//! When the person's options name a binary cache (their own Cachix cache,
//! `ronoctua_person::options::BinaryCache`, read from the working flake's
//! `person/` AFTER the root applier validated it), `nixos-install` already
//! downloads from it: the cache and its key on its command line
//! ([`BINARY_CACHE_OPTIONS`], filled in by the executor), and for a PRIVATE
//! cache the token too — read out of the sealed part with the passphrase
//! before the disk is touched ([`BINARY_CACHE_TOKEN`]), in a netrc of the
//! installer's own that `nixos-install` is pointed at, and copied to the
//! machine's root netrc after it ([`BINARY_CACHE_NETRC`]). The token is never
//! an argument: the belongings' CLI writes it to a file, and the netrc is a
//! file.

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

    /// Where the plain part of the repository is copied out: the staged
    /// tree `ronoctua-person apply` reads (`person/`, `brain-modules/`).
    #[must_use]
    pub fn plain(&self) -> String {
        format!("{}/plain", self.root)
    }

    /// Root's lock for the person's applier and verifier while they work
    /// on the working flake.
    #[must_use]
    pub fn lock(&self) -> String {
        format!("{}/person.lock", self.root)
    }

    /// The restore's runtime folder, the PERSON'S (`0700`): the
    /// passphrase's file it reads and deletes, its session.
    #[must_use]
    pub fn person_runtime(&self) -> String {
        format!("{}/person-run", self.root)
    }

    /// The passphrase's file the copy of the plain part reads (and deletes),
    /// in the runtime folder of the belongings' children run as root.
    #[must_use]
    pub fn plain_passphrase(&self) -> String {
        format!("{}/plain-passphrase", self.runtime())
    }

    /// Where the copy of the plain part leaves the signing key the
    /// passphrase opened, for the pin — beside the copy, never in it.
    #[must_use]
    pub fn signer(&self) -> String {
        format!("{}/{}", self.root, ronoctua_belongings::plain::SIGNER_FILE)
    }

    /// The answers a hop hands the release's installer (`0600`, deleted by
    /// the one that reads it).
    #[must_use]
    pub fn answers(&self) -> String {
        format!("{}/answers", self.root)
    }

    /// The person's binary cache's folder (`0700`): the passphrase the token
    /// is taken with, the token for the moment it is read, and the netrc
    /// `nixos-install` reads a private cache through.
    #[must_use]
    pub fn binary_cache(&self) -> String {
        format!("{}/binary-cache", self.root)
    }

    /// The runtime folder `take` reads the passphrase from.
    #[must_use]
    pub fn binary_cache_runtime(&self) -> String {
        format!("{}/run", self.binary_cache())
    }

    /// Where `take` writes the token, deleted as soon as the netrc is made.
    #[must_use]
    pub fn binary_cache_token(&self) -> String {
        format!("{}/token", self.binary_cache())
    }

    /// The installer's netrc for a private cache.
    #[must_use]
    pub fn binary_cache_netrc(&self) -> String {
        format!("{}/netrc", self.binary_cache())
    }
}

/// An internal command: the push token of the person's PRIVATE binary cache,
/// read out of the sealed part (`ronoctua-belongings take`, the passphrase
/// in a file) into the installer's netrc ([`Scratch::binary_cache_netrc`]).
/// It does nothing for a public cache, or with none.
pub const BINARY_CACHE_TOKEN: &str = "binary-cache-token";

/// The mark in `nixos-install`'s command where the person's binary cache
/// goes — `--option extra-substituters …` and its key, and the installer's
/// netrc for a private one — filled in by the executor from the working
/// flake's validated `person/options.json`; nothing when there is no cache.
pub const BINARY_CACHE_OPTIONS: &str = "{binary-cache}";

/// An internal command: the installer's netrc to the machine's root netrc
/// ([`target_netrc`]), `0600`, root's; nothing when there is none.
pub const BINARY_CACHE_NETRC: &str = "binary-cache-netrc";

/// The machine's root netrc, on the target (`ronoctua_cachix::NETRC`).
#[must_use]
pub fn target_netrc() -> String {
    format!("/mnt{}", ronoctua_cachix::NETRC)
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

/// [`Scratch::plain`] of the installer's.
#[must_use]
pub fn plain_dir() -> String {
    Scratch::installer().plain()
}

/// [`Scratch::lock`] of the installer's.
#[must_use]
pub fn person_lock() -> String {
    Scratch::installer().lock()
}

/// [`Scratch::person_runtime`] of the installer's.
#[must_use]
pub fn person_runtime() -> String {
    Scratch::installer().person_runtime()
}

/// [`Scratch::plain_passphrase`] of the installer's.
#[must_use]
pub fn plain_passphrase() -> String {
    Scratch::installer().plain_passphrase()
}

/// [`Scratch::signer`] of the installer's.
#[must_use]
pub fn scratch_signer() -> String {
    Scratch::installer().signer()
}

/// Where the person's `init` leaves the signing key it made, in their
/// runtime folder (theirs, `0600`).
#[must_use]
pub fn person_signer() -> String {
    format!(
        "{}/{}",
        person_runtime(),
        ronoctua_belongings::plain::SIGNER_FILE
    )
}

/// The mark in a command where the person's uid goes — read from the
/// target's `/etc/passwd` by the executor once `nixos-install` made them.
pub const PERSON_UID: &str = "{person-uid}";

/// The passphrase's file the restore reads.
#[must_use]
pub fn person_passphrase() -> String {
    format!(
        "{}/{}",
        person_runtime(),
        ronoctua_belongings::places::PASSPHRASE_FILE
    )
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
    let empty = r.belongings_empty == Some(true);
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
    // THE LOCK: the distro's revision the machine is built from, and every
    // input of the distro's exactly as its release pinned them.
    sequence.push(Step::root(
        key::INST_STEP_LOCK,
        vec![
            "nix".into(),
            "flake".into(),
            "lock".into(),
            work_flake.clone(),
        ],
        false,
    ));
    // THE SYSTEM'S NAME, asked of the flake: `nixosConfigurations` has the
    // one the machine builds (`ronoctua.machine.attr`), and spelling it here
    // would be the person's name written a second time.
    sequence.push(Step {
        captures: Some(Capture::Attribute),
        ..Step::root(
            key::INST_STEP_ATTRIBUTE,
            vec![
                "nix".into(),
                "eval".into(),
                "--json".into(),
                format!("{work_flake}#nixosConfigurations"),
                "--apply".into(),
                "builtins.attrNames".into(),
            ],
            false,
        )
    });
    if !empty {
        // THE PERSON'S DECLARATIONS, before the disk is touched — see the
        // module's header. An empty repository has none.
        //
        // THE PASSPHRASE FOR THE COPY, in a `0600` file of root's that the
        // copy deletes: it opens `key.age`, and the key it holds is the one
        // the machine pins.
        sequence.push(Step::root(
            key::INST_STEP_PLAIN_PASSPHRASE,
            vec![WRITE.into(), plain_passphrase()],
            false,
        ));
        let plain = vec![
            BELONGINGS.into(),
            "plain".into(),
            "--repository".into(),
            r.belongings.clone(),
            "--keys".into(),
            scratch_keys(),
            "--out".into(),
            plain_dir(),
            "--passphrase".into(),
            plain_passphrase(),
            "--signer".into(),
            scratch_signer(),
            "--json".into(),
        ];
        // THE ADDRESS is on this command line because that is how the CLI
        // takes it, and it is no secret; the access and the passphrase are
        // FILES (`--keys`, `--passphrase`).
        sequence.push(Step {
            env: home_env(&scratch_home()),
            ..Step::root(key::INST_STEP_PLAIN, plain, false)
        });
        // THE KEY THE PASSPHRASE OPENED, pinned in the working flake's
        // `local/` before anything of the person's is applied there — so
        // the applier takes only what that key signed.
        sequence.push(Step {
            only_if: Some(scratch_signer()),
            ..Step::root(
                key::INST_STEP_PIN,
                vec![
                    PERSON.into(),
                    "pin".into(),
                    "--pin".into(),
                    scratch_signer(),
                    "--machine".into(),
                    WORK_DIRECTORY.into(),
                    "--lock".into(),
                    person_lock(),
                ],
                false,
            )
        });
        sequence.push(Step {
            only_if: Some(format!("{}/{}", plain_dir(), ronoctua_person::tree::PERSON)),
            ..Step::root(
                key::INST_STEP_PERSON,
                vec![
                    PERSON.into(),
                    "apply".into(),
                    "--staged".into(),
                    plain_dir(),
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
        sequence.push(Step {
            env: vec![("RONOCTUA_NIX".to_owned(), origin.nix.clone())],
            only_if: Some(format!(
                "{WORK_DIRECTORY}/{}",
                ronoctua_person::names::MACHINE_PERSON
            )),
            ..Step::root(
                key::INST_STEP_VERIFY,
                vec![
                    PERSON.into(),
                    "verify".into(),
                    "--machine".into(),
                    WORK_DIRECTORY.into(),
                    "--lock".into(),
                    person_lock(),
                ],
                false,
            )
        });
        // THE PRIVATE CACHE'S TOKEN, before the disk is touched — from the
        // options root's applier just validated (and so after the plain
        // part's verification), for `nixos-install` to read the cache with.
        sequence.push(Step {
            only_if: Some(format!(
                "{WORK_DIRECTORY}/{}/{}",
                ronoctua_person::names::MACHINE_PERSON,
                ronoctua_person::names::OPTIONS
            )),
            ..Step::root(
                key::INST_STEP_CACHE_TOKEN,
                vec![BINARY_CACHE_TOKEN.into()],
                false,
            )
        });
    }
    sequence.push(Step::root(
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
        // the person's `person/` and root's `local/` — and its `flake.nix`
        // with the lock made for it, aimed or not: the pair agrees.
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
    sequence.push(Step::root(
        key::INST_STEP_INSTALL,
        vec![
            "nixos-install".into(),
            "--flake".into(),
            format!("{TARGET_DIRECTORY}#{ATTRIBUTE}"),
            "--no-root-password".into(),
            // THE PERSON'S BINARY CACHE, when their options name one.
            BINARY_CACHE_OPTIONS.into(),
        ],
        true,
    ));
    if !empty {
        // THE PRIVATE CACHE'S NETRC, to the machine's root: after
        // `nixos-install`, which made the target's `/var`.
        sequence.push(Step {
            only_if: Some(Scratch::installer().binary_cache_netrc()),
            ..Step::root(
                key::INST_STEP_CACHE_NETRC,
                vec![BINARY_CACHE_NETRC.into()],
                true,
            )
        });
    }
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
    // NO STEP POINTS THE FLAKE BACK at the release: an aimed machine keeps
    // the pair it was built from — see `Origin`.
    // THE PERSON'S HOME, after `nixos-install` made them (and so the uid
    // and gid every file is given, read from the target's `/etc/passwd`).
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
        // THE SETTINGS BEFORE THE RESTORE, which reads the repository and
        // its access from them — so the restore's command line names
        // neither. The address alone and how it is reached; no key, no
        // token, no passphrase.
        key::INST_STEP_BELONGINGS,
        vec![WRITE.into(), belongings_path()],
        true,
    ));
    sequence.push(Step::root(
        key::INST_STEP_PASSPHRASE,
        vec![WRITE.into(), person_passphrase()],
        true,
    ));
    let verb = if empty { "init" } else { "restore" };
    sequence.push(Step {
        runs_as: Runner::Person,
        env: home_env(&target_home()),
        ..Step::root(
            if empty {
                key::INST_STEP_INIT
            } else {
                key::INST_STEP_RESTORE
            },
            vec![
                BELONGINGS.into(),
                verb.into(),
                "--home".into(),
                target_home(),
                "--runtime".into(),
                person_runtime(),
                // The NEW machine's name, which the index and `fleet.json`
                // remember — not the installer's.
                "--machine".into(),
                r.hostname.clone(),
                "--progress".into(),
            ],
            true,
        )
    });
    if empty {
        // THE REPOSITORY JUST MADE has a new signing key: the machine pins
        // the one `init` made with the passphrase, from the file it left in
        // the person's runtime folder (theirs: `--owner`, their uid on the
        // target).
        sequence.push(Step {
            only_if: Some(person_signer()),
            ..Step::root(
                key::INST_STEP_PIN,
                vec![
                    PERSON.into(),
                    "pin".into(),
                    "--pin".into(),
                    person_signer(),
                    "--owner".into(),
                    PERSON_UID.into(),
                    "--machine".into(),
                    TARGET_DIRECTORY.into(),
                    "--lock".into(),
                    person_lock(),
                ],
                true,
            )
        });
    }
    // THERE IS NO TPM STEP, and its absence is the decision rather than an
    // omission: `systemd-cryptenroll --tpm2-pcrs=7+11+12+15` could not deliver
    // what it named without a measured UKI. `nix/disko/default.nix` carries
    // the whole argument. The Wi-Fi key IS still sealed in the TPM — see
    // `SEAL`, a different mechanism and a different question.
    sequence
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
            resume_offset: Some(123_456),
            belongings: "git@example.org:someone/belongings.git".into(),
            belongings_access: format!("{}/deploy", scratch_keys()),
            belongings_passphrase: "Qw4-belongings-passphrase".into(),
            belongings_empty: Some(false),
            swap: None,
            aimed: false,
        }
    }

    fn origin() -> Origin {
        Origin {
            distro: "/etc/ronoctua/distro".into(),
            aimed: false,
            nix: "/run/current-system/sw/bin/nix".into(),
        }
    }

    /// FAILS IF: the private cache's token is read after the disk is
    /// touched or before root's applier validated the options, an empty
    /// repository reads one, `nixos-install` has no place for the cache, or
    /// the root netrc is written before `nixos-install` made the target —
    /// or a step names the token itself.
    #[test]
    fn the_persons_cache_reaches_nixos_install_and_its_netrc_the_target() {
        let plan = steps(&answers(), &origin());
        let token = at(&plan, key::INST_STEP_CACHE_TOKEN);
        assert!(
            at(&plan, key::INST_STEP_PERSON) < token && at(&plan, key::INST_STEP_VERIFY) < token
        );
        assert!(token < at(&plan, key::INST_STEP_PARTITION));
        assert_eq!(plan[token].command, [BINARY_CACHE_TOKEN]);
        assert!(!plan[token].destructive);
        let install = at(&plan, key::INST_STEP_INSTALL);
        assert!(
            plan[install]
                .command
                .iter()
                .any(|a| a == BINARY_CACHE_OPTIONS)
        );
        let netrc = at(&plan, key::INST_STEP_CACHE_NETRC);
        assert!(install < netrc);
        assert_eq!(
            plan[netrc].only_if.as_deref(),
            Some(Scratch::installer().binary_cache_netrc().as_str())
        );
        assert_eq!(target_netrc(), "/mnt/var/lib/ronoctua-binary-cache/netrc");
        let empty = Answers {
            belongings_empty: Some(true),
            ..answers()
        };
        let made = steps(&empty, &origin());
        assert!(!made.iter().any(|s| s.label == key::INST_STEP_CACHE_TOKEN));
        assert!(!made.iter().any(|s| s.label == key::INST_STEP_CACHE_NETRC));
        for step in &plan {
            assert!(
                !step.command.iter().any(|a| a.contains("eyJ")),
                "{:?}",
                step.command
            );
        }
    }

    fn at(p: &[Step], label: &str) -> usize {
        p.iter()
            .position(|s| s.label == label)
            .unwrap_or_else(|| panic!("no step `{label}`"))
    }

    /// FAILS IF: the belongings' address reaches `host.nix`, the settings
    /// are written anywhere but the person's home or before
    /// `nixos-install` made them, or the restore runs before the access, the
    /// consent, the settings and the passphrase it reads are in place.
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
        let restore = at(&plan, key::INST_STEP_RESTORE);
        for before in [
            key::INST_STEP_ACCESS,
            key::INST_STEP_CONSENT,
            key::INST_STEP_BELONGINGS,
            key::INST_STEP_PASSPHRASE,
        ] {
            let b = at(&plan, before);
            assert!(installed < b && b < restore, "`{before}` out of order");
        }
        assert!(
            !host_nix(&r).contains("example.org"),
            "the address reached host.nix"
        );
        // THE RESTORE NAMES NEITHER THE REPOSITORY NOR ITS ACCESS: it reads
        // both from the settings and the folder the steps before wrote.
        assert!(
            !plan[restore]
                .command
                .iter()
                .any(|a| a.contains("example.org")),
            "{:?}",
            plan[restore].command
        );
    }

    /// FAILS IF: the person's declarations are fetched, applied or tried
    /// after the disk is touched, out of their order, against anything but
    /// the working flake, or with the access's content on a command line.
    #[test]
    fn the_declarations_are_applied_to_the_flake_before_the_disk_is_touched() {
        let p = steps(&answers(), &origin());
        let order = [
            key::INST_STEP_TEMPLATE,
            key::INST_STEP_HOST_NIX,
            key::INST_STEP_FACTER,
            key::INST_STEP_LOCK,
            key::INST_STEP_ATTRIBUTE,
            key::INST_STEP_PLAIN_PASSPHRASE,
            key::INST_STEP_PLAIN,
            key::INST_STEP_PIN,
            key::INST_STEP_PERSON,
            key::INST_STEP_VERIFY,
            key::INST_STEP_PARTITION,
        ];
        let places: Vec<usize> = order.iter().map(|l| at(&p, l)).collect();
        assert!(
            places.windows(2).all(|w| w[0] < w[1]),
            "out of order: {places:?}"
        );
        assert!(!p[..places[10]].iter().any(|s| s.destructive));
        let apply = &p[at(&p, key::INST_STEP_PERSON)];
        assert!(apply.command.contains(&WORK_DIRECTORY.to_owned()));
        assert!(apply.command.contains(&plain_dir()));
        assert_eq!(
            apply.only_if.as_deref(),
            Some(format!("{}/person", plain_dir()).as_str()),
            "a repository with no declarations has nothing to apply"
        );
        let verify = &p[at(&p, key::INST_STEP_VERIFY)];
        assert!(
            verify
                .env
                .iter()
                .any(|(n, v)| n == "RONOCTUA_NIX" && v.starts_with('/')),
            "verify refuses a nix that is not a path"
        );
        let plain = &p[at(&p, key::INST_STEP_PLAIN)];
        assert!(plain.command.contains(&scratch_keys()));
        // THE PASSPHRASE IS A FILE, and the key it opens goes where the pin
        // reads it — beside the copy, never inside what the applier reads.
        let flag = |s: &Step, f: &str| {
            s.command
                .iter()
                .position(|a| a == f)
                .and_then(|i| s.command.get(i + 1))
                .cloned()
        };
        assert_eq!(flag(plain, "--passphrase"), Some(plain_passphrase()));
        assert_eq!(flag(plain, "--signer"), Some(scratch_signer()));
        assert!(
            !plain
                .command
                .iter()
                .any(|a| a.contains("Qw4-belongings-passphrase")),
            "the passphrase reached a command line"
        );
        assert!(!scratch_signer().starts_with(&plain_dir()));
        let pin = &p[at(&p, key::INST_STEP_PIN)];
        assert_eq!(pin.command[..2], [PERSON, "pin"]);
        assert_eq!(flag(pin, "--pin"), Some(scratch_signer()));
        assert_eq!(flag(pin, "--machine").as_deref(), Some(WORK_DIRECTORY));
        assert_eq!(flag(pin, "--owner"), None, "the copy is root's");
        assert_eq!(pin.only_if, Some(scratch_signer()));
        assert!(
            plain
                .env
                .iter()
                .any(|(n, v)| n == "XDG_CONFIG_HOME" && v.starts_with(SCRATCH)),
            "the copy would ask the consent of a home that is not the scratch one"
        );
    }

    /// FAILS IF: an empty repository is restored from (there is nothing),
    /// its declarations fetched, or it is not MADE by the person after the
    /// install.
    #[test]
    fn an_empty_repository_is_made_instead_of_restored() {
        let mut r = answers();
        r.belongings_empty = Some(true);
        let p = steps(&r, &origin());
        for gone in [
            key::INST_STEP_PLAIN,
            key::INST_STEP_PERSON,
            key::INST_STEP_VERIFY,
            key::INST_STEP_RESTORE,
        ] {
            assert!(!p.iter().any(|s| s.label == gone), "{gone}");
        }
        let init = &p[at(&p, key::INST_STEP_INIT)];
        assert_eq!(init.runs_as, Runner::Person);
        assert_eq!(init.command.get(1).map(String::as_str), Some("init"));
        assert!(at(&p, key::INST_STEP_INIT) > at(&p, key::INST_STEP_INSTALL));
        // THE NEW KEY, pinned on the target from what `init` left, after it.
        let pin = at(&p, key::INST_STEP_PIN);
        assert!(pin > at(&p, key::INST_STEP_INIT));
        let pin = &p[pin];
        assert_eq!(pin.runs_as, Runner::Root);
        assert!(pin.command.contains(&TARGET_DIRECTORY.to_owned()));
        assert!(pin.command.contains(&person_signer()));
        assert!(pin.command.contains(&PERSON_UID.to_owned()));
        assert_eq!(pin.only_if, Some(person_signer()));
        assert!(!p.iter().any(|s| s.label == key::INST_STEP_PLAIN_PASSPHRASE));
    }

    /// FAILS IF: the restore runs as root (what it writes would be root's in
    /// the person's home), into a home that is not the target's, under the
    /// installer's name instead of the new machine's, or with roots that
    /// are not that home's.
    #[test]
    fn the_restore_runs_as_the_person_in_their_home() {
        let p = steps(&answers(), &origin());
        let restore = &p[at(&p, key::INST_STEP_RESTORE)];
        assert_eq!(restore.runs_as, Runner::Person);
        assert_eq!(restore.command[..2], [BELONGINGS, "restore"]);
        let flag = |f: &str| {
            restore
                .command
                .iter()
                .position(|a| a == f)
                .and_then(|i| restore.command.get(i + 1))
                .cloned()
        };
        assert_eq!(flag("--home"), Some(target_home()));
        assert_eq!(flag("--runtime"), Some(person_runtime()));
        assert_eq!(flag("--machine").as_deref(), Some("noctua-portatil"));
        for (n, v) in &restore.env {
            assert!(v.starts_with(&target_home()), "{n}={v}");
        }
        assert!(restore.shown().starts_with("[noctua] "));
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
                ["nix", "flake", "lock", &format!("path:{WORK_DIRECTORY}")]
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
    /// machine's flake for it, or uses the mark anywhere but where `disko`
    /// and `nixos-install` take the flake.
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
        assert_eq!(marked, [key::INST_STEP_PARTITION, key::INST_STEP_INSTALL]);
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
        for p in steps(&r, &origin()) {
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
        let p = steps(&answers(), &origin());
        let first = p.iter().position(|x| x.destructive).unwrap();
        assert_eq!(p[first].label, key::INST_STEP_PARTITION);
    }
}
