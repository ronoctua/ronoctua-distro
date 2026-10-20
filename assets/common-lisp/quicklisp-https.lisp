;;;; Quicklisp's downloads, over HTTPS.
;;;;
;;;; `quicklisp-install` puts this file in `~/quicklisp/local-init/`, which
;;;; Quicklisp loads every time it starts and BEFORE its first download (the
;;;; list of libraries it fetches on the first start comes after it).
;;;;
;;;; Quicklisp's own client fetches over plain HTTP, and of what it fetches it
;;;; checks only the size. This hands every download to curl, over HTTPS
;;;; only — a redirect to anything but HTTPS is refused — with the
;;;; post-quantum key exchange offered first. HTTPS proves the host that
;;;; answered: beta.quicklisp.org, or the HTTPS host one of its redirects
;;;; names. It does not prove who wrote what the host serves, because
;;;; Quicklisp signs nothing.
;;;;
;;;; A download lands in `FILE.part` and becomes FILE only whole and with a
;;;; 200: a cut connection or an error page never leaves a file Quicklisp
;;;; would take for the real one. An HTTP error is Quicklisp's own condition
;;;; (`ql-http:unexpected-http-status`), so its fallbacks — the plain file
;;;; when the gzipped one is missing, a client version that is not there —
;;;; work as they do with its own fetcher. Its proxy (`ql-http:*proxy-url*`)
;;;; is curl's too.
;;;;
;;;; It names curl by the system's path and no store path: this file lives in
;;;; the person's home, and a store path in it would outlive the generation
;;;; that held it.

(defpackage #:ronoctua-quicklisp-https
  (:use #:cl)
  (:export #:fetch))

(in-package #:ronoctua-quicklisp-https)

(defvar *curl* "/run/current-system/sw/bin/curl"
  "The curl every download runs.")

(defun https (url)
  "URL, with an `http://` at its start made `https://`."
  (if (and (>= (length url) 7) (string-equal "http://" url :end2 7))
      (concatenate 'string "https://" (subseq url 7))
      url))

(defun fetch (url file &key quietly &allow-other-keys)
  "Writes what URL holds to FILE, the way Quicklisp's `http-fetch` does, and
answers what it answers: a header that says 200, and the file."
  (let* ((url (https url))
         (file (merge-pathnames file))
         ;; `systems.txt.part` as a NAME with no type: renamed onto FILE,
         ;; nothing of it is merged into a FILE that has no type.
         (part (make-pathname :name (format nil "~A~@[.~A~].part"
                                            (pathname-name file) (pathname-type file))
                              :type nil
                              :defaults file))
         (proxy (let ((p ql-http:*proxy-url*))
                  (and p (if (stringp p) p (ql-http::urlstring p))))))
    (ensure-directories-exist file)
    (when (probe-file part)
      (delete-file part))
    (multiple-value-bind (said error-said status)
        (uiop:run-program
         (append (list *curl* "--location"
                       "--proto" "=https" "--proto-redir" "=https"
                       "--tlsv1.2" "--curves" "X25519MLKEM768:X25519:P-256"
                       "--write-out" "%{http_code}")
                 (when proxy
                   (list "--proxy" proxy))
                 (when quietly
                   (list "--silent" "--show-error"))
                 (list "--output" (uiop:native-namestring part) url))
         :output :string
         :error-output :interactive
         :ignore-error-status t)
      (declare (ignore error-said))
      (let ((code (ignore-errors (parse-integer (string-trim '(#\Space #\Newline) said)))))
        (unless (and (eql status 0) (eql code 200))
          (when (probe-file part)
            (delete-file part))
          (if (and (eql status 0) code)
              (error 'ql-http:unexpected-http-status :url url :status-code code)
              (error "Could not fetch ~A: curl exited with ~A" url status)))
        (uiop:rename-file-overwriting-target part file)))
    (values (make-instance 'ql-http::header :status 200)
            (probe-file file))))

;;; BOTH SCHEMES: Quicklisp's lists name `http://` addresses, and a future one
;;; may name `https://`.
(setf ql-http:*fetch-scheme-functions*
      (list* (cons "http" 'fetch)
             (cons "https" 'fetch)
             (remove-if (lambda (entry)
                          (member (car entry) '("http" "https") :test #'string=))
                        ql-http:*fetch-scheme-functions*)))
