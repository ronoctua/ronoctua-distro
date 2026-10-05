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
# `systemctl --user stop` is the panel's cancel for every job at once: the
# queue's `SIGTERM` handler stops them all and waits for the copies to unwind
# — removing what they wrote — before it exits (`Shared::wind_down`), within
# four seconds. `TimeoutStopSec` is five, and short on purpose: a queue that
# takes ninety seconds to die would hold up every logout, and only a copy that
# does not unwind in four seconds is left to the `SIGKILL` and its piece.
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
      Comment=Adds the transfer to ronoctua's queue
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
      description = "LEADER t's transfer queue.";
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
        Where an identified download's cookies come from.

        `browser` reads the browser's own database, which only works because
        `nix/modules/browser.nix` forces `--password-store=basic` — and that is
        exactly the keyring yt-dlp calls `basictext`.

        `file` reads a file in Netscape format, exported once. It is the option
        safer of the two: YouTube rotates the cookies it hands out, and a
        a download using the ones from the browser in use can invalidate them — which
        shows up as the browser logging itself out.
      '';
    };

    cookiesBrowser = lib.mkOption {
      type = lib.types.str;
      default = "chrome+basictext";
      description = ''
        yt-dlp's `BROWSER[+KEYRING]` spelling. The `+basictext` is not a detail:
        without it, decrypting Chrome's database on Linux asks for a keyring this
        distro deliberately does not have.
      '';
    };

    # THE DEVICE ATTESTATION, and the reason it is a switch rather than always
    # on.
    #
    # A PO token says the request comes from a real device — the job OAuth2 did
    # until YouTube removed it. It is NOT about identity: an anonymous download
    # gets one bound to a visitor id minted on the spot, which is exactly the
    # case YouTube increasingly refuses without one.
    #
    # What it costs is 53,6 MiB — the minter and its vendored JavaScript.
    # Measured, and smaller than it looks: minting runs Google's BotGuard
    # challenge and therefore needs a JavaScript runtime, but `nix why-depends`
    # says Deno is ALREADY in the closure because nixpkgs' own yt-dlp depends on
    # it. So the runtime is paid for either way, and this option adds only the
    # minter.
    #
    # ON WHERE THE MEDIA PROFILE IS (2026-09-30): a machine that plays and
    # downloads video is one YouTube refuses without the attestation — the
    # player's YouTube did not play — and the minter costs only while a token
    # is minted. Off where there is no yt-dlp to use it (`ronoctua.media`), and
    # where the flake did not hand the minter to the modules (a test's
    # stripped-down machine), which cannot mint.
    #
    # `on-demand` IS THE ONLY MODE, and the name is the promise: the minter is a
    # SCRIPT that runs once per token and exits. Upstream also ships an HTTP
    # server that stays resident and caches; `nix/pot-provider.nix` deliberately
    # does not install that half, so there is no port, no resident process, and
    # nothing anybody has to remember to stop.
    poToken = lib.mkOption {
      type = lib.types.enum [
        "none"
        "on-demand"
      ];
      default = if config.ronoctua.media.enable && ronoctua ? potProvider then "on-demand" else "none";
      defaultText = lib.literalExpression ''if config.ronoctua.media.enable then "on-demand" else "none"'';
      description = ''
        Whether this installation generates a PO token for yt-dlp — for the
        queue's downloads and the player's YouTube alike.

        `on-demand` runs the generator on request and it exits; nothing stays standing. The
        tokens live in a cache under `XDG_CACHE_HOME`, so the cost of a few
        seconds is paid once every few hours, and not per download.

        It costs the generator in the closure (Deno is there already, through
        yt-dlp) — hence on only where the media profile is.
      '';
    };

    cookiesFile = lib.mkOption {
      type = lib.types.str;
      default = "%h/.config/ronoctua/cookies.txt";
      description = ''
        The Netscape file, when `cookies = "file"`. systemd's specifiers
        apply here — `%h` is the person's home. The file IS BORN on the
        machine; none of it goes into the repository (invariant 13).
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
      description = "ronoctua — the transfer queue";
      partOf = [ "graphical-session.target" ];
      after = [ "graphical-session.target" ];
      wantedBy = [ "graphical-session.target" ];
      # THE SESSION'S `PATH`, NOT A UNIT'S — the footer's reason, one process
      # along. The queue is the parent of every program an install, update or
      # removal starts (`ronoctua-software`, and whatever Flatpak, Nix or an
      # AppImage start in turn), and a child inherits its parent's `PATH`: we
      # name our programs by store path, but a bare name anywhere further down
      # that tree resolves against this unit's. NixOS gives a unit five
      # packages (coreutils, findutils, grep, sed, systemd) unless told
      # otherwise; off and with no `path`, no `Environment=PATH` is written
      # (`nixos/lib/systemd-lib.nix`) and the user manager's is inherited —
      # the one NixOS builds from `environment.profiles`.
      enableDefaultPath = false;
      # WHERE yt-dlp IS, and why it arrives here rather than being baked in.
      #
      # `option_env!` with a store path is a REFERENCE, and this one would point
      # at a package that is deliberately absent from the reduced profile:
      # yt-dlp lives under `ronoctua.media.enable`, which is `mkDefault full`.
      # Baked in, it would drag yt-dlp into `minimal` through this binary — the
      # trap `RONOCTUA_EMACSCLIENT` fell into, with the same fix.
      #
      # It used to say "yt-dlp and ffmpeg". `ffmpeg-full` is unconditional now
      # (see `nix/modules/media.nix`), so it is no longer part of what this
      # would drag in — but yt-dlp still is, and the argument stands on it.
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
        }
        # ALL THREE OR NONE: the minter is TypeScript and needs the runtime
        # that reads it, and yt-dlp asks it for a token only through its plugin,
        # which it loads from the folder it is told and nowhere else — without
        # `RONOCTUA_POT_PLUGINS` not one token was ever minted. The Rust refuses
        # a part of the three (`ronoctua_ytdlp::pot_flags_of`). The three are
        # the minter's own (`potProvider.environment`, `nix/pot-provider.nix`):
        # where its plugin is is a fact of its layout, and a unit that spelled
        # it once pointed one level too deep.
        // lib.optionalAttrs (cfg.poToken == "on-demand") ronoctua.potProvider.environment;
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
        # See the header: the stop unwinds in four seconds, and a long timeout
        # only delays a logout.
        TimeoutStopSec = 5;
      };
    };
  };
}
