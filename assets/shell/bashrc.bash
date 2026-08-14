# shellcheck shell=bash
# =============================================================================
# .bashrc — only what is bash's own
# =============================================================================
#
# zsh is the login shell on this distro. bash is here because it is what a
# script, an ssh from somewhere else and every `#!/usr/bin/env bash` land in —
# and a bash without history or completion is a bad surprise on exactly the day
# one is in a hurry.
#
# Everything shared lives in `.shellrc`, sourced in section 4.
# =============================================================================


# -----------------------------------------------------------------------------
# 1. Interactivity guard
# -----------------------------------------------------------------------------
# scp and rsync speak over stdout. Anything printed by a non-interactive bash
# corrupts the transfer, and the error blames the file, not this line.

case $- in
    *i*) ;;
    *) return ;;
esac


# -----------------------------------------------------------------------------
# 2. History
# -----------------------------------------------------------------------------
# Native file as a BACKUP; atuin (section 6) is the source of truth. Same
# reasoning as in `.zshrc`: a plugin failing must not cost the history.

HISTFILE="${XDG_STATE_HOME:-$HOME/.local/state}/bash/history"
HISTSIZE=100000
HISTFILESIZE=200000
HISTCONTROL=ignoreboth        # no duplicates, and a leading space hides the line
HISTTIMEFORMAT='%F %T '
mkdir -p "$(dirname "$HISTFILE")"

shopt -s histappend           # append instead of overwriting on exit
shopt -s cmdhist              # a multi-line command stays one entry
shopt -s checkwinsize         # keep $LINES/$COLUMNS right after a resize
shopt -s globstar             # `**` recurses
shopt -s autocd               # `..` is enough, like zsh's AUTO_CD


# -----------------------------------------------------------------------------
# 3. Completion
# -----------------------------------------------------------------------------

if ! shopt -oq posix; then
    if [ -r /run/current-system/sw/share/bash-completion/bash_completion ]; then
        . /run/current-system/sw/share/bash-completion/bash_completion
    elif [ -r /usr/share/bash-completion/bash_completion ]; then
        . /usr/share/bash-completion/bash_completion
    fi
fi


# -----------------------------------------------------------------------------
# 4. What is shared with zsh
# -----------------------------------------------------------------------------

[ -r "$HOME/.shellrc" ] && . "$HOME/.shellrc"


# -----------------------------------------------------------------------------
# 5. Keybindings
# -----------------------------------------------------------------------------
# Same Up-arrow behaviour as zsh: filter by what has been typed.

bind '"\e[A": history-search-backward' 2>/dev/null
bind '"\e[B": history-search-forward' 2>/dev/null


# -----------------------------------------------------------------------------
# 6. bash-preexec, then atuin
# -----------------------------------------------------------------------------
# bash has NO native preexec/precmd — that is zsh's. atuin needs them to know
# when a command started and when it ended, so `bash-preexec.sh` comes FIRST.
# Without it atuin loads, appears to work, and records nothing.

if [ -r /run/current-system/sw/share/bash-preexec/bash-preexec.sh ]; then
    . /run/current-system/sw/share/bash-preexec/bash-preexec.sh
fi

if command -v atuin >/dev/null 2>&1; then
    eval "$(atuin init bash --disable-up-arrow)"
fi


# -----------------------------------------------------------------------------
# 7. Starship — LAST
# -----------------------------------------------------------------------------

if command -v starship >/dev/null 2>&1; then
    eval "$(starship init bash)"
fi
