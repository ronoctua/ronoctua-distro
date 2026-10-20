;;;; ~/.sbclrc — SBCL reads it when it starts.
;;;;
;;;; It is yours: the system writes it only when it is missing — at the
;;;; first activation with Common Lisp, and again at the next one if you
;;;; delete it — and never over a file or a link that is there. It travels
;;;; with your belongings (the `common-lisp` target).

;;; ASDF: the one the system's libraries were built with, which the `sbcl`
;;; command names in $ASDF; else, or when that one does not load, SBCL's own
;;; (a bare SBCL, one under qlot).
(unless (find-package "ASDF")
  (let ((asdf (sb-ext:posix-getenv "ASDF")))
    (unless (and asdf
                 (plusp (length asdf))
                 (handler-case (progn (load asdf) t)
                   (error (e)
                     (format *error-output* "~&;; ~A did not load (~A); SBCL's own ASDF instead.~%"
                             asdf e)
                     nil)))
      (require "asdf"))))

;;; Quicklisp, once `quicklisp-install` put it in your home — and not under
;;; qlot, whose `qlot exec` names the project's own in QUICKLISP_HOME.
(let ((setup (merge-pathnames "quicklisp/setup.lisp" (user-homedir-pathname))))
  (when (and (probe-file setup)
             (not (uiop:getenv "QUICKLISP_HOME")))
    (load setup)))
