;;; config.el -*- lexical-binding: t; -*-
;;
;; Versioned; home-manager writes it to ~/.config/doom. Edit it in the
;; repository, not in $HOME — the copy there is replaced on the next rebuild.

;; -----------------------------------------------------------------------------
;; Identity
;; -----------------------------------------------------------------------------
;; NOT SET HERE, and not by accident: `user-full-name` and `user-mail-address`
;; are personal data, and invariant 13 keeps them out of the repository. They
;; belong in `~/.doom.d/local.el`, loaded at the end of this file, which is
;; never versioned — the same arrangement as `~/.shellrc.local`.

;; -----------------------------------------------------------------------------
;; Typography — the system font, and only it
;; -----------------------------------------------------------------------------
;; The whole desktop draws in `noctua-iosevka`. Emacs picking its own font would
;; be the one window whose text does not match the footer two lines below it.
;;
;; The size is NOT a token: Emacs is a foreign application (section 4.2) and does
;; not read `assets/tokens/`. This is a value chosen to match, and if the token
;; ever moves this line has to move with it — which is exactly why it is called
;; out instead of being left to look like a coincidence.
(setq doom-font (font-spec :family "noctua-iosevka" :size 14)
      doom-variable-pitch-font (font-spec :family "noctua-iosevka" :size 14)
      doom-theme 'doom-one)

;; -----------------------------------------------------------------------------
;; No second desktop
;; -----------------------------------------------------------------------------
;; Invariant 14: the footer is the system's only chrome. Emacs drawing its own
;; toolbar, scroll bar and tooltips would put a small traditional desktop inside
;; a system built not to have one.
(setq default-frame-alist
      (append '((tool-bar-lines . 0)
                (menu-bar-lines . 0)
                (vertical-scroll-bars . nil)
                (right-fringe . 0))
              default-frame-alist))
(setq tooltip-use-echo-area t)

;; Line numbers off: the footer already says where the point is, and a gutter of
;; numbers in every buffer is chrome by another name.
(setq display-line-numbers-type nil)

;; -----------------------------------------------------------------------------
;; Files stay out of $HOME's root
;; -----------------------------------------------------------------------------
;; Same rule the shell follows. Emacs is one of the worst offenders here: without
;; this it writes backups, auto-saves and `custom.el` straight into $HOME.
(setq custom-file (expand-file-name "custom.el" doom-cache-dir))
(setq backup-directory-alist
      `(("." . ,(expand-file-name "backup" doom-cache-dir))))
(setq auto-save-file-name-transforms
      `((".*" ,(expand-file-name "auto-save/" doom-cache-dir) t)))

;; -----------------------------------------------------------------------------
;; Projects
;; -----------------------------------------------------------------------------
;; `XDG_PROJECTS_DIR` is where the code lives on this machine. Reading it from
;; the environment instead of hard-coding the path keeps ONE source of truth —
;; `nix/modules/home.nix` sets it, and section 2 fixes it.
(setq projectile-project-search-path
      (list (or (getenv "XDG_PROJECTS_DIR")
                (expand-file-name "main/prj" (getenv "HOME")))))

;; -----------------------------------------------------------------------------
;; The daemon
;; -----------------------------------------------------------------------------
;; `e` and `et` in the shell talk to it. Confirming on exit is what keeps a
;; stray `C-x C-c` in one frame from taking the daemon — and every other frame —
;; down with it.
(setq confirm-kill-emacs #'yes-or-no-p)

;; -----------------------------------------------------------------------------
;; This machine's own config — LAST, so it wins
;; -----------------------------------------------------------------------------
(let ((local (expand-file-name "local.el" doom-user-dir)))
  (when (file-readable-p local)
    (load local nil 'nomessage)))
