# The nested loop's watcher — milestone 3's acceptance criterion.
#
# "the nested loop (`nix run .#dev`) starts niri + shell **with hot reload**"
#
# # Why this is a package of its own, and not ten lines inside `dev.nix`
#
# Because the milestone's criterion speaks of BEHAVIOUR, and behaviour inside
# `dev.nix` could only be tested by bringing up a compositor, a GPU and a font
# tree — too expensive to run in `nix flake check`, which is where the milestone
# is proved.
#
# Separated out, the mechanism becomes a function from (directory, command) to
# "the command ran": a check touches a `.rs` in a temporary directory and asserts
# that the command fired. Seconds, no VM, no GPU.
#
# This separation is the correction of a concrete mistake: `check-m3` verified
# that `nix run .#dev` **existed and was executable**, which an app that never
# reloads anything satisfies easily. The milestone was closed that way.
{
  lib,
  writeShellApplication,
  inotify-tools,
  procps,
}:

writeShellApplication {
  name = "ronoctua-reload";

  # `procps` for `pkill`: the trap has to reach the children, and `kill` alone
  # only reaches what `$!` returned.
  runtimeInputs = [
    inotify-tools
    procps
  ];

  text = ''
    # Usage: RONOCTUA_RELOAD_CMD='...' ronoctua-reload <dir> [<dir>...]
    #
    # The command comes in through a VARIABLE and not through an argument because it
    # is a whole shell line ("compile and restart"), and passing it as `argv` would
    # force every caller to get the quoting right. The mistake would show up as a
    # command cut in half, months later.
    : "''${RONOCTUA_RELOAD_CMD:?defina o command a rodar a cada mudanca}"

    if [ $# -eq 0 ]; then
      echo "ERRO: nenhum diretorio para observar." >&2
      exit 2
    fi

    for d in "$@"; do
      [ -d "$d" ] || { echo "ERRO: '$d' nao e diretorio." >&2; exit 2; }
    done

    # `close_write` and not `modify`: editors write in pieces, and `modify`
    # fires on every block. `moved_to` covers whoever writes to a temporary file
    # and renames over the top — which is what vim and rust-analyzer do.
    # THE PIPELINE GOES INTO THE BACKGROUND, AND THE SCRIPT WAITS ON IT.
    #
    # # Why, and what that cost
    #
    # It was the last command in the FOREGROUND, and bash **defers** a signal
    # until the running command finishes — an `inotifywait` that stays up never
    # finishes. The `kill` that `nix/dev.nix` sends on the way out hung
    # forever: the watcher survived the Ctrl+C, was adopted by
    # `systemd --user`, and every `nix run .#dev` left one more behind. I found three
    # of them alive, the oldest two hours old, recompiling the tree on every
    # file saved.
    #
    # `wait` is the documented exception: a signal with a trap **interrupts** the
    # `wait`, and the trap runs at once. That is why the pipeline goes into the background.
    inotifywait -qmr -e close_write -e moved_to --format '%w%f' "$@" \
    | while read -r file; do
        case "$file" in
          *.rs | *.wit) ;;
          # It ignores the rest silently: a misplaced `target/` or an editor's
          # `.swp` would fill the terminal with noise.
          *) continue ;;
        esac

        # Drains whatever arrives in the next 300 ms.
        #
        # Saving a file usually generates several events, and `cargo build` on
        # each of them queues builds that are born obsolete. The first
        # event is what counts; the ones right behind it are the same save.
        while read -r -t 0.3 _; do :; done

        echo "reload: ''${file##*/}"
        # WHAT CHANGED, BY NAME AND EXPORTED.
        #
        # `eval` runs in this shell, so `$file` would already be in scope — and
        # that is precisely the problem: an accident of scope is not a contract.
        # `shellcheck` says so out loud on the other side ("referenced but not
        # assigned"), and it is right. Exported, the command can also hand it to
        # a child, and a caller that ignores it goes on working.
        export RONOCTUA_RELOAD_FILE="$file"
        # `|| true`: a compile error is the NORMAL case in a development
        # loop. Taking the watcher down at the first syntax error would
        # force restarting the whole desktop to fix a typo.
        eval "$RONOCTUA_RELOAD_CMD" || true
      done &
    watcher_proc=$!

    # `pkill -P $$` and not just `kill "$observador"`: the `$!` of a pipeline is its
    # LAST command — the reading loop. `inotifywait` is its sibling, not its
    # child, and would survive waiting for an event nobody reads any more.
    shutdown() {
      pkill -P $$ 2>/dev/null || true
      kill "$watcher_proc" 2>/dev/null || true
    }
    trap shutdown EXIT INT TERM

    wait "$watcher_proc"
  '';

  meta = {
    description = "Observa .rs/.wit e roda um comando a cada mudanca";
    platforms = lib.platforms.linux;
  };
}
