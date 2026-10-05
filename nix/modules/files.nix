# The file manager — `LEADER .`, and the one narrow door to what needs root.
#
# WHAT IS PRIVILEGED HERE, AND WHAT IS NOT
# ----------------------------------------
# Almost nothing. Listing, renaming, copying, trashing, deleting, compressing
# and changing the permissions of one's OWN files need no privilege at all, and
# the manager does them as the person. Four operations do not fit that:
# changing an owner, changing a group one does not belong to, changing the
# permissions of a file one does not own, and telling the device which blocks
# are free.
#
# Those go through ONE unit with a FIXED name — see below — started through
# the SUDO DOOR (`ronoctua_security::authority`, verb `files-root`): the root
# helper runs `systemctl start` on exactly this unit, after `sudo -n` or the
# person's password typed into the door that asked. There is NO polkit rule
# for it: this session has no polkit agent, so the `auth_self_keep` rule that
# used to name it could never ask anybody, and every change of owner was
# refused at once. `files-root-is-narrow` holds the three names together (the
# crate's unit, this module's, the helper's) and refuses a polkit rule that
# names the unit again.
#
# DELETING AND MOVING ARE DELIBERATELY ABSENT from the privileged verbs. They
# are the two that cannot be undone, and a root delete one key away from a
# listing is how a file manager erases somebody's system. Whoever needs that
# opens the root terminal the menu offers, where what is happening is visible.
{
  config,
  lib,
  pkgs,
  ronoctua,
  ...
}:

let
  cfg = config.ronoctua.files;
  user = config.ronoctua.user;

  # THE UNIT'S NAME IS FIXED, and that is the whole design.
  #
  # A templated unit taking the path as its instance would be the obvious shape
  # and it is the wrong one: `%i` is a free string, so the root helper's verb
  # could not name what it starts — it would have to take a free argument, and
  # a free argument to a root helper is a door of unknown width. With a fixed
  # name, the request travels through a directory the person owns and the unit
  # VALIDATES it.
  unit = "ronoctua-files-root.service";

  # Where the request is written. One file per field: a path may contain a
  # newline, a colon and a space, and any single-line encoding is a parser
  # waiting to disagree with its writer.
  staging = "/run/ronoctua-files/staged";

  # THE SIDEBAR'S DEFAULTS, resolved from keys to names.
  #
  # `nix/files-sidebar.nix` names WHICH directories, by key; `nix/user-dirs.nix`
  # says where each key points, relative to home. Resolving here rather than in
  # either of them is what keeps both files free of the other's concern — and
  # what makes a key that names nothing a build error instead of an empty row.
  dirs = import ../user-dirs.nix;
  chosen = import ../files-sidebar.nix;
  nameOf =
    key:
    if key == "start" then
      import ../files-start.nix
    else if dirs ? ${key} then
      dirs.${key}
    else
      throw "ronoctua: nix/files-sidebar.nix cites \"${key}\", which does not exist in nix/user-dirs.nix";
  # COLON-SEPARATED, the habit `XDG_DATA_DIRS` already set. None of these is a
  # path, so none of them can contain a colon: they are names relative to home.
  sidebar = lib.concatStringsSep ":" (map nameOf chosen);
in
{
  options.ronoctua.files = {
    enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "The file manager of LEADER . and of the window.";
    };

    start = lib.mkOption {
      type = lib.types.str;
      # ONE PLACE, and the nested loop reads the same file — see it for why.
      default = import ../files-start.nix;
      description = ''
        Which folder the manager opens in, RELATIVE to home — a name, never a
        path. If it does not exist on the machine, it opens in home; that is not an error.

        An option and not a literal in the Rust because the folder somebody keeps their
        things in is a preference OF THE MACHINE (invariant 13), and because writing
        an absolute path here would be personal.
      '';
    };

    sidebar = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      # ONE PLACE, and by KEY — see `nix/files-sidebar.nix` and
      # `nix/user-dirs.nix`. The nested loop reads the same two files.
      default = lib.splitString ":" sidebar;
      description = ''
        Which folders the manager's sidebar is born with, RELATIVE to home —
        names, never paths, for the same reason as `start`.

        Whoever adjusts the sidebar afterwards does not touch this: what the person
        adds, hides or removes lives in
        `$XDG_CONFIG_HOME/ronoctua/files-sidebar.json` and beats this list. A folder
        from here that does not exist on the machine does not become a line, and that
        is not an error.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    # WHERE THE PERSON WRITES WHAT THEY WANT DONE.
    #
    # `0700` and theirs: nobody else on the machine may put a request where a
    # unit that runs as root will read it.
    systemd.tmpfiles.rules = [
      "d /run/ronoctua-files 0700 ${user} ${config.users.users.${user}.group} -"
      "d ${staging} 0700 ${user} ${config.users.users.${user}.group} -"
    ];

    # THE ONE THING THAT RUNS AS ROOT.
    #
    # `Type=oneshot`: it does one operation and exits, so `systemctl start`
    # answers when the work is done and the manager can re-read the directory
    # knowing the answer. The binary is EXTRACTED from the workspace
    # (`rust.filesRoot`) rather than being the whole of it — the trap
    # `AGENTS.md` records: a derivation references what its bytes cite, and
    # shipping the shell here would put wgpu, the wasm runtime and the renderer
    # in a unit that runs as root.
    systemd.services.${lib.removeSuffix ".service" unit} = {
      description = "Applies a change of owner, group or permission asked for by the manager";
      serviceConfig = {
        Type = "oneshot";
        ExecStart = "${ronoctua.filesRoot}/bin/ronoctua-files-root ${staging}";
        # It reads a directory the person owns and touches one path. Nothing
        # else: no network, no new privileges beyond the ones it was started
        # with, and no private tmp that would hide the staging directory from
        # it.
        RestrictAddressFamilies = "AF_UNIX";
        PrivateNetwork = true;
        ProtectHostname = true;
        ProtectKernelTunables = true;
        ProtectKernelModules = true;
        ProtectControlGroups = true;
        RestrictRealtime = true;
        # NOT `ProtectSystem`: it exists to change files outside the home, which
        # is the whole reason it is privileged. The refusal list lives in the
        # binary, where it can be read and tested.
      };
    };

    # WHO MAY START IT: whoever passes the sudo door. Seeing is free, acting
    # proves who you are (the person's own password, the authority they
    # already have), and sudo's own timestamp keeps six files from being six
    # passwords. No polkit rule: see the top of this file.

    # WHERE IT OPENS. A session variable and not a baked constant, so changing
    # it is a rebuild and not a recompile.
    environment.sessionVariables.RONOCTUA_FILES_START = cfg.start;

    # AND WHAT THE SIDEBAR STARTS WITH, for the same reason and by the same
    # route. `dev-exports-before-it-starts` demands the nested loop export it
    # BEFORE the footer comes up: a variable that arrives late is a variable
    # that did not arrive, and a missing one here is a fallback and not an
    # error — the sidebar would simply come up with no places in it.
    environment.sessionVariables.RONOCTUA_FILES_SIDEBAR = lib.concatStringsSep ":" cfg.sidebar;
  };
}
