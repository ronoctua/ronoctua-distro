# The boot trial's watch: its name and the line under it, and the one thing it says.
brain-task-boot-fallback = Systems that did not start
brain-task-boot-fallback-does = Reads the machine's record and runs nothing. When an update's new system did not start, the machine has already come back to the last good one by itself (and when it was rolled back from the rescue console, the person sent it back); this says so once, on the minibuffer, so it does not go unnoticed.
notif-boot-fallback-failed-summary = The new system was set aside
notif-boot-fallback-failed-body = It did not start, or it was rolled back from the rescue console. This machine goes back to the system that last started well, and the next update tries again only once the distro or nixpkgs has moved on.
