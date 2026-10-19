# shellcheck shell=sh
# =============================================================================
# .shellrc — what bash and zsh share
# =============================================================================
#
# Sourced by BOTH `.zshrc` and `.bashrc`. Anything that does not depend on one
# shell's own features lives here: environment variables, aliases, functions,
# and the init of tools that accept both shells.
#
# What does NOT belong here:
#   - `setopt`, `bindkey`             → .zshrc
#   - `shopt`, `bind`                 → .bashrc
#   - `atuin init zsh` vs `... bash`  → each in its own file
#
# EVERY BLOCK IS GUARDED. This file is byte-identical on every machine, so a
# section whose tool is absent has to be a silent no-op — that is what lets one
# file serve a laptop, a VM and the installer ISO without a single branch on
# hostname. What is specific to ONE machine goes in `~/.shellrc.local`, which
# this file sources last and which is never versioned (invariant 13).
# =============================================================================


# -----------------------------------------------------------------------------
# Environment detection
# -----------------------------------------------------------------------------
# Which shell is running, and which OS. Available to the rest of the file and
# to `~/.shellrc.local`.

if [ -n "${ZSH_VERSION:-}" ]; then
    CURRENT_SHELL="zsh"
elif [ -n "${BASH_VERSION:-}" ]; then
    CURRENT_SHELL="bash"
else
    CURRENT_SHELL="sh"
fi
export CURRENT_SHELL

OS_NAME="$(uname -s | tr '[:upper:]' '[:lower:]')"
export OS_NAME

# On NixOS this is the honest answer to "can I write to /etc?" — no.
if [ -e /etc/NIXOS ]; then
    export IS_NIXOS=1
fi


# -----------------------------------------------------------------------------
# XDG Base Directory Specification
# -----------------------------------------------------------------------------
# Many apps honour these when set, and stop dropping files straight into $HOME.
# Spec: https://specifications.freedesktop.org/basedir-spec/

export XDG_CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}"
export XDG_DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
export XDG_STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"
export XDG_CACHE_HOME="${XDG_CACHE_HOME:-$HOME/.cache}"

mkdir -p "$XDG_CONFIG_HOME" "$XDG_DATA_HOME" "$XDG_STATE_HOME" "$XDG_CACHE_HOME"

# Apps that honour XDG when pointed at it.
export CARGO_HOME="$XDG_DATA_HOME/cargo"
export GOPATH="$XDG_DATA_HOME/go"
export BUN_INSTALL="$XDG_DATA_HOME/bun"
export LESSHISTFILE="$XDG_STATE_HOME/less/history"
export STARSHIP_CACHE="$XDG_CACHE_HOME/starship"
export NPM_CONFIG_CACHE="$XDG_CACHE_HOME/npm"
export PYTHONPYCACHEPREFIX="$XDG_CACHE_HOME/python"

mkdir -p "$XDG_STATE_HOME/less" "$STARSHIP_CACHE"

# NO RUSTUP_HOME. On this distro the toolchain comes from the flake
# (`rust-overlay`), and a second `rustup` would be a second source of truth for
# which compiler is in use — invariant 6. `~/.shellrc.local` may still override
# it on a machine that needs a nightly rustup for something else.


# -----------------------------------------------------------------------------
# PATH, idempotent
# -----------------------------------------------------------------------------
# Add only if the directory exists and is not already there. Being idempotent
# is what lets this file be re-sourced (`exec zsh`) without the PATH growing.

path_prepend() {
    [ -d "$1" ] || return 0
    case ":$PATH:" in
        *":$1:"*) ;;
        *) PATH="$1:$PATH" ;;
    esac
}

path_append() {
    [ -d "$1" ] || return 0
    case ":$PATH:" in
        *":$1:"*) ;;
        *) PATH="$PATH:$1" ;;
    esac
}

# Personal directories, highest priority first.
path_prepend "$HOME/.local/bin"
path_prepend "$HOME/bin"
path_prepend "$HOME/main/bin"

# Toolchains. On this distro they come from the system closure and are already
# in PATH; these lines matter for anything installed by hand into $HOME, and
# are harmless no-ops otherwise.
path_append "$CARGO_HOME/bin"
path_append "$GOPATH/bin"
path_append "$BUN_INSTALL/bin"

export PATH


# -----------------------------------------------------------------------------
# General environment
# -----------------------------------------------------------------------------

# The editor, first that exists. `emacsclient` comes first because the daemon
# is up: it opens in the already-loaded process instead of paying the whole
# startup again.
if command -v emacsclient >/dev/null 2>&1; then
    export EDITOR="emacsclient -t -a ''"
elif command -v nano >/dev/null 2>&1; then
    export EDITOR="nano"
elif command -v vi >/dev/null 2>&1; then
    export EDITOR="vi"
fi
export VISUAL="$EDITOR"

# TERMINAL — which emulator programs should open. Read by launchers, file
# managers and `$TERMINAL -e COMMAND`. Only ever ADD branches here: removing one
# can break a machine that depends on exactly that terminal.
if command -v xdg-terminal-exec >/dev/null 2>&1; then
    export TERMINAL="xdg-terminal-exec"
elif command -v ghostty >/dev/null 2>&1; then
    export TERMINAL="ghostty"
elif command -v foot >/dev/null 2>&1; then
    export TERMINAL="foot"
fi

# GPG's pinentry needs to know the active TTY.
GPG_TTY="$(tty 2>/dev/null)"
export GPG_TTY

# less:
#   -i  case-insensitive search, unless the query has a capital
#   -M  verbose prompt          -R  keep ANSI escapes (colour survives)
#   -S  no wrapping             -x4 tab = 4      -F  quit if it fits one screen
export PAGER="less"
export LESS="-iMRSx4F --mouse"
export LESSCHARSET="utf-8"

# Coloured man pages through bat.  `col -bx` strips the backspace overstrike
# that man emits; without MANROFFOPT=-c the colours come out corrupted.
if command -v bat >/dev/null 2>&1; then
    export MANPAGER="sh -c 'col -bx | bat -l man -p'"
    export MANROFFOPT="-c"
fi

# The Nix glibc's locale archive. The system normally exports it; this is the
# fallback that keeps Nix-built programs from warning on every start.
if [ -z "${LOCALE_ARCHIVE:-}" ] && [ -e /run/current-system/sw/lib/locale/locale-archive ]; then
    export LOCALE_ARCHIVE=/run/current-system/sw/lib/locale/locale-archive
fi


# -----------------------------------------------------------------------------
# Aliases: colour and listing
# -----------------------------------------------------------------------------

alias ls='ls --color=auto'
alias grep='grep --color=auto'
alias egrep='grep -E --color=auto'
alias fgrep='grep -F --color=auto'

# `eza` when present, plain `ls` otherwise — same command name either way.
if command -v eza >/dev/null 2>&1; then
    alias ll='eza -lah --group-directories-first --icons=auto'
    alias la='eza -la --group-directories-first --icons=auto'
    alias lt='eza --tree --level=2 --group-directories-first --icons=auto'
    alias list='eza -lah --group-directories-first --icons=auto'
else
    alias ll='ls -lah --color=auto --group-directories-first'
    alias la='ls -la --color=auto'
    alias list='ls -lah --color=auto --group-directories-first'
fi

command -v tree >/dev/null 2>&1 && alias tree2='tree -L 2 --dirsfirst'

# `cat` with syntax highlighting, without losing plain `cat`.
if command -v bat >/dev/null 2>&1; then
    alias catp='bat --paging=never'
fi

# The trailing space makes the shell expand the NEXT token as an alias too, so
# `sudo ll` works. See https://askubuntu.com/q/22037
alias sudo='sudo '


# -----------------------------------------------------------------------------
# Aliases: git
# -----------------------------------------------------------------------------
# The classic oh-my-zsh git plugin set, trimmed to what actually gets typed.

alias g='git'
alias gst='git status'
alias ga='git add'
alias gaa='git add --all'
alias gc='git commit -v'
alias gcm='git commit -m'
alias gco='git checkout'
alias gcb='git checkout -b'
alias gd='git diff'
alias gds='git diff --staged'
alias gl='git pull'
alias gp='git push'
alias gb='git branch'
alias glog='git log --oneline --decorate --graph'
alias glg='git log --stat'


# -----------------------------------------------------------------------------
# Aliases: Emacs in daemon mode
# -----------------------------------------------------------------------------
# WHY A DAEMON: one Emacs process loads the person's configuration ONCE and
# every `e` after that opens in it. Without the daemon each file pays the whole
# startup, which with a configuration of any size is seconds — enough that one
# stops using it as `$EDITOR`.
#
#   e   in the terminal itself (`-t`), the everyday one
#   et  a graphical frame (`-c`), for when a window is wanted
# Both carry `-a ''`: if no daemon is up, start one instead of failing.

if command -v emacsclient >/dev/null 2>&1; then
    alias e="emacsclient -t -a ''"
    alias et="emacsclient -c -n -a ''"
fi


# -----------------------------------------------------------------------------
# Aliases: system
# -----------------------------------------------------------------------------

alias ..='cd ..'
alias ...='cd ../..'
alias ....='cd ../../..'
alias df='df -h'
alias du='du -h'
alias free='free -h'
alias mkdir='mkdir -p'
alias path='echo "$PATH" | tr ":" "\n"'
alias ports='ss -tulpn'
alias myip='curl -s https://ifconfig.me && echo'

command -v duf >/dev/null 2>&1 && alias df='duf'
command -v dust >/dev/null 2>&1 && alias du='dust'
command -v procs >/dev/null 2>&1 && alias ps='procs'
command -v btop >/dev/null 2>&1 && alias top='btop'

# NixOS: the four that get typed every day.
if [ -n "${IS_NIXOS:-}" ]; then
    alias nrs='sudo nixos-rebuild switch --flake'
    alias nrt='sudo nixos-rebuild test --flake'
    # The system's old generations by count (the newest, the running and the
    # booted stay; no other profile is touched), then the garbage. Never
    # `--delete-older-than` as root: that deletes every profile's history.
    alias ngc='sudo systemctl start ronoctua-system-generations.service && sudo nix-collect-garbage'
    alias nfu='nix flake update'
fi


# -----------------------------------------------------------------------------
# Functions: network
# -----------------------------------------------------------------------------

# Local and public addresses, one per line.
ips() {
    echo "local:"
    ip -brief address show 2>/dev/null | awk '$2 == "UP" { print "  " $1 "  " $3 }'
    echo "public:"
    echo "  $(curl -s --max-time 5 https://ifconfig.me || echo 'unreachable')"
}

# Scan the local network. Without an argument, guesses the /24 of the default
# route — which is what one actually wants nine times out of ten.
scan() {
    command -v nmap >/dev/null 2>&1 || { echo "nmap is not installed" >&2; return 1; }
    target="${1:-}"
    if [ -z "$target" ]; then
        target="$(ip -4 route show default 2>/dev/null | awk '{ print $3 }' | head -1)"
        [ -n "$target" ] || { echo "could not guess the network; pass a range" >&2; return 1; }
        target="${target%.*}.0/24"
    fi
    echo "scanning $target"
    nmap -sn "$target"
}


# -----------------------------------------------------------------------------
# Functions: general
# -----------------------------------------------------------------------------

# Versions of what is installed. Every line is guarded, so the output is a map
# of the machine and not a wall of "command not found".
v() {
    for tool in rustc cargo go python3 node bun deno dart flutter emacs nix git; do
        if command -v "$tool" >/dev/null 2>&1; then
            printf '%-10s %s\n' "$tool" "$("$tool" --version 2>&1 | head -1)"
        fi
    done
}

# extract FILE [FILE...]
#
# Unpacks by extension. Every branch checks its tool first, so a missing one
# gives a clear message instead of a shell error. For loose .gz/.bz2/.xz/.zst
# it uses the `-k` form, which KEEPS the original — the destructive default of
# those tools has eaten files before.
extract() {
    [ $# -gt 0 ] || { echo "usage: extract FILE [FILE...]" >&2; return 1; }
    for archive in "$@"; do
        [ -f "$archive" ] || { echo "not a file: $archive" >&2; continue; }
        case "$archive" in
            *.tar.bz2|*.tbz2) tar xjf "$archive" ;;
            *.tar.gz|*.tgz)   tar xzf "$archive" ;;
            *.tar.xz|*.txz)   tar xJf "$archive" ;;
            *.tar.zst)        tar --zstd -xf "$archive" ;;
            *.tar)            tar xf "$archive" ;;
            *.gz)             gunzip -k "$archive" ;;
            *.bz2)            bunzip2 -k "$archive" ;;
            *.xz)             unxz -k "$archive" ;;
            *.zst)            unzstd -k "$archive" ;;
            *.zip)            unzip "$archive" ;;
            *.rar)            unrar x "$archive" ;;
            *.7z)             7zz x "$archive" ;;
            *.deb)            ar x "$archive" ;;
            *.iso)            7zz x "$archive" ;;
            *)
                # `bsdtar` reads more formats than all of the above together;
                # it is the honest last resort before giving up.
                if command -v bsdtar >/dev/null 2>&1; then
                    bsdtar xf "$archive"
                else
                    echo "unknown format: $archive" >&2
                fi
                ;;
        esac
    done
}

# Make a directory and go into it.
mkcd() {
    mkdir -p "$1" && cd "$1" || return 1
}


# -----------------------------------------------------------------------------
# fzf
# -----------------------------------------------------------------------------
# `fd` as the source: it honours .gitignore and skips .git, which is what makes
# the list usable in a repository. Falls back to fzf's default walker.

if command -v fzf >/dev/null 2>&1; then
    if command -v fd >/dev/null 2>&1; then
        export FZF_DEFAULT_COMMAND='fd --type f --hidden --follow --exclude .git'
        export FZF_CTRL_T_COMMAND="$FZF_DEFAULT_COMMAND"
        export FZF_ALT_C_COMMAND='fd --type d --hidden --follow --exclude .git'
    fi
    export FZF_DEFAULT_OPTS='--height 40% --layout=reverse --border --info=inline'
    if command -v bat >/dev/null 2>&1; then
        export FZF_CTRL_T_OPTS="--preview 'bat -n --color=always {}' --preview-window right:60%"
    fi
fi


# -----------------------------------------------------------------------------
# direnv and zoxide
# -----------------------------------------------------------------------------
# Both know how to emit code for either shell, so the hook belongs here and not
# in each file. `$CURRENT_SHELL` is what makes one line serve both.

if command -v direnv >/dev/null 2>&1; then
    eval "$(direnv hook "$CURRENT_SHELL")"
fi

if command -v zoxide >/dev/null 2>&1; then
    eval "$(zoxide init "$CURRENT_SHELL")"
fi


# -----------------------------------------------------------------------------
# The default LLM accounts (`LEADER m a`)
# -----------------------------------------------------------------------------
# `ronoctua-llm` writes this file whenever the person picks a default account,
# so `claude` and `codex` typed here open it: an `export CLAUDE_CONFIG_DIR=…`
# or `CODEX_HOME=…` for a default that is not the agent's own folder, and an
# `unset` otherwise — so a shell that inherited an older default (a terminal
# multiplexer started before the change) follows the new one. Before
# `~/.shellrc.local`, which may still override it.

if [ -r "${XDG_CONFIG_HOME:-$HOME/.config}/ronoctua/llm-default.env" ]; then
    . "${XDG_CONFIG_HOME:-$HOME/.config}/ronoctua/llm-default.env"
fi


# -----------------------------------------------------------------------------
# The machine's own config — LAST, so it can override anything above
# -----------------------------------------------------------------------------
# `~/.shellrc.local` holds what belongs to ONE machine: tokens, credentials,
# paths that exist only here. It is created empty by the system and is NEVER
# versioned (invariant 13). Sourcing it last is what lets it win.

[ -r "$HOME/.shellrc.local" ] && . "$HOME/.shellrc.local"


# -----------------------------------------------------------------------------
# PATH dedupe — the very last step
# -----------------------------------------------------------------------------
# `~/.shellrc.local` may have added entries that were already there. This keeps
# the FIRST occurrence of each, which is the one that wins anyway.

PATH="$(printf '%s' "$PATH" | awk -v RS=: -v ORS=: '!seen[$0]++' | sed 's/:$//')"
export PATH
