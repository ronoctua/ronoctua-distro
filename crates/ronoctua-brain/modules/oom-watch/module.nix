# Nothing of its own on the machine: the watch runs inside the brain, on the
# journal follower the core already starts (`RONOCTUA_JOURNALCTL`, set by
# `nix/modules/brain.nix`), and the person reads the system's journal through
# the ACL journald gives `wheel`. What makes a kill rare — `systemd-oomd`
# acting first, on `app.slice` — is `nix/modules/memory.nix`'s.
#
# A FILE ALL THE SAME, because every module has one: the loader gates it with
# `ronoctua.brain.modules.<id>.enable`, and a module that one day needs a unit
# or a rule adds it here, where the gate already stands.
{ }
