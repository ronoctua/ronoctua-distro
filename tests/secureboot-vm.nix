# Signed UKI, Secure Boot and the PCR 15 proof — the rest of milestone 8.
#
# THE CHAIN, AND WHY ALL OF IT IS NECESSARY
# -----------------------------------------
# The clause "a forged LUKS volume does not unlock" depends on PCR 15. But
# `systemd-cryptsetup` only extends PCR 15 when the boot came from a **measured
# UKI** (`ConditionSecurity=measured-uki`). And the UKI is only measured if the
# `systemd-stub` is in the path — which requires lanzaboote, which in turn
# requires Secure Boot keys. No link may be skipped:
#
#   keys -> signed UKI -> firmware measures (PCR 7 and 11) -> measured-uki
#        -> tpm2-measure-pcr starts holding -> PCR 15 extended
#        -> forged volume refused
#
# TWO BOOTS
# ---------
# OVMF starts in *setup mode*: there is firmware capable of Secure Boot, but
# with no key enrolled — and that is why the VM boots with it OFF. The first
# boot enrolls the keys; the second is the one that matters.
#
# The keys are DISPOSABLE and generated at build time
# (`nix/testing/sb-keys.nix`): the private one is in the store, and is
# therefore public. They are no good for a real machine, and the directory
# itself says so.
{
  pkgs,
  lib,
  sbKeys,
  lanzabooteModule,
}:

pkgs.testers.runNixOSTest {
  name = "ronoctua-secureboot";

  nodes.machine = {
    imports = [ lanzabooteModule ];

    virtualisation = {
      tpm.enable = true;
      useBootLoader = true;
      useEFIBoot = true;
      # It gives firmware capable of Secure Boot. It enrolls no key: whoever does
      # that is the first boot, below.
      useSecureBoot = true;

      # OVMF WITH TPM SUPPORT.
      #
      # The default of `useSecureBoot` knows how to do Secure Boot but **does not
      # measure**: with no TCG protocol compiled in, PCRs 7 and 11 stay zeroed even
      # with Secure Boot on and the UKI signed. The symptom is deceptive —
      # `bootctl` says `Secure Boot: enabled` and the loader even reports active
      # PCR banks — but nothing is extended, and the `tpm2-measure-pcr` stays
      # inert.
      efi.OVMF =
        (pkgs.OVMF.override {
          secureBoot = true;
          tpmSupport = true;
        }).fd;
      memorySize = 3072;
      diskSize = 8192;
      emptyDiskImages = [ 512 ];
    };

    # lanzaboote REPLACES systemd-boot: it signs the UKI with the keys and
    # installs it. `mkForce` because the VM module turns systemd-boot on by itself.
    boot.loader.systemd-boot.enable = lib.mkForce false;
    boot.loader.efi.canTouchEfiVariables = true;
    # `/etc/secureboot` is a REAL directory, with writable copies — not links
    # to the store.
    #
    # Two requirements meet here. lanzaboote refuses a `pkiBundle` in the store,
    # and rightly so: a private key in the store is a public key. And `sbctl`
    # uses that same directory as its bank and needs to WRITE to it; with links
    # to the store it stops at "permission denied", in a message that talks
    # about root and not about file permissions.
    #
    # It runs as an activation script because the boot manager's installation —
    # which is what signs the UKI — happens AFTER the activation. On a real
    # machine the keys are created on the computer itself, and section 7.2
    # routes `/etc/secureboot` to `/persist`: otherwise the root's wipe would
    # delete them and it would no longer boot.
    system.activationScripts.testSbKeys = {
      deps = [ "etc" ];
      text = ''
        rm -rf /etc/secureboot
        mkdir -p /etc/secureboot
        cp -rL ${sbKeys}/. /etc/secureboot/
        chmod -R u+rwX /etc/secureboot
      '';
    };

    boot.lanzaboote = {
      enable = true;
      pkiBundle = "/etc/secureboot";
    };
    boot.initrd.systemd.enable = true;

    security.tpm2 = {
      enable = true;
      tctiEnvironment.enable = true;
    };

    environment.systemPackages = with pkgs; [
      cryptsetup
      tpm2-tools
      sbctl
      efitools
    ];
  };

  testScript = ''
    PHRASE = "test-passphrase-12345"


    def pcr(n):
        """A PCR's value, the digits alone."""
        out_text = machine.succeed(f"tpm2_pcrread sha256:{n}")
        for line in out_text.splitlines():
            if line.strip().startswith(f"{n} ") or line.strip().startswith(f"{n}:"):
                return line.split("0x")[1].strip()
        raise AssertionError(f"could not find PCR {n} in:\n{out_text}")


    ZERO = "0" * 64

    machine.start()
    machine.wait_for_unit("multi-user.target")

    with subtest("the first boot is in setup mode, with no Secure Boot"):
        # It is the factory state of an OVMF with no key. If it already came with
        # Secure Boot on, the enrollment below would be unnecessary — and the test
        # would be proving something else.
        state = machine.succeed("bootctl status 2>&1 || true")
        print(state)
        assert "Secure Boot: disabled" in state, state

    with subtest("enrols the keys into the firmware"):
        # `sbctl` reads from its own bank in `/var/lib/sbctl`, and the keys arrived
        # through `/etc/secureboot` as LINKS to the store — which is read-only. It
        # needs writable copies, otherwise it stops at "permission denied" in a
        # message that talks about root.
        machine.succeed("mkdir -p /var/lib/sbctl")
        machine.succeed("cp -rL /etc/secureboot/. /var/lib/sbctl/")
        machine.succeed("chmod -R u+rwX /var/lib/sbctl")

        # The bank's format changed between versions of sbctl; without migrating,
        # it warns and goes on using the old path.
        print(machine.succeed("sbctl setup --migrate 2>&1 || true"))

        # NO `|| true`: if the enrollment fails, here is where the test has to
        # stop. Swallowing the error makes the next subtest fail for a reason that
        # was already known three lines earlier.
        print(machine.succeed(
            "sbctl enroll-keys --yes-this-might-brick-my-machine 2>&1"
        ))
        print(machine.succeed("sbctl status 2>&1 || true"))

    with subtest("after a restart, Secure Boot is ON"):
        machine.shutdown()
        machine.start()
        machine.wait_for_unit("multi-user.target")
        state = machine.succeed("bootctl status 2>&1 || true")
        print(state)
        assert "Secure Boot: enabled" in state, state

    with subtest("the firmware measured the UKI: PCRs 7 and 11 have a value"):
        p7, p11 = pcr(7), pcr(11)
        print(f"PCR 7 = {p7}\nPCR 11 = {p11}")
        assert p7 != ZERO, "PCR 7 is zeroed: the firmware did not measure Secure Boot"
        assert p11 != ZERO, (
            "PCR 11 is zeroed: the UKI was not measured, and `tpm2-measure-pcr` "
            "will stay inert"
        )

    with subtest("unlocking EXTENDS PCR 15"):
        machine.succeed(
            f"echo -n '{PHRASE}' | cryptsetup luksFormat --type luks2 "
            "--pbkdf argon2id --pbkdf-force-iterations 4 --pbkdf-memory 32768 "
            "/dev/vdb -"
        )
        prior = pcr(15)
        machine.succeed(
            f"echo -n '{PHRASE}' | systemd-cryptsetup attach medido /dev/vdb /dev/stdin "
            "headless=yes,tpm2-measure-pcr=yes"
        )
        after = pcr(15)
        machine.succeed("systemd-cryptsetup detach medido")
        print(f"PCR 15 before = {prior}\nPCR 15 after  = {after}")
        assert prior != after, (
            "PCR 15 did not change even with a measured UKI — the sealing does not "
            "would protect against a disk swap"
        )

    with subtest("a secret sealed in PCR 15 only opens with the RIGHT volume"):
        # WHAT PCR 15 REALLY PROTECTS.
        #
        # Copying the whole LUKS header is not forging: it is CLONING. The clone has
        # the same volume key, so PCR 15 measures the same value and it unlocks —
        # correctly, because cryptographically it IS the same volume. I found that
        # out by trying: the clone's `attach` passed.
        #
        # PCR 15 does not stop the initial unlock. It ties everything that comes
        # AFTERWARDS to the identity of the volume that was opened: a secret sealed
        # in it does not open if whoever unlocked was another disk. That is what
        # protects the system from an attacker who swaps the disk for one of their
        # own — they open theirs, and do not reach the rest.
        machine.shutdown()
        machine.start()
        machine.wait_for_unit("multi-user.target")

        # Volume A, the legitimate one. It unlocks and MEASURES.
        machine.succeed(
            f"echo -n '{PHRASE}' | systemd-cryptsetup attach vol-a /dev/vdb /dev/stdin "
            "headless=yes,tpm2-measure-pcr=yes"
        )
        pcr_a = pcr(15)
        print(f"PCR 15 com o volume A = {pcr_a}")

        # A secret sealed IN THAT state.
        machine.succeed(
            "echo -n 'secret-content' | systemd-creds encrypt "
            "--with-key=tpm2 --tpm2-pcrs=15 - /var/secret.cred"
        )
        decrypted = machine.succeed("systemd-creds decrypt /var/secret.cred -").strip()
        assert decrypted == "secret-content", decrypted
        machine.succeed("systemd-cryptsetup detach vol-a")

    with subtest("with ANOTHER volume, the same secret does NOT open"):
        machine.shutdown()
        machine.start()
        machine.wait_for_unit("multi-user.target")

        # Volume B: another disk, another key. It is what an attacker puts in
        # place of yours.
        machine.succeed(
            "echo -n 'attackers-key' | cryptsetup luksFormat --type luks2 "
            "--pbkdf argon2id --pbkdf-force-iterations 4 --pbkdf-memory 32768 "
            "/dev/vdb -"
        )
        machine.succeed(
            "echo -n 'attackers-key' | systemd-cryptsetup attach vol-b /dev/vdb "
            "/dev/stdin headless=yes,tpm2-measure-pcr=yes"
        )
        pcr_b = pcr(15)
        print(f"PCR 15 with volume B = {pcr_b}")
        assert pcr_b != pcr_a, (
            "PCR 15 does not tell the volumes apart — the sealing would not tie "
            "nothing to the disk's identity"
        )

        # And volume A's secret does not open. That is the proof.
        machine.fail("systemd-creds decrypt /var/secret.cred - 2>/dev/null")
        machine.succeed("systemd-cryptsetup detach vol-b")
  '';
}
