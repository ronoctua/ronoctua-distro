# TPM2 sealing — part of the acceptance criterion of milestone 8.
#
# WHAT THIS TEST PROVES
# ---------------------
#   1. the TPM2 seals the volume's key in PCRs 7+11+12+15;
#   2. the legitimate volume unlocks WITHOUT a passphrase, only through the TPM;
#   3. a volume with no sealed token does not unlock.
#
# WHAT IT DOES **NOT** PROVE, AND WHY
# -----------------------------------
# The clause "a forged LUKS volume does not unlock" of section 7.2 depends on
# PCR 15, and PCR 15 **is not extended in this VM**. The boot log says why:
#
#     Make TPM PCR Policy skipped, unmet condition check
#     ConditionSecurity=measured-uki
#
# `systemd-cryptsetup` only measures the volume into PCR 15 when the system
# booted through a **measured UKI** — with `systemd-stub` in the path. The
# `runNixOSTest` VM loads the kernel directly, with no boot manager, so the
# condition is never satisfied and `tpm2-measure-pcr=yes` stays inert.
#
# The honest consequence: subtest 3 above passes because the forged volume
# does not HAVE the sealed token — I reformatted it. That is weaker than the
# real property, in which the attacker COPIES the LUKS header (with the token)
# to another volume; then PCRs 7, 11 and 12 match and only 15 saves it. That
# proof requires the whole boot chain and is in `tests/secureboot-vm.nix`.
#
# AND THE DISTRO DOES NOT INSTALL ANY OF THIS. What this VM builds is its own
# configuration; `docs/ARCHITECTURE.md` carries why the machine asks for its
# passphrase instead. The test stays as the executable record of how the chain
# is wired, for the day somebody wants it.
#
# The TPM is `swtpm`, a real emulator — the same PCR state machine, the same
# policy. What it does not prove is the chip's physical resistance, which is
# not what this criterion asks for.
{
  pkgs,
  lib,
}:

pkgs.testers.runNixOSTest {
  name = "ronoctua-tpm";

  nodes.machine = {
    virtualisation = {
      # An emulated TPM 2.0 (swtpm). Without this there is nothing to seal.
      tpm.enable = true;
      memorySize = 2048;
      emptyDiskImages = [ 512 ];
    };

    security.tpm2 = {
      enable = true;
      tctiEnvironment.enable = true;
    };

    environment.systemPackages = with pkgs; [
      cryptsetup
      tpm2-tools
      systemd
    ];
  };

  testScript = ''
    machine.start()
    machine.wait_for_unit("multi-user.target")

    # The passphrase is a TEST one and lives only in this VM. No real secret
    # goes into a file of the repository (section 9).
    PHRASE = "test-passphrase-12345"

    def enroll(blockDevice, pcrs):
        """Seals the volume's key into the TPM, against the given PCRs."""
        return machine.succeed(
            f"echo -n '{PHRASE}' | systemd-cryptenroll --unlock-key-file=/dev/stdin "
            f"--tpm2-device=auto --tpm2-pcrs={pcrs} {blockDevice} 2>&1"
        )

    with subtest("the TPM2 exists and answers"):
        machine.succeed("test -c /dev/tpmrm0")
        pcrs = machine.succeed("tpm2_pcrread sha256:7,11,12,15")
        print(pcrs)

    with subtest("cria o volume LUKS legitimo"):
        machine.succeed(
            f"echo -n '{PHRASE}' | cryptsetup luksFormat --type luks2 "
            "--pbkdf argon2id --pbkdf-force-iterations 4 --pbkdf-memory 32768 "
            "/dev/vdb -"
        )
        # The UUID is what a forger would copy to pass for the original.
        uuid = machine.succeed("cryptsetup luksUUID /dev/vdb").strip()
        print(f"uuid legitimo: {uuid}")

    with subtest("seals the key into the TPM against all four PCRs"):
        out_text = enroll("/dev/vdb", "7+11+12+15")
        print(out_text)
        # `systemd-cryptenroll` prints the created slot.
        assert "tpm2" in out_text.lower(), out_text

    with subtest("the LEGITIMATE volume unlocks through the TPM, with NO phrase"):
        # With no `--unlock-key-file` and no passphrase on stdin: if it asked for
        # the passphrase, the test would pass with the TPM having done nothing. The
        # proof is in systemd's message, checked below.
        out_text = machine.succeed(
            "systemd-cryptsetup attach legitimo /dev/vdb - "
            "tpm2-device=auto,headless=yes,tpm2-measure-pcr=yes 2>&1"
        )
        print(out_text)
        assert "TPM2 token unlocks volume" in out_text, out_text
        machine.succeed("test -b /dev/mapper/legitimo")
        machine.succeed("systemd-cryptsetup detach legitimo")

    with subtest("PCR 15 is NOT extended with no measured UKI"):
        # This documents a LIMIT of the test, and not a success. If one day it
        # starts changing here, the comment at the top of this file is out of date
        # — and the forged-volume proof can move over here.
        pcr15 = machine.succeed("tpm2_pcrread sha256:15")
        print(pcr15)
        assert "0x00000000" in pcr15, (
            "PCR 15 changed in this VM — the `measured-uki` condition started to "
            "hold, and the forged-volume proof can leave secureboot-vm"
        )
        cond = machine.succeed("journalctl -b --no-pager | grep -c 'measured-uki' || true")
        print(f"lines mentioning measured-uki at boot: {cond.strip()}")

    with subtest("a volume WITHOUT THE TOKEN does not unlock"):
        # Weaker than the real property — see the top of the file. What is proved
        # here is that unlocking depends on the sealed token, not that it resists a
        # copied header.
        machine.succeed(
            "echo -n 'attackers-other-phrase' | cryptsetup luksFormat --type luks2 "
            "--pbkdf argon2id --pbkdf-force-iterations 4 --pbkdf-memory 32768 "
            f"--uuid {uuid} /dev/vdb -"
        )
        machine.fail(
            "systemd-cryptsetup attach forjado /dev/vdb - "
            "tpm2-device=auto,headless=yes"
        )
        machine.fail("test -b /dev/mapper/forjado")
  '';
}
