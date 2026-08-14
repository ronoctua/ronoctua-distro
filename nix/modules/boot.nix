# Secure Boot and TPM2 unlock — section 7.2, milestone 8.
#
# # Why lanzaboote, and not `systemd-boot` with a shim
#
# NixOS's `systemd-boot` signs nothing. With Secure Boot on, the machine
# simply would not boot. lanzaboote signs the UKI (kernel, initrd and cmdline
# in a single binary) with the machine's LOCAL keys — there is no third-party
# key in the path, and "I trust this machine" is literal.
#
# # The four PCRs, and what each one answers
#
#   7  — Secure Boot state and the enrolled keys.
#        Answers: "are the keys still mine?"
#   11 — the UKI.
#        Answers: "are the kernel and the initrd the ones I installed?"
#   12 — the kernel parameters.
#        Answers: "did anyone add `init=/bin/sh`?"
#   15 — the VOLUME.
#        Answers: "is this LUKS mine?"
#
# 15 is the only one an attacker with physical access cannot reproduce by
# swapping the disk: it is extended by systemd-cryptsetup WITH THE KEY of the
# volume. Without it, the other three still match on a forged disk — the
# firmware and the kernel are the same — and the TPM would hand over the
# secret.
#
# That is why `tpm2-measure-pcr=yes` is NOT optional here.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.ronoctua.boot;
in
{
  options.ronoctua.boot = {
    secureBoot = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Assina a UKI com lanzaboote e exige Secure Boot.

        As chaves ficam em `/etc/secureboot`, que a secao 7.2 roteia para
        `/persist` — senao o wipe do root as apagaria e a maquina nao daria
        mais boot.
      '';
    };

    tpm2 = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "Destrava o LUKS pelo TPM2, selado nos PCRs 7+11+12+15.";
    };

    pcrs = lib.mkOption {
      type = lib.types.str;
      default = "7+11+12+15";
      description = ''
        Os PCRs do selamento. **Mexer aqui e decisao de seguranca.**

        Tirar o 15 faz um disco forjado destravar: os outros tres nao mudam
        quando so o volume muda.
      '';
    };
  };

  config = {
    # Always a UKI, even without Secure Boot: it is what PCR 11 measures, and
    # having two boot formats depending on the option would make the sealing mean
    # different things on different machines.
    boot.loader.systemd-boot.enable = lib.mkForce (!cfg.secureBoot);
    boot.loader.efi.canTouchEfiVariables = true;

    boot.lanzaboote = lib.mkIf cfg.secureBoot {
      enable = true;
      pkiBundle = "/etc/secureboot";
    };

    # systemd initrd: lanzaboote needs it for the UKI, and
    # `systemd-cryptsetup` (which does the TPM unlock) only exists there.
    # With a shell initrd, the wipe's `postDeviceCommands` would also be
    # ignored — see `persist.nix`.
    boot.initrd.systemd.enable = true;

    boot.initrd.luks.devices = lib.mkIf cfg.tpm2 {
      cryptroot = {
        crypttabExtraOpts = [
          "tpm2-device=auto"
          "tpm2-pcrs=${cfg.pcrs}"
          # THE LINE THAT MAKES PCR 15 WORTH ANYTHING.
          #
          # Without it PCR 15 is never extended, stays at zero, and a sealing
          # that includes it starts holding for ANY volume — the forged one
          # included. The sealing would keep on "working", which is the worst
          # way to fail.
          "tpm2-measure-pcr=yes"
        ];
      };
    };

    security.tpm2 = lib.mkIf cfg.tpm2 {
      enable = true;
      tctiEnvironment.enable = true;
    };

    environment.systemPackages = lib.mkIf cfg.tpm2 (
      with pkgs;
      [
        tpm2-tools
        sbctl
      ]
    );

    assertions = [
      {
        assertion = !cfg.secureBoot || config.boot.initrd.systemd.enable;
        message = "ronoctua.boot.secureBoot exige initrd do systemd (a UKI).";
      }
      {
        # Milestone 8's criterion as an assertion. A missing PCR 15 is a
        # sealing that does not protect against a disk swap, and that has to
        # fail the BUILD, not show up as a surprise later.
        assertion = !cfg.tpm2 || lib.hasInfix "15" cfg.pcrs;
        message = ''
          ronoctua.boot.pcrs sem o PCR 15.

          Sem ele, um volume LUKS forjado DESTRAVA: firmware, UKI e cmdline
          continuam os mesmos, e sao eles que os PCRs 7, 11 e 12 medem. O 15
          e o unico que muda quando o disco muda.
        '';
      }
    ];
  };
}
