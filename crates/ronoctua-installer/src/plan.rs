//! What the installer DOES with the answers — section 7.1.
//!
//! Node Nix logic is reimplemented here (invariant 5). The installer
//! writes a `host.nix` with what describes the MACHINE and calls `disko` and
//! `nixos-install`; what knows how to partition, mount and build is Nix.
//!
//! This module's role is that of a form: turning answers into a
//! file and a sequence of commands, both inspectable before running.

use std::fmt::Write as _;

use ronoctua_i18n::key;

use crate::model::Answers;

/// Where the generated `host.nix` is written, on the target system.
///
/// Gitignored in the repository: it contains the hostname and the machine's disk, which
/// are installation data. Password and passphrase do **not** enter it — they go
/// to `sops`//persist, per section 9.
pub const HOST_NIX_PATH: &str = "/mnt/etc/nixos/host.nix";

/// The flake's directory on the target system. It is what `nixos-install` receives.
pub const TARGET_DIRECTORY: &str = "/mnt/etc/nixos";

/// Where the flake is prepared BEFORE the disk exists.
///
/// # Why there is a working directory
///
/// `disko` needs the `host.nix` — it is where `ronoctua.disk` comes from, the
/// disk to erase. But the `host.nix` lives inside the flake, and at that moment the
/// two places the flake could be are impossible:
///
///   - `/mnt/etc/nixos` does not exist yet: it is disko that is going to mount it.
///   - the ISO's `/etc/nixos` is a store link, READ-ONLY (the same trap
///     as `/etc/fonts/conf.d` in CLAUDE.md).
///
/// So the flake is copied here, the `host.nix` is written here, and it is from here
/// that `disko` evaluates. After mounting, all of this goes to the target.
pub const WORK_DIRECTORY: &str = "/tmp/ronoctua-flake";

/// The working directory's `host.nix` — the one `disko` reads.
pub const HOST_NIX_WORK_PATH: &str = "/tmp/ronoctua-flake/host.nix";

/// Where the system's flake comes from, on the ISO.
///
/// The ISO carries the flake's own tree here (`nix/installer/iso.nix`).
/// Without that `nixos-install --flake` would have nothing to build — and that was
/// exactly the state: the plan pointed at `/mnt/etc/nixos` and nothing
/// ever put a `flake.nix` there. Node check saw it, because DRY RUN mode
/// prints the command without running it.
pub const FLAKE_ORIGIN: &str = "/etc/nixos";

/// Where the passphrase is left for `disko` to read.
///
/// `nix/disko/default.nix` expects exactly this path in
/// `passwordFile` — it is not an interactive prompt on purpose, and the comment over
/// there explains why. **Outside the target disk**, and deleted at the end: the
/// passphrase exists only in the TPM and in the head of whoever installed.
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

/// An internal command: seal the Wi-Fi password into the TPM.
///
/// It is not [`WRITE`] because it is not writing content the installer
/// computes — it is running `systemd-creds` against the machine's TPM and storing
/// what it returns. `write_of` would refuse a path it does not know, and
/// that is why this command has a name of its own.
pub const SEAL: &str = "selar-wifi";

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
pub const WRITE: &str = "escrever";

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
        "# GERADO pelo instalador. Descreve esta maquina, e so ela.\n\
         #\n\
         # NAO ha segredo aqui: senha e frase-secreta vivem em /persist, pela\n\
         # secao 9. Este arquivo pode ir para backup sem risco.\n\
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
    // THE LOCALE IS ITS OWN LINE, and not derived from the language: the two
    // answer different questions, and the module stopped tying them.
    let _ = writeln!(s, "  ronoctua.locale = \"{}\";", r.system_locale);
    let _ = writeln!(s, "  time.timeZone = \"{}\";", r.timezone);
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
    let _ = writeln!(s, "  ronoctua.keyboard.options = \"{}\";", r.compose_key);
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
    match r.resume_offset {
        // Without the offset there is NO hibernation. Writing `0` would be worse than omitting it:
        // the kernel would accept it and hibernation would fail in use, not at boot.
        Some(off) => {
            let _ = writeln!(s, "  boot.kernelParams = [ \"resume_offset={off}\" ];");
        }
        None => {
            let _ = writeln!(
                s,
                "  # sem resume_offset: hibernacao desligada nesta maquina"
            );
        }
    }
    let _ = writeln!(s, "}}");
    s
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
}

/// The installation's sequence.
///
/// Returned as DATA, and not executed here: that way the TUI can show everything
/// that will run before running it, and the test can check the order without touching
/// any disk.
#[must_use]
pub fn steps(r: &Answers, flake: &str) -> Vec<Step> {
    let mut sequence = vec![
        Step {
            // BEFORE partitioning, and not alongside: `disko` reads the file in the
            // first second, and a step that fails for a missing file is
            // better discovered before the disk has been touched.
            label: key::INST_STEP_KEY,
            command: vec![WRITE.into(), LUKS_KEY_PATH.into()],
            destructive: false,
        },
        Step {
            // The flake to a WRITABLE place, before anything else. See
            // `WORK_DIRECTORY`.
            label: key::INST_STEP_FLAKE,
            command: vec![
                "cp".into(),
                // `-T`: without it, an existing destination becomes `<destination>/nixos`
                // and `nixos-install` would say there is no `flake.nix`.
                //
                // `-L`: AND THIS ONE IS NOT COSMETIC. `/etc/nixos` is a SYMLINK
                // into the store (`environment.etc."nixos".source`), and `cp -r`
                // does not follow a symlink given on the command line — it
                // copies the LINK. The working directory then WAS the store
                // path, and writing the `host.nix` into it failed with
                // "Read-only file system": EROFS, not a permission error, which
                // is what named the cause.
                //
                // The dry run could never have shown this: it prints the command
                // instead of running it. What found it was the VM test, on the
                // first boot it managed.
                "-rTL".into(),
                // The flake comes from the store, read-only. Copied with the source's
                // mode, writing the `host.nix` inside it would fail with
                // "Permission denied".
                "--no-preserve=mode".into(),
                FLAKE_ORIGIN.into(),
                WORK_DIRECTORY.into(),
            ],
            destructive: false,
        },
        Step {
            // The `host.nix` DISKO reads: `ronoctua.disk` comes from it. Without
            // this step `disko --flake …#noctua` does not know which disk
            // to erase — and the value CANNOT come as a command-line
            // argument, because the following `nixos-install` needs the same
            // value and does not accept one.
            label: key::INST_STEP_HOST_NIX,
            command: vec![WRITE.into(), HOST_NIX_WORK_PATH.into()],
            destructive: false,
        },
        Step {
            // THE HARDWARE REPORT COMES BEFORE `disko`, and it has to.
            //
            // The generated `host.nix` says `facterReport = ./facter.json`, and
            // `disko --flake <work>#noctua` EVALUATES that file. With the report
            // written afterwards — into `/mnt`, which disko has not mounted yet
            // — the evaluation died with "path '…/facter.json' does not exist",
            // and no installation could ever have got past step four.
            //
            // The dry run could not see it: it prints the commands instead of
            // running them. What found it was the VM test, on the first run that
            // reached this far.
            //
            // Into the WORKING directory, which a later step copies whole to the
            // target — so the report travels with the flake instead of needing a
            // second write.
            label: key::INST_STEP_FACTER,
            command: vec![
                "nixos-facter".into(),
                "-o".into(),
                format!("{WORK_DIRECTORY}/facter.json"),
            ],
            // It reads hardware and writes one file outside the target: nothing
            // it does is destructive.
            destructive: false,
        },
        Step {
            label: key::INST_STEP_PARTITION,
            // `disko` does everything: it partitions, creates the LUKS, formats, mounts.
            // Reimplementing any part of that in Rust would violate
            // invariant 5.
            //
            // It evaluates the WORKING directory: `/mnt` only comes into existence
            // after this command.
            command: vec![
                "disko".into(),
                "--mode".into(),
                "destroy,format,mount".into(),
                "--flake".into(),
                format!("{WORK_DIRECTORY}#noctua"),
            ],
            destructive: true,
        },
        // THERE IS NO SWAPFILE STEP, AND ITS ABSENCE IS THE FIX.
        //
        // There used to be one: `btrfs filesystem mkswapfile --size 16G
        // /mnt/swap/arquivo`, and the `resume_offset` was measured from THAT
        // file. Meanwhile `disko` had already created and activated
        // `/swap/swapfile` from `nix/disko/default.nix`, which is the one that
        // reaches `swapDevices`.
        //
        // So the machine got TWO 16 GiB swapfiles — 16 of them wasted — and the
        // offset in `host.nix` pointed at the one the kernel does not use, which
        // is a hibernation that fails in use rather than at boot.
        //
        // The offset is now taken from `disko`'s own file, right after the
        // `disko` step. See `executor::run_command`.
        Step {
            // Now with the `resume_offset`, which only exists after the swap.
            label: key::INST_STEP_HOST_NIX,
            command: vec![WRITE.into(), HOST_NIX_WORK_PATH.into()],
            destructive: true,
        },
        Step {
            // The whole flake to the target, with the complete `host.nix` already.
            label: key::INST_STEP_FLAKE,
            command: vec![
                "cp".into(),
                "-rT".into(),
                "--no-preserve=mode".into(),
                WORK_DIRECTORY.into(),
                TARGET_DIRECTORY.into(),
            ],
            destructive: true,
        },
        Step {
            // BEFORE `nixos-install`: `mutableUsers = false` makes the
            // activator read the `hashedPasswordFile` during the installation. If the
            // file does not exist yet, the user is born with no valid password and
            // it is only found out at the first login.
            label: key::INST_STEP_PASSWORD,
            command: vec![WRITE.into(), PASSWORD_HASH_PATH.into()],
            destructive: true,
        },
        Step {
            label: key::INST_STEP_WIFI,
            command: vec![WRITE.into(), WIFI_PROFILE_PATH.into()],
            destructive: true,
        },
        Step {
            // THE SEALING, right after the profile.
            //
            // The SSID goes in the command and the PASSWORD does not: `/proc/<pid>/cmdline` is
            // readable by any process, and the SSID already goes over the air in
            // the clear on every beacon. The password reaches `systemd-creds`
            // through stdin, inside the executor.
            label: key::INST_STEP_SEAL_WIFI,
            command: vec![
                SEAL.into(),
                WIFI_SECRET_DIRECTORY.into(),
                r.wifi_ssid.clone(),
            ],
            destructive: true,
        },
        Step {
            label: key::INST_STEP_INSTALL,
            command: vec![
                "nixos-install".into(),
                "--flake".into(),
                format!("{flake}#noctua"),
                "--no-root-password".into(),
            ],
            destructive: true,
        },
        Step {
            label: key::INST_STEP_TPM,
            // After `nixos-install`: the sealing measures what is already on the
            // disk, and enrolling before would seal a state that is going to change.
            command: vec![
                "systemd-cryptenroll".into(),
                "--tpm2-device=auto".into(),
                "--tpm2-pcrs=7+11+12+15".into(),
                luks_partition(&r.disk),
            ],
            destructive: false,
        },
    ];
    // THE NETWORK STEP ONLY EXISTS WHEN THERE IS A NETWORK.
    //
    // Filtering here, instead of building the list with an `if`, keeps the sequence
    // readable top to bottom — and keeps the test that checks the ORDER valid
    // for both cases. With no network chosen the step would end up
    // writing a profile with an empty `id=`, which NetworkManager accepts and
    // never connects with.
    if r.wifi_ssid.is_empty() {
        sequence.retain(|p| {
            p.command.get(1).map(String::as_str) != Some(WIFI_PROFILE_PATH)
                && p.command.first().map(String::as_str) != Some(SEAL)
        });
    }
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
            // "secret" and "frase-secret_one", and a test that looked for those
            // words would flag a leak where there is only an explanation.
            password: "Zq7-senha-de-teste".into(),
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
        }
    }

    #[test]
    fn the_host_nix_carries_no_secret() {
        // Section 9 forbids personal data in a file. This one goes into backups.
        let s = host_nix(&answers());
        assert!(!s.contains("Zq7-senha-de-teste"), "a senha vazou:\n{s}");
        assert!(
            !s.contains("Kx9-passphrase-de-teste"),
            "a passphrase vazou:\n{s}"
        );
        assert!(s.contains("noctua-portatil"));
        assert!(s.contains("/dev/nvme0n1"));
    }

    #[test]
    fn the_wifi_password_goes_neither_through_argv_nor_through_host_nix() {
        // `/proc/<pid>/cmdline` is readable by any process on the machine:
        // a password in `argv` is exposed for as long as the command runs. And the
        // `host.nix` goes into backups by design (section 9).
        let mut r = answers();
        r.wifi_ssid = "casa-5g".into();
        r.wifi_password = "Zq7-senha-do-radio".into();

        for p in steps(&r, "/flake") {
            for arg in &p.command {
                assert!(
                    !arg.contains("Zq7-senha-do-radio"),
                    "a senha do Wi-Fi vazou no comando: {:?}",
                    p.command
                );
            }
        }
        assert!(!host_nix(&r).contains("Zq7-senha-do-radio"));
        assert!(
            !host_nix(&r).contains("casa-5g"),
            "o SSID vazou no host.nix"
        );

        // AND IT IS NOT IN THE PROFILE — which is the whole change.
        //
        // The world's default puts `psk=<password>` here, and that is why
        // any backup of /persist carries the password along. With
        // `psk-flags=1` NetworkManager knows the password lives outside the
        // file and asks the agent for it over D-Bus, and the agent opens it from the TPM.
        let profile = wifi_profile(&r);
        assert!(
            !profile.contains(&r.wifi_password),
            "a senha voltou para dentro do perfil:\n{profile}"
        );
        assert!(
            profile.contains("psk-flags=1"),
            "sem `psk-flags`, o NM exige a senha no arquivo:\n{profile}"
        );
        // The SSID stays in the profile, and has to: it is not a secret
        // (it goes over the air on every beacon) and it is how NM finds the network.
        assert!(profile.contains(&format!("ssid={}", r.wifi_ssid)));
    }

    #[test]
    fn with_no_network_chosen_the_profile_step_does_not_exist() {
        // Without this the step would write a profile with an empty `id=` — which
        // NetworkManager accepts and never uses.
        let r = answers();
        assert!(r.wifi_ssid.is_empty());
        assert!(
            !steps(&r, "/flake")
                .iter()
                .any(|p| p.command.get(1).map(String::as_str) == Some(WIFI_PROFILE_PATH)),
            "gravaria um perfil de rede nenhuma"
        );

        let mut with_it = answers();
        with_it.wifi_ssid = "casa-5g".into();
        with_it.wifi_password = "12345678".into();
        assert!(
            steps(&with_it, "/flake")
                .iter()
                .any(|p| p.command.get(1).map(String::as_str) == Some(WIFI_PROFILE_PATH)),
            "escolheu rede e o perfil nao seria gravado"
        );
    }

    #[test]
    fn the_sealing_follows_the_profile_and_carries_the_ssid_and_not_the_password() {
        // The profile step without the sealing one would give a machine with
        // `psk-flags=1` and no stored secret: NM would ask for the password on the
        // first boot, and the installer would have asked for nothing.
        let mut r = answers();
        r.wifi_ssid = "casa-5g".into();
        r.wifi_password = "Zq7-senha-do-radio".into();
        let p = steps(&r, "/flake");

        let selagem = p
            .iter()
            .find(|x| x.command.first().map(String::as_str) == Some(SEAL))
            .expect("o perfil seria gravado sem senha nenhuma guardada");
        assert_eq!(selagem.command.get(2).map(String::as_str), Some("casa-5g"));
        for arg in &selagem.command {
            assert!(
                !arg.contains(&r.wifi_password),
                "a senha entrou no comando: {:?}",
                selagem.command
            );
        }

        // And the two disappear together when there is no network.
        let sem = answers();
        assert!(
            !steps(&sem, "/flake")
                .iter()
                .any(|x| x.command.first().map(String::as_str) == Some(SEAL))
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
        assert!(s.contains("hibernacao desligada"));
    }

    #[test]
    fn the_luks_partition_knows_about_nvme() {
        assert_eq!(luks_partition("/dev/nvme0n1"), "/dev/nvme0n1p2");
        assert_eq!(luks_partition("/dev/mmcblk0"), "/dev/mmcblk0p2");
        assert_eq!(luks_partition("/dev/sda"), "/dev/sda2");
        assert_eq!(luks_partition("/dev/vda"), "/dev/vda2");
    }

    #[test]
    fn the_tpm_sealing_uses_the_four_pcrs() {
        // 7 (Secure Boot), 11 (the UKI), 12 (the kernel's parameters) and 15 (the
        // volume). 15 is what keeps a forged LUKS from unlocking, and it is
        // milestone 8's criterion — without it the rest is theatre.
        let p = steps(&answers(), "/mnt/etc/nixos");
        let tpm = p.iter().find(|x| x.label == "inst-step-tpm").unwrap();
        assert!(
            tpm.command.iter().any(|a| a == "--tpm2-pcrs=7+11+12+15"),
            "{:?}",
            tpm.command
        );
    }

    #[test]
    fn the_sealing_comes_after_the_installation() {
        // Sealing earlier would measure a state that is still going to change, and the unlock
        // would fail on the first boot.
        let p = steps(&answers(), "f");
        let i = p
            .iter()
            .position(|x| x.label == "inst-step-install")
            .unwrap();
        let t = p.iter().position(|x| x.label == "inst-step-tpm").unwrap();
        assert!(t > i, "o TPM foi selado antes do nixos-install");
    }

    #[test]
    fn disko_reads_a_host_nix_that_already_exists() {
        // `disko` needs `ronoctua.disk`, which comes from the `host.nix`. If
        // it ran first, it would evaluate a flake without the file and stop —
        // after the command was already written into the plan and before any
        // warning. And the `host.nix` cannot be written in `/mnt` (it does not exist
        // yet) nor in `/etc/nixos` (a store link, read-only).
        let p = steps(&answers(), "/mnt/etc/nixos");
        let idx = |prog: &str| {
            p.iter()
                .position(|x| x.command.first().map(String::as_str) == Some(prog))
                .unwrap_or_else(|| panic!("nenhum passo roda `{prog}`"))
        };
        let host_trabalho = p
            .iter()
            .position(|x| x.command.contains(&HOST_NIX_WORK_PATH.to_owned()))
            .expect("ninguem escreve o host.nix de trabalho");
        let copia_trabalho = p
            .iter()
            .position(|x| x.command.contains(&WORK_DIRECTORY.to_owned()))
            .expect("ninguem copia o flake para o diretorio de trabalho");

        assert!(copia_trabalho < host_trabalho, "o host.nix antes do flake");
        assert!(host_trabalho < idx("disko"), "o disko antes do host.nix");

        // And disko evaluates the WORKING directory, never `/mnt`: `/mnt` is
        // what it mounts.
        let alvo_do_disko = p[idx("disko")]
            .command
            .iter()
            .find(|a| a.ends_with("#noctua"))
            .expect("o disko nao recebe flake");
        assert!(
            alvo_do_disko.starts_with(WORK_DIRECTORY),
            "o disko avalia `{alvo_do_disko}`, que so existe depois dele"
        );
    }

    #[test]
    fn the_flake_goes_to_the_target_before_nixos_install() {
        // `nixos-install --flake /mnt/etc/nixos#noctua` does not find
        // `flake.nix` if nobody puts it there. That was exactly the state:
        // nothing ever copied the flake, and DRY RUN mode prints the command without
        // running it.
        let p = steps(&answers(), "/mnt/etc/nixos");
        let copia_alvo = p
            .iter()
            .position(|x| x.command.contains(&TARGET_DIRECTORY.to_owned()))
            .expect("ninguem copia o flake para o alvo");
        let instala = p
            .iter()
            .position(|x| x.command.first().map(String::as_str) == Some("nixos-install"))
            .expect("nenhum passo instala");
        let disko = p
            .iter()
            .position(|x| x.command.first().map(String::as_str) == Some("disko"))
            .expect("nenhum passo particiona");

        assert!(disko < copia_alvo, "copia antes de o alvo estar montado");
        assert!(copia_alvo < instala, "o flake chega depois de instalar");

        for cmd in p
            .iter()
            .filter(|x| x.command.first().map(String::as_str) == Some("cp"))
        {
            // `--no-preserve=mode`: the flake comes from the store, read-only.
            // Without this, writing the `host.nix` inside the copy fails with
            // "Permission denied".
            assert!(
                cmd.command.contains(&"--no-preserve=mode".to_owned()),
                "copia sem --no-preserve=mode: {:?}",
                cmd.command
            );
            // `-T`: without it an existing destination gains a subdirectory.
            assert!(
                cmd.command
                    .iter()
                    .any(|a| a.starts_with('-') && a.contains('T')),
                "copia sem -T: {:?}",
                cmd.command
            );
        }
    }

    #[test]
    fn partitioning_is_the_first_destructive_step() {
        // The TUI warns before the FIRST destructive one. Warning at every step is
        // not warning.
        let p = steps(&answers(), "f");
        let first = p.iter().position(|x| x.destructive).unwrap();
        assert_eq!(p[first].label, "inst-step-partition");
    }
}
