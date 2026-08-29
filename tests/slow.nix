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
# It is not "felt slow". It is one of three things:
#
#   1. it builds an INSTALLATION — the `system.build.toplevel` of one of the
#      four NixOS systems, or the `isoImage`;
#   2. it partitions a disk, reboots the machine, or emulates a TPM;
#   3. it is an `mN`, which is the question "did the milestone close?" — a
#      question for publishing time, and one that mixes costs by construction
#      (`m1` drags in the ISO, `m7` drags in `impermanence-vm`).
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
]
