# WHAT `nix build .#shots` PHOTOGRAPHS, one id a scene.
#
# # Why the list exists
#
# Because the battery had one shape — all of it — and the owner read the cost
# off the deliveries: *"os mesmos screenshots dos mesmos locais estão sendo
# gerados toda vez que você testa algo novo … enquanto faria muito mais sentido
# focar os screenshots apenas nas interfaces das coisas recém implementadas ou
# modificadas"*. A run is four minutes and thirty-five pictures, of which one
# delivery changes two; the other thirty-three are noise in the report and
# minutes on the clock.
#
# So every subtest of `nix/shots.nix` carries an id from this list and the
# flake exposes one package per id — `nix build .#shots-manager` photographs
# the file manager and nothing else. `nix build .#shots` is still every one of
# them, and is still what proves the whole screen: SELECTING IS FOR THE LOOP,
# not for the gate.
#
# # And the list is checked against the script
#
# `shots-scenes-are-total` reads both sides: an id here that no subtest uses is
# a package that photographs nothing, and a subtest with no id is a picture
# nobody can ask for on its own. It is `tests/slow.nix`'s argument, one battery
# along.
#
# The ORDER is the order they run in, which is the order the script has them.
#
# # `needs`, and why it is not optional
#
# Most scenes open their own door — an `Escape` or two and a `control(…)` — and
# a run of one of those is a run of one. TWO of them do not: the document
# preview and the miniplayer both ride on the window the manager's own scene
# opened, and selecting either of them alone photographed AN EMPTY DESKTOP.
#
# It was worse than that, and it is why this field exists rather than a
# sentence in a comment: the subtest still PASSED. Its measurement asked
# whether `text.primary` covered twenty pixels of the panel's box, and the
# mouse pointer is white — so the assertion answered with the cursor. A scene
# that photographs nothing and says it worked is the thing this whole battery
# exists to prevent.
#
# So a scene that cannot stand on its own says what it stands on, and asking
# for it brings those along in declared order. `shots-scenes-are-total` reads
# both sides, and it also refuses a scene that opens no door of its own and
# declares no need.
[
  { id = "clipboard"; }
  { id = "history"; }
  { id = "tor"; }
  # THE ONE SCENE THAT OPENS NOTHING ON PURPOSE: what it photographs is the
  # footer with nothing on it, so anything it opened would be the wrong
  # picture. `alone` says that out loud, because the check's rule is "a scene
  # opens a view or names whose it borrows" and this is neither.
  {
    id = "footer";
    alone = true;
  }
  { id = "band"; }
  { id = "queue"; }
  { id = "loud"; }
  { id = "screenshot"; }
  { id = "notifications"; }
  { id = "catalogue"; }
  { id = "manager"; }
  # THE WINDOW DOOR IS REACHED THROUGH THE FOOTER'S. It creates its own files
  # and opens its own window, and it still cannot stand alone: the `BackSpace`
  # that puts it in `/root/shots` before asking for the window is a key sent to
  # the FOOTER's manager, which the scene above opened. Run without it the
  # window came up somewhere else, the filter found nothing, and the syntax
  # assertion read a panel of the wrong file — `#6CD586 covers 0 pixel(s)`,
  # which is this battery catching exactly what it is for.
  {
    id = "manager-window";
    needs = [ "manager" ];
  }
  # THE WINDOW DOOR IS WHERE BOTH OF THESE HAPPEN. Neither opens one: they walk
  # the directory the scene above left them in.
  {
    id = "document";
    needs = [ "manager-window" ];
  }
  {
    id = "miniplayer";
    needs = [ "manager-window" ];
  }
  # THE GLOBE, which rides on the window door for the same reason the two above
  # do: it is drawn in the info panel of the manager that scene opened, and on
  # its own it would photograph an empty desktop.
  {
    id = "globe";
    needs = [ "manager-window" ];
  }
  # THE FACTS GIVEN THE WHOLE REGION, which rides on the window door for the
  # reason the three above do: what it photographs is a panel of the manager
  # that scene opened, and alone it would photograph an empty desktop.
  {
    id = "facts-wide";
    needs = [ "manager-window" ];
  }
  # THE LISTING'S OTHER SHAPE. It rides on the window the manager's scene
  # opened, like the three above it, and it makes its own folder: the picture it
  # counts pixels of has to be one this repository does not carry (invariant 13).
  {
    id = "thumbnails";
    needs = [ "manager-window" ];
  }
  # THE REGIONS' VIEW, which opens its own door: `geo-open` is a command, so
  # this scene needs nothing from the ones before it.
  { id = "geo"; }
  { id = "brain"; }
  # THE BRAIN'S OWN WINDOW. It needs the scene above, which is what starts the
  # daemon — without one the buffer opens and draws the screen it draws before
  # a mind arrives, which is the picture this repository already has too many
  # of.
  {
    id = "brain-buffer";
    needs = [ "brain" ];
  }
  { id = "update"; }
  { id = "theme"; }
  { id = "hints"; }
  { id = "lifeline"; }
]
