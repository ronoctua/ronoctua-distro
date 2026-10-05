# The clock-sync module's own texts: its name and the line under it, the
# three things it may say on the strip, its one action, and what a refused
# try says — to the press, and in the activity log.
brain-task-clock-sync = Watch the clock
brain-task-clock-sync-does = Checks, a few times a day, that something is keeping this machine's clock right over the internet — and turns it back on when nothing is. It does not ask anybody for the time itself: the system's own time client does that continuously, and a second one would be two clocks disagreeing. Switch this off to keep the clock by hand.
clock-sync-says-not-automatic = Nothing is keeping this machine's clock right. It will drift, and every timestamp on this machine drifts with it.
clock-sync-says-not-synchronised = The clock is being looked after, but it has not been able to reach a time server yet.
cmd-clock-sync-keep = Keep it right
clock-sync-says-refused = Nothing is keeping this machine's clock right, and turning it on was refused; see: journalctl -u systemd-timedated
clock-sync-why-refused = The clock could not be handed to the network; see: journalctl -u systemd-timedated
clock-sync-noted-refused = Turning the clock's keeping on was refused: { $said }
