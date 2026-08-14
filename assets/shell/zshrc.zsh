# shellcheck shell=bash
# =============================================================================
# .zshrc — only what is zsh's own
# =============================================================================
#
# Everything shared with bash lives in `.shellrc`, sourced in section 4.
# Here: `setopt`, `bindkey`, the completion system, and the two plugins plus
# atuin and starship, whose init differs per shell.
#
# ORDER MATTERS, and section 7 says why: starship is last so nothing overwrites
# the prompt, and syntax-highlighting is after autosuggestions because it wraps
# the widgets the other one installs.
# =============================================================================


# -----------------------------------------------------------------------------
# 1. Interactivity guard
# -----------------------------------------------------------------------------
# A non-interactive shell (scp, rsync, a script) has no use for any of this,
# and printing anything here breaks scp in a way that takes an hour to find.

case $- in
    *i*) ;;
    *) return ;;
esac


# -----------------------------------------------------------------------------
# 2. History
# -----------------------------------------------------------------------------
# The native file stays as a BACKUP. The source of truth is atuin (section 6);
# if it ever fails to load, this is what is left, and losing the history of a
# machine because a plugin broke is not acceptable.

HISTFILE="${XDG_STATE_HOME:-$HOME/.local/state}/zsh/history"
HISTSIZE=100000
SAVEHIST=100000
mkdir -p "${HISTFILE:h}"

setopt EXTENDED_HISTORY        # timestamp on every entry
setopt INC_APPEND_HISTORY      # write as it goes, not only on exit
setopt SHARE_HISTORY           # two terminals see each other's history
setopt HIST_IGNORE_DUPS
setopt HIST_IGNORE_SPACE       # a leading space keeps it out of the history
setopt HIST_REDUCE_BLANKS
setopt HIST_VERIFY             # `!!` expands for review, does not fire blind


# -----------------------------------------------------------------------------
# 3. Options
# -----------------------------------------------------------------------------

setopt AUTO_CD                 # `..` is enough, no `cd`
setopt AUTO_PUSHD              # every cd pushes; `cd -<Tab>` browses
setopt PUSHD_IGNORE_DUPS
setopt EXTENDED_GLOB
setopt NO_BEEP
setopt INTERACTIVE_COMMENTS    # `#` works when pasting a documented command
setopt COMPLETE_IN_WORD
setopt ALWAYS_TO_END


# -----------------------------------------------------------------------------
# 4. What is shared with bash
# -----------------------------------------------------------------------------
# `.shellrc` defines: XDG, PATH, aliases, functions, fzf, direnv, zoxide, and
# sources `~/.shellrc.local`. It comes BEFORE the plugins because they read the
# PATH it builds.

[ -r "$HOME/.shellrc" ] && . "$HOME/.shellrc"


# -----------------------------------------------------------------------------
# 5. Completion
# -----------------------------------------------------------------------------
# `compinit` with a cache in XDG. The `-C` skips the security check of the
# completion files, which on NixOS is a store path and cannot have been
# tampered with — it is the difference between a prompt in 40 ms and in 400.

autoload -Uz compinit
ZSH_COMPDUMP="${XDG_CACHE_HOME:-$HOME/.cache}/zsh/zcompdump"
mkdir -p "${ZSH_COMPDUMP:h}"
compinit -C -d "$ZSH_COMPDUMP"

zstyle ':completion:*' menu select
zstyle ':completion:*' matcher-list 'm:{a-zA-Z}={A-Za-z}'   # case-insensitive
zstyle ':completion:*' list-colors "${(s.:.)LS_COLORS}"
zstyle ':completion:*' use-cache on
zstyle ':completion:*' cache-path "${XDG_CACHE_HOME:-$HOME/.cache}/zsh/zcompcache"


# -----------------------------------------------------------------------------
# 6. Keybindings
# -----------------------------------------------------------------------------
# Emacs bindings, deliberately: the whole desktop behaves like Emacs, and a
# shell in vi mode inside it would be the one place where C-a means something
# else.

bindkey -e
bindkey '^[[A' history-search-backward     # Up filters by what was typed
bindkey '^[[B' history-search-forward
bindkey '^[[1;5C' forward-word             # Ctrl+Right
bindkey '^[[1;5D' backward-word            # Ctrl+Left
bindkey '^[[3~' delete-char
bindkey '^[[H' beginning-of-line
bindkey '^[[F' end-of-line


# -----------------------------------------------------------------------------
# 7. Plugins
# -----------------------------------------------------------------------------
# The paths come from the system closure. Each is guarded: on a machine where
# the package is absent the line is a silent no-op, which is what keeps this
# file identical everywhere.
#
# SYNTAX-HIGHLIGHTING GOES LAST, and it is not a style choice: it wraps the
# ZLE widgets, so anything installing widgets after it stays uncoloured.

if [ -r /run/current-system/sw/share/zsh-autosuggestions/zsh-autosuggestions.zsh ]; then
    . /run/current-system/sw/share/zsh-autosuggestions/zsh-autosuggestions.zsh
    ZSH_AUTOSUGGEST_STRATEGY=(history completion)
    ZSH_AUTOSUGGEST_BUFFER_MAX_SIZE=20
    bindkey '^ ' autosuggest-accept          # Ctrl+Space takes the suggestion
fi

if [ -r /run/current-system/sw/share/zsh-syntax-highlighting/zsh-syntax-highlighting.zsh ]; then
    . /run/current-system/sw/share/zsh-syntax-highlighting/zsh-syntax-highlighting.zsh
fi


# -----------------------------------------------------------------------------
# 8. atuin — history in SQLite
# -----------------------------------------------------------------------------
# zsh has native `preexec`/`precmd` hooks, so atuin plugs straight in.
#
# `--disable-up-arrow`: the Up arrow keeps filtering by prefix (section 6),
# which is muscle memory. Ctrl+R is what opens atuin's search.

if command -v atuin >/dev/null 2>&1; then
    eval "$(atuin init zsh --disable-up-arrow)"
fi


# -----------------------------------------------------------------------------
# 9. Starship — LAST
# -----------------------------------------------------------------------------
# Anything that touches PROMPT after this wins over it, and the symptom is a
# prompt that "sometimes comes back to the old one".

if command -v starship >/dev/null 2>&1; then
    eval "$(starship init zsh)"
fi
