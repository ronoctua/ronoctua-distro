# The emacs-daemon module's own texts: its name and the line under it, the
# two things it may say on the strip, its one action, and what a refused
# start says.
brain-task-emacs-daemon = Watch the Emacs daemon
brain-task-emacs-daemon-does = Looks, a few times a day, for the Emacs server that holds your session and your configuration — and for an Emacs that is running WITHOUT it, which looks the same on screen and is not: it loaded its own copy, its buffers are not the session's, and closing it loses them. It reads the process table and nothing else, and the only thing it ever starts is the distro's own service.
emacs-daemon-says-no-daemon = The Emacs daemon is not running. Every editor window will load the whole configuration again, and none of them will share your buffers.
emacs-daemon-says-not-the-client = This Emacs is not connected to the daemon: it loaded its own copy of the configuration, and its buffers belong to it alone. Close it and open the file again from the file manager, which opens it in the daemon's Emacs.
cmd-emacs-daemon-start = Start it
emacs-daemon-why-refused = The Emacs daemon could not be started; see: journalctl --user -u emacs
