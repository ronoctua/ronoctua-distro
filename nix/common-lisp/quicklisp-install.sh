# The body of `quicklisp-install`: `nix/common-lisp/default.nix` prepends the
# pinned files (`tar_file`, `setup_file`, `asdf_file`, `client_info_file`),
# the HTTPS fetcher (`fetcher_file`), the SBCL to run (`sbcl`) and the
# client's version (`client_version`).

usage() {
    cat <<'EOF'
Usage: quicklisp-install [--path DIR]

Puts Quicklisp, the Common Lisp library manager, in DIR (default ~/quicklisp),
from copies of its client the system already holds, and has it fetch the list
of libraries over HTTPS. The init files the system seeds (~/.sbclrc,
~/.ccl-init.lisp, ~/.eclrc) load ~/quicklisp/setup.lisp when it is there, and
no other.

On a Quicklisp that is already there, it only writes the HTTPS fetcher again
(local-init/ronoctua-https.lisp) — and, when the list of libraries never
arrived whole, fetches it again.

  --path DIR  where to put it
  -h, --help  show this text
EOF
}

path="$HOME/quicklisp"
while [ $# -gt 0 ]; do
    case "$1" in
        --path)
            if [ $# -lt 2 ]; then
                usage >&2
                exit 2
            fi
            path=$2
            shift 2
            ;;
        --path=*)
            path=${1#--path=}
            shift
            ;;
        -h | --help)
            usage
            exit 0
            ;;
        *)
            echo "quicklisp-install: unknown argument: $1" >&2
            usage >&2
            exit 2
            ;;
    esac
done
if [ -z "$path" ]; then
    echo "quicklisp-install: --path needs a directory" >&2
    exit 2
fi

writes_the_fetcher() {
    install -d -m 0755 "$path/local-init"
    install -m 0644 "$fetcher_file" "$path/local-init/ronoctua-https.lisp"
}

# THE LIST OF LIBRARIES IS WHOLE when its three files are there. Quicklisp
# makes `dists/` before its first download and never makes it again, and it
# fetches a missing index lazily but trusts one that is there — so a first
# download that did not finish (no network at the time, a cut connection)
# would leave a Quicklisp that never knows.
dist="$path/dists/quicklisp"
whole() {
    [ -s "$dist/distinfo.txt" ] && [ -s "$dist/systems.txt" ] && [ -s "$dist/releases.txt" ]
}

# A QUICKLISP THAT IS THERE is the person's: its libraries, its dists, its
# local projects. Only the fetcher is ours, and only it is written again —
# unless the list of libraries is not whole: then it goes, and is fetched
# again.
if [ -e "$path/setup.lisp" ]; then
    writes_the_fetcher
    if whole; then
        echo "quicklisp-install: Quicklisp is already in $path; only its HTTPS fetcher was written again ($path/local-init/ronoctua-https.lisp)."
        exit 0
    fi
    rm -rf -- "$dist"
    echo "quicklisp-install: Quicklisp is in $path and its list of libraries is not whole; its HTTPS fetcher was written again, and the list is fetched again."
else
    if [ -e "$path" ] && [ ! -d "$path" ]; then
        echo "quicklisp-install: $path is not a directory" >&2
        exit 1
    fi
    if [ -d "$path" ] && [ -n "$(ls -A "$path")" ]; then
        echo "quicklisp-install: $path is not empty and holds no Quicklisp; nothing was written. Name an empty or new directory with --path." >&2
        exit 1
    fi

    # THE CLIENT, from the store: the archive holds `quicklisp/`, the
    # client's own sources, and the three files beside it are what the
    # bootstrap would have fetched. Copies of the store's are read-only, so
    # each is installed writable — the client rewrites them when it updates
    # itself.
    install -d -m 0755 "$path"
    tar --extract --no-same-owner --no-same-permissions --file "$tar_file" --directory "$path"
    chmod -R u+w "$path/quicklisp"
    install -m 0644 "$setup_file" "$path/setup.lisp"
    install -m 0644 "$asdf_file" "$path/asdf.lisp"
    install -m 0644 "$client_info_file" "$path/client-info.sexp"
    writes_the_fetcher
    echo "quicklisp-install: Quicklisp's client $client_version is in $path, copied from the system; nothing was downloaded for it."
fi

cat <<EOF

Now Quicklisp fetches its list of libraries from beta.quicklisp.org, and every library you load with it later comes from there too.

Quicklisp's own client fetches over plain HTTP and checks only each file's size. This one fetches over HTTPS ($path/local-init/ronoctua-https.lisp), and a redirect only to HTTPS. HTTPS proves the files come from beta.quicklisp.org, or from the HTTPS host one of its redirects names; it does not prove who wrote them, because Quicklisp signs nothing.

EOF

# THE PERSON'S INIT FILE IS NOT READ: it loads `setup.lisp` itself when it is
# there, and it is there now. Loading it makes the first download when
# `dists/` is new; when `dists/` holds other dists, the quicklisp one is
# installed by name. The path goes by the environment, so no quote in it can
# end the Lisp string; a failure is said in a sentence, not a backtrace.
if ! QUICKLISP_INSTALL_SETUP="$path/setup.lisp" "$sbcl" --noinform --non-interactive --no-userinit \
    --eval '(unless (find-package "ASDF") (load (sb-ext:posix-getenv "ASDF")))' \
    --eval '(handler-case
                (progn
                  (load (sb-ext:posix-getenv "QUICKLISP_INSTALL_SETUP"))
                  (unless (funcall (find-symbol "FIND-DIST" "QL-DIST") "quicklisp")
                    (funcall (find-symbol "INSTALL-DIST" "QL-DIST")
                             "http://beta.quicklisp.org/dist/quicklisp.txt" :prompt nil)))
              (error (e)
                (format *error-output* "~&quicklisp-install: Quicklisp stopped: ~a~%" e)
                (sb-ext:exit :code 1)))' || ! whole; then
    echo "quicklisp-install: Quicklisp's list of libraries did not arrive whole; run quicklisp-install again to fetch it again." >&2
    exit 1
fi

if [ "$path" = "$HOME/quicklisp" ]; then
    echo "quicklisp-install: done. (ql:quickload \"name\") loads a library; ~/.sbclrc, ~/.ccl-init.lisp and ~/.eclrc load Quicklisp."
else
    echo "quicklisp-install: done. The init files the system seeds load ~/quicklisp only: load $path/setup.lisp yourself to use this one."
fi
