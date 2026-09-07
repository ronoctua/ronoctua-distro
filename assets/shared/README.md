# What everybody gets

A filter written on one machine lives on that machine, under
`$XDG_CONFIG_HOME/ronoctua`. This directory is the other half: what somebody
decided was worth everybody's while.

`notifications.json` is a list of filters in the shape
`ronoctua_notify::Rule` serialises — the same shape the preferences file uses,
so nothing translates between them. `nix/notifications.nix` reads it at build
time and the rules join the distro's own table, which means they reach a
machine the only way distro content ever does: a build from the flake.

**This is a public repository.** What lands here is published, and there is no
practical way back — git keeps it and a public mirror copies it. The shell
never promotes anything on its own: it puts one named filter here, after a
screen that says exactly this and shows the text that would go.
