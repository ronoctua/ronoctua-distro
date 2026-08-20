# The distro's base — section 2 (identity) and section 9 (personal data).
#
# Invariant 16: `noctua` and `ronoctua-distro` are defined in ONE place only.
# This is that place.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.ronoctua;
in
{
  options.ronoctua = {
    # Section 2: a single user per device, always with this name.
    user = lib.mkOption {
      type = lib.types.str;
      default = "noctua";
      readOnly = true;
      description = "Usuario do sistema. Fixo por desenho (secao 2).";
    };

    distroName = lib.mkOption {
      type = lib.types.str;
      default = "ronoctua-distro";
      readOnly = true;
      description = "Nome da distro. Fixo por desenho (secao 2).";
    };

    hostnamePrefix = lib.mkOption {
      type = lib.types.str;
      default = "noctua-";
      readOnly = true;
      description = "Padrao de hostname (secao 2). Ex.: noctua-z16.";
    };

    # Section 9: the hash NEVER enters the repository. The installer writes it to
    # /persist and this module only points there.
    hashedPasswordFile = lib.mkOption {
      type = lib.types.path;
      default = "/persist/etc/shadow/${cfg.user}";
      description = ''
        Caminho do hash de senha do usuario, gerado pelo instalador e
        guardado em /persist. Secao 9: nenhum dado pessoal nas configs.
      '';
    };

    # THE KEYBOARD, with `nix/keyboard-xkb.nix` as the DEFAULT.
    #
    # It used to be that file and nothing else, imported directly by the
    # compositor, by the console and by the nested loop — "the keyboard has ONE
    # place". That is still true of the DEFAULT, and it is the right default: it
    # is the layout of the machine this was written on.
    #
    # What it could not do was answer a different machine. Whoever installed
    # elsewhere typed on a layout that was not theirs, and the worst moment to
    # discover that is while typing the disk's passphrase — which is not echoed
    # and cannot be corrected afterwards.
    #
    # So the file became the default of an option, and the installer's sixth
    # question fills it in. The single place survives: nobody writes a layout
    # anywhere else, they override THIS.
    keyboard =
      let
        base = import ../keyboard-xkb.nix;
        field =
          name: value:
          lib.mkOption {
            type = lib.types.str;
            default = value;
            description = "xkb `${name}`. Padrao: nix/keyboard-xkb.nix.";
          };
      in
      {
        layout = field "layout" base.layout;
        variant = field "variant" base.variant;
        # THE COMPOSE KEY IS THE ANSWER; `options` IS WHAT XKB GETS.
        #
        # The installer fills in this one, and the other is built from it plus
        # what the system adds either way (`always`, in that same file). They
        # were a single option, and then the seventh question — which asks only
        # about accents — would have quietly taken the Caps Lock toggle away
        # from whoever answered it.
        compose = field "compose" base.compose;
        options = lib.mkOption {
          type = lib.types.str;
          default = builtins.concatStringsSep "," (
            (if config.ronoctua.keyboard.compose == "" then [ ] else [ config.ronoctua.keyboard.compose ])
            ++ base.always
          );
          defaultText = lib.literalMD "a tecla de acentos escolhida, mais `nix/keyboard-xkb.nix`";
          description = "xkb `options`, completo. Padrao: derivado de `compose`.";
        };
        console = field "console" base.console;
      };

    # THE DEV OPTIONS ARE DECLARED HERE, not in nix/modules/dev.nix.
    #
    # `base.nix` reads `ronoctua.dev.android` to decide `nixpkgs.config`, and a
    # VM test imports base without importing dev — over there the option would
    # not exist and evaluation dies with "attribute 'dev' missing". All the
    # `ronoctua.*` options live in one place anyway (invariant 16).
    dev = {
      android = lib.mkEnableOption "o SDK do Android, o Studio e o Flutter" // {
        default = true;
      };
      androidNdk = lib.mkEnableOption "o NDK (mais alguns GB; so para codigo nativo)";
      toolchains = lib.mkEnableOption "os compiladores: rust, go, python, node, gcc" // {
        default = true;
      };
    };

    # WHICH OF THE TWO INSTALLATIONS THIS MACHINE IS.
    #
    # Declared here for the same reason written just above: what reads the
    # profile is `nix/modules/profile.nix`, which does not import this file.
    #
    # It is an OPTION, and not a second `nixosConfiguration`. The installer
    # still aims at `#noctua` in both cases, `host.nix` gains one line, and
    # changing one's mind later is a `nixos-rebuild` — not a reinstall.
    profile = lib.mkOption {
      type = lib.types.enum [
        "full"
        "minimal"
      ];
      default = "full";
      description = ''
        `full` traz tudo (23,2 GiB de closure medidos) e e a recomendada.

        `minimal` deixa de fora o Android, os compiladores, o navegador, o
        editor, a midia, as 27 variantes da fonte e o superpacote de icones —
        cerca de 17 GiB. Nao deixa de fora nada do que FAZ a distro: niri,
        rodape, fonte primaria, teclado, rede, impermanencia e terminal ficam.

        Ligar um grupo de volta e uma linha no `host.nix`: por exemplo
        `ronoctua.browser.enable = true;` para instalar so o navegador.

        A tabela que liga o perfil a cada chave esta em `nix/modules/profile.nix`.
      '';
    };
  };

  config = {
    # -----------------------------------------------------------------
    # Identity
    # -----------------------------------------------------------------
    networking.hostName = lib.mkDefault "${cfg.hostnamePrefix}vm";

    assertions = [
      {
        assertion = lib.hasPrefix cfg.hostnamePrefix config.networking.hostName;
        message = ''
          O hostname precisa comecar com "${cfg.hostnamePrefix}" (secao 2).
          Atual: "${config.networking.hostName}".
        '';
      }
    ];

    # Section 9: no mutable users. No password in the repository.
    users.mutableUsers = false;
    users.users.${cfg.user} = {
      isNormalUser = true;
      description = "ronoctua";
      extraGroups = [
        "wheel"
        "video"
        "input"
        "audio"
        "networkmanager"
      ];
      # If the file does not exist (dev VM), it falls back to an empty password via
      # nix/modules/vm.nix. In production the installer always writes it.
      hashedPasswordFile = lib.mkDefault cfg.hashedPasswordFile;
    };

    users.users.root.hashedPassword = lib.mkDefault "!"; # root with no login

    # -----------------------------------------------------------------
    # Languages and formats
    # -----------------------------------------------------------------
    #
    # THE DEFAULT IS BRITISH ENGLISH, and the installer offers another. It is a
    # default and not a decision about who installs: `lib.mkDefault` means the
    # generated `host.nix` overrides every line below.
    #
    # WHY `en_GB` AND NOT `en_US`
    #
    # Because a locale is not only a language: it carries date order, decimal
    # separator, paper size and units. `en_US` brings inches, Fahrenheit and
    # Letter along with the English, and this system is used with none of the
    # three.
    i18n = {
      defaultLocale = lib.mkDefault "en_GB.UTF-8";
      supportedLocales = [
        "en_GB.UTF-8/UTF-8"
        "pt_BR.UTF-8/UTF-8"
        "en_US.UTF-8/UTF-8"
        "C.UTF-8/UTF-8"
      ];
      extraLocaleSettings = {
        LC_MESSAGES = lib.mkDefault "en_GB.UTF-8";
        LC_TIME = lib.mkDefault "en_GB.UTF-8";
        LC_NUMERIC = lib.mkDefault "en_GB.UTF-8";
        # THE METRIC SYSTEM, WHATEVER THE LANGUAGE. `LC_MEASUREMENT` is the only
        # key glibc has for this, and it is NOT `mkDefault`: choosing `en_US` in
        # the installer chooses a language, never inches.
        LC_MEASUREMENT = "en_GB.UTF-8";
        # Paper travels with the units, and for the same reason: A4 is what a
        # printer here takes.
        LC_PAPER = "en_GB.UTF-8";
      };
    };

    time.timeZone = lib.mkDefault "America/Sao_Paulo";

    # -----------------------------------------------------------------
    # Nix — section 7.1: everything through flakes, nothing imperative.
    # -----------------------------------------------------------------
    nix.settings = {
      experimental-features = [
        "nix-command"
        "flakes"
      ];
      auto-optimise-store = true;
      trusted-users = [
        "root"
        cfg.user
      ];
    };
    nix.gc = {
      automatic = true;
      dates = "weekly";
      options = "--delete-older-than 30d";
    };

    # -----------------------------------------------------------------
    # Boot — section 7.2: systemd-boot, not GRUB.
    # -----------------------------------------------------------------
    boot.loader.systemd-boot = {
      enable = lib.mkDefault true;
      # The ESP is 1 GiB (section 7.2) because NixOS keeps kernel+initrd for
      # several generations in there. Even so, cap it.
      configurationLimit = lib.mkDefault 10;
      editor = false; # an editable cmdline at boot destroys the PCR 12 measurement
    };
    boot.loader.efi.canTouchEfiVariables = lib.mkDefault true;

    # Section 3: the system does not look like a traditional desktop. That starts
    # at boot: no logo, no progress bar.
    boot.plymouth.enable = false;
    boot.kernelParams = [
      "quiet"
      "loglevel=3"
    ];

    # -----------------------------------------------------------------
    # Minimalism — section 3: minimal NixOS.
    # -----------------------------------------------------------------
    documentation.nixos.enable = false;

    security.polkit.enable = true;
    security.rtkit.enable = true;
    services.pipewire = {
      enable = true;
      alsa.enable = true;
      pulse.enable = true;
    };

    # THE NETWORK IS NOT HERE. It belongs to `nix/modules/network.nix`, together
    # with the TCP stack tuning and the radio's regulatory domain — leaving the
    # `enable` here and the rest over there would give two places to answer "does
    # this machine use NetworkManager?", and the day they disagreed would be the
    # day the network comes up without the tuning and nobody notices.
    #
    # The user's `networkmanager` group stays in the list above: it belongs to the
    # person's identity, not to the service's configuration.

    environment.systemPackages = with pkgs; [
      git
      jq
      ripgrep
      fd
    ];

    system.stateVersion = lib.mkDefault "26.05";
  };
}
