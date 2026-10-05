# WHAT DOES NOT FIT IN AN EVERYDAY ROUND.
#
# WHY THE SLOW SIDE IS NAMED, AND NOT THE FAST ONE
# ------------------------------------------------
# Because the right default for a new check is to join the everyday battery. A
# cheap check left out by forgetfulness would give no signal at all — which is
# exactly what happened to the hand-written list of `nix run .#quick`, which
# promised "the checks that do not bring up a VM" and left dozens of them out,
# including nine written the same day this was.
#
# Naming the slow side makes forgetfulness fall on the safe side: whoever
# invents an expensive check and does not register it here makes `most` slower,
# and that shows up on the clock. The opposite — a check silently dropping out
# of verification — never shows up at all.
#
# THE GUARD
# ---------
# `test-tiers-are-total`, in `tests/checks.nix`, demands that every name here
# exist in `checks`. A renamed or removed check makes the guard fail SAYING
# WHICH, instead of leaving a dead line that excludes nothing.
#
# THE CRITERION IS STRUCTURAL
# ---------------------------
# It is not "felt slow". It is one of four things:
#
#   1. it builds an INSTALLATION — the `system.build.toplevel` of one of the
#      four NixOS systems, or the `isoImage`;
#   2. it partitions a disk, reboots the machine, or emulates a TPM;
#   3. it is an `mN`, which is the question "did the milestone close?" — a
#      question for publishing time, and one that mixes costs by construction
#      (`m1` drags in the ISO, `m7` drags in `impermanence-vm`);
#   4. it compiles again something the battery has already compiled, only for
#      another target — which is one derivation and one reason, and is written
#      out at the bottom rather than folded into the three above.
#
# Where there is a number, it is on the line, and it was measured — not guessed.
[
  # ---------------------------------------------------------------
  # 1. THEY BUILD AN INSTALLATION
  # ---------------------------------------------------------------
  # The two whole systems side by side, to compare what goes into each profile.
  # `full` carries Android Studio: 1.47 GB of download the first time.
  "profile-minimal-closes"
  # The ISO, measured at 3712 MiB. It has no ceiling any more — see the
  # check itself — but building the image is still building an installation.
  "iso-is-measured"
  "iso-has-no-toolchain"
  "iso-carries-the-seed"
  # These two read the `toplevel` of the VM's system — cheap to evaluate,
  # expensive to build the first time.
  "browser-has-no-keyring"
  "memory-policy-shape"

  # ---------------------------------------------------------------
  # 2. IT PARTITIONS, REBOOTS OR EMULATES A TPM
  # ---------------------------------------------------------------
  # lanzaboote, a signed UKI, swtpm — and a DISK IMAGE of the whole system,
  # which is criterion 1 under another name. MEASURED: with the image already
  # in the store the script takes 98 s (105 s wall); what costs is building it.
  # The old note of "~2 h with no output" was the FIRST time, not the check.
  "secureboot-vm"

  # WHAT WAS MEASURED AND DID NOT GET IN.
  #
  # These candidates looked like they belonged in this section by what they do
  # — partition a disk, reboot the machine, emulate a TPM. Timed with
  # `--rebuild`, which forces them to run again:
  #
  #   disko-vm          77 s   really partitions, and boots afterwards
  #   oom-vm            49 s   two machines, one under pressure
  #   x11-vm            30 s   Xvfb
  #   wifi-tpm-vm       30 s   swtpm and the NetworkManager agent
  #   files-vm          28 s   the file manager, nine subtests
  #   tpm-vm            22 s   swtpm
  #   impermanence-vm   21 s   two boots
  #
  # The battery's other VMs take 17 to 48 s. None of these falls far outside
  # that band, so none is "very slow" — and taking them out would cost coverage
  # without giving back time. They stay in `.#most`.
  #
  # That is why this list's criterion is STRUCTURAL and the exception is
  # measured: what these do SOUNDS expensive, and is not.

  # ---------------------------------------------------------------
  # 3. THE EIGHT MILESTONES
  # ---------------------------------------------------------------
  # Apart from four checks that only existed inside them — and that now have
  # attributes of their own — the `mN` are duplication of what already is an
  # attribute. So taking them out of the everyday battery takes NOTHING from it.
  "m1"
  "m2"
  "m3"
  "m4"
  "m5"
  "m6"
  "m7"
  "m8"

  # ---------------------------------------------------------------
  # 4. IT BUILDS THE SAME THING TWICE
  # ---------------------------------------------------------------
  # A FOURTH CRITERION, and it is written here rather than smuggled under one of
  # the three above, because it does not meet any of them: `buffers-test` builds
  # no installation, partitions nothing, and is not an `mN`.
  #
  # What it does is compile the eight buffers a SECOND time, for the host, so
  # their sixty-nine tests can run — a test has to run, and this machine does
  # not run `wasm32-wasip2`. Each buffer is its own workspace, so
  # `ronoctua-design`, `ronoctua-protocol` and `wit-bindgen` are compiled eight
  # times over with nothing shared, and the three browsers put their 1-to-5 MB
  # embedded database through the compiler again.
  #
  # MEASURED, with `--rebuild`: the eight go from 27.8 s to 2 min 44 s. The
  # +2 min 16 s is +14% on a battery of sixteen minutes, for a set of tests that
  # changes only when a buffer's own source does.
  #
  # The exception this criterion admits is narrow on purpose: "it duplicates a
  # compilation the battery has already paid for". A check that is merely slow
  # still does not belong here — section 2's measured table is what that rule
  # looks like when it is applied honestly.
  "buffers-test"
  # And the THIRD compilation of the same eight, for clippy. It is here by the
  # same criterion and the same number: the buffers are built once for the
  # `.wasm` every check links, a second time to run their tests, and a third
  # to be read. What it buys is the only lint they have — `cargo clippy
  # --workspace` cannot reach a buffer, and a buffer with a baked table could
  # not be linted outside Nix at all, which is how a dead function sat in the
  # old Unicode browser across releases.
  "buffers-lint"

  # THE HOME GIVEN BACK FROM /persist: two boots and a third, each a
  # machine coming up — the structural criterion of the reboots above.
  "persist-home-vm"

  # `security-vm` meets none of the four criteria: three small nodes built
  # from one module, no installation, no disk, no reboot (the sleep hook is
  # started, never slept), no TPM, one compilation. It stays in `.#most`; its
  # time is to be measured the first time the battery runs it.

  # THE RESIZER, IN A COMPOSITOR: a VM of its own for one tool of the
  # developer's, which the everyday battery has no reason to pay for.
  "resizer-vm"

  # IT REBOOTS NOW (criterion 2): the extension's id has to survive a boot,
  # kept in `/persist` — two boots of one machine.
  "extension-vm"

  # `~/.cache` A SUBVOLUME ACROSS THREE REBOOTS of a machine with a disk of
  # its own (criterion 2).
  "cache-subvolume-vm"

  # ---------------------------------------------------------------
  # WEIGHED, AND LEFT IN THE EVERYDAY BATTERY
  # ---------------------------------------------------------------
  # The software centre's eleven checks meet none of the four criteria. They
  # read sources and EVALUATED options — the systems are evaluated, never
  # built — and their derivations were read (`nix-store -q --requisites`)
  # to hold nothing of the workspace, the shell, Waydroid or appimage-run.
  # `nix-index-is-whole` reads the built search index, which every system
  # needs anyway and which is data, not an installation (the M1 gate
  # measured 4.2 GB at the peak and 36 s to build it).
  #
  # The update's never-asks check (its brain module's, now) changed
  # underneath: its runner names the software CLI, which is copied out of
  # the same `workspace` derivation as `rust.shell`. That is a compilation the battery already pays for, not a
  # second one, so criterion 4 does not reach it either.
  #
  # `software-vm` boots ONE machine from a handful of modules — not the
  # `toplevel` of any of the four systems — and partitions nothing, reboots
  # nothing and emulates no TPM. What it builds beyond its script is the same
  # workspace every VM test links, `appimage-run` and Flatpak. MEASURED with
  # `--rebuild`, the workspace already built: 35 s, the script 26.5 s — inside
  # the band of the battery's other VMs.
]
