//! The bridge's wire — what the extension says, and what it is told.
//!
//! # Three hops, one language
//!
//! The extension talks to the host binary in Chrome's native-messaging
//! framing (a length and a JSON object); the host talks to the footer over a
//! Unix socket, one JSON object per line. The OBJECTS are the same on both
//! hops — the host adds what only it can read (the pinned extensions and each
//! extension's popup page, from the profile's files) and otherwise relays —
//! so there is one set of types, here, for the three programs.
//!
//! # Whole state, never a delta
//!
//! Every daemon of this system sends its WHOLE state each time something
//! changes (AGENTS.md), and the extension does too: a footer that restarts,
//! or a line lost in a reconnect, is healed by the next message instead of
//! carrying a wrong picture forever.
//!
//! # Tolerant of fields, strict about words
//!
//! A field this build does not know is ignored — an extension one version
//! ahead still draws. A `said` or an `order` this build does not know is
//! REFUSED, because acting on an order nobody understood is how a bridge does
//! the wrong thing politely.

use serde::{Deserialize, Serialize};

/// The version of this wire. The extension puts it in every state, and the
/// footer draws a refusal instead of guessing when the two differ.
pub const VERSION: u32 = 1;

/// What the extension says, relayed by the host to the footer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "said", rename_all = "kebab-case")]
pub enum Said {
    /// Everything the footer draws from, sent whole after every change.
    State(State),
    /// The answer to one [`Envelope`], by its serial.
    Done(Done),
}

/// The answer to one order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Done {
    /// The serial of the [`Envelope`] this answers.
    pub serial: u64,
    /// Whether Chrome did it.
    pub ok: bool,
    /// Why not, in Chrome's or the extension's words — a word such as
    /// `no-history`, `no-future`, `refused-url`, `no-tab`, or Chrome's own
    /// `lastError` message. Shown to the person through a text key when it is
    /// one of ours, and quoted inside one when it is Chrome's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub why: Option<String>,
}

/// The whole state of one Chrome profile's kiosks.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    /// The wire's version, [`VERSION`] when both ends agree.
    pub v: u32,
    /// The profile directory this connection speaks for (`Default`,
    /// `Profile 1`…): one host per profile, one connection per host.
    #[serde(default)]
    pub profile: Option<String>,
    /// Every kiosk window of this profile, in the order they were born.
    #[serde(default)]
    pub kiosks: Vec<Kiosk>,
    /// Every extension of this profile, ours included (the footer leaves ours
    /// out of the lists: it is the bridge, not something to open).
    #[serde(default)]
    pub extensions: Vec<Extension>,
    /// The Chrome window that has the focus, if any of this profile's does.
    #[serde(default)]
    pub focused_window: Option<i64>,
}

/// One kiosk window.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Kiosk {
    /// Chrome's `windowId`.
    pub window: i64,
    /// The nonce the footer put in the kiosk page's address: the half of the
    /// pair that the extension can see (the compositor sees it in the title).
    pub nonce: String,
    /// The profile directory it was opened in.
    #[serde(default)]
    pub profile: String,
    /// True while the kiosk page is still waiting for [`Order::Go`] — the
    /// footer has not paired it with a compositor window yet.
    #[serde(default)]
    pub born: bool,
    /// Chrome's own word for the window's state: `normal`, `fullscreen`,
    /// `maximized` or `minimized`. `fullscreen` is what the compositor's
    /// application fullscreen looks like from inside Chrome.
    #[serde(default)]
    pub state: String,
    /// The tabs, in Chrome's order.
    #[serde(default)]
    pub tabs: Vec<Tab>,
}

/// One tab of a kiosk.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Tab {
    /// Chrome's `tabId`, stable while the tab lives — what a cursor keeps.
    pub id: i64,
    /// Its place in the window, from 0.
    pub index: u32,
    /// The tab in front.
    #[serde(default)]
    pub active: bool,
    /// Pinned in Chrome.
    #[serde(default)]
    pub pinned: bool,
    /// The page's title (data, drawn as it is).
    #[serde(default)]
    pub title: String,
    /// The page's address.
    #[serde(default)]
    pub url: String,
    /// Still loading.
    #[serde(default)]
    pub loading: bool,
    /// Playing sound.
    #[serde(default)]
    pub audible: bool,
    /// Muted by the person.
    #[serde(default)]
    pub muted: bool,
    /// The page's zoom, in percent (100 is none).
    #[serde(default = "hundred")]
    pub zoom_percent: u32,
}

fn hundred() -> u32 {
    100
}

/// One extension of the profile.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Extension {
    /// Chrome's extension id (32 letters `a`–`p`).
    pub id: String,
    /// Its name, as it names itself.
    pub name: String,
    /// Its version string.
    #[serde(default)]
    pub version: String,
    /// Turned on.
    #[serde(default)]
    pub enabled: bool,
    /// Its options page, when it has one.
    #[serde(default)]
    pub options_url: Option<String>,
    /// Its toolbar popup, a path inside the extension (`popup.html`), read by
    /// the HOST from the extension's manifest — no API tells another
    /// extension this.
    #[serde(default)]
    pub popup: Option<String>,
    /// Its place among the pinned ones on Chrome's toolbar, read by the HOST
    /// from the profile's `Preferences`; `None` when it is not pinned.
    #[serde(default)]
    pub pinned_at: Option<u32>,
}

/// One order and its serial, as it travels.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Envelope {
    /// A number the footer chose; the [`Done`] carries it back.
    pub serial: u64,
    /// What to do.
    #[serde(flatten)]
    pub order: Order,
}

/// What the footer may ask of a kiosk.
///
/// Narrow on purpose, like the queue's own door: nothing here installs,
/// uninstalls or turns off an extension, reads a page's contents or runs
/// script in one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "order", rename_all = "kebab-case")]
pub enum Order {
    /// The pair is made: the kiosk page may go to its address.
    Go {
        /// The kiosk's nonce.
        nonce: String,
    },
    /// Bring a tab to the front.
    Activate {
        /// The tab.
        tab: i64,
    },
    /// Close a tab.
    Close {
        /// The tab.
        tab: i64,
    },
    /// Put a tab at another place in its window.
    Move {
        /// The tab.
        tab: i64,
        /// Its new place, from 0.
        index: u32,
    },
    /// Open a new tab in a kiosk: at an address, or Chrome's new-tab page.
    Create {
        /// The kiosk's Chrome window.
        window: i64,
        /// Where to, already checked by [`crate::url::allowed`]; `None` for
        /// the new-tab page.
        #[serde(default)]
        url: Option<String>,
    },
    /// Send a tab to an address.
    Navigate {
        /// The tab.
        tab: i64,
        /// Where to, already checked by [`crate::url::allowed`].
        url: String,
    },
    /// Search the person's default engine for words.
    Search {
        /// The tab to search in (or beside, with `new_tab`).
        tab: i64,
        /// The words.
        text: String,
        /// In a new tab instead of this one.
        #[serde(default)]
        new_tab: bool,
    },
    /// One page back in a tab's history.
    Back {
        /// The tab.
        tab: i64,
    },
    /// One page forward.
    Forward {
        /// The tab.
        tab: i64,
    },
    /// Load the page again.
    Reload {
        /// The tab.
        tab: i64,
    },
    /// Set a tab's zoom.
    Zoom {
        /// The tab.
        tab: i64,
        /// The zoom in percent (100 is none).
        percent: u32,
    },
    /// Open one of an extension's pages in a new tab of a kiosk: its popup,
    /// its options or its entry in Chrome's extension manager.
    OpenExtension {
        /// The kiosk's Chrome window.
        window: i64,
        /// The page, already checked by [`crate::url::extension_page`].
        url: String,
    },
    /// The window stops being a kiosk: the extension forgets it (the footer
    /// has already told the compositor to show Chrome's bars again).
    Release {
        /// The kiosk's Chrome window.
        window: i64,
    },
    /// Send the whole state now (a footer that just connected asks this).
    Look,
}

/// What the host tells the extension about the footer, on its own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "said", rename_all = "kebab-case")]
pub enum HostSaid {
    /// Whether a footer is connected: a kiosk page with no footer to pair it
    /// goes straight to its address instead of waiting.
    Footer {
        /// Connected.
        up: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_state_reads_with_fields_it_does_not_know() {
        let line = r#"{"said":"state","v":1,"later":true,"kiosks":[{"window":45,"nonce":"ab","born":false,"state":"fullscreen","tabs":[{"id":7,"index":0,"active":true,"title":"A","url":"https://a.example/","zoom_percent":125,"shiny":1}]}],"extensions":[],"focused_window":45}"#;
        let said: Said = serde_json::from_str(line).expect("parses");
        let Said::State(state) = said else {
            panic!("a state")
        };
        assert_eq!(state.v, VERSION);
        assert_eq!(state.kiosks[0].tabs[0].zoom_percent, 125);
        assert_eq!(state.focused_window, Some(45));
    }

    #[test]
    fn a_word_it_does_not_know_is_refused() {
        assert!(serde_json::from_str::<Said>(r#"{"said":"shout"}"#).is_err());
        assert!(serde_json::from_str::<Envelope>(r#"{"serial":1,"order":"format-disk"}"#).is_err());
    }

    #[test]
    fn an_order_travels_flat_with_its_serial() {
        let e = Envelope {
            serial: 12,
            order: Order::Move { tab: 7, index: 2 },
        };
        let text = serde_json::to_string(&e).expect("serialises");
        assert_eq!(text, r#"{"serial":12,"order":"move","tab":7,"index":2}"#);
        assert_eq!(serde_json::from_str::<Envelope>(&text).expect("reads"), e);
    }

    #[test]
    fn a_missing_zoom_is_a_hundred() {
        let tab: Tab = serde_json::from_str(r#"{"id":1,"index":0}"#).expect("parses");
        assert_eq!(tab.zoom_percent, 100);
    }
}
