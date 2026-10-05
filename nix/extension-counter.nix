# The extension's FOURTH NUMBER — the counter Chrome compares — as a program
# of its own.
#
# WHY A PROGRAM, AND NOT LINES OF THE SETUP SCRIPT
# ------------------------------------------------
# Chrome replaces a force-installed extension only with a HIGHER version, so
# the same build must keep its number and another build must take the next
# one. That rule lived inline in `nix/modules/browser-extension.nix`, which
# writes into `/var/lib/ronoctua-browser` and `/etc/opt/chrome` — so only a
# booted VM could ever run it. Here it takes its file as an argument, the
# module calls it, and `extension-version-moves` runs it in the sandbox: two
# builds, the same build twice, a file somebody broke. The BASE it counts on
# is `nix/extension-version.nix`.
#
#   ronoctua-extension-counter next <built-file> <source>   prints the count
#   ronoctua-extension-counter keep <built-file> <source> <count>
#
# `<built-file>` holds the store path last packed and its count; `keep`
# replaces it whole (a draft renamed over it).
{ writeShellApplication, coreutils }:

writeShellApplication {
  name = "ronoctua-extension-counter";
  runtimeInputs = [ coreutils ];
  text = ''
    verb=''${1:-}
    built=''${2:-}
    source=''${3:-}
    case "$verb" in
      next)
        last=""
        count=0
        if [ -f "$built" ]; then
          read -r last count < "$built" || true
        fi
        # A COUNT THAT IS NOT A NUMBER starts again: a file somebody broke is
        # not a reason to stop packing.
        case "$count" in
          ""|*[!0-9]*) count=0 ;;
        esac
        if [ "$last" != "$source" ]; then
          count=$((count + 1))
        fi
        echo "$count"
        ;;
      keep)
        count=''${4:-}
        printf '%s %s\n' "$source" "$count" > "$built.tmp"
        mv "$built.tmp" "$built"
        ;;
      *)
        echo "usage: ronoctua-extension-counter next|keep <built-file> <source> [count]" >&2
        exit 2
        ;;
    esac
  '';
}
