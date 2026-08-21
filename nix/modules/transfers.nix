# The queue of everything that takes a while — `LEADER t`.
#
# WHY IT IS A UNIT AND NOT PART OF THE FOOTER
# -------------------------------------------
# Two reasons, and each holds on its own.
#
# The same list has to be visible from THREE processes: the footer, which draws
# the modeline's percentage and the panel in its region; the file manager's
# window, which is a re-exec of the shell's binary and therefore somebody else
# entirely; and the wasm buffer's host window. A queue inside the footer would
# be invisible to the other two, and each would grow its own.
#
# And a copy of forty gibibytes must not die when the footer does. On this
# machine the footer restarts on failure in one second — that is right for a
# screen and wrong for an hour of copying.
#
# WHAT IT IS NOT ALLOWED TO BE
# ----------------------------
# It is `ronoctua.transfers`, EXTRACTED from the workspace, and not
# `ronoctua.shell`. A unit whose closure carried wgpu, the wasm runtime, the
# renderer and three embedded fonts would be a hundred megabytes resident to
# copy files, and would hold every one of those store paths alive for as long as
# the machine is on. `nix why-depends` on this unit is what would say so, and
# nobody runs that on a Tuesday — so the check `transfers-unit-is-narrow` runs
# it instead.
#
# WHAT STOPPING IT COSTS, PLAINLY
# -------------------------------
# A copy interrupted by `systemctl --user stop` leaves the half-written file in
# the destination. Cancelling from the panel does not — that path unwinds and
# removes what it wrote — but a kill is a kill, and the process has no handler
# for it. `TimeoutStopSec` is short on purpose: a queue that takes ninety
# seconds to die would hold up every logout.
{
  config,
  lib,
  pkgs,
  ronoctua,
  ...
}:

let
  cfg = config.ronoctua.transfers;

  # THE ONE DESKTOP ENTRY IN THIS REPOSITORY, and it exists for a reason no
  # other feature has had: a magnet is a LINK, and a link is opened by whatever
  # the desktop database says opens `x-scheme-handler/magnet`. Without an entry
  # there is no answer, and clicking a magnet does nothing at all — in a browser,
  # in a chat window, in the file manager, anywhere.
  #
  # It is `NoDisplay`, so the locator (`LEADER a`) does not list it: it is not
  # an application one starts, it is an answer to a question the system asks.
  # `nix/modules/emacs.nix` refuses `install = true` for the mirror image of
  # this reason.
  handler = pkgs.writeTextFile {
    name = "ronoctua-transfers-desktop";
    destination = "/share/applications/ronoctua-transfers.desktop";
    text = ''
      [Desktop Entry]
      Type=Application
      Name=ronoctua
      Comment=Adiciona a transferencia a fila da ronoctua
      Exec=${ronoctua.transfers}/bin/ronoctua-transfers open %u
      Terminal=false
      NoDisplay=true
      MimeType=x-scheme-handler/magnet;application/x-bittorrent;
    '';
  };
in
{
  options.ronoctua.transfers = {
    enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "A fila de transferencias do LEADER t.";
    };

    # WHERE THE PERSON'S SESSION COMES FROM, when a download asks to be them.
    #
    # `LEADER t i …` is the only thing that uses it, and what it buys is the
    # four things an account is needed for: age-restricted video, private
    # video, a members' club, and 1080p Premium. What it costs is anonymity,
    # which is why the key says so and this is only the SOURCE.
    #
    # It is a machine decision and not a per-download one, so it is declared
    # here — and nothing about the cookies themselves is in this repository: a
    # cookie jar is a session, and `nix/modules/secrets.nix` puts a session in
    # the same sentence as a password.
    cookies = lib.mkOption {
      type = lib.types.enum [
        "none"
        "browser"
        "file"
      ];
      default = if config.ronoctua.browser.enable then "browser" else "none";
      defaultText = lib.literalExpression ''if config.ronoctua.browser.enable then "browser" else "none"'';
      description = ''
        De onde saem os cookies de um download identificado.

        `browser` le a base do proprio navegador, o que so funciona porque
        `nix/modules/browser.nix` forca `--password-store=basic` — e e
        exatamente esse o chaveiro que o yt-dlp chama de `basictext`.

        `file` le um arquivo no formato Netscape, exportado uma vez. E a opcao
        mais segura das duas: o YouTube gira os cookies que entrega, e um
        download que usa os do navegador em uso pode invalida-los — o que
        aparece como o navegador se deslogando sozinho.
      '';
    };

    cookiesBrowser = lib.mkOption {
      type = lib.types.str;
      default = "chrome+basictext";
      description = ''
        A grafia `BROWSER[+KEYRING]` do yt-dlp. O `+basictext` nao e detalhe:
        sem ele, decifrar a base do Chrome no Linux pede um chaveiro que esta
        distro deliberadamente nao tem.
      '';
    };

    cookiesFile = lib.mkOption {
      type = lib.types.str;
      default = "%h/.config/ronoctua/cookies.txt";
      description = ''
        O arquivo Netscape, quando `cookies = "file"`. Os especificadores do
        systemd valem aqui — `%h` e a casa da pessoa. O arquivo NASCE na
        maquina; nada dele entra no repositorio (invariante 13).
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    environment.systemPackages = [ handler ];

    # WHAT ANSWERS A MAGNET. `xdg.mime.defaultApplications` writes
    # `/etc/xdg/mimeapps.list`, which is what `xdg-open` reads before it falls
    # back to guessing — and guessing, with one entry, would still work until
    # the day a second program claims the same scheme.
    xdg.mime = {
      enable = true;
      defaultApplications = {
        "x-scheme-handler/magnet" = "ronoctua-transfers.desktop";
        "application/x-bittorrent" = "ronoctua-transfers.desktop";
      };
    };

    # THE SEEDING PORTS, named where a firewall rule can see them.
    #
    # They are the same nine `crates/ronoctua-transfers/src/torrent.rs` gives
    # the session, and the shim has UPnP and NAT-PMP switched off — a program
    # that asked the router to open a port on its own would have changed this
    # machine's exposure without anybody deciding to.
    networking.firewall = {
      allowedTCPPortRanges = [
        {
          from = 6881;
          to = 6889;
        }
      ];
      allowedUDPPortRanges = [
        {
          from = 6881;
          to = 6889;
        }
      ];
    };

    systemd.user.services.ronoctua-transfers = {
      description = "ronoctua — a fila de transferências";
      partOf = [ "graphical-session.target" ];
      after = [ "graphical-session.target" ];
      wantedBy = [ "graphical-session.target" ];
      # WHERE yt-dlp IS, and why it arrives here rather than being baked in.
      #
      # `option_env!` with a store path is a REFERENCE, and this one would point
      # at a package that is deliberately absent from the reduced profile:
      # yt-dlp lives under `ronoctua.media.enable`, which is `mkDefault full`.
      # Baked in, it would drag yt-dlp and ffmpeg into `minimal` through this
      # binary — the trap `RONOCTUA_EMACSCLIENT` fell into, with the same fix.
      environment =
        lib.optionalAttrs config.ronoctua.media.enable {
          RONOCTUA_YT_DLP = "${pkgs.yt-dlp}/bin/yt-dlp";
        }
        # EXACTLY ONE OF THE TWO, never both: the Rust reads the file first, so
        # setting both would make the browser's setting silently dead. An
        # option that does nothing is worse than one that is absent.
        // lib.optionalAttrs (cfg.cookies == "browser") {
          RONOCTUA_COOKIES_BROWSER = cfg.cookiesBrowser;
        }
        // lib.optionalAttrs (cfg.cookies == "file") {
          RONOCTUA_COOKIES_FILE = cfg.cookiesFile;
        };
      serviceConfig = {
        ExecStart = "${ronoctua.transfers}/bin/ronoctua-transfers";
        Restart = "on-failure";
        RestartSec = 1;
        # `session.slice` and NOT `app.slice`.
        #
        # `app.slice` is where every foreign application goes and where
        # `nix/modules/memory.nix` points oomd's pressure killer. A copy holds
        # page cache by definition — that is what copying IS — so a queue in the
        # applications' slice would be among the first things chosen under
        # pressure, and the failure would look like "big copies die at random".
        Slice = "session.slice";
        # If the kernel's OOM killer takes a thread of this, the queue's list is
        # gone and the jobs are orphans. Stopping the unit is the honest answer:
        # it comes back with an empty list instead of a wrong one.
        OOMPolicy = "stop";
        # See the header: a kill leaves a partial file, and a long timeout only
        # delays a logout without saving it.
        TimeoutStopSec = 5;
      };
    };
  };
}
