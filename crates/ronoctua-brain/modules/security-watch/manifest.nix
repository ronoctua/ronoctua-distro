# THE SECURITY CENTRE, WATCHED — and one shield on the modeline, in the
# danger colour, only while something is off.
#
# A watch and nothing more: the core's journal feed (the kernel's refused
# lines, sudo, the firewall's units) and inotify on the two folders the root
# helper writes. Why each piece is so is in `why.md`, beside this.
{
  what = "Watch refused connections, sudo and the firewall; mark the modeline only while something is off.";

  needs = {
    root = false;
    network = false;
    # THE FILES AND THE WORDS OF THE CENTRE: the status and the settings the
    # root helper writes, the window's end, and the prefix the declared
    # firewall logs with — read through the crate that writes them.
    boundaries = [ "ronoctua-security" ];
  };

  # A WATCH: nothing runs on a schedule.
  schedule = null;

  # A SHIELD WITH AN ALERT: the security centre's own shield, marked.
  glyphs = {
    shield = "md-shield_alert";
  };

  # ONE GLYPH, FIVE MEANINGS: the module draws only the loudest of what is
  # off, and each has its own sentence under the pointer.
  marks = {
    firewall-failed = {
      glyph = "shield";
      hint = "security-watch-mark-firewall-failed";
    };
    firewall-off = {
      glyph = "shield";
      hint = "security-watch-mark-firewall-off";
    };
    sudo-open = {
      glyph = "shield";
      hint = "security-watch-mark-sudo-open";
    };
    shields = {
      glyph = "shield";
      hint = "security-watch-mark-shields";
    };
    refused = {
      glyph = "shield";
      hint = "security-watch-mark-refused";
    };
  };

  # "N CONNECTIONS REFUSED ON wlan0", put away until the next one, or put off
  # for an hour.
  notices = {
    refused = {
      level = "warn";
      dismiss = true;
      snooze = [ 60 ];
    };
  };

  # A NETWORK BORN PUBLIC, said once on the minibuffer, with the shell's
  # "treat as home" beside it — the one thing the person may want to do about
  # it, through the sudo door.
  events = {
    network-public = {
      notification = {
        show = "full";
        life = "quick";
        loud = false;
        level = 30;
        icon = "security-shield";
        tags = [ "security" ];
        actions = [
          {
            id = "treat-as-home";
            command = "security-home";
          }
        ];
      };
    };
  };

  # WHAT IT RELIES ON AND DOES NOT OWN: the centre's crate and the module that
  # declares the firewall's log prefix and the folders.
  uses = [
    "crates/ronoctua-security"
    "nix/modules/security.nix"
  ];

  # THE SECURITY CENTRE'S PAGE asks this module for its page by its id, and
  # draws the blocked connections and sudo's uses in its own sections.
  namedBy = {
    "crates/ronoctua-shell/src/security.rs" = [ "security-watch" ];
  };
}
