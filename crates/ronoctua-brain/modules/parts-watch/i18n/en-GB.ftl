# The parts' watch's own texts: its name and the line under it, its marks,
# its two words on the minibuffer, and its page.
brain-task-parts-watch = Parts on their way
brain-task-parts-watch-does = While any of the machine's parts — what arrives after the installation, each on its own — is on its way or needs you, a mark on the modeline says how many, and a click on it opens the parts' page. It reads the records the parts' realizer writes, so it runs nothing and costs nothing while every part is here.
parts-watch-mark-arriving = { $count ->
        [one] One part is on its way
       *[other] { $count } parts are on their way
    }
parts-watch-mark-needs-you = { $count ->
        [one] One part needs you
       *[other] { $count } parts need you
    }
notif-parts-watch-arrived-summary = { $name } arrived
notif-parts-watch-arrived-body = It is ready to use.
notif-parts-watch-needs-you-summary = { $name } needs you
notif-parts-watch-needs-you-body = Its download keeps failing. It is still tried, less often each time; the parts' page has other ways to bring it in.
parts-watch-heading-parts = Parts
parts-watch-row-unwatched = Not being watched
parts-watch-phase-here = here
parts-watch-phase-arriving = on its way
parts-watch-phase-waiting = waiting for a network it may use
parts-watch-phase-needs-you = needs you
parts-watch-phase-unwanted = no longer wanted
