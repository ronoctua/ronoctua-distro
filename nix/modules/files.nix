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
# Those go through ONE unit with a FIXED name — see below — and a polkit rule
# that names it. `files-polkit-is-narrow` compares the two lists in both
# directions, so a verb without its unit fails in front of the person and a
# unit left behind after its verb went away is privilege granted to nobody.
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
  # and it is the wrong one: `%i` is a free string, so the polkit rule could not
  # enumerate what is allowed — it would have to match a prefix, and a prefix
  # match is a door of unknown width. With a fixed name, the request travels
  # through a directory the person owns and the unit VALIDATES it.
  unit = "ronoctua-files-root.service";

  # Where the request is written. One file per field: a path may contain a
  # newline, a colon and a space, and any single-line encoding is a parser
  # waiting to disagree with its writer.
  staging = "/run/ronoctua-files/staged";
in
{
  options.ronoctua.files = {
    enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "O gerenciador de arquivos do LEADER . e da janela.";
    };

    start = lib.mkOption {
      type = lib.types.str;
      # ONE PLACE, and the nested loop reads the same file — see it for why.
      default = import ../files-start.nix;
      description = ''
        Em que pasta o gerenciador abre, RELATIVA a home — um nome, nunca um
        caminho. Se ela nao existir na maquina, ele abre na home; nao e erro.

        Uma opcao e nao um literal no Rust porque a pasta em que alguem guarda o
        que faz e preferencia DA MAQUINA (invariante 13), e porque escrever um
        caminho absoluto aqui seria pessoal.
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
    # `CLAUDE.md` records: a derivation references what its bytes cite, and
    # shipping the shell here would put wgpu, the wasm runtime and the renderer
    # in a unit that runs as root.
    systemd.services.ronoctua-files-root = {
      description = "Aplica uma mudanca de dono, grupo ou permissao pedida pelo gerenciador";
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

    # WHO MAY START IT, AND ONLY IT.
    #
    # `auth_self_keep` and not `yes`: this is the line the owner drew for the
    # rescue console and it holds here for the same reason — seeing is free,
    # acting proves who you are, and the proof is remembered for the session so
    # changing the owner of six files is not six passwords.
    #
    # The verb is `start` and nothing else: this rule cannot stop a service and
    # cannot name a unit that is not this one.
    security.polkit.extraConfig = ''
      polkit.addRule(function (action, subject) {
        if (
          action.id == "org.freedesktop.systemd1.manage-units" &&
          action.lookup("verb") == "start" &&
          action.lookup("unit") == "${unit}" &&
          subject.user == "${user}"
        ) {
          return polkit.Result.AUTH_SELF_KEEP;
        }
      });
    '';

    # WHERE IT OPENS. A session variable and not a baked constant, so changing
    # it is a rebuild and not a recompile.
    environment.sessionVariables.RONOCTUA_FILES_START = cfg.start;
  };
}
