# The screens in a VM — what a machine with ONE synthetic output, no backlight
# and no I2C bus can prove, which is the rules and none of the hardware.
#
# WHAT IS PROVED HERE
# -------------------
# 1. The module's udev rule for the backlight is installed, PARSES under a real
#    udev, and names the group the option asked for. `tests/checks.nix` reads
#    the rule's text; only a machine with udev on it can say the text is a rule.
# 2. `i2c-dev` is loaded AT BOOT, which is the module's own doing — the
#    external-brightness path's first half, without a monitor to point it at.
#    Upstream's rule both tags the node `uaccess` and puts it in an `i2c`
#    group; nobody is in that group, so `uaccess` is the door this uses. The
#    header used to say the group was what reached it.
# 3. The module adds NO polkit rule of its own: the whole directory is read and
#    nothing in it names a display action.
# 4. The view opens in the footer over a real compositor and the region grows for
#    its rows. WHICH rows is not readable here — see the paragraph below —
#    so what is compared against the compositor's own answer is the connector the
#    FRAGMENT files the screen under, in claim 5.
# 5. The fragment survives the round trip: the view writes `scroll-display.conf`,
#    the config includes it, and the compositor RELOADS with no error — which is the one
#    thing a static check of the file cannot say.
# 6. Nothing on a machine with no backlight and no I2C walks a brightness
#    path at all. What the ROW says instead is drawn to a GPU surface and never
#    logged, so it is in the paragraph below rather than this one.
# 7. The second door — its own process — opens, draws a frame, and its clock
#    puts a change back by itself when nobody answers.
#
# WHAT CANNOT BE PROVED HERE, and it is most of the feature: a second monitor,
# so the arrangement level and "the last lit screen" have nothing to stand on;
# a backlight, so no percentage is ever written; an I2C bus with a monitor on
# it, so DDC/CI is never spoken; a mode list, because the X11 backend offers
# exactly one; a HiDPI output, because the scale the compositor reports here is
# whatever the config says rather than a panel's. Nor can it read a SENTENCE:
# every row of every view is drawn to a GPU surface and none of it reaches a
# journal, so "the row says the brightness is on the monitor's own buttons" is
# not a thing this file can assert. Those are proved on the owner's machine, by
# photograph — `AGENTS.md`'s rule that a screen is looked at before anybody
# says it is done.
{
  pkgs,
  lib,
  scroll,
  ronoctua-shell,
  fonts,
  ronoctua,
}:

let
  xvfb = import ./compositor-xvfb.nix { inherit pkgs lib; };
  # THE COMPOSITOR'S OWN CONFIG, WITH THE INCLUDE THE SYSTEM WRITES.
  #
  # `start_compositor`'s default carries none, and without it the compositor never
  # opens the fragment — so "the compositor reloads it" would be an assertion about a
  # file nothing read, passing whether the fragment were valid or arbitrary bytes.
  # `start_compositor`'s own docstring says a test passes its own config when what it
  # is proving IS a piece of the configuration; this is that case.
  #
  # A MISSING FILE IS NOT FATAL HERE BY CONSTRUCTION — `cmd_include` answers
  # success when the path does not resolve — so there is no flag to pass, which
  # is one thing fewer to get wrong than the `optional=true` this replaced.
  #
  # The path is the same SHAPE `nix/modules/scroll.nix` writes: that module
  # builds `${XDG_CONFIG_HOME:-$HOME/.config}/…` and this node runs as root, so
  # the line here is hand-written to `/root/…` rather than derived. What that
  # costs is honest to say: renaming `fragment::FILE` would leave this subtest
  # green, because the `test -f` below is spelt from the same hand-written
  # string. `display-fragment-validates` reads the file's CONTENT and not its
  # name, so nothing closes that; what does is that both halves are one
  # constant away from each other and a rename touches both.
  configWithFragment = pkgs.writeText "scroll-display-test.conf" ''
    # NO ANIMATIONS — see `tests/compositor-xvfb.nix` for the measurement.
    animations enabled no

    default_border none
    titlebar_padding 1
    gaps inner 0
    gaps outer 0
    scrollnag_command -

    include "/root/.config/ronoctua/scroll-display.conf"
    include "/root/.config/ronoctua/scroll-pointers.conf"
  '';
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-display";

  nodes.machine =
    { config, ... }:
    {
      imports = [
        ../nix/modules/base.nix
        ../nix/modules/display.nix
      ];
      _module.args.ronoctua = ronoctua;

      # A GROUP THAT IS NOT THE DEFAULT, which is what makes the assertions
      # about it mean anything. Both this test and `display-brightness-rule-
      # lands` compare the rule against `config.ronoctua.display.
      # brightnessGroup` — read back out of the same evaluated config the
      # module wrote — so with the default a module that ignored its own
      # option and hardcoded `users` was indistinguishable from one that reads
      # it. `video` is the group the option's own doc names as the narrower
      # alternative, so it is the value a person would actually set.
      ronoctua.display.brightnessGroup = "video";

      virtualisation.memorySize = 2048;
      virtualisation.cores = 2;
      networking.hostName = lib.mkForce "noctua-display";
      users.users.root.hashedPassword = lib.mkForce null;

      # A SEAT SESSION for the user, so the groups the module hands out are
      # about a session that exists — the same reason `power-vm.nix` gives.
      services.getty.autologinUser = config.ronoctua.user;
      users.users.${config.ronoctua.user}.linger = true;

      environment.systemPackages = [
        scroll
        ronoctua-shell
        pkgs.i2c-tools
      ]
      ++ xvfb.packages;

      fonts.enableDefaultPackages = false;
      fonts.packages = [ fonts.noctua-iosevka-core ];
      hardware.graphics.enable = true;
      services.dbus.enable = true;
    };

  testScript =
    { nodes, ... }:
    ''
      ${xvfb.prelude}

      def press_key(name):
          """One key, into the compositor's X window.

          `focus_compositor` from the prelude has to have run first: with no window
          manager nobody calls `XSetInputFocus`, so the focus stays on
          `PointerRoot` and every injected key is lost with no error anywhere.
          `leader-vm.nix` carries the same pair for the same reason.
          """
          machine.succeed(f"DISPLAY=:99 ${pkgs.xdotool}/bin/xdotool key {name}")
          machine.sleep(1)

      machine.start()
      machine.wait_for_unit("multi-user.target")

      with subtest("the backlight rule is installed, parses, and names the group asked for"):
          rules = machine.succeed("cat /etc/udev/rules.d/99-local.rules")
          # THE BACKLIGHT LINE, and not the whole file. `power.nix` writes a
          # `chgrp users` of its own into the same file, so looking for the
          # group's VALUE anywhere in it was satisfied by a rule that is not
          # this one — and would have been satisfied by a literal `users` here
          # in place of the option.
          ours = [line for line in rules.splitlines() if 'SUBSYSTEM=="backlight"' in line]
          assert ours, "the backlight rule is not in the file:\n" + rules
          assert len(ours) == 1, "two backlight rules; which one wins?\n" + rules
          assert "${nodes.machine.ronoctua.display.brightnessGroup}" in ours[0], ours[0]
          # A RULE THAT PARSES, which is the half a text check cannot make:
          # `udevadm verify` is udev reading its own file.
          machine.succeed("udevadm verify /etc/udev/rules.d/99-local.rules")
          # AND THE GROUP EXISTS. A rule chgrp-ing to a group nobody created
          # fails at run time, on a machine, silently — the brightness simply
          # never moves.
          machine.succeed("getent group ${nodes.machine.ronoctua.display.brightnessGroup}")

      with subtest("i2c-dev is loaded at boot, which is the module's doing"):
          # WITHOUT A `modprobe`, which is the whole assertion. `i2c-dev` is an
          # ordinary module of the NixOS kernel, so loading it by hand and then
          # finding it loaded says nothing about our module at all — the
          # subtest passed with `hardware.i2c.enable` deleted. What only this
          # machine can say is that it is there BEFORE anybody asks, which is
          # nixpkgs' `hardware/i2c.nix` putting `i2c-dev` in
          # `boot.kernelModules` because `nix/modules/display.nix` asked it to.
          machine.succeed("test -e /sys/module/i2c_dev")
          # AND THE NODE IS REACHED BY `uaccess`, not by a group: nixpkgs'
          # `70-i2c.rules` tags it for the seat's user. The rule is what has to
          # be on the machine; the bus is not, because QEMU's virtual GPU
          # exposes no DDC/CI channel and there is nothing to point it at.
          rules = machine.succeed(
              "cat /etc/udev/rules.d/*.rules /run/udev/rules.d/*.rules "
              "2>/dev/null | grep -i i2c || true"
          )
          assert "uaccess" in rules, (
              "no udev rule opens an i2c node to the seat's user:\n" + rules
          )
          print(machine.execute("ls -l /dev/i2c-* 2>&1")[1])

      with subtest("the module adds no polkit rule of its own"):
          # THE WHOLE DIRECTORY, and not a file we name: a rule added under any
          # name would be missed by a check that looks for one.
          out = machine.succeed(
              "cat /etc/polkit-1/rules.d/*.rules 2>/dev/null || true"
          )
          # A POSITIVE CONTROL FIRST. An empty or absent directory passes every
          # assertion below by having nothing in it, which is a check that
          # cannot fail. What it reads is nixpkgs' own unconditional
          # `10-nixos.rules` — this node imports `base.nix` and `display.nix`
          # and neither writes a rule, so that file is the whole of what is
          # there and its absence would mean the sweep is reading nothing.
          assert "polkit" in out.lower() or "action" in out.lower(), (
              "the polkit directory read as empty; the sweep is not reaching it:\n" + out
          )
          # THE WORDS THAT WOULD BE OURS. Not `output`, which is common enough
          # in an upstream rule's prose to say nothing when it appears.
          for word in ("backlight", "brightness", "ronoctua.display"):
              assert word not in out.lower(), (
                  f"a polkit rule mentions {word}; the display module writes none:\n" + out
              )

      def scale_of_output():
          """What scale the compositor says its output is running at.

          PARSED, like the connector below: this IPC answers JSON, and the
          number comes out as a float — read as TEXT it would make `1.0` and
          `1` different scales, and `1.3333333730697632` a string nobody can
          compare.
          """
          import json as _json

          sock = socket_of("compositor")
          said = machine.succeed(f"SCROLLSOCK={sock} ${scroll}/bin/scrollmsg -t get_outputs")
          listed = _json.loads(said)
          assert listed, "the compositor lists no output at all:\n" + said
          return listed[0]["scale"]

      with subtest("the view opens over a real compositor and the region grows for it"):
          # THE BRAIN STAYS OUT OF IT — and `start_xvfb` is what writes that,
          # for every test that uses this prelude. Writing it here as well was
          # the same bytes twice.
          start_xvfb()
          start_compositor("compositor", "${scroll}", config="${configWithFragment}")
          sock = socket_of("compositor")
          connectors = machine.succeed(f"SCROLLSOCK={sock} ${scroll}/bin/scrollmsg -t get_outputs")
          print(connectors)
          # THE CONNECTOR THE COMPOSITOR REPORTS, kept and compared below — it was
          # fetched, printed and dropped, which is a claim in the header and
          # nothing in the test.
          #
          # PARSED, not scanned: this IPC answers `get_outputs` with JSON, and
          # a regexp written for another compositor's prose answers "no output
          # at all" over a reply that lists one.
          import json as _json
          listed = _json.loads(connectors)
          assert listed, "the compositor lists no output at all:\n" + connectors
          connector = listed[0]["name"]
          machine.succeed(
              "systemd-run --unit=footer ${xvfb.env} "
              "--setenv=WAYLAND_DISPLAY=wayland-1 "
              "--setenv=HOME=/root "
              f"--setenv=SCROLLSOCK={sock} "
              "--setenv=RUST_LOG=ronoctua_shell=debug,ronoctua_display=debug "
              "${ronoctua-shell}/bin/ronoctua-shell"
          )
          machine.wait_until_succeeds(
              "journalctl -u footer --no-pager | grep 'footer layout resolved' > /dev/null",
              timeout=60,
          )
          # THE CONTROL SOCKET IS `$XDG_RUNTIME_DIR/ronoctua-<display>.sock`, so
          # both variables are needed: without them the command has nowhere to
          # look and says so rather than guessing.
          machine.succeed(
              "XDG_RUNTIME_DIR=/run/user/0 WAYLAND_DISPLAY=wayland-1 "
              f"SCROLLSOCK={sock} ${ronoctua-shell}/bin/ronoctua-shell command display-open"
          )
          # THE SURFACE GREW, which is the footer saying the region is holding
          # rows. It logs its resolved geometry and not the text of the rows,
          # so this is the observable it actually has — and it is not a weak
          # one: the height comes from `expansion_rows`, which is the count of
          # rows the view built.
          # MEASURED AGAINST ITSELF, not against a number written here. 53 px is
          # the resting height at 100% today and a token away from not being;
          # and `grep -o 'height=[0-9]*'` unanchored also matches the
          # "layer-shell surface created" line, which carries a DIFFERENT
          # number under the same name. `network-vm.nix` records both mistakes.
          def heights():
              lines = machine.succeed(
                  "journalctl -u footer --no-pager | grep 'layout resolved' "
                  "| grep -o 'height=[0-9]*' || true"
              ).split()
              return [int(x.split("=")[1]) for x in lines]

          resting = min(heights())
          # THREE TIMES THE RESTING HEIGHT: this machine's list is five rows
          # on top of the two fixed lines — a heading, the one screen, a second
          # heading, the arrangement and the size of the text — so anything
          # less than that is the region not having taken the view. The margin
          # is deliberate: a number tuned to what it happens to be today is a
          # number that fails the day a row is added.
          machine.wait_until_succeeds(
              "test $(journalctl -u footer --no-pager | grep 'layout resolved' "
              f"| grep -o 'height=[0-9]*' | cut -d= -f2 | sort -n | tail -1) -ge {resting * 3}",
              timeout=60,
          )
          print(f"the footer rests at {resting} px and grew to {max(heights())}")
          # AND THE READING GOT THROUGH. `ronoctua_display` is at debug on this
          # unit; a view drawn over an empty snapshot would pass the height
          # test above by drawing its "nothing here" sentence.
          journal = machine.succeed("journalctl -u footer --no-pager")
          assert "no screen thread" not in journal, journal[-4000:]

      with subtest("a knob turned and kept writes the fragment, and the compositor reloads it"):
          # THE WHOLE PATH, driven by KEYS. The view writes nothing until
          # somebody keeps a change, so a test that only opened it would be
          # asserting about a file the feature never produces. The keys go in
          # through `xdotool` on Xvfb, which is the route a physical keyboard
          # takes — `leader-vm.nix`'s own note on this.
          focus_compositor()
          # WHAT THE COMPOSITOR CHOSE FOR ITSELF, before a key is pressed.
          # The re-read below frees every output config and builds them again
          # from the files, so this is the number it goes back to when nothing
          # of ours is loaded.
          untouched = scale_of_output()
          # RET into the screen's sheet, then down to the scale. The rows
          # before it are the two facts, the power row, the resolution and the
          # rate — the last three drawn refusing on this synthetic output,
          # which the cursor still walks.
          press_key("Return")
          for _ in range(5):
              press_key("Down")
          # `Right` applies it provisionally and raises the question, whose
          # cursor is on "put it back"; `Down` then `Return` is "keep it".
          press_key("Right")
          press_key("Down")
          press_key("Return")

          fragment = "/root/.config/ronoctua/scroll-display.conf"
          machine.wait_until_succeeds(f"test -f {fragment}", timeout=60)
          written = machine.succeed(f"cat {fragment}")
          print(written)
          # THE CONNECTOR THE COMPOSITOR REPORTED, which is the header's claim and was
          # fetched and dropped: the fragment files this screen under its
          # connector because this backend's output gives no serial, so the
          # name in the file has to be the name the compositor gave.
          assert f'output "{connector}"' in written, (
              f"the fragment does not name `{connector}`:\n" + written
          )
          assert "scale" in written, "the scale that was kept is not in it:\n" + written
          # AND THE COMPOSITOR READ IT, which is the half this subtest is
          # named for and did not have.
          #
          # It used to COUNT config-load announcements in the footer's
          # journal, because a refusal only ever arrived asynchronously and
          # the compositor replayed one to every new client — so asserting
          # presence passed even with the `include` line deleted from the
          # config, which is the very case the subtest exists for.
          #
          # What replaced it asserted two things that CANNOT FAIL: `reload`
          # answers `{"success": true}` whatever parsed (measured — see
          # `ronoctua-scroll`'s `reread`), and every output reports a scale.
          # Both were satisfied by a compositor that had ignored the file, so
          # the subtest was asking nothing at all.
          #
          # The answer is two readings, and the first of them is the one that
          # found the defect: the compositor has to ACCEPT the file. It used
          # to answer every line of the header with `Unknown/invalid command
          # '//'` and then `Error(s) loading config!`, so the whole fragment
          # was on the floor — every monitor's settings refused over a comment
          # marker, on every machine, with `reload` answering `success` and
          # `scrollnag_command -` refusing the one window that would have
          # said so.
          #
          # NOT COMPARED NUMBER FOR NUMBER, deliberately. `verify_good_scale`
          # snaps a scale that does not divide the panel into whole logical
          # pixels, and on this backend it can: the X11 output answers
          # `"modes": []` with a populated `current_mode`, so `Screen::mode()`
          # — which indexes the list — is `None` and the scale ladder falls
          # back to its panel-blind rungs. What the file asks for and what the
          # compositor settles on may legitimately differ by one snap.
          #
          # WHAT MAY NOT DIFFER is that the scale is still not the one the
          # compositor chooses on its own. A re-read frees `output_configs`
          # and builds them again from the FILES, so every transient this
          # subtest applied is gone by then — and a scale that is still not
          # the untouched one can only have come from the fragment.
          sock = socket_of("compositor")
          since = len(machine.succeed("journalctl -u compositor --no-pager").splitlines())
          machine.succeed(f"SCROLLSOCK={sock} ${scroll}/bin/scrollmsg reload")
          machine.sleep(2)
          fresh = "\n".join(
              machine.succeed("journalctl -u compositor --no-pager").splitlines()[since:]
          )
          assert "Error(s) loading config" not in fresh, (
              "the compositor REFUSED the fragment the view had just written:\n" + fresh
          )
          running = scale_of_output()
          assert running != untouched, (
              f"the re-read put the scale back to {running}, which is what the "
              "compositor chooses on its own: the file was written and forgotten"
          )

      with subtest("the footer's own clock puts a change back when nobody answers"):
          # THE PRIMARY DOOR'S HALF, and it had none: the subtest above answers
          # its question with a key inside fifteen seconds, so the FOOTER's
          # countdown — the one `s d d` uses, driven by `display_tick` off the
          # footer's own second — was never allowed to run out. Deleting the
          # one call that drives it left every test green.
          fragment = "/root/.config/ronoctua/scroll-display.conf"
          before = machine.succeed(f"cat {fragment}")
          focus_compositor()
          # BACK TO THE LIST FIRST. The subtest above left the view INSIDE the
          # sheet with the cursor on the scale, so the same walk from here
          # counted five rows further down and landed on "Opens focused here" —
          # a row that writes straight down, with no question and no clock. The
          # test then read a changed file and blamed the countdown, which is
          # the assertion doing its job over the wrong keystrokes.
          press_key("Escape")
          press_key("Return")
          for _ in range(5):
              press_key("Down")

          # WHAT THE COMPOSITOR IS ACTUALLY DOING, before anything is asked.
          #
          # Every assertion this subtest used to make was an ABSENCE. A
          # provisional change is IPC-only and the fragment is written only on
          # "keep", so `after == before` is true BEFORE the keys are pressed —
          # it passes on a machine where the whole feature is dead. And "the
          # countdown ran out" is logged just before `apply(errand)` runs, so
          # it proves the clock FIRED, not that anything was put back:
          # deleting the `apply` call left both halves green while a scale
          # nobody kept stayed on the running compositor for ever.
          #
          # The scale the compositor reports is the observable that was missing, and the
          # test already knew how to ask for it.
          was = scale_of_output()
          press_key("Right")
          # THE PREVIEW REACHED THE COMPOSITOR. Without this the assertion
          # below passes because nothing ever changed in the first place.
          machine.sleep(2)
          previewed = scale_of_output()
          assert previewed != was, (
              f"the preview never reached the compositor: still {was}"
          )

          machine.sleep(20)
          after = machine.succeed(f"cat {fragment}")
          assert after == before, (
              "a change nobody kept was written to the file:\n"
              + before + "\n--- became ---\n" + after
          )
          journal = machine.succeed("journalctl -u footer --no-pager")
          assert "the countdown ran out" in journal, (
              "the footer's clock never fired; nothing put the change back:\n"
              + journal[-4000:]
          )
          # AND THE PICTURE CAME BACK. This is the half the subtest was named
          # for and never checked.
          restored = scale_of_output()
          assert restored == was, (
              f"the clock fired and the change stayed: {was} -> {previewed} -> {restored}"
          )
          press_key("Escape")

      with subtest("the reading finds no light and no bus, and nothing moved one"):
          # WHAT THIS CAN AND CANNOT SAY. The row's own sentence
          # (`display-light-on-its-buttons`) is
          # drawn to a GPU surface and never logged, so a VM cannot read it;
          # that half is proved by the unit tests and by a photograph on the
          # owner's machine. What a VM CAN say is that no brightness path was
          # walked: `Order::Bright` is the only thing that reaches
          # `Eyes::brighten`, and every refusal it can produce is a sentence
          # this journal would carry.
          journal = machine.succeed("journalctl -u footer --no-pager")
          for said in ("brightness", "backlight"):
              assert said not in journal.lower(), journal[-4000:]
          # AND THE POSITIVE HALF, which this subtest did not have: every
          # assertion in it was an ABSENCE, and an absence is satisfied by a
          # build where `Eyes::brighten`, `backlight.rs` and `ddc.rs` were all
          # deleted. The reading now says what it found on each connector, so
          # this asserts that the sweep of `/sys/class/drm` HAPPENED and that
          # it found no light and no bus — which is the machine's own truth
          # and would go red the day either of those halves started lying.
          wired = [
              line for line in journal.splitlines()
              if "what is wired to this connector" in line
          ]
          assert wired, (
              "the reading never walked `/sys/class/drm`:\n" + journal[-4000:]
          )
          print(wired[-1])
          assert "light=true" not in wired[-1] and "bus=true" not in wired[-1], (
              "this machine has neither a panel nor a bus, and the reading "
              "claims one:\n" + wired[-1]
          )
          machine.succeed("systemctl stop footer")

      with subtest("the second door opens and draws the same view"):
          # THE WINDOW, which is what this deliverable IS and what the test did
          # not touch: every line of `display_window.rs` had no coverage here.
          # It runs as its own process (`ronoctua-shell display`), so what says
          # it worked is its own journal.
          sock = socket_of("compositor")
          machine.succeed(
              "systemd-run --unit=screens-window ${xvfb.env} "
              "--setenv=WAYLAND_DISPLAY=wayland-1 "
              "--setenv=HOME=/root "
              f"--setenv=SCROLLSOCK={sock} "
              "--setenv=RUST_LOG=ronoctua_shell=debug,ronoctua_display=debug "
              "${ronoctua-shell}/bin/ronoctua-shell display"
          )
          # A WINDOW IS AN `xdg_toplevel`, so the compositor is what says it
          # exists — and it says the app id, which is what the modeline turns
          # into `mode-display`.
          machine.wait_until_succeeds(
              f"SCROLLSOCK={sock} ${scroll}/bin/scrollmsg -t get_tree "
              "| grep 'br.ronoctua.display' > /dev/null",
              timeout=60,
          )
          windows = machine.succeed(
              f"SCROLLSOCK={sock} ${scroll}/bin/scrollmsg -t get_tree"
          )
          print(windows)
          # AND ITS TITLE IS ITS OWN. It read `*Bluetooth*` for one commit,
          # because the mould's key was copied with everything else.
          assert "*Screens*" in windows, "the window's title is not its own:\n" + windows
          # AND THE READER STARTED IN IT TOO — the window has its own thread,
          # its own the compositor connection and its own `display.json` watch.
          w = machine.succeed("journalctl -u screens-window --no-pager")
          assert "no screen thread" not in w, w[-4000:]
          # AND IT DREW — POSITIVELY. `set_title`/`set_app_id` all run before
          # the event loop and before `assemble_gpu`, so a window whose `paint`
          # is empty satisfies every assertion above and every absence below.
          # The door says so on each frame that goes out, the way `buffer.rs`
          # does; nothing else in this file could tell a drawn window from a
          # mapped one.
          machine.wait_until_succeeds(
              "journalctl -u screens-window --no-pager "
              "| grep 'the screens window drew' > /dev/null",
              timeout=60,
          )
          w = machine.succeed("journalctl -u screens-window --no-pager")
          # AND THE SENTENCES IT WOULD CARRY IF IT HAD NOT. `no graphics
          # context` is the footer's and the terminal's wording and does not
          # exist in this door — the null-pointer path here says something
          # else, so one of the four guards could never have fired.
          for broke in (
              "no usable GPU",
              "assembling the text failed",
              "drawing the frame failed",
              "null Wayland pointers",
          ):
              assert broke not in w, f"the window did not draw ({broke}):\n" + w[-4000:]

      with subtest("the window's own clock puts a change back when nobody answers"):
          # THE ONE THING THIS DOOR HAS THAT THE OTHERS DO NOT, and it had no
          # coverage of any kind: `display_window.rs` carries no `#[test]`, so
          # this is the only place the countdown, `arm_the_clock` and the
          # message line can be exercised at all.
          #
          # THE FOOTER IS STOPPED by the subtest before this one, so the clock
          # that runs here is the window's own.
          fragment = "/root/.config/ronoctua/scroll-display.conf"
          before = machine.succeed(f"cat {fragment}")
          focus_compositor()
          press_key("Return")
          for _ in range(5):
              press_key("Down")
          # `Right` applies it provisionally and raises the question. Nobody
          # answers: `PATIENCE` is fifteen seconds, and the door's own thread
          # is what has to notice.
          press_key("Right")
          machine.sleep(20)
          after = machine.succeed(f"cat {fragment}")
          assert after == before, (
              "a change nobody kept was written to the file:\n"
              + before + "\n--- became ---\n" + after
          )
          # AND THE CLOCK IS WHAT PUT IT BACK, not the absence of a keystroke:
          # the door sends `Undo` when the question expires, and says so.
          w = machine.succeed("journalctl -u screens-window --no-pager")
          assert "the countdown ran out" in w, (
              "the question never expired; nothing put the change back:\n" + w[-4000:]
          )
          machine.succeed("systemctl stop screens-window")
          stop_compositor("compositor")
    '';
}
