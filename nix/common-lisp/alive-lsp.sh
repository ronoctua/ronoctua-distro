# The body of `alive-lsp`: `nix/common-lisp/default.nix` prepends `sbcl=`, the
# SBCL that sees the libraries and alive-lsp itself.

usage() {
    cat <<'EOF'
Usage: alive-lsp [--port N] [--stdio]

Starts alive-lsp, the Common Lisp language server, in SBCL with the
system's libraries.

  --port N    listen on 127.0.0.1, port N. The default, 0, is a free
              port the system picks; the server prints "Started on port N".
  --stdio     speak the protocol on standard input and output instead: the
              server listens on a free loopback port, this command bridges
              to it, and the server takes that one connection and no other.
              Closing standard input stops the server.
  -h, --help  show this text

Prefer --stdio. With --port the server keeps accepting connections while it
runs, so any program of this machine — another user's, or a sandboxed app
with network — can connect and run code as you: the same risk as a SLIME
or SLY server.
EOF
}

port=0
stdio=0
while [ $# -gt 0 ]; do
    case "$1" in
        --port)
            if [ $# -lt 2 ]; then
                usage >&2
                exit 2
            fi
            port=$2
            shift 2
            ;;
        --port=*)
            port=${1#--port=}
            shift
            ;;
        --stdio)
            stdio=1
            shift
            ;;
        -h | --help)
            usage
            exit 0
            ;;
        *)
            echo "alive-lsp: unknown argument: $1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

case "$port" in
    '' | *[!0-9]*)
        echo "alive-lsp: the port is a number, and \"$port\" is not" >&2
        exit 2
        ;;
esac
if [ "$port" -gt 65535 ]; then
    echo "alive-lsp: the port is at most 65535, and $port is past it" >&2
    exit 2
fi
if [ "$stdio" = 1 ] && [ "$port" != 0 ]; then
    echo "alive-lsp: --stdio picks its own port; --port is for the server alone" >&2
    exit 2
fi

# THE SERVER, the same in both modes. ASDF is the one the libraries were
# built with (`$ASDF`, set by the SBCL wrapper), loaded unless the person's
# init file already did.
loads_asdf='(unless (find-package "ASDF") (load (sb-ext:posix-getenv "ASDF")))'
loads_alive='(asdf:load-system "alive-lsp")'

if [ "$stdio" = 0 ]; then
    exec "$sbcl" --noinform --eval "$loads_asdf" --eval "$loads_alive" \
        --eval "(bt:join-thread (alive/server:start :port $port))"
fi

# --stdio: the server writes its log to a pipe of ours — an anonymous one,
# so nothing is ever made in a folder that a killed bridge would leave
# behind — which is where the port it picked is read from; the rest of the
# log goes to standard error, which an LSP client keeps apart from the
# protocol.
server=""
drain=""
bridge=""
# Run by the EXIT trap below, which shellcheck does not follow.
# shellcheck disable=SC2329
cleanup() {
    for pid in $bridge $server $drain; do
        kill "$pid" 2>/dev/null || true
    done
    for pid in $bridge $server $drain; do
        wait "$pid" 2>/dev/null || true
    done
}
trap cleanup EXIT
# A SIGNAL ENDS THE SCRIPT, and the EXIT trap with it; without these a
# client that kills the bridge would leave the server listening.
trap 'exit 143' TERM
trap 'exit 130' INT
trap 'exit 129' HUP

# ONE CLIENT, the bridge: `:accept-once` (our patch, `alive-lsp.nix`)
# closes the listener after the first connection, and the server ends with
# that session. Its standard input is nothing — a server that fails to load
# must not wait in the debugger for a line of the client's protocol — and it
# dies with this command, however this command dies (`--pdeathsig`: a
# SIGKILL runs no trap).
exec 3< <(exec setpriv --pdeathsig KILL "$sbcl" --noinform --eval "$loads_asdf" --eval "$loads_alive" \
    --eval "(bt:join-thread (alive/server:start :port 0 :accept-once t))" </dev/null)
server=$!

picked=""
while IFS= read -r line <&3; do
    printf '%s\n' "$line" >&2
    case "$line" in
        *"Started on port "*)
            picked=${line##*Started on port }
            break
            ;;
    esac
done
case "$picked" in
    '' | *[!0-9]*)
        echo "alive-lsp: the server stopped before it said which port it listens on" >&2
        exit 1
        ;;
esac
cat <&3 >&2 &
drain=$!

# THE BRIDGE IN THE BACKGROUND, waited for: a trap waits for a command in the
# foreground to end, and this one ends only when the client closes its side.
# `<&0` because a command put in the background reads nothing otherwise.
setpriv --pdeathsig KILL socat - "TCP:127.0.0.1:$picked" <&0 &
bridge=$!
status=0
wait "$bridge" || status=$?
bridge=""
exit "$status"
