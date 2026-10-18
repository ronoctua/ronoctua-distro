//! THE SCREENS' READER, AS THE FOOTER KEEPS IT FOR THE SESSION — not a view.
//!
//! The footer starts the screens' reader at login and keeps it: the
//! brightness keys order it with no view open, every hotplug refreshes what it
//! read, and the size of the text lives in the file it watches. The screens'
//! VIEW is a region of its own crate (`ronoctua-shell-display`),
//! told every reading and every order finished as the footer hears them, and
//! handed a line to this reader when it opens (`Footer::standing`). What is
//! here is what the footer does whether the view is open or not.

use super::*;

/// The screens' view's id, as the footer ranks it (`PRECEDENCE`) — the
/// region the binary registers under it, hosted by its id alone.
const SCREENS: &str = "display";

impl Footer {
    /// The size of the text changed under us, in the file.
    ///
    /// THE FOOTER'S OWN HEIGHT MOVES WITH IT, which is the part that is easy
    /// to miss: two lines of taller text is a taller footer, and the layer's
    /// size and its exclusive zone are both computed from the size. Leaving
    /// them would give a surface that draws text it has no room for, and a
    /// desktop whose windows keep the old gap at the bottom.
    pub(super) fn display_file_changed(&mut self) {
        // THE HEIGHT IS TAKEN BEFORE THE SIZE MOVES. `wanted_height` reads
        // the size live, so asking it after the reload compares two values
        // that are both the NEW one — a guard that always says "no configure
        // is coming" and always draws. It was measured doing exactly that:
        // one frame per change at the old height with the new columns, laid
        // out for 79 columns into a 53-pixel surface.
        let was = self.wanted_height();
        // THE FILE CARRIES TWO THINGS, and this guard only ever asked about
        // one. `display.json` holds the size of the text AND every screen's
        // `Wanted` — the source of every number the screens view draws — and
        // it is a file this repository says a person may edit by hand. A block
        // changed there produced nothing at all: the size had not moved, so
        // this returned, and the view went on showing the old scale until an
        // order or a hotplug forced a reading.
        //
        // AND ONLY WITH THE VIEW OPEN. This file is the one every `Order::Keep`
        // rewrites, so unconditionally the shell asked for a reading of its
        // OWN writes — a walk of `/sys/class/drm` and two connections to the compositor,
        // on top of the one the order itself takes, up to twice a second while
        // an arrow is held (`Notice` absorbs the rest of the burst). With the
        // view shut nothing would draw the answer: `open_display` takes a DEEP
        // reading of its own, so a hand edit made while it was closed is on
        // the screen the moment it opens. `Order::Bright`'s doc removed a
        // third reading on this same repeating path for the same reason.
        if self.regions.is_open(SCREENS)
            && let Some(h) = self.display_orders.as_ref()
        {
            h.send(ronoctua_display::Order::Look { deep: false });
        }
        if !crate::toolkits::reload() {
            return;
        }
        let now = tokens::text::percent();
        tracing::info!(percent = now, "the text size changed");
        // THE FOREIGN APPLICATIONS TOO, and only from HERE. They hear dconf
        // and not this file, and doing it from every door would spawn one
        // `dconf` per open window for one change.
        crate::toolkits::tell_them(now);

        // THE SHAPERS HAVE TO HEAR IT. Each was built with a size and keeps
        // it: moving the columns, the cell and every hit test while the
        // letters stay is a grid at one size with text at another, which
        // reads as a broken font rather than as a setting that took.
        for screen in &mut self.screens {
            if let Some(t) = screen.text.as_mut() {
                t.draw_at(tokens::text::size());
            }
        }

        // AND THE PIXMAPS GO — BUT THAT IS A MEMORY BOUND, NOT THE FIX.
        //
        // What makes a size change correct is that `launch::pixmap_key` carries
        // the side, so the new size asks the caches a question they have not
        // been asked. Emptying these two maps cannot do it and it is worth
        // writing down why, because for one round it was what stood here: the
        // picture that is DRAWN lives on the GPU, in `Images::uploads`, and
        // that map has no per-key removal at all — its `clear()` empties the
        // frame's batch and says "the textures stay", and its `dump()` only
        // acts above a ceiling of 39 and then keeps whatever is on screen. So
        // the host re-decoded at 36 px, `missing()` answered "already up"
        // about the 18 px texture, and the finder drew it stretched into the
        // new box for the life of the footer — the exact defect the clearing
        // was written to remove.
        //
        // What the clearing IS for: without it these two maps would hold one
        // entry per (icon, side) for every size the session passes through.
        // The GPU side is bounded by `dump()`; this side would not be.
        self.icons.clear();
        self.icons_requested.clear();

        // AND THE HEIGHT A NEW SCREEN IS BORN AT. `apply_geometry` reads
        // `wanted_height()` live, so the surfaces that exist are already
        // covered by the call below; what this field is read for is
        // `create_screen`, and without it a monitor plugged in AFTER the
        // change would come up two lines of yesterday's size tall.
        // AND THE HEIGHT COMPARED IS THE ONE THAT IS SENT. `requested_height`
        // is the two fixed lines; what `apply_geometry` commits is
        // `wanted_height()`, which is those plus the band, the fold, the
        // brain's line and the expansion. They happen to move together for
        // every step of the range — both are `ceil(line_height x n)` — so
        // comparing the wrong one gave the right answer by coincidence, and a
        // coincidence is not a guard.
        self.requested_height = tokens::metrics::footer_height(tokens::text::size()).ceil() as u32;
        self.apply_geometry(self.interactivity());

        // AND THE FRAME WAITS FOR THE CONFIGURE. Committing a new layer size
        // and painting in the same breath paints into a texture the
        // compositor has not resized yet: at 200% the footer wants twice the
        // height and the surface is still the old one, so the minibuffer's
        // line lands off it for one frame. When the height did NOT change
        // there is no configure coming, and then this is the only chance to
        // draw.
        // AND THE TWO THREADS THAT DRAW INTO THE PANEL, ALWAYS — before the
        // branch, because the branch is not taken on the ordinary path.
        //
        // `wanted_height()` is `ceil(line_height x n)`, so a change of text
        // size MOVES it and the `if` below is false: the frame waits for the
        // configure. And `Footer::configure` calls `draw()` and `paint()`, not
        // `draw_and_paint()` — so what the shown region's threads are asked
        // before and after a frame (`Region::before_frame`, `after_frame`)
        // was skipped on exactly the path a size change takes. The window
        // door asks them by name and its comment says
        // why: a change of size resizes the panel WITHOUT a `configure` (the
        // pixels did not move, the cells did), which is a way in that
        // enumerating the others always misses.
        //
        // What it looked like: the manager open in the footer with a shape or
        // a film under the cursor, the size stepped from the display window,
        // and the stage still rendering into the old box — a piece sitting
        // small and centred in a panel that had grown, until any key or
        // pointer event reached the footer. A running film healed on its next
        // frame; a still shape did not.
        // ON THE PAGE THE FRAME WILL LOOK THE PICTURES UP UNDER, as before
        // every frame (`wear_the_pictures_page`).
        self.wear_the_pictures_page();
        self.hosted_before_frame();
        self.hosted_after_frame();
        if self.wanted_height() == was {
            self.draw_and_paint();
        }
    }

    /// A reading of the screens arrived, or an order finished.
    pub(super) fn display_event(&mut self, event: ronoctua_display::Event) {
        // WHAT THE FOOTER HEARD, told to every region of the slot host first:
        // the screens' view settles on a reading, and carries a refusal out
        // itself (`display::refusal`, the table both its doors read).
        self.hosted_heard::<ronoctua_display::Event>(&event);
        let shown = self.view() == ViewState::Region(SCREENS);
        match event {
            ronoctua_display::Event::Snapshot(now) => {
                self.display_snapshot = *now;
                // A MONITOR ARRIVING IS A ROW ARRIVING, and the surface has to
                // grow for it — the radio's own arrival does this. Without it
                // the last row fell off the bottom until the next keypress.
                if shown {
                    self.apply_geometry(self.interactivity());
                }
                self.settle_region();
            }
            ronoctua_display::Event::Done { order, refused } => {
                // A BRIGHTNESS KEY REFUSED WITH NO VIEW OPEN says so on the
                // message line: the key has no view to put the sentence in,
                // and a key that does nothing reads as a broken key.
                let open = self.regions.is_open(SCREENS);
                if let (Some(why), ronoctua_display::Order::Bright { .. }, false) =
                    (&refused, &order, open)
                {
                    let said = crate::screens_words::refused(why, &self.texts);
                    // THE FLASH GOES WITH IT: it showed a number that never
                    // landed.
                    if self
                        .sound
                        .as_ref()
                        .is_some_and(|s| s.what == crate::sound::Source::Screen)
                    {
                        let was = self.above_rows();
                        self.sound = None;
                        self.glance = None;
                        if self.above_rows() != was {
                            self.apply_geometry(self.interactivity());
                        }
                    }
                    self.echo_message(said);
                    self.draw_and_paint();
                    return;
                }
                // ONE GEOMETRY FOR THE REFUSAL the view carried out: the
                // message line is a row too, so this asks for a size whether
                // a question came down or not.
                if refused.is_some() && shown {
                    self.apply_geometry(self.interactivity());
                }
            }
        }
        if shown || self.regions.is_open(SCREENS) {
            self.draw_and_paint();
        }
    }

    /// Puts a standing preview back before this process ends.
    ///
    /// A preview lives in the compositor's running configuration, and only
    /// the thread that applied it can take it down. The footer usually quits
    /// because the session is going down and the compositor is going with it
    /// — but not always: `nix run .#dev` restarts the footer under a
    /// compositor that keeps running, the rescue quits, and a second footer
    /// taking this display sends `Order::Quit`. There the change would stand
    /// with nothing left that could undo it.
    ///
    /// EVERY REGION LETS GO FIRST (`Region::closing`): the screens' view puts
    /// a standing preview back through its line to the reader — and that line
    /// is a sender the reader's loop waits on, so it has to be gone before
    /// `finish` waits for the loop to drain. AND IT WAITS FOR WHATEVER WAS
    /// SENT, not only for a question that is still standing — the window's
    /// door carries that reason too (`Display::leave`).
    pub(super) fn put_a_preview_back(&mut self) {
        self.let_every_region_go();
        let Some(h) = self.display_orders.take() else {
            return;
        };
        h.finish(None);
    }
}
