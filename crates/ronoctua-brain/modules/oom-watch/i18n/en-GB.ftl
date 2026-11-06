# The oom-watch module's own texts: its name and the line under it, its page,
# the record it writes in the activity, the notification of a kill, and the
# hint of the RAM's mark ($count is the share of the RAM in use, in per cent).
brain-task-oom-watch = Closed for lack of memory
brain-task-oom-watch-does = When the machine runs out of memory, the kernel or systemd-oomd closes a program so the rest can keep going. Each time, this writes it down in the activity and tells you once: what was closed, where, how much memory it held, how much the machine was stalling, and when. While the RAM is nearly all in use, a mark on the modeline says how much of it is.
brain-task-oom-watch-brief = Writes down every program closed because memory ran out.
oom-watch-mark-ram = { $count }% of the RAM is in use. Click to see what is using it.
oom-watch-heading-journal = Following the journal
oom-watch-heading-recent = Closed lately
oom-watch-row-none = Nothing has been closed for lack of memory since this began watching
oom-watch-nowhere = a group the journal did not name
oom-watch-by-kernel = The kernel
oom-watch-by-machine = Out of memory
oom-watch-by-limit = Its group's limit
oom-watch-by-oomd = systemd-oomd
oom-watch-by-pressure = oomd: stalling
oom-watch-by-full = oomd: memory full
oom-watch-unmeasured = not measured
oom-watch-noted-killed = { $what } was closed for lack of memory
notif-oom-watch-killed-summary = { $what } was closed for lack of memory
notif-oom-watch-killed-body = { $by ->
        [machine] The kernel killed it: the whole machine had run out of memory.
        [limit] The kernel killed it: it had reached the memory limit of its group.
        [kernel] The kernel killed it for lack of memory.
        [pressure] systemd-oomd closed it: the machine had been stalling for memory { $stall }% of the time, over its limit of { $limit }%.
        [full] systemd-oomd closed it: memory and swap were nearly full.
       *[oomd] systemd-oomd closed it for lack of memory.
    } { $pid ->
        [0] Where: { $where }.
       *[other] Process { $pid }, in { $where }.
    } Memory: { $memory }. { $measured ->
        [yes] The machine was stalling { $stall }% of the time. At { $at }.
       *[no] At { $at }.
    }
