//! The player's OTHER MACHINES: the page of the ones on the tailnet this one
//! plays in step with (`LEADER p m`, and the row on the home), and the
//! pairing — a name, then the code the other screen shows, sent off the
//! door's loop.

use crate::player::{Target, trouble_sentence};
use crate::player_rows::Builder;
use ronoctua_player::{
    remote::Role,
    wire::{Ask, State},
};

/// A pairing's code: six digits read off another screen, exchanged for the
/// key two machines share. A secret — its `Debug` says nothing, so no log line
/// that prints an errand prints it.
#[derive(Clone, PartialEq, Eq)]
pub struct Code(pub String);

impl std::fmt::Debug for Code {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Code(…)")
    }
}

/// What a door remembers between a pairing's two questions: the machine's
/// name, until its code is typed. The DOOR's and not the view's, because the
/// footer asks it from `LEADER :` with no view open.
#[derive(Debug, Default)]
pub struct Pairing {
    name: Option<String>,
}

impl Pairing {
    /// The question was let go: the name goes with it, so a code typed later
    /// under another question is not taken for this one.
    pub fn let_go(&mut self) {
        self.name = None;
    }

    /// The machine's name, typed: its code is asked next.
    pub fn name(&mut self, name: &str) {
        self.name = Some(name.to_owned());
    }

    /// The name the code pairs with — once: a second code pairs with nothing.
    pub fn take(&mut self) -> Option<String> {
        self.name.take()
    }
}

/// How long a pairing may take: the player answers only once the other
/// machine has — its own patience is five seconds to connect and five to
/// hear back — and the three seconds every other ask gets ended the wait
/// while the pairing went on, and said the player was not running.
const PAIR_PATIENCE: std::time::Duration = std::time::Duration::from_secs(12);

/// How a pairing sent off the loop ended — the words are said on the loop,
/// where the language is ([`paired_said`]).
#[derive(Debug)]
pub enum Paired {
    /// The player answered: paired, or refused with its reason.
    Answered(ronoctua_player::wire::Say),
    /// No player on this machine to pair.
    NotInstalled,
    /// The player could not be started or reached.
    NotRunning,
    /// The player was asked and had not answered by [`PAIR_PATIENCE`].
    NoAnswer,
}

/// Pairs with `to`, OFF THE LOOP — both doors send the code this way — and
/// hands `done` how it ended, on the thread that waited.
///
/// THE CODE LEAVES FOR THE PLAYER, and only there: it is the one path it has,
/// down the player's socket, and it is never printed on the way.
pub fn pair_then(to: String, code: Code, done: impl FnOnce(Paired) + Send + 'static) {
    crate::daemon::about_to_act("the player's daemon, asked to pair");
    let born = std::thread::Builder::new()
        .name("player-pair".to_owned())
        .spawn(move || {
            let paired = match ronoctua_player::client::ensure() {
                Err(e) if e.is::<ronoctua_player::client::NotInstalled>() => Paired::NotInstalled,
                Err(_) => Paired::NotRunning,
                Ok(_) => match ronoctua_player::client::ask_within(
                    &Ask::PairWith { to, code: code.0 },
                    PAIR_PATIENCE,
                ) {
                    Ok(say) => Paired::Answered(say),
                    Err(e) if timed_out(&e) => Paired::NoAnswer,
                    Err(_) => Paired::NotRunning,
                },
            };
            done(paired);
        });
    if let Err(e) = born {
        tracing::error!(error = %e, "could not start the thread that pairs");
    }
}

/// Whether an ask ended because the answer did not come in time.
fn timed_out(e: &anyhow::Error) -> bool {
    e.downcast_ref::<std::io::Error>().is_some_and(|io| {
        matches!(
            io.kind(),
            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
        )
    })
}

/// What a door says of a pairing that ended: nothing when it paired, the
/// player's reason when it refused, and which of the other three it was.
#[must_use]
pub fn paired_said(paired: &Paired, texts: &ronoctua_i18n::Texts) -> Option<String> {
    use crate::key;
    match paired {
        Paired::Answered(ronoctua_player::wire::Say::Refused(t)) => {
            Some(trouble_sentence(t, texts))
        }
        Paired::Answered(_) => None,
        Paired::NotInstalled => Some(texts.t(key::PLAYER_NOT_INSTALLED)),
        Paired::NotRunning => Some(texts.t(key::PLAYER_NOT_RUNNING)),
        Paired::NoAnswer => Some(texts.t(key::PLAYER_PAIR_NO_ANSWER)),
    }
}

/// The page: what it is for, the ones this machine can join, the ones that
/// can join it, the code this one shows and the pairing — under its title,
/// which the view draws (`player_page::PageId::key`).
pub fn machines(b: &mut Builder<'_>, s: &State) {
    use crate::key;
    if s.tailnet.is_none() {
        b.note(key::PLAYER_NO_TAILNET);
        return;
    }
    b.note(key::PLAYER_ABOUT_MACHINES);
    for host in &s.hosts {
        let joined = s.joined.as_ref().filter(|j| &j.host == host);
        let value = joined.map_or_else(
            || b.t(key::PLAYER_JOIN),
            |j| {
                b.texts.t_with(
                    key::PLAYER_JOINED_AS,
                    &[
                        ("role", &b.t(role_key(j.role))),
                        ("ms", &j.drift_ms.to_string()),
                    ],
                )
            },
        );
        b.pair(
            host.clone(),
            value,
            Target::Host(host.clone()),
            &format!("host-{host}"),
        );
    }
    for guest in &s.guests {
        b.pair(
            guest.clone(),
            b.t(key::PLAYER_GUEST),
            Target::Guest(guest.clone()),
            &format!("guest-{guest}"),
        );
    }
    b.pair(
        b.t(key::PLAYER_INVITE),
        s.invite.clone().unwrap_or_default(),
        Target::Invite,
        "invite",
    );
    b.pair(b.t(key::PLAYER_PAIR), String::new(), Target::Pair, "pair");
}

/// A role's name.
pub const fn role_key(r: Role) -> &'static str {
    match r {
        Role::Both => crate::key::PLAYER_ROLE_BOTH,
        Role::Screen => crate::key::PLAYER_ROLE_SCREEN,
        Role::Sound => crate::key::PLAYER_ROLE_SOUND,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::{Errand, PageId, Press, View, carry_out};
    use crate::player_question::{PAIR, PAIR_CODE, Question, answered, question};

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").expect("the base language")
    }

    fn on_the_tailnet() -> State {
        State {
            tailnet: Some("here".into()),
            ..crate::player::fixture::alone("/music/one.flac", Vec::new())
        }
    }

    /// The view on its Machines page, walked to as a person does: the row on
    /// the home, clicked twice.
    fn on_the_machines_page(s: &State) -> View {
        let t = texts();
        let mut v = View::blank(None);
        v.settle_in(Some(s), 80, &t);
        let rows = v.rows(Some(s), 80, &t);
        let at = rows
            .iter()
            .position(|r| r.target == Target::Go(PageId::Machines))
            .expect("the home has a row for the machines");
        v.clicked(at, &rows);
        assert_eq!(v.key(Press::Enter, Some(s), 80, &t), Errand::Nothing);
        assert_eq!(v.page(), PageId::Machines);
        v
    }

    #[test]
    fn a_paired_machine_joins_turns_and_leaves() {
        let mut s = on_the_tailnet();
        s.hosts = vec!["den".into()];
        let t = texts();
        let mut v = on_the_machines_page(&s);
        let rows = v.rows(Some(&s), 80, &t);
        let at = rows
            .iter()
            .position(|r| r.target == Target::Host("den".into()))
            .expect("the machine is a row of the page");
        v.clicked(at, &rows);
        assert_eq!(
            v.key(Press::Enter, Some(&s), 80, &t),
            Errand::Tell(Ask::Join {
                host: "den".into(),
                role: Role::Both
            })
        );
        s.joined = Some(ronoctua_player::wire::Joined {
            host: "den".into(),
            role: Role::Both,
            drift_ms: 12,
        });
        assert_eq!(
            v.key(Press::More, Some(&s), 80, &t),
            Errand::Tell(Ask::Join {
                host: "den".into(),
                role: Role::Screen
            })
        );
        assert_eq!(
            v.key(Press::Less, Some(&s), 80, &t),
            Errand::Tell(Ask::Join {
                host: "den".into(),
                role: Role::Sound
            })
        );
        assert_eq!(
            v.key(Press::Enter, Some(&s), 80, &t),
            Errand::Tell(Ask::Leave)
        );
    }

    /// FAILS IF: a pairing is sent on a door's loop — `carry_out` telling
    /// the player, which answers only after the other machine does — or a
    /// wait that ran out is said as "the player is not running" while the
    /// pairing may still succeed.
    #[test]
    fn a_pairing_is_the_doors_to_send_off_the_loop_and_a_slow_one_is_said_so() {
        let t = texts();
        let pair = Errand::Pair {
            to: "den".into(),
            code: Code("123456".into()),
        };
        assert_eq!(carry_out(pair.clone(), &t, None), Err(pair));
        let slow = anyhow::Error::from(std::io::Error::from(std::io::ErrorKind::WouldBlock));
        assert!(timed_out(&slow));
        let also = anyhow::Error::from(std::io::Error::from(std::io::ErrorKind::TimedOut));
        assert!(timed_out(&also));
        let gone = anyhow::Error::from(std::io::Error::from(std::io::ErrorKind::ConnectionRefused));
        assert!(!timed_out(&gone));
        assert!(!timed_out(&anyhow::anyhow!("the player did not answer")));
        let said = |p: Paired| paired_said(&p, &t);
        assert_eq!(
            said(Paired::NoAnswer),
            Some(t.t(crate::key::PLAYER_PAIR_NO_ANSWER))
        );
        assert_ne!(said(Paired::NoAnswer), said(Paired::NotRunning));
        assert_eq!(
            said(Paired::NotRunning),
            Some(t.t(crate::key::PLAYER_NOT_RUNNING))
        );
    }

    /// FAILS IF: pairing stops being two questions — the name in the clear,
    /// then the code MASKED — or stops being a row of the Machines page, or
    /// the code pairs with no name asked before it, or a code's `Debug`
    /// prints the digits.
    #[test]
    fn pairing_asks_the_name_and_then_the_secret_code() {
        let t = texts();
        let s = on_the_tailnet();
        let mut v = on_the_machines_page(&s);
        let rows = v.rows(Some(&s), 80, &t);
        let at = rows
            .iter()
            .position(|r| r.target == Target::Pair)
            .expect("the pairing row");
        v.clicked(at, &rows);
        assert_eq!(
            v.key(Press::Enter, Some(&s), 80, &t),
            Errand::Ask(Question::plain(PAIR, false))
        );
        let mut pairing = Pairing::default();
        assert_eq!(
            answered(PAIR, " den ", &mut pairing),
            Errand::Ask(Question::plain(PAIR_CODE, true))
        );
        let paired = answered(PAIR_CODE, " 123456 ", &mut pairing);
        assert_eq!(
            paired,
            Errand::Pair {
                to: "den".into(),
                code: Code("123456".into())
            }
        );
        assert!(!format!("{paired:?}").contains("123456"), "{paired:?}");
        // A CODE WITH NO NAME BEFORE IT is never mind, and so is an empty one.
        assert_eq!(answered(PAIR_CODE, "123456", &mut pairing), Errand::Nothing);
        answered(PAIR, "den", &mut pairing);
        pairing.let_go();
        assert_eq!(answered(PAIR_CODE, "123456", &mut pairing), Errand::Nothing);
        answered(PAIR, "den", &mut pairing);
        assert_eq!(answered(PAIR_CODE, "  ", &mut pairing), Errand::Nothing);
        assert_eq!(question(PAIR_CODE).map(|q| q.secret), Some(true));
    }

    /// FAILS IF: the page's sentences leave it — off the tailnet, what it is
    /// for — or the pairing rows show where they cannot work.
    #[test]
    fn off_the_tailnet_the_page_says_why_and_offers_no_pairing() {
        let t = texts();
        let mut s = on_the_tailnet();
        s.tailnet = None;
        let v = on_the_machines_page(&s);
        let rows = v.rows(Some(&s), 80, &t);
        let said: String = rows
            .iter()
            .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
            .collect::<Vec<_>>()
            .join(" ");
        let why = t.t(crate::key::PLAYER_NO_TAILNET);
        assert!(
            said.contains(why.split_whitespace().next().unwrap_or_default()),
            "{said}"
        );
        assert!(!rows.iter().any(|r| r.target == Target::Pair));
    }
}
