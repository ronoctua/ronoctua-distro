//! The daemon that consumes the compositor's event stream — milestone 4.
//!
//! # Why a thread, and not the fd in calloop
//!
//! It would be possible to hang the compositor's socket straight off `calloop` as a
//! `Generic` and read in the main loop. That is not what we do, for two reasons:
//!
//! 1. `ronoctua-scroll` stays **synchronous and runtime-free** — which lets the
//!    contract binary exercise exactly the same code as the shell, with no
//!    executor at all. A contract that tests another path tests nothing.
//! 2. A JSON line can arrive split across two reads. In the main loop
//!    that would become a state machine; on a thread with a blocking `read_line`
//!    it is one line of code.
//!
//! The price is a channel, which calloop already offers ready-made.
//!
//! # Why the state lives here, and not in the surface
//!
//! Section 9.2 of the plan: if the focused output disappears in the middle of a prompt, the
//! state migrates to the new output without losing what was typed. State
//! tied to a surface would not survive that. Designed this way from
//! now — patching later would be redoing it.

use std::thread;

use ronoctua_compositor::{Event, State};
use ronoctua_i18n::{Texts, key};
use smithay_client_toolkit::reexports::calloop::channel::Sender;

/// Starts the thread that reads the compositor's event stream.
///
/// It returns `false` when there is no compositor to listen to. **It is not an error**: the
/// footer runs without one in tests and in the installer (section 7.1), and taking the
/// shell down over that would trade a degraded modeline for none.
/// Opens a connection, or says there is nobody to talk to.
///
/// # THE ONE PLACE IN THIS CRATE THAT NAMES AN ADAPTER
///
/// Every caller holds a `dyn Compositor` and asks it things, which is what
/// makes "this crate does not know which compositor it is" a fact about the
/// code rather than a sentence in a document. There were FOUR of these, and
/// more than one is how a crate drifts back into naming a product: the day a
/// second adapter exists, three of the four get updated.
///
/// It lives HERE and not beside its busiest caller because this module is the
/// one about the connection — the event stream below opens the same socket for
/// the same reason, and a crate with an opener in one file and a second one in
/// another has two.
///
/// The DEED TABLE is the exception, and it is a different kind of naming: it
/// is generated from `assets/compositor-deeds.nix` and the text in it is this
/// compositor's own command grammar, so it lives in the adapter by nature
/// rather than by accident. `ronoctua-compositor` holds the `Deed` and the
/// `Arity`; only the words are the adapter's.
///
/// `only-the-composition-root-names-the-compositor` counts the connections.
///
/// # Errors
///
/// There is no compositor on this session, or it refused the connection.
pub fn open_the_compositor() -> anyhow::Result<Box<dyn ronoctua_compositor::Compositor>> {
    about_to_act("the compositor, opened");
    ronoctua_scroll::Scroll::open()
}

/// The machine is about to be ACTED ON: the person's own compositor opened,
/// their player's daemon asked, a window of theirs started or ended.
///
/// Under `cargo test` that is a defect, and it FAILS RIGHT HERE, before
/// acting. A test that reaches one of these is a door built detached that
/// lost its outbox (`player_window::Reach`) — and the act would land on
/// the machine the tests run on: a play pressed in a test paused the
/// person's film, and a stage opened in one started the test binary again
/// as `player maximized`, which libtest reads as a filter that matches the
/// very test that started it. Everywhere else it does nothing.
///
/// `tests-do-not-touch-the-machine` finds a program a test names; this
/// finds the path a test takes, whatever it names.
#[inline]
pub fn about_to_act(on: &str) {
    #[cfg(test)]
    panic!("a test reached the machine: {on}");
    #[cfg(not(test))]
    let _ = on;
}

pub fn listen_to_the_compositor(channel: Sender<Event>) -> bool {
    let wm = match open_the_compositor() {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(
                error = %e,
                "no compositor IPC; the modeline keeps its start-up state"
            );
            return false;
        }
    };

    let events = match wm.watch() {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!(error = %e, "the compositor refused the event stream");
            return false;
        }
    };

    thread::Builder::new()
        .name("compositor-events".into())
        .spawn(move || {
            let mut events = events;
            loop {
                match events.next_one() {
                    Ok(Some(event)) => {
                        // Every event into the debug log. Without this, "the modeline
                        // did not update" does not tell "the compositor did not
                        // send" from "it sent and we ignored it" — which was
                        // exactly the doubt while building the VM test.
                        tracing::debug!(?event, "an event from the compositor");
                        // A send error only happens if the main loop has already
                        // died. Then the thread ends silently — there is
                        // nothing useful to log in a shell that is on its way out.
                        if channel.send(event).is_err() {
                            return;
                        }
                    }
                    Ok(None) => {
                        tracing::info!("the compositor ended the event stream");
                        return;
                    }
                    Err(e) => {
                        // A malformed event does not take the stream down: that would be
                        // trading a slightly wrong modeline for none.
                        tracing::warn!(error = %e, "an event from the compositor ignored");
                    }
                }
            }
        })
        .expect("the compositor's thread cannot fail to be born");

    true
}

/// The `buffer-name` segment's text from the compositor's state.
///
/// A cascading fallback: title, then app id, then a marker. A
/// window may legitimately have neither — leaving it empty would make the
/// segment disappear, and section 7.2 of the plan says `buffer-name` **elides, but
/// never disappears**.
#[must_use]
pub fn buffer_name(state: &State, texts: &Texts) -> String {
    // EMPTY, and not `*scratch*`. The name came from Emacs, where the scratch is a
    // REAL buffer — it exists, accepts text, can be saved. Here there is
    // nothing behind it: the modeline announced an open buffer that was never
    // implemented. With no focused window there is no buffer, and the segment disappears
    // (see `crate::segments`), for the same reason as the mode.
    let Some(j) = state.focused() else {
        return String::new();
    };
    // The buffer's IDENTITY comes from the cgroup, never from the `app_id` (section 7.5).
    //
    // `app_id` does not distinguish two windows of the same application, and the name
    // the modeline shows is the TITLE's — which changes per window. The correlation by
    // scope comes in here for the day the shell starts spawning: a window of ours is
    // recognised even after an `exec`, and one from outside becomes an orphan instead
    // of being confused with its sibling.
    if let Some(pid) = j.pid {
        let origin = crate::foreign::origin_of_pid(pid);
        tracing::trace!(pid, ?origin, "the focused window's source");
    }
    match j
        .title
        .as_deref()
        .filter(|t| !t.is_empty())
        .or(j.app_id.as_deref())
    {
        // A window's title comes from the APPLICATION: it does not go through i18n, and it
        // must not. Translating the name of an open file would be absurd.
        Some(t) => t.to_owned(),
        None => texts.t(key::BUFFER_NO_NAME),
    }
}

/// The `major-mode` segment's text: the focused window's app id (section 4.3).
///
/// `catalogue` is the launcher's — the entries the session scanned —, and it
/// is what names a window whose `app_id` a program's `.desktop` declares; see
/// [`mode_label`]. An empty one only loses that.
#[must_use]
pub fn buffer_mode(
    state: &State,
    texts: &Texts,
    catalogue: &[ronoctua_apps::DesktopEntry],
) -> String {
    // EMPTY, and not an invented name. With no focused window there is no mode, and
    // writing `fundamental` took up a column asserting something false — the reader
    // went looking for a buffer that does not exist. The segment disappears (see `segments`).
    let Some(j) = state.focused() else {
        return String::new();
    };
    match j.app_id.as_deref().filter(|a| !a.is_empty()) {
        Some(a) => mode_label(texts, a, catalogue),
        None => texts.t(key::MODE_UNKNOWN),
    }
}

/// The prefix an `app_id` of OURS carries.
///
/// It is what `crate::buffer::host_it` writes, and the only thing on screen that
/// tells a buffer of this system from any other window.
pub const OURS: &str = "br.ronoctua.";

/// The mode segment's label for an `app_id`.
///
/// # Why ours are translated and the rest are not
///
/// A foreign application's `app_id` is DATA that belongs to it — the same as its
/// title. Nobody can write a key for every program that might be installed, and
/// translating `ghostty` would be inventing a name for somebody else's software.
/// So it keeps the LAST COMPONENT of the reverse-DNS id, which is what a person
/// recognises out of `with_it.mitchellh.ghostty`.
///
/// Ours are the opposite: the id is a name we chose, English by invariant 18,
/// and what the screen shows is interface, which invariant 8 says comes from a
/// key. Keeping the two the same string is exactly how `glifos` came to be drawn
/// raw on an English-language screen.
///
/// A mode of ours with no key shows `[mode-x]`, and it is meant to: what stops
/// that from reaching a screen is `buffer-modes-have-text`, at build time.
///
/// # And a WEB APP's window shows the name its `.desktop` gives it
///
/// The last dotted component is a guess about a reverse-DNS id, and a web
/// app's window is where it guesses wrong: Chrome names that window after its
/// address — `chrome-web.whatsapp.com__-Default` —, and the guess drew
/// `com__-Default`. The entry that opens it says so in `StartupWMClass=`
/// ([`ronoctua_apps::declared_window`]), and its `Name=` is the web app's own
/// data just as the `app_id` is: the word the launcher lists it under, and
/// still nothing this system invented.
///
/// ONLY FOR CHROME'S WINDOWS (`terminal::CHROME_APP_PREFIX`). Any other
/// program whose entry declares a class already had a reverse-DNS or plain
/// `app_id` the guess reads well — `google-chrome`, `gimp` —, and swapping it
/// for the entry's name would change what the modeline says about every such
/// program, which is not what this was for.
#[must_use]
pub fn mode_label(
    texts: &Texts,
    app_id: &str,
    catalogue: &[ronoctua_apps::DesktopEntry],
) -> String {
    if let Some(id) = app_id.strip_prefix(OURS) {
        return texts.t(&format!("mode-{id}"));
    }
    if app_id.starts_with(crate::terminal::CHROME_APP_PREFIX)
        && let Some(e) = ronoctua_apps::declared_window(catalogue, app_id)
    {
        return e.name.clone();
    }
    app_id.rsplit('.').next().unwrap_or(app_id).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_compositor::Window;

    fn texts() -> Texts {
        Texts::fresh("pt-BR").expect("pt-BR has to exist")
    }

    fn with_focus(title: Option<&str>, app_id: Option<&str>) -> State {
        let mut e = State::default();
        e.apply_to(&Event::Windows(vec![Window {
            id: 1,
            title: title.map(ToOwned::to_owned),
            app_id: app_id.map(ToOwned::to_owned),
            pid: Some(7),
            is_focused: true,
            floating: false,
            fullscreen: false,
            sticky: false,
            drawer: false,
            hidden: false,
            trailmark: false,
            rect: None,
        }]));
        e
    }

    #[test]
    fn with_no_window_there_is_no_name_and_no_mode() {
        // The state the compositor comes up in.
        //
        // NO `*scratch*`: that name came from Emacs, where the scratch is
        // a REAL buffer. Here there was nothing behind it, and the
        // modeline announced an open buffer that was never implemented.
        let (e, t) = (State::default(), texts());
        assert!(
            buffer_name(&e, &t).is_empty(),
            "with no window there is no buffer to name"
        );
        // The MODE disappears. With no window there is no mode, and writing
        // `fundamental` took up a column asserting something false: the reader
        // went looking for a buffer that does not exist.
        assert!(
            buffer_mode(&e, &t, &[]).is_empty(),
            "with no window the mode has to be empty, so the segment goes away"
        );
    }

    #[test]
    fn an_empty_title_falls_back_to_the_app_id() {
        // A `Some("")` title really does happen — and an `Option` does not catch it.
        let e = with_focus(Some(""), Some("ghostty"));
        assert_eq!(buffer_name(&e, &texts()), "ghostty");
    }

    #[test]
    fn the_mode_is_the_last_component_of_the_app_id() {
        let (e, t) = (
            with_focus(Some("arq.rs"), Some("com.mitchellh.ghostty")),
            texts(),
        );
        // The title is NOT translated: it comes from the application.
        assert_eq!(buffer_name(&e, &t), "arq.rs");
        assert_eq!(buffer_mode(&e, &t, &[]), "ghostty");
    }

    #[test]
    fn a_window_with_no_title_and_no_app_id_still_has_a_name() {
        let (e, t) = (with_focus(None, None), texts());
        // pt-BR: a pt-BR VALUE, which is what this asserts arrives.
        assert_eq!(buffer_name(&e, &t), "*sem nome*");
        assert_eq!(
            buffer_mode(&e, &t, &[]),
            t.t(ronoctua_i18n::key::MODE_UNKNOWN)
        );
    }

    #[test]
    fn a_mode_of_ours_is_translated_and_a_foreign_one_is_not() {
        let t = texts();
        // OURS. The id is English and the screen is not — which is the whole
        // point of the split, and the reason `glifos` used to be the label.
        // pt-BR: the VALUE of a text key, asserted — the screen's word.
        assert_eq!(mode_label(&t, "br.ronoctua.glyphs", &[]), "glifos");
        assert_eq!(mode_label(&t, "br.ronoctua.memory", &[]), "memória");
        // FOREIGN. Nobody can write a key for every program on earth, and
        // inventing a name for somebody else's software is worse than showing
        // the one they chose.
        assert_eq!(mode_label(&t, "com.mitchellh.ghostty", &[]), "ghostty");
        assert_eq!(mode_label(&t, "firefox", &[]), "firefox");
    }

    #[test]
    fn our_buffer_shows_the_translated_mode_on_the_modeline() {
        // End to end through the state, because `buffer_mode` used to take the
        // last component of EVERY app id — and that is what drew `glifos` raw.
        let (e, t) = (
            with_focus(Some("*glifos*"), Some("br.ronoctua.glyphs")),
            texts(),
        );
        // pt-BR: the VALUE of a text key, asserted — the screen's word.
        assert_eq!(buffer_mode(&e, &t, &[]), "glifos");
    }

    /// An entry of the catalogue, declaring (or not) a window class.
    fn entry(id: &str, name: &str, class: Option<&str>) -> ronoctua_apps::DesktopEntry {
        ronoctua_apps::DesktopEntry {
            id: id.to_owned(),
            name: name.to_owned(),
            description: String::new(),
            icon: String::new(),
            categories: Vec::new(),
            mime_types: Vec::new(),
            command: (id.to_owned(), Vec::new()),
            terminal: false,
            working_directory: None,
            startup_wm_class: class.map(ToOwned::to_owned),
            key: ronoctua_apps::fold::fold(name),
        }
    }

    /// THE DEFECT: a web app's window is named after its address, and the
    /// last dotted component of `chrome-web.whatsapp.com__-Default` is
    /// `com__-Default` — which is what the modeline would have drawn.
    #[test]
    fn a_window_a_desktop_entry_declares_shows_that_entrys_name() {
        let t = texts();
        let catalogue = [
            entry("google-chrome", "Google Chrome", Some("google-chrome")),
            entry(
                "ronoctua-web-whatsapp",
                "WhatsApp",
                Some("chrome-web.whatsapp.com__-Default"),
            ),
            entry("com.mitchellh.ghostty", "Ghostty", None),
        ];
        let web = with_focus(Some("WhatsApp"), Some("chrome-web.whatsapp.com__-Default"));
        assert_eq!(buffer_mode(&web, &t, &catalogue), "WhatsApp");
        // WITHOUT the catalogue it is still the guess — the defect, kept as
        // the proof that the catalogue is what fixed it.
        assert_eq!(buffer_mode(&web, &t, &[]), "com__-Default");
        // The same site in another Chrome profile is ANOTHER window class, and
        // nothing declares it: the guess, not a neighbour's name.
        assert_eq!(
            mode_label(&t, "chrome-web.whatsapp.com__-Profile_1", &catalogue),
            "com__-Profile_1"
        );
        // An entry with no class names nothing, even with the app id as its id:
        // the desktop id is not a window class.
        assert_eq!(
            mode_label(&t, "com.mitchellh.ghostty", &catalogue),
            "ghostty"
        );
        // OURS stay translated, whatever an entry declares.
        let ours = [entry("x", "Not Ours", Some("br.ronoctua.glyphs"))];
        assert_eq!(
            mode_label(&t, "br.ronoctua.glyphs", &ours),
            t.t("mode-glyphs")
        );
    }

    /// ONLY A WEB APP'S WINDOW takes its entry's name: every other program
    /// keeps what the modeline always said, whatever its entry declares.
    #[test]
    fn a_program_that_is_not_a_web_app_keeps_its_app_id() {
        let t = texts();
        let catalogue = [
            entry("google-chrome", "Google Chrome", Some("google-chrome")),
            entry(
                "org.gimp.GIMP",
                "GNU Image Manipulation Program",
                Some("gimp"),
            ),
        ];
        assert_eq!(mode_label(&t, "google-chrome", &catalogue), "google-chrome");
        assert_eq!(mode_label(&t, "gimp", &catalogue), "gimp");
        // AND A CHROME WINDOW NOBODY DECLARES keeps the guess.
        assert_eq!(
            mode_label(&t, "chrome-mail.example.org__-Default", &catalogue),
            "org__-Default"
        );
    }
}
