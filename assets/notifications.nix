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
    # It is the compositor that sends it (`show_screenshot_notification`, in
    # the pinned niri), with `app_name = "niri"` and an English summary. Two
    # consequences, and both are deliberate:
    #
    #   - `rewrite = true`, so the sentence on screen is ours and exists in
    #     both languages (invariant 8). Upstream's is not translated and never
    #     will be.
    #   - the pair (app, summary) is the recognition, and it depends on a
    #     string that belongs to somebody else. The
    #     `screenshot-notification-contract` check reads the pinned niri's
    #     source and fails if that string moves — without it, the entry would
    #     stop matching in silence and the screenshot would arrive in English,
    #     loud, with no actions.
    screenshot = {
      when = [
        {
          field = "app";
          is = "niri";
        }
        {
          field = "summary";
          is = "Screenshot captured";
        }
      ];
      rewrite = true;
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
          key = "e";
          run = "edit";
          args = [
            "-f"
            "file"
          ];
        }
        {
          id = "open";
          key = "a";
          run = "open";
          args = [ "file" ];
        }
        {
          id = "folder";
          key = "p";
          run = "open";
          args = [ "folder" ];
        }
        {
          id = "copy-path";
          key = "c";
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
          key = "m";
          command = "memory-open";
        }
      ];
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
