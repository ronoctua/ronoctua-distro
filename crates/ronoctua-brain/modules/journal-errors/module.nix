# Nothing of its own on the machine: the watch runs inside the brain, on the
# journal follower the core already starts (`RONOCTUA_JOURNALCTL`, set by
# `nix/modules/brain.nix`), which reads the system's journal and the person's
# own — the first through the ACL journald gives `wheel`.
#
# A FILE ALL THE SAME, because every module has one: the loader gates it with
# `ronoctua.brain.modules.<id>.enable`, and a module that one day needs a unit
# or a rule adds it here, where the gate already stands.
{ }
