# WHAT A BLUETOOTH SERVICE ID MEANS — one place, and generated from here.
#
# A device announces what it can do as a list of UUIDs, and a person reading
# `00001812-0000-1000-8000-00805f9b34fb` learns nothing. This file is the
# translation, and — in the same shape as `assets/notifications.nix` — it
# becomes three things at build time: the Rust table `ronoctua_bluetooth`
# consults, the list of text keys BOTH languages have to carry, and the refusal
# of an entry that is neither named nor hidden.
#
# Written as a `match` in Rust instead, "which services does this system know?"
# would be answerable only by a grep, and the day somebody added one without a
# sentence the screen would say `[bluetooth-profile-…]` in the middle of a list
# nobody audits. That is exactly what happened to the notifications, and it is
# why they moved here first.
#
# # ONLY THE SHORT FORM, AND ONLY THE REAL BASE
#
# The key is the four hex digits of a SIG-assigned 16-bit id, which the Bluetooth
# base UUID wraps as `0000XXXX-0000-1000-8000-00805f9b34fb`. Everything else on
# the wire is a manufacturer's own 128-bit id, it has no published meaning, and
# it is drawn RAW — which is the truth, and is what a search engine takes.
# `ronoctua_bluetooth::profile_key` refuses to slice a UUID that does not carry
# that exact tail; before this it sliced blindly, and ten of the UUIDs the
# devices in this house announce are not of that shape.
#
# # THE TWO FIELDS
#
#   key      the text key both languages owe. Several ids share one on purpose:
#            `1124` and `1812` are the same thing over BR/EDR and over BLE, and
#            a person does not care which radio carries their keyboard.
#   hidden   true for a service that says nothing about what the device does FOR
#            YOU. Every BLE device carries `1800` and `1801` because the
#            specification obliges it; drawing "says its own name" in a list
#            headed *what it can do* is noise that pushes the real answer down.
#
# An entry has ONE of the two. Both, or neither, takes the build down with the id.
#
# # WHY THE LIST STOPS WHERE IT DOES
#
# It covers what the machines here actually announce, plus the siblings of those.
# A table of every id the SIG ever assigned would be a list of sentences nobody
# wrote and nobody could check — and an id nobody has ever seen is already
# handled: it comes out raw.
{
  # ------------------------------------------------------------------ audio
  "1108" = {
    key = "bluetooth-profile-headset";
  };
  # The gateway half of the same profile. A headset announces both.
  "1131" = {
    key = "bluetooth-profile-headset";
  };
  "111e" = {
    key = "bluetooth-profile-handsfree";
  };
  "111f" = {
    key = "bluetooth-profile-handsfree";
  };
  "110b" = {
    key = "bluetooth-profile-audio-sink";
  };
  "110a" = {
    key = "bluetooth-profile-audio-source";
  };
  # THE UMBRELLA OVER THOSE TWO, and it is hidden for that reason: a device
  # that announces `110d` always announces `110a` or `110b` beside it, and
  # those two say WHICH WAY the sound goes. This one would be a third row
  # repeating the other two with less information.
  "110d" = {
    hidden = true;
  };
  "110c" = {
    key = "bluetooth-profile-remote-control";
  };
  "110e" = {
    key = "bluetooth-profile-remote-control";
  };
  "110f" = {
    key = "bluetooth-profile-remote-control";
  };

  # ------------------------------------------------------------------ input
  "1124" = {
    key = "bluetooth-profile-human-interface";
  };
  # THE SAME THING OVER BLE, and the one that sent this file into existence:
  # the owner's keyboard speaks BLE, so it announces `1812` and never `1124`,
  # and the row that mattered most was the one drawn raw.
  "1812" = {
    key = "bluetooth-profile-human-interface";
  };

  # ------------------------------------------------------------------ files
  "1106" = {
    key = "bluetooth-profile-file-transfer";
  };
  "1105" = {
    key = "bluetooth-profile-object-push";
  };

  # ------------------------------------------------------------------ phone
  "112f" = {
    key = "bluetooth-profile-phone-book";
  };
  "1130" = {
    key = "bluetooth-profile-phone-book";
  };
  "1132" = {
    key = "bluetooth-profile-message-access";
  };
  "1133" = {
    key = "bluetooth-profile-message-access";
  };
  "112d" = {
    key = "bluetooth-profile-sim-access";
  };

  # ---------------------------------------------------------------- network
  "1115" = {
    key = "bluetooth-profile-network";
  };
  "1116" = {
    key = "bluetooth-profile-network";
  };

  # ------------------------------------------------------------- it says so
  "180f" = {
    key = "bluetooth-profile-battery";
  };
  "180a" = {
    key = "bluetooth-profile-device-information";
  };
  # The BR/EDR twin of `180a`. A classic device carries this one instead.
  "1200" = {
    key = "bluetooth-profile-device-information";
  };
  "1101" = {
    key = "bluetooth-profile-serial";
  };

  # -------------------------------------------------------------- plumbing
  # Its own name and its own appearance. Mandatory on every BLE device.
  "1800" = {
    hidden = true;
  };
  # How to speak GATT to it. Mandatory on every BLE device.
  "1801" = {
    hidden = true;
  };
}
