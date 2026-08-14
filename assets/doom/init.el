;;; init.el -*- lexical-binding: t; -*-
;;
;; GENERATED-ADJACENT: this file is versioned and home-manager writes it to
;; ~/.config/doom. Editing the copy in $HOME works until the next rebuild, which
;; overwrites it — edit `assets/doom/init.el` in the repository instead.
;;
;; WHAT IS DELIBERATELY OFF
;; ------------------------
;; The desktop already IS the Emacs metaphor: the footer is the modeline and the
;; minibuffer, `LEADER` is the leader, and which-key lives there. Turning on
;; Doom's own UI chrome would give two of everything, and section 3 says the
;; system does not look like a traditional desktop.

(doom! :input

       :completion
       (corfu +orderless)               ; in-buffer completion
       (vertico +icons)                 ; the minibuffer's own selector

       :ui
       doom                             ; the theme
       hl-todo
       modeline
       ophints                          ; flash what an operator touched
       (popup +defaults)
       vc-gutter
       vi-tilde-fringe
       workspaces

       :editor
       (evil +everywhere)               ; modal editing, opt-in per buffer
       file-templates
       fold
       (format +onsave)
       snippets

       :emacs
       (dired +icons)
       electric
       (ibuffer +icons)
       undo
       vc

       :term
       ;; NO `vterm`, NO `eshell`. The system has a terminal buffer of its own
       ;; (`crates/ronoctua-term`), and a second one inside Emacs would be a
       ;; second answer to "where do I run this".

       :checkers
       syntax
       (spell +flyspell)

       :tools
       direnv                           ; the devshell follows the buffer
       (eval +overlay)
       lookup
       lsp
       magit
       tree-sitter

       :lang
       (cc +lsp)
       data
       emacs-lisp
       (json +lsp)
       markdown
       (nix +lsp)
       (org +pretty)
       (python +lsp +pyright)
       (rust +lsp)
       (sh +lsp)
       (web +lsp)
       yaml

       :config
       (default +bindings +smartparens))
