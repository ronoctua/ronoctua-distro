# The reminders' own texts: its name and the line under it, the alarm and the
# missed notice, its page, its actions and their questions, what it writes into
# the activity log, why it refuses — and the words an answer is read with
# (`reminders-grammar-*`: one line per class of word, the alternatives
# separated by commas).
brain-task-reminders = Reminders and alarms
brain-task-reminders-does = Alarms and reminders you ask for in your own words: once (“tomorrow 9:00 call mum”, “in 2 h with a sound take the pizza out”), every day, on days of the week, on a day of the month, or on a date every year, like a birthday. At its moment it rings on the brain’s line, ahead of everything else, and stays there until you dismiss it or put it off; one that came due while the machine was off shows as missed instead. A reminder with a sound plays a chime a few times while it rings.
reminders-mark-ringing = A reminder is ringing
reminders-says-due = { $more ->
        [0] { $text }
       *[other] { $text } (and { $more } more)
    }
reminders-says-missed = { $more ->
        [0] Missed while the machine was off: { $text }, due { $at }
       *[other] Missed while the machine was off: { $text }, due { $at } (and { $more } more)
    }
reminders-says-unreadable = { $held ->
        [0] The reminders file cannot be read, so none rings: fix or remove it
        [one] The reminders file cannot be read: fix or remove it. The one read before it broke still rings
       *[other] The reminders file cannot be read: fix or remove it. The { $held } read before it broke still ring
    }
reminders-untitled = Reminder
reminders-why-not-understood = I could not tell when. Try “tomorrow 9:00 call mum”, “in 2 h tea”, “every monday 8:30 standup” or “every 12/03 Ana’s birthday”.
reminders-why-passed = That moment has already passed.
reminders-why-no-such-day = That day is not in the calendar.
reminders-why-unreadable = The reminders file cannot be read, so nothing changes until it is fixed or removed.
reminders-why-gone = That reminder is not there any more.
reminders-why-not-ringing = That reminder is not ringing.
reminders-why-full = There are too many reminders; forget some first.
cmd-reminders-new = New reminder
ask-reminders-new = When, and what? (tomorrow 9:00 call mum)
cmd-reminders-when = Change when it rings
ask-reminders-when = When? (tomorrow 9:00, every monday 8:30)
cmd-reminders-repeat = Change how it repeats
cmd-reminders-sound = Turn its chime on or off
cmd-reminders-forget = Forget this reminder
cmd-reminders-dismiss = Dismiss this alarm
cmd-reminders-snooze = Put this alarm off
reminders-heading-ringing = Ringing now
reminders-heading-missed = Missed while the machine was off
reminders-heading-list = Your reminders
reminders-row-none = No reminders yet
reminders-row-file = The file
reminders-row-unreadable = It cannot be read: fix or remove it to go on
reminders-row-dismiss = dismiss it
reminders-row-snooze = Put it off, minutes
reminders-row-repeat = Repeat it another way
reminders-row-days = On
reminders-row-day = On day
reminders-row-sound = Sound
reminders-row-forget = Forget it
reminders-repeat-once = Once
reminders-repeat-day = Every day
reminders-repeat-week = Every week
reminders-repeat-month = Every month
reminders-repeat-year = Every year
reminders-sound-on = Plays a chime
reminders-sound-off = Silent
reminders-weekday-monday = Mondays
reminders-weekday-tuesday = Tuesdays
reminders-weekday-wednesday = Wednesdays
reminders-weekday-thursday = Thursdays
reminders-weekday-friday = Fridays
reminders-weekday-saturday = Saturdays
reminders-weekday-sunday = Sundays
reminders-noted-set = Set: { $text }, for { $at }
reminders-noted-rang = Rang: { $text }
reminders-noted-missed = Missed: { $text }, due { $at }
reminders-grammar-in = in, within
reminders-grammar-minute = min, mins, minute, minutes, m
reminders-grammar-hour = h, hr, hrs, hour, hours
reminders-grammar-day = day, days
reminders-grammar-week = week, weeks
reminders-grammar-month = month, months
reminders-grammar-year = year, years
reminders-grammar-one = a, an, one
reminders-grammar-half-hour = half an hour, half hour, half-hour
reminders-grammar-today = today
reminders-grammar-tomorrow = tomorrow, tmrw
reminders-grammar-after-tomorrow = the day after tomorrow, day after tomorrow
reminders-grammar-every = every, each
reminders-grammar-daily = daily
reminders-grammar-weekly = weekly
reminders-grammar-monthly = monthly
reminders-grammar-yearly = yearly, annually
reminders-grammar-at = at, @
reminders-grammar-and = and, &
reminders-grammar-filler = on, the, of
reminders-grammar-am = am, a.m.
reminders-grammar-pm = pm, p.m.
reminders-grammar-noon = noon, midday
reminders-grammar-midnight = midnight
reminders-grammar-ordinal = st, nd, rd, th
reminders-grammar-monday = monday, mondays, mon
reminders-grammar-tuesday = tuesday, tuesdays, tue, tues
reminders-grammar-wednesday = wednesday, wednesdays, wed
reminders-grammar-thursday = thursday, thursdays, thu, thur, thurs
reminders-grammar-friday = friday, fridays, fri
reminders-grammar-saturday = saturday, saturdays, sat
reminders-grammar-sunday = sunday, sundays, sun
reminders-grammar-january = january, jan
reminders-grammar-february = february, feb
reminders-grammar-march = march, mar
reminders-grammar-april = april, apr
reminders-grammar-may = may
reminders-grammar-june = june, jun
reminders-grammar-july = july, jul
reminders-grammar-august = august, aug
reminders-grammar-september = september, sep, sept
reminders-grammar-october = october, oct
reminders-grammar-november = november, nov
reminders-grammar-december = december, dec
reminders-grammar-sound-on = with a sound, with sound, with an alarm, with alarm, loud
reminders-grammar-sound-off = without a sound, without sound, silent, silently, quietly
