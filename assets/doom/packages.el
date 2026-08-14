;; -*- no-byte-compile: t; -*-
;;; packages.el
;;
;; Extra packages, beyond what the modules in `init.el` already pull.
;;
;; KEEP THIS SHORT. Every package here is one more thing that can break a
;; `doom sync` on the day one is in a hurry, and the modules already cover most
;; of what a package would be added for.

;; The distro writes Nix, and `nix-mode` is what makes that bearable outside of
;; the `:lang nix` module's LSP (which handles the editing, not the REPL).
(package! nix-mode)

;; KDL — the compositor's config language. niri's config is KDL, and without
;; this it opens as plain text, which is how a misplaced brace becomes a
;; compositor that does not start.
(package! kdl-mode)

;; WIT — the buffer/host boundary (`crates/ronoctua-protocol/wit/`). No mode
;; ships for it; `fundamental-mode` on the file that defines the whole contract
;; is worse than a rough approximation.
(package! wat-mode)
