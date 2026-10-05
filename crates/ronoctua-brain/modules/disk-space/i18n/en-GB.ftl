# The disk-space module's own texts: its name and the line under it, the
# reason its row gives instead of a date, the hint of its mark, its page, and
# the two things it tells the footer.
brain-task-disk-space = Make room on the disk
brain-task-disk-space-does = When the disk that holds the system or your home runs short of space, it makes room at once, without waiting for the night or for a quiet machine: first the builds of compilers nobody uses and, while every cargo build under your cache weighs more than its share of the disk, the ones built longest ago; then the store's garbage, until the disk is back where it should be. It takes a build only when no cargo is building in it, never follows a link, never removes what Nix counts as a root and never a generation, and after a try that could not make enough room it waits until the disk has filled further before trying again. While the disk is nearly full, a mark on the modeline says so.
disk-space-why-when-short = When it runs low
disk-space-mark-full = The disk is nearly full
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
