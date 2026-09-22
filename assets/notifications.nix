# The distro's notifications and filters — section 4.2.
#
# ONE PLACE. Everything this system raises, and every filter it ships with, is
# declared here and generated from here: the Rust table, the list of text keys
# both languages have to carry, and the refusal of an invalid field — all at
# build time, in the same shape as `assets/tokens/*.nix` (section 7.6).
#
# Written as Rust instead, "every notification the distro sends" would be a
# `match` in one file and a `format!` in another, and the honest answer to
# "which ones are there?" would be a grep. The manager draws THIS list.
#
# HOW AN ENTRY IS RECOGNISED
# --------------------------
# Not all of ours originate here. The screenshot is sent by the compositor, in
# English, with the file's path in the icon field — so an entry says how to
# RECOGNISE a notification and what to make of it, and the same machinery then
# serves one we raise and one that arrives from outside.
#
# The ones we raise carry a `tag` equal to their own id, which is why most
# `when` blocks are one line. That works from INSIDE the shell (the
# out-of-memory one) and from any other app of ours over the bus:
#
#     notify-send --hint=string:tags:my-thing 'resumo' 'corpo'
#
# — which is the whole of "an app of the distro creates a customised
# notification at the source". A hint and not a method of our own: `Notify` is
# what every client already speaks, and a private method would make OUR apps
# the ones needing a special library.
#
# THE FIELDS
# ----------
#   show   hidden | brief | full        not shown · one line · up to three
#   life   quick | normal | long        3 s · 6 s · 20 s. There is no "for ever".
#   loud   true | false                 the shouting formatting, or the quiet one
#   level  0..100                       ours; it orders the queue and filters test it
#   icon   a token of assets/tokens/icons.nix (`camera`), OR any Nerd Fonts
#          glyph by NAME (`md-camera_iris`), without the `nf-` — the same names
#          the icon browser shows. The token is the ordinary case, because a
#          token is a decision a theme can change; the glyph is the door for
#          when nothing in the design system says what this notification is.
#          Both are resolved at BUILD time, so the footer carries no table.
#   blink | vibrate | audio             see docs/PENDING.md for the last two
#
# A term is `{ field = ...; <test> = ...; }` where the test is one of `is`,
# `contains`, `starts-with`, `at-least`, `at-most`. The numeric two only apply
# to `level`, and the generator refuses them anywhere else.
{
  # ===================================================================
  # THE DISTRO'S OWN NOTIFICATIONS
  # ===================================================================
  entries = {
    # THE SCREENSHOT, and the owner asked for it by name: it appears
    # DISCREETLY, in the band, and it offers to edit what was just captured.
    #
    # THE CAPTURE IS OURS, so the recognition is a TAG. The compositor has no
    # capture verb at all — the picture is taken by `grim`, which speaks
    # `wlr-screencopy` and writes a file and nothing else — and the footer
    # raises the notification itself (`capture_the_screen`), with the summary
    # and body from our keys and the `file://` URL of what was written.
    #
    # It used to be recognised by the pair (app, summary), because the sender
    # was somebody else's program and an English sentence was all it gave us.
    # That cost a `rewrite = true` to get invariant 8 back, and a check that
    # read the sender's SOURCE so the pair could not move in silence. Both are
    # gone: a tag we write cannot drift away from a tag we match, and the
    # sentence was never in another language to begin with.
    screenshot = {
      when = [
        {
          field = "tag";
          is = "screenshot";
        }
      ];
      # `full`, and `loud = false`. They are two different things and the owner
      # asked for both: DISCREET is the formatting — no shouting colour — and
      # `full` is the SIZE. A capture offers to be edited, opened and copied,
      # and an action nobody can see is an action nobody knows about; one line
      # has no room for the row that carries them.
      show = "full";
      life = "normal";
      loud = false;
      # THIRTY, and not twenty. At twenty the distro's own
      # `small-things-are-quick` catches it (`level at-most 20`) and puts it
      # back to one line — the entry would say `full` and the screen would show
      # `brief`, with only the manager able to say why. A capture one may want
      # to act on is not "nothing".
      level = 30;
      # A GLYPH BY NAME, and not a token — the second door this table opens.
      #
      # Nothing in the design system says "screen capture": the `camera` token
      # is a video camera (`cod-device_camera`), which is where a photograph
      # came FROM and not what this is. `md-monitor_screenshot` is the drawing,
      # it lives in the font, and it is resolved at build time like any token.
      icon = "md-monitor_screenshot";
      tags = [
        "screenshot"
        "media"
      ];
      actions = [
        # EDIT is first because it is the one the owner asked for. `swappy`
        # only exists in the `full` profile, and the action DISAPPEARS where it
        # is not installed rather than failing when clicked.
        {
          id = "edit";
          run = "edit";
          args = [
            "-f"
            "file"
          ];
        }
        {
          id = "open";
          run = "open";
          args = [ "file" ];
        }
        {
          id = "folder";
          run = "open";
          args = [ "folder" ];
        }
        {
          id = "copy-path";
          copy-path = true;
        }
      ];
    };

    # THE MACHINE CLOSED AN APPLICATION SO IT WOULD NOT FREEZE.
    #
    # It used to be built by hand in `surface.rs`, with its own urgency, its own
    # never-expires and its own three text keys — the only notification in the
    # system with a code path of its own. Here it is a row like the others.
    #
    # `full` and `loud`: a window vanished and the person is owed the sentence
    # that says why. `long`, because it is the rare exception the duration
    # exists for.
    memory-killed = {
      when = [
        {
          field = "tag";
          is = "memory-killed";
        }
      ];
      rewrite = false;
      show = "full";
      life = "long";
      loud = true;
      level = 95;
      icon = "memory";
      tags = [
        "memory-killed"
        "system"
      ];
      actions = [
        # The panel that shows the policy that did it, and lets it be loosened.
        {
          id = "memory-panel";
          command = "memory-open";
        }
      ];
    };

    # THE OTHER SIDE IS ASKING, and the view could not open to say so —
    # another view is in the footer holding something somebody built by hand.
    # The question is the one thing here another machine is waiting on, so it
    # is critical and it carries its two answers: the same commands `s b y`
    # and `s b n` run.
    #
    # THE CARD IS SHORTER THAN THE QUESTION, and the body says what to do
    # about it. `long` is twenty seconds, nothing here stays for ever — not
    # even a critical one — and the agent holds the question for fifty-five.
    # So for thirty-five seconds the card is gone while the question stands,
    # and the two buttons that were the whole point of it go with it.
    #
    # There is no fourth `life`, and inventing one would put "for ever" back
    # in under another name. What the body does instead is NAME THE TWO KEYS,
    # which work for the whole fifty-five and do not depend on a card being on
    # screen. The keys arrive as arguments, never as text (`keys-are-never-text`).
    #
    # And the other end is closed too: `bluetooth_unasked` dismisses this card
    # the moment the question is answered, taken back or times out, so the
    # card never outlives the question.
    # AND ALL FOUR GO TO THE BLUETOOTH VIEW. Whatever the radio has to say,
    # the answer is in one place — reconnecting, forgetting, charging, saying
    # yes to a pairing — so every one of them offers the way there. The pairing
    # card's own body already said "open Bluetooth to answer"; now it does.
    #
    # AND ALL FOUR ARRIVE READ (`auto-read`), which the owner asked for after
    # reading his own screen: a keyboard that reconnects three times had left
    # three things on the bell to clear, and none of them was a thing to do.
    #
    # It is about the BELL and not about the band — all four still show, which
    # is the moment they are worth anything. Suppressing them would have been
    # the wrong tool: it takes them off the screen exactly when they matter.
    #
    # The pairing card and the flat battery are in the list because he chose
    # the four. It costs one thing, and it is worth writing down: a pairing
    # question missed while it was on screen leaves no unread trace afterwards.
    # It is still in the record; it just does not call.
    bluetooth-pairing = {
      when = [
        {
          field = "tag";
          is = "bluetooth-pairing";
        }
      ];
      rewrite = false;
      show = "full";
      life = "long";
      loud = true;
      level = 90;
      auto-read = true;
      icon = "bluetooth-connected";
      tags = [
        "bluetooth-pairing"
        "bluetooth"
      ];
      actions = [
        {
          id = "bluetooth-accept";
          command = "bluetooth-accept";
        }
        {
          id = "bluetooth-refuse";
          command = "bluetooth-refuse";
        }
        {
          id = "bluetooth-panel";
          command = "bluetooth-open";
        }
      ];
    };

    # WHAT THE RADIO DID, and only the two moments worth a line.
    #
    # A DEVICE THAT DROPS IS THE ONE THAT MATTERS, and the owner's own case
    # says why: the keyboard is Bluetooth. When it goes, the machine stops
    # answering keys — and reading the modeline for a glyph that has just
    # DISAPPEARED is not something a person can do. So this one is `full`,
    # names the device and lives a little longer than its twin.
    #
    # THE ONE THAT CONNECTS is `brief` and quick: it confirms something that
    # was asked for, and the mark on the modeline says it too.
    #
    # NEITHER REWRITES: two devices coming back after a suspend are two
    # sentences, and one overwriting the other would say the wrong name.
    bluetooth-connected = {
      when = [
        {
          field = "tag";
          is = "bluetooth-connected";
        }
      ];
      rewrite = false;
      show = "brief";
      life = "quick";
      loud = false;
      level = 20;
      auto-read = true;
      icon = "bluetooth-connected";
      tags = [
        "bluetooth-connected"
        "bluetooth"
      ];
      actions = [
        {
          id = "bluetooth-panel";
          command = "bluetooth-open";
        }
      ];
    };
    bluetooth-disconnected = {
      when = [
        {
          field = "tag";
          is = "bluetooth-disconnected";
        }
      ];
      rewrite = false;
      show = "full";
      life = "normal";
      loud = false;
      level = 30;
      auto-read = true;
      icon = "bluetooth-off";
      tags = [
        "bluetooth-disconnected"
        "bluetooth"
      ];
      actions = [
        {
          id = "bluetooth-panel";
          command = "bluetooth-open";
        }
      ];
    };

    # AND THE DEVICE'S OWN BATTERY, on the SAME number the modeline turns red
    # at (`ronoctua_bluetooth::battery::DANGER_FROM`). It is raised on the
    # crossing, once per device, exactly as the machine's own battery is: a
    # mouse sitting at 12% for an afternoon is one line and not a hundred.
    #
    # It exists because the machine's battery had a second voice and the
    # devices had none: the percentage on the modeline appears at 30% and
    # turns red at 15%, and a mark that changes colour is not something one
    # notices while typing.
    bluetooth-battery-low = {
      when = [
        {
          field = "tag";
          is = "bluetooth-battery-low";
        }
      ];
      rewrite = false;
      show = "full";
      life = "normal";
      loud = false;
      level = 60;
      auto-read = true;
      icon = "battery-danger";
      tags = [
        "bluetooth-battery-low"
        "bluetooth"
      ];
      actions = [
        {
          id = "bluetooth-panel";
          command = "bluetooth-open";
        }
      ];
    };

    # THE BATTERY'S THREE ALARMS, raised by the footer on a CROSSING of
    # UPower's warning level — never on the level itself, so a battery that
    # sits at "low" for an hour is one notification and not sixty
    # (`ronoctua_power::alarms`). The tag is how the row is recognised, the
    # `memory-killed` shape.
    #
    # `low` is `full` and `long`, quiet: it is the one worth reading and not
    # the one worth jumping at. `critical` is loud and above the
    # `important-gets-room` line: the daemon is about to act on its own.
    # `charged` is a brief line that goes quickly — the rule
    # `small-things-are-quick` would have made it so anyway, and saying it
    # here is what a reader of this file expects to find.
    battery-low = {
      when = [
        {
          field = "tag";
          is = "battery-low";
        }
      ];
      rewrite = false;
      show = "full";
      life = "long";
      loud = false;
      level = 60;
      icon = "battery-danger";
      tags = [
        "battery-low"
        "power"
      ];
      actions = [
        {
          id = "power-panel";
          command = "power-open";
        }
      ];
    };
    battery-critical = {
      when = [
        {
          field = "tag";
          is = "battery-critical";
        }
      ];
      rewrite = false;
      show = "full";
      life = "long";
      loud = true;
      level = 90;
      icon = "battery-danger";
      tags = [
        "battery-critical"
        "power"
      ];
      actions = [
        {
          id = "power-panel";
          command = "power-open";
        }
      ];
    };
    battery-charged = {
      when = [
        {
          field = "tag";
          is = "battery-charged";
        }
      ];
      rewrite = false;
      show = "brief";
      life = "quick";
      loud = false;
      level = 10;
      icon = "battery-charging-full";
      tags = [
        "battery-charged"
        "power"
      ];
      actions = [ ];
    };
  };

  # ===================================================================
  # THE FILTERS THE DISTRO SHIPS WITH
  # ===================================================================
  #
  # ORDER MATTERS: the FIRST one that catches decides, and the manager shows
  # which. Every matching rule laying its verdict over the last would make the
  # answer to "why is this quiet?" a list, and the person would have to simulate
  # the whole book in their head.
  #
  # The person's own filters are tried BEFORE these — a filter of one's own that
  # could not overrule the distro's would be a suggestion, not a filter.
  #
  # None of these SUPPRESSES anything. Suppression is a decision about what one
  # wants not to be told, and the distro is not in a position to make it; the
  # manager is where it gets made, and the right-hand column is where the result
  # can be audited.
  rules = [
    # SOMETHING IMPORTANT GETS ROOM. A high level with one elided line says
    # there is something and hides what — which is the worst of both.
    {
      id = "important-gets-room";
      when = [
        {
          field = "level";
          at-least = 80;
        }
      ];
      apply = {
        show = "full";
        loud = true;
      };
    }
    # AND SOMETHING UNIMPORTANT DOES NOT HOLD THE SCREEN. The band pushes the
    # content up while it is there; six seconds of "copied" is six seconds of
    # every window being one line shorter.
    {
      id = "small-things-are-quick";
      when = [
        {
          field = "level";
          at-most = 20;
        }
      ];
      apply = {
        show = "brief";
        life = "quick";
        loud = false;
      };
    }
  ];
}
