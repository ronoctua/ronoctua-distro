//! THE POWER VIEW IN THE FOOTER'S REGION — the second door onto
//! [`crate::power`], beside its window ([`crate::power_window`]), and the one
//! errand handler both doors carry out ([`carry_out`]).
//!
//! # What it reads, and through what
//!
//! The footer's own power threads — alive for the whole session, because the
//! modeline draws the battery and the profile from them, the alarms are
//! raised from them, and the two locks (file descriptors) live in the
//! process that outlives every window — their last reading, and the profile
//! the person chose (`power.json`, which the footer applies): all lent by
//! type ([`crate::region::kept`]). The chosen profile is a `Cell` the region
//! shares with the footer: a choice made here is the footer's wish before the
//! file is written, so the file's watch finds its own write and does not
//! send the same order twice.
//!
//! # The commands that work with the view shut
//!
//! A profile, the two locks: each is done through the same lines with no view
//! open (`LEADER s p e`, `s p k`). The four that empty the screen and the
//! thresholds OPEN the view on their row — armed, for the four — and `RET`
//! confirms: none acts on the key alone.

use std::cell::Cell;
use std::rc::Rc;

use crate::power::{Errand, Press, Target, View, Which};
use crate::region::{self, Closed, Doorway, Entry, Region, Row, Seen, Taken};
use ronoctua_power::{Action, Can, Handle, Keep, Order, Profile, Snapshot};

/// The region's id, as the footer ranks it (`surface::PRECEDENCE`).
pub const ID: &str = "power";

/// The view's region, as the binary registers it (`regions.rs`).
pub const ENTRY: Entry = Entry {
    id: ID,
    closed,
    open,
};

/// The view's own gestures, by name, and the press each is.
const GESTURES: &[(&str, Press)] = &[
    ("power-enter", Press::Enter),
    ("power-less", Press::Left),
    ("power-more", Press::Right),
    ("power-up", Press::Up),
    ("power-down", Press::Down),
];

/// The three profiles, by the command that chooses each.
const PROFILES: &[(&str, Profile)] = &[
    ("power-profile-performance", Profile::Performance),
    ("power-profile-balanced", Profile::Balanced),
    ("power-profile-saver", Profile::PowerSaver),
];

/// The two locks, by the command that takes or drops each
/// (`power_words::keep_command`).
const LOCKS: &[(&str, Keep)] = &[
    ("power-keep-awake", Keep::Sleep),
    ("power-keep-lid", Keep::Lid),
];

/// The four that empty the screen, by the command that arms each.
const ARMED: &[(&str, Action)] = &[
    ("power-suspend", Action::Suspend),
    ("power-hibernate", Action::Hibernate),
    ("power-reboot", Action::Reboot),
    ("power-off", Action::PowerOff),
];

/// The press a key is — the five gestures, all of them the view's
/// (`power::OF_THE_VIEW`), whatever is held with them. A key outside the
/// table is the view's to swallow, as every modal swallows the keyboard it
/// holds.
#[must_use]
pub fn press_of_key(key: ronoctua_keymap::Key) -> Option<Press> {
    use ronoctua_keymap::Sym;
    Some(match key.sym {
        Sym::Esc => Press::Escape,
        Sym::Up => Press::Up,
        Sym::Down => Press::Down,
        Sym::Left => Press::Left,
        Sym::Right => Press::Right,
        Sym::Enter => Press::Enter,
        _ => return None,
    })
}

/// What a command does while the view is closed.
fn closed(command: &str, argument: Option<&str>, way: &mut dyn Doorway) -> Closed {
    if let Some((_, profile)) = PROFILES.iter().find(|(id, _)| *id == command) {
        let (orders, wish) = lines(way);
        carry_out(
            Errand::Choose(*profile),
            orders.as_deref(),
            Some(&wish),
            way,
        );
        return Closed::Taken;
    }
    if let Some((_, keep)) = LOCKS.iter().find(|(id, _)| *id == command) {
        lock(*keep, argument, way);
        return Closed::Taken;
    }
    match command {
        "power-open" | "power-thresholds" => Closed::Opens,
        "power-window" => {
            way.open_window(vec!["power".to_owned()]);
            Closed::Taken
        }
        id if ARMED.iter().any(|(a, _)| *a == id) => Closed::Opens,
        // A GESTURE OF A CLOSED VIEW does nothing — taken, and not handed on
        // to be called a command nobody answers.
        id if GESTURES.iter().any(|(g, _)| *g == id) => Closed::Taken,
        _ => Closed::NotMine,
    }
}

/// Opens the view at its top, on the footer's last reading and chosen
/// profile; the command that opened it is handed to it next, and aims it
/// (the thresholds, an armed action).
fn open(_: &str, _: Option<&str>, way: &mut dyn Doorway) -> Box<dyn Region> {
    Box::new(Power {
        view: View::fresh(),
        snapshot: region::kept::<Snapshot>(way),
        wish: wish_of(way),
    })
}

/// The profile the footer follows, shared — or, for a door that keeps none,
/// the person's own word read now (`wished`).
fn wish_of(way: &dyn Doorway) -> Rc<Cell<Profile>> {
    region::kept::<Cell<Profile>>(way).unwrap_or_else(|| Rc::new(Cell::new(wished())))
}

/// The lines a door asks the power daemons through, and the wish it follows
/// — the footer's, lent.
fn lines(way: &dyn Doorway) -> (Option<Rc<Handle>>, Rc<Cell<Profile>>) {
    (region::kept::<Handle>(way), wish_of(way))
}

/// The profile the person chose, or the distro's floor — read from
/// `power.json` and the session's variable, as the footer applies it.
#[must_use]
pub fn wished() -> Profile {
    let (wanted, _, complaints) = ronoctua_power::prefs::resolve_in(
        ronoctua_power::prefs::home().as_deref(),
        std::env::var(ronoctua_power::prefs::ENV).ok().as_deref(),
    );
    for c in complaints {
        tracing::warn!(%c, "power.json");
    }
    wanted
}

/// One of the two locks, held by the footer's thread: SET when the word says
/// which way (`power_words::keep_word`), and flipped from what logind says is
/// held when nothing does — never from memory: a footer that restarted holds
/// nothing. A word that is neither is refused; taking it for a flip would do
/// the opposite of what was asked half the time.
fn lock(keep: Keep, word: Option<&str>, way: &mut dyn Doorway) {
    let on = match word {
        Some(w) => {
            let Some(on) = crate::power_words::keep_wanted(w) else {
                tracing::warn!(
                    word = w,
                    "a lock asked for with a word that is not on or off"
                );
                return;
            };
            on
        }
        None => !region::kept::<Snapshot>(way).is_some_and(|s| s.held().contains(&keep)),
    };
    let orders = region::kept::<Handle>(way);
    send(Order::KeepAwake(keep, on), orders.as_deref(), way);
}

/// An order to the daemons, through `orders` — and said, when there is
/// nobody to send it to.
fn send(order: Order, orders: Option<&Handle>, way: &mut dyn Doorway) {
    match orders {
        Some(h) => h.send(order),
        None => {
            let said = way.texts().t(crate::key::POWER_NO_DAEMON);
            way.say(said, crate::echo::Echo::Normal);
        }
    }
}

/// The view, open in the footer's region.
struct Power {
    view: View,
    /// The footer's last reading — the same `Rc` the footer holds.
    snapshot: Option<Rc<Snapshot>>,
    /// The profile the person chose — the footer's own `Cell`.
    wish: Rc<Cell<Profile>>,
}

impl Power {
    /// One press — a key, a gesture by name, a second click — and what it
    /// asks for, carried out. THE PAGE FOLLOWS THE CURSOR, as every list
    /// here does: without it the cursor walked off the region and the page
    /// stayed on the battery — twenty-six presses, the same screen.
    fn press(&mut self, press: Press, way: &mut dyn Doorway) {
        let errand = self.view.key(
            press,
            self.snapshot.as_deref(),
            self.wish.get(),
            way.columns(),
            way.texts(),
        );
        way.follow();
        let orders = region::kept::<Handle>(way);
        carry_out(errand, orders.as_deref(), Some(&self.wish), way);
    }

    /// The view built again, on a row somebody asked for — the thresholds,
    /// or one of the four, ARMED: `RET` confirms, `Esc` cancels. Shown as a
    /// view just opened on that row is: from the first page, the cursor
    /// followed from there (`Doorway::first_page`) — `LEADER q s` lands on
    /// the thirtieth row.
    fn aim(&mut self, at: Option<Target>, arm: Option<Action>, way: &mut dyn Doorway) {
        let can = arm.map(|a| {
            self.snapshot
                .as_deref()
                .map_or(Can::NotAvailable, |s| s.sleep.can(a))
        });
        let want = at.or_else(|| arm.zip(can).map(|(a, c)| Target::Action(a, c)));
        let mut view = match want {
            Some(t) => View::fresh_at(self.snapshot.as_deref(), self.wish.get(), t, way.texts()),
            None => View::fresh(),
        };
        if let Some((a, c)) = arm.zip(can) {
            view.arm(a, c);
        }
        self.view = view;
        way.first_page();
        way.follow();
        way.repaint();
    }
}

impl Region for Power {
    fn id(&self) -> &'static str {
        ID
    }

    /// THE POWER VIEW'S ROWS, as `fields` — its own columns. A heading is not
    /// a row to click on.
    fn rows(&self, seen: &Seen<'_>) -> Vec<Row> {
        self.view
            .rows(
                self.snapshot.as_deref(),
                self.wish.get(),
                seen.columns,
                seen.texts,
            )
            .into_iter()
            .map(|r| Row {
                takes: !r.heading,
                fields: r.fields,
            })
            .collect()
    }

    fn hint(&self, seen: &Seen<'_>) -> Vec<crate::chrome::Piece> {
        self.view.hint(seen.texts)
    }

    fn cursor(&self) -> Option<usize> {
        Some(self.view.cursor())
    }

    fn key(&mut self, key: ronoctua_keymap::Key, way: &mut dyn Doorway) -> Taken {
        let Some(press) = press_of_key(key) else {
            return Taken::No;
        };
        self.press(press, way);
        Taken::Yes
    }

    fn command(&mut self, id: &str, argument: Option<&str>, way: &mut dyn Doorway) -> Taken {
        if let Some((_, profile)) = PROFILES.iter().find(|(c, _)| *c == id) {
            let orders = region::kept::<Handle>(way);
            carry_out(
                Errand::Choose(*profile),
                orders.as_deref(),
                Some(&self.wish),
                way,
            );
            return Taken::Yes;
        }
        if let Some((_, keep)) = LOCKS.iter().find(|(c, _)| *c == id) {
            lock(*keep, argument, way);
            return Taken::Yes;
        }
        if let Some((_, press)) = GESTURES.iter().find(|(c, _)| *c == id) {
            self.press(*press, way);
            return Taken::Yes;
        }
        if let Some((_, action)) = ARMED.iter().find(|(c, _)| *c == id) {
            self.aim(None, Some(*action), way);
            return Taken::Yes;
        }
        match id {
            // ALREADY OPEN AND NOTHING ASKED FOR: left exactly as it is.
            //
            // It used to build a fresh view every time, which threw away
            // whatever the view was in the middle of. The way in was the
            // MOUSE: `LEADER q p` arms "power off", the pointer brushes the
            // battery glyph, the segment runs `power-open`, and the armed
            // question is gone with nothing on screen saying so. A door onto
            // a room you are already in does not rearrange the furniture —
            // the page only follows its cursor.
            "power-open" => way.follow(),
            // WITH A TARGET IT STILL MOVES, because then somebody asked for a
            // row: `s p t` opens on the thresholds whether or not the view
            // was up.
            "power-thresholds" => self.aim(Some(Target::Threshold(Which::End)), None, way),
            "power-window" => way.open_window(vec!["power".to_owned()]),
            _ => return Taken::No,
        }
        Taken::Yes
    }

    /// THE CLICK SELECTS, AND THE SECOND ONE ACTS — invariant 19: a click on
    /// the row the cursor is already on is that row's `RET`. The four things
    /// here that empty the screen stay two steps away even so: the second
    /// click ARMS them, as `RET` does, and only a third on the armed row
    /// confirms.
    fn clicked(&mut self, row: usize, way: &mut dyn Doorway) {
        let rows = self.view.rows(
            self.snapshot.as_deref(),
            self.wish.get(),
            way.columns(),
            way.texts(),
        );
        if self.view.clicked(row, &rows) {
            self.press(Press::Enter, way);
        } else {
            way.repaint();
        }
    }

    /// It says when its rows came or went (`Doorway::rows_changed`): a frame
    /// it asks for on news is only a frame.
    fn says_when_its_rows_change(&self) -> bool {
        true
    }

    /// What the footer heard: a fresh reading, or the profile the person
    /// chose changed under it (`power.json`, written by the other door or by
    /// hand) — the profile now wished, told as news.
    fn heard(&mut self, news: &dyn std::any::Any, way: &mut dyn Doorway) {
        if let Some(fresh) = news.downcast_ref::<Rc<Snapshot>>() {
            // THE OPEN VIEW FOLLOWS THE NEW ROWS: a peripheral that switched
            // off took its row with it, and a pair written elsewhere makes
            // the one the arrows were moving a lie.
            let moved = self.snapshot.as_ref().map(|b| &b.thresholds) != Some(&fresh.thresholds);
            if moved {
                self.view.forget_pending();
            }
            self.snapshot = Some(Rc::clone(fresh));
            let rows = self.view.rows(
                self.snapshot.as_deref(),
                self.wish.get(),
                way.columns(),
                way.texts(),
            );
            self.view.settle(&rows);
            way.rows_changed();
        } else if news.is::<Profile>() {
            // THE "(your choice)" MARK moves with the file — it is read from
            // the shared wish at the next frame, and no row comes or goes.
            way.repaint();
        }
    }
}

/// CARRIES OUT WHAT THE VIEW ASKED FOR, in either door — the footer's region
/// and the window — each with the line it asks the daemons through
/// (`orders`) and the profile it follows (`wish`: the footer's own `Cell`, or
/// the window's).
///
/// One handler, so the two doors cannot do one errand two ways: what is said
/// is said in the door it was asked in, for as long as its class says.
pub fn carry_out(
    errand: Errand,
    orders: Option<&Handle>,
    wish: Option<&Cell<Profile>>,
    way: &mut dyn Doorway,
) {
    match errand {
        Errand::Nothing => {}
        // AN INFORMATIVE ROW COPIES ITSELF — see `chrome::information_of`,
        // and `chrome::copied` for the one place the doors share: a glance.
        Errand::Copy(text) => {
            let said = crate::chrome::copied(&text, way.texts());
            way.say(said, crate::echo::Echo::Brief);
        }
        Errand::Close => way.close(),
        // THE LOCKS ARE HELD BY THE FOOTER: an inhibitor is a file
        // descriptor, and a window will close. So the order goes there as the
        // leader's own command with the state this row decided (`keep_word`),
        // never a flip decided again from a reading a moment older or newer —
        // run by the footer itself in its own door, down the control socket
        // from a window, where a footer that is not there is a sentence.
        Errand::Order(Order::KeepAwake(keep, on)) => way.run(
            crate::power_words::keep_command(keep),
            Some(crate::power_words::keep_word(on).to_owned()),
        ),
        Errand::Order(order) => send(order, orders, way),
        // A CHOICE is the order AND the file. The wish is set first, so that
        // the file's own notice — inotify sees our write like anybody's —
        // finds nothing new and does not apply it twice.
        Errand::Choose(profile) => {
            if let Some(wish) = wish {
                wish.set(profile);
            }
            send(Order::SetProfile(profile), orders, way);
            // NOT WRITTEN IS SAID: a profile that comes back as the old one at
            // the next start is a profile somebody thinks they chose.
            if let Some(dir) = ronoctua_power::prefs::home()
                && let Err(e) = ronoctua_power::prefs::persist(&dir, profile)
            {
                tracing::warn!(error = %e, file = ronoctua_power::prefs::FILE, "a preference could not be written");
                let said = way.texts().t_with(
                    crate::key::ECHO_PREFERENCE_NOT_WRITTEN,
                    &[
                        ("file", ronoctua_power::prefs::FILE),
                        ("why", &e.to_string()),
                    ],
                );
                way.say(said, crate::echo::Echo::Long);
            }
        }
    }
    way.repaint();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").expect("the base language")
    }

    /// The footer's doorway, lending `kept`.
    fn footer(kept: Vec<region::Kept>) -> region::Recorder {
        region::Recorder::new(
            Rc::new(texts()),
            100,
            true,
            ronoctua_shell_door::sudo::Gate::window(),
            ronoctua_shell_kit::wake::Wake::new(|| {}),
        )
        .keeping(kept)
    }

    /// FAILS IF: one of the view's commands does something else with the
    /// view shut than the footer did — the open command, the thresholds and
    /// the four that empty the screen OPEN it; the window opens a window; a
    /// gesture is taken and does nothing — or a command that is not the
    /// view's is taken for it.
    #[test]
    fn a_closed_view_answers_its_own_commands_and_no_other() {
        let mut way = footer(Vec::new());
        for opens in [
            "power-open",
            "power-thresholds",
            "power-suspend",
            "power-hibernate",
            "power-reboot",
            "power-off",
        ] {
            assert_eq!(closed(opens, None, &mut way), Closed::Opens, "{opens}");
        }
        for (gesture, _) in GESTURES {
            assert_eq!(closed(gesture, None, &mut way), Closed::Taken, "{gesture}");
        }
        assert!(way.owed.is_empty(), "{:?}", way.owed);
        assert_eq!(closed("power-window", None, &mut way), Closed::Taken);
        assert_eq!(
            way.owed,
            vec![region::Effect::OpenWindow(vec!["power".to_owned()])]
        );
        assert_eq!(closed("audio-open", None, &mut way), Closed::NotMine);
    }

    /// FAILS IF: a lock asked with a word other than `on` or `off` is taken
    /// for a flip, or a lock with no daemon to carry it is swallowed instead
    /// of said.
    #[test]
    fn a_lock_is_set_by_its_word_and_said_when_nobody_carries_it() {
        let mut way = footer(Vec::new());
        assert_eq!(
            closed("power-keep-awake", Some("sideways"), &mut way),
            Closed::Taken
        );
        assert!(way.owed.is_empty(), "a word that is neither did something");
        assert_eq!(
            closed("power-keep-lid", Some("on"), &mut way),
            Closed::Taken
        );
        let said = texts().t(crate::key::POWER_NO_DAEMON);
        assert_eq!(
            way.owed,
            vec![region::Effect::Say(said, crate::echo::Echo::Normal)]
        );
    }

    /// FAILS IF: the region draws, aims or chooses with a profile of its own
    /// instead of the footer's — the "(your choice)" mark would stay on the
    /// old row, and the footer's watch would take the region's own write
    /// for a new wish and send the order a second time.
    #[test]
    fn the_region_shares_the_footers_wish() {
        let wish = Rc::new(Cell::new(Profile::Balanced));
        let way = footer(vec![wish.clone()]);
        assert!(
            Rc::ptr_eq(&wish_of(&way), &wish),
            "the region's wish is a copy"
        );
        // AND THE ROWS READ IT LIVE: the footer's watch moves the wish, and
        // the "(your choice)" mark moves at the next frame.
        let snapshot: Rc<Snapshot> = Rc::new(Snapshot {
            profiles: Some(ronoctua_power::Profiles {
                active: Profile::Balanced,
                offered: Profile::ALL.to_vec(),
                degraded: None,
                inhibited: None,
                holds: Vec::new(),
                battery_aware: Some(true),
                version: "0.30".into(),
            }),
            ..Snapshot::default()
        });
        let mut way = footer(vec![wish.clone(), snapshot]);
        let opened = open("power-open", None, &mut way);
        let texts = texts();
        let seen = Seen {
            texts: &texts,
            columns: 100,
            lines: 30,
            zone: &ronoctua_files::when::Zone::utc(),
            photographing: true,
            lent: crate::region::Lent::none(),
        };
        let drawn = |r: &dyn Region| -> Vec<String> {
            r.rows(&seen)
                .iter()
                .flat_map(|row| row.fields.iter().map(|f| f.text.clone()))
                .collect()
        };
        let as_active = drawn(opened.as_ref());
        wish.set(Profile::Performance);
        assert_ne!(
            drawn(opened.as_ref()),
            as_active,
            "a wish moved by the footer did not move the region's rows"
        );
    }

    /// FAILS IF: the open view, aimed at a row of its own (the thresholds,
    /// an armed action), is not shown from the first page as a view just
    /// opened on that row is — or the open command, with nothing asked,
    /// does more than follow the cursor — or a click that only selects
    /// moves the page, or a press does not.
    #[test]
    fn an_open_view_aimed_again_starts_from_the_first_page() {
        let mut way = footer(Vec::new());
        let mut opened = open("power-open", None, &mut way);
        way.owed.clear();
        assert_eq!(opened.command("power-open", None, &mut way), Taken::Yes);
        assert_eq!(
            way.owed,
            vec![region::Effect::Follow],
            "the open view was moved"
        );
        for aimed in ["power-thresholds", "power-suspend", "power-off"] {
            way.owed.clear();
            assert_eq!(opened.command(aimed, None, &mut way), Taken::Yes);
            let page: Vec<&region::Effect> = way
                .owed
                .iter()
                .filter(|e| matches!(e, region::Effect::FirstPage | region::Effect::Follow))
                .collect();
            assert_eq!(
                page,
                vec![&region::Effect::FirstPage, &region::Effect::Follow],
                "{aimed}"
            );
        }
        // A CLICK THAT ONLY SELECTS does not move the page; a press does.
        way.owed.clear();
        opened.clicked(3, &mut way);
        assert!(
            !way.owed.contains(&region::Effect::Follow),
            "{:?}",
            way.owed
        );
        way.owed.clear();
        assert_eq!(opened.command("power-down", None, &mut way), Taken::Yes);
        assert!(way.owed.contains(&region::Effect::Follow), "{:?}", way.owed);
    }

    /// FAILS IF: a lock asked by a row of the view is sent by the door that
    /// drew it, instead of going to the footer as the leader's own command
    /// with the state the row decided — in both doors alike.
    #[test]
    fn a_lock_goes_to_the_footer_with_the_state_wanted_in_both_doors() {
        let mut way = footer(Vec::new());
        carry_out(
            Errand::Order(Order::KeepAwake(Keep::Sleep, true)),
            None,
            None,
            &mut way,
        );
        assert_eq!(
            way.owed.first(),
            Some(&region::Effect::Run(
                "power-keep-awake",
                Some("on".to_owned())
            ))
        );
        let mut ui = crate::door::Ui::for_tests(texts());
        carry_out(
            Errand::Order(Order::KeepAwake(Keep::Lid, false)),
            None,
            None,
            &mut ui,
        );
        assert_eq!(ui.orders(), &["command power-keep-lid off".to_owned()]);
    }

    /// FAILS IF: a key the view does not teach is taken for a press, or one
    /// it does is lost — the same five the window reads, with anything held.
    #[test]
    fn the_five_gestures_are_the_keys() {
        let press = |n: &str| press_of_key(ronoctua_keymap::Key::parse(n).expect("a key"));
        assert_eq!(press("ESC"), Some(Press::Escape));
        assert_eq!(press("RET"), Some(Press::Enter));
        assert_eq!(press("<left>"), Some(Press::Left));
        assert_eq!(press("S-<right>"), Some(Press::Right));
        assert_eq!(press("<up>"), Some(Press::Up));
        assert_eq!(press("<down>"), Some(Press::Down));
        assert_eq!(press("q"), None);
    }

    /// FAILS IF: a reading or the profile wished, told by the type the
    /// footer tells each (`Rc<Snapshot>`, `Profile`: `surface/mod.rs`), is
    /// not the one the region takes — the open view would freeze on the old
    /// battery — or a reading does not say its rows may have come or gone
    /// (a peripheral that switched off takes its row), or a wish asks more
    /// than a frame, or a news of a type it does not read asks anything.
    #[test]
    fn what_the_footer_tells_is_heard_by_its_type() {
        let wish = Rc::new(Cell::new(Profile::Balanced));
        let mut way = footer(vec![wish.clone()]);
        let mut power = Power {
            view: View::fresh(),
            snapshot: None,
            wish,
        };
        let fresh = Rc::new(Snapshot::default());
        Region::heard(&mut power, &fresh, &mut way);
        assert!(
            power
                .snapshot
                .as_ref()
                .is_some_and(|s| Rc::ptr_eq(s, &fresh)),
            "the reading told is not the one drawn"
        );
        assert_eq!(way.owed, vec![region::Effect::RowsChanged]);
        way.owed.clear();
        Region::heard(&mut power, &Profile::Performance, &mut way);
        assert_eq!(way.owed, vec![region::Effect::Repaint]);
        way.owed.clear();
        Region::heard(&mut power, &(), &mut way);
        assert!(way.owed.is_empty(), "{:?}", way.owed);
    }
}
