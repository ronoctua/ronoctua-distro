# The texts `ronoctua-shell-power-words` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## Power — the battery, the charger, the profile and sleep (LEADER s p)

power-time-empty = { $time } left
power-time-full = { $time } to full
power-time-unknown = time unknown
power-duration = { $h } h { $m } min
power-duration-minutes = { $m } min
power-state-unknown = unknown
power-state-charging = charging
power-state-discharging = discharging
power-state-empty = empty
power-state-full = full
power-state-pending-charge = waiting to charge
power-state-pending-discharge = waiting to discharge
power-profile-power-saver = power saver
power-keep-awake = keep awake: no sleep, no idle action
power-keep-lid = ignore the lid: closing it does nothing
# THE DAEMONS' WORDS — logind's kebab-case and UPower's CamelCase land on
# the same key (`power_words::daemon_word`); a word outside this list is shown raw.
power-duration-hours = { $h } h
power-word-ignore = do nothing
power-word-poweroff = power off
power-word-reboot = reboot
power-word-halt = halt
power-word-kexec = reboot through kexec
power-word-suspend = suspend
power-word-hibernate = hibernate
power-word-hybrid-sleep = go into hybrid sleep
power-word-suspend-then-hibernate = suspend, then hibernate
power-word-lock = lock the screen
power-word-factory-reset = factory reset
power-word-soft-reboot = soft reboot
# THE KERNEL'S TWO CHARGE VOCABULARIES — `charge_behaviour` and `charge_type`,
# from the `power_supply` class; the raw word shows only for one we do not know.
power-behaviour-auto = charge normally
power-behaviour-inhibit-charge = hold the charge where it is
power-behaviour-inhibit-charge-awake = hold the charge while awake
power-behaviour-force-discharge = run off the battery
power-charge-type-unknown = unknown
power-charge-type-na = not applicable
power-charge-type-trickle = trickle
power-charge-type-fast = fast
power-charge-type-standard = standard
power-charge-type-adaptive = adaptive
power-charge-type-custom = custom
power-charge-type-long-life = long life
power-charge-type-bypass = straight from the charger
power-what-sleep = sleep
power-what-idle = idling
power-what-shutdown = shutdown
power-what-handle-power-key = the power key
power-what-handle-suspend-key = the suspend key
power-what-handle-hibernate-key = the hibernate key
power-what-handle-lid-switch = the lid
power-what-handle-reboot-key = the reboot key
power-word-sleep = sleep
power-what-profile = the profile
power-what-lock = the lock
power-what-thresholds = the thresholds
power-what-action = the action
power-refused-authority = not allowed for this session
power-refused-absent = the daemon is not there, or the hardware cannot
power-refused-invalid = the value was refused
power-refused-other = refused
echo-power-refused = { $what } { $why }

## file info editing — everything the info panel can change

# The power and Bluetooth refusals that had no name, named: the daemon's
# own words go to the journal, never to the line (`power_words.rs`,
# `bluetooth_words.rs`).
power-refused-timed-out = got no answer in time
power-refused-failed = could not be done
power-refused-no-bus = could not reach the system bus
