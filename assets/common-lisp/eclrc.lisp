;;;; ~/.eclrc — ECL reads it when it starts.
;;;;
;;;; It is yours: the system writes it only when it is missing — at the
;;;; first activation with Common Lisp, and again at the next one if you
;;;; delete it — and never over a file or a link that is there. It travels
;;;; with your belongings (the `common-lisp` target).

;;; ASDF, ECL's own. The `ecl` command finds the system's libraries through
;;; the ASDF configuration it puts on XDG_CONFIG_DIRS, and compiles each the
;;; first time it is loaded, into ~/.cache/common-lisp.
(require "asdf")

;;; Quicklisp, once `quicklisp-install` put it in your home — and not under
;;; qlot, whose `qlot exec` names the project's own in QUICKLISP_HOME.
(let ((setup (merge-pathnames "quicklisp/setup.lisp" (user-homedir-pathname))))
  (when (and (probe-file setup)
             (not (uiop:getenv "QUICKLISP_HOME")))
    (load setup)))
