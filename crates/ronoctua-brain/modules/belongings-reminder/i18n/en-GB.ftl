# The belongings' reminder: its name and the line under it, the hints of its
# four marks, and its page.
brain-task-belongings-reminder = Your belongings
brain-task-belongings-reminder-does = Reminds you to sync your belongings — your configuration, kept encrypted in your own repository — once the period you chose has gone by since the last sync, with a clock set for that moment alone. While a sync waits for your decisions, after one that failed, and before the first sync of a new machine, a mark on the modeline says so; a click on it opens the belongings, asking the passphrase. It runs nothing by itself: a sync asks your passphrase.
belongings-reminder-mark-due = A sync of your belongings is due
belongings-reminder-mark-conflicts = { $count ->
    [one] A sync of your belongings waits for one decision
   *[other] A sync of your belongings waits for { $count } decisions
    }
belongings-reminder-mark-failed = The last sync of your belongings did not work
belongings-reminder-mark-restore = Restore your belongings on this machine
belongings-reminder-heading = Your belongings
belongings-reminder-row-repository = The repository
belongings-reminder-row-set = set
belongings-reminder-row-last = Last sync
belongings-reminder-row-never = never on this machine
belongings-reminder-row-next = Reminded again
belongings-reminder-row-waiting = Decisions waiting
belongings-reminder-row-failed = The last sync did not work
belongings-reminder-row-unset = No repository is set: there is nothing to remind of.
