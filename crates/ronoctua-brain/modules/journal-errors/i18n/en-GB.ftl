# The journal-errors module's own texts: its name and the line under it, its
# page, the hint of its mark, its four actions and the words it refuses with.
brain-task-journal-errors = Errors in the journal
brain-task-journal-errors-does = Errors the system and your own programs write in the journal, grouped: the same failure with other numbers is one row, with a count. A mark on the modeline counts the kinds you have not seen yet.
journal-errors-heading-journal = Following the journal
journal-errors-heading-unseen = Not seen yet — choose one to mark it seen
journal-errors-heading-earlier = Not seen yet, from an earlier boot — choose one to mark it seen
journal-errors-heading-seen = Seen — choose one to ignore its kind from now on
journal-errors-heading-ignored = Ignored, with how many since — choose one to hear it again
journal-errors-row-none = No error has been written in the journal since this began watching
journal-errors-mark-unseen = { $count ->
        [one] One kind of error in the journal not seen yet
       *[other] { $count } kinds of error in the journal not seen yet
    }
journal-errors-why-gone = That error is no longer in the list
journal-errors-why-no-row = Choose an error on its page first: this acts on one row
journal-errors-why-unreadable = The file of ignored kinds does not parse, so it is left as it is: fix it or remove it first
cmd-journal-errors-seen = Mark this error seen
cmd-journal-errors-ignore = Ignore this kind of error
cmd-journal-errors-unignore = Hear this kind of error again
cmd-journal-errors-seen-all = Mark every error seen
