# The disk-space module's own texts: its name and the line under it, the
# reason its row gives instead of a date, the hint of its mark, its page, and
# the two things it tells the footer.
brain-task-disk-space = Make room on the disk
brain-task-disk-space-does = When the disk with the system or your home runs low on space, it frees space right away, without waiting for the night or for a quiet machine. First it removes compiler builds nobody uses and, while cargo's builds in your cache take more than their share of the disk, the oldest of them; then the store's garbage, until the disk is back where it should be. It never touches a build cargo is working in, never follows a link, and never removes what Nix keeps as a root or any generation. If a try could not free enough, it waits for the disk to fill further before trying again. While the disk is nearly full, a mark on the modeline says so, with how full it is.
brain-task-disk-space-brief = Makes room on the disk as soon as it runs short of space.
disk-space-why-when-short = When it runs low
disk-space-mark-full = The disk is nearly full: { $count }% of it is used
disk-space-heading-now = The disk now
disk-space-row-free = Free now
disk-space-row-size = Size
disk-space-row-floor = Room is made under
disk-space-row-nearly-full = Nearly full under
disk-space-row-unmeasured = The disk has not been measured yet
notif-disk-space-relieved-summary = Room made on the disk: { $freed } freed
notif-disk-space-relieved-body = { $free }% of it is free now.
notif-disk-space-still-short-summary = The disk is still short of space
notif-disk-space-still-short-body = Only { $free }% of it is free, and nothing more could go without you. What fills it is yours to choose: files, programs, the history of your own profile, or the system's old generations (System memory can trim them). This is said once, until room is made again.
