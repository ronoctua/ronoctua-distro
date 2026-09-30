//! THE AMBIENT SOUNDS' PAGE (`player_page::PageId::Ambience`, `LEADER p e`)
//! and its credits.
//!
//! # What they are
//!
//! Recordings — rain, a storm, falling water — each approved by ear and
//! given to everyone by the person who made it, that play BESIDE the queue,
//! round and round, until switched off (`assets/ambience.nix`). The player
//! keeps them apart from the queue: stopping, the next item, the media keys
//! and the player's own volume never reach them; only their own switch,
//! "silence them all", holding them, and closing the player do.
//!
//! # A sound a row
//!
//! Each sound is a row: the glyph of its group (`ambience-*`), its name
//! (`player-ambience-<id>` — one the door has no word for is said as one,
//! never by its raw id), and its own volume as a bar, with whether it is on.
//! `RET` — and the second click — turns it on or off, and on it plays at once;
//! `←`/`→` and the bar turn its volume, on or off, which the player keeps for
//! the next time. Past the machine's `at_once` a sound more is refused in the
//! player's own sentence before it is sent.
//!
//! While any is on, a row holds them all or lets them go on, and one silences
//! them all. The credits are a page of their own: who recorded each, where
//! it was published, and under what licence.
//!
//! A machine with none says why, as the player says it (`Ambience::trouble`)
//! — the recordings come with the player once they are published.

use crate::player::{Errand, PageId, Target, VOLUME_MOST, trouble_sentence};
use crate::player_rows::{Builder, Slid, share};
use ronoctua_i18n::key;
use ronoctua_player::limit::{Limit, TooMany};
use ronoctua_player::wire::{Licence, Sounding, State, Trouble};
use ronoctua_tokens as tokens;

/// Ambient sound `id` in `s`, when this machine has it.
#[must_use]
pub fn sound<'a>(s: &'a State, id: &str) -> Option<&'a Sounding> {
    s.ambience.sounds.iter().find(|x| x.id == id)
}

/// The glyph of a sound's group, first in its row — the plain wave for a
/// group this door has no drawing of: it still plays, and its row still says
/// its name.
#[must_use]
pub fn glyph_of(group: &str) -> char {
    match group {
        "water" => tokens::icon::AMBIENCE_WATER,
        "storm" => tokens::icon::AMBIENCE_STORM,
        _ => tokens::icon::AMBIENCE_SOUND,
    }
}

/// What a sound's row says after its bar: its volume while it sounds, and
/// otherwise that it is off, or held — the bar says the volume it keeps. ONE
/// short word: a column is narrow, and a longer value squeezed the sound's
/// name to a stub cut mid-word.
fn state_of(sound: &Sounding, held: bool, texts: &ronoctua_i18n::Texts) -> String {
    match (sound.on, held) {
        (true, false) => format!("{}%", sound.volume_percent),
        (true, true) => texts.t(key::PLAYER_AMBIENCE_HELD),
        // THE SWITCH'S OWN WORD, the one every switch of the view says.
        (false, _) => crate::player_rows::on_off(false, texts),
    }
}

/// Turning ambient sound `id` on or off: on, a sound past the machine's
/// `at_once` is refused in the player's own sentence before it is sent.
#[must_use]
pub fn turned(id: &str, s: &State, texts: &ronoctua_i18n::Texts) -> Errand {
    use ronoctua_player::wire::Ask;
    let Some(sound) = sound(s, id) else {
        return Errand::Nothing;
    };
    let on = s.ambience.sounds.iter().filter(|x| x.on).count();
    if !sound.on && s.ambience.at_once > 0 && on >= usize::from(s.ambience.at_once) {
        return Errand::Said(trouble_sentence(
            &Trouble::TooMany(TooMany {
                what: Limit::Ambience,
                most: usize::from(s.ambience.at_once),
            }),
            texts,
        ));
    }
    Errand::Tell(Ask::Ambient {
        id: sound.id.clone(),
        on: !sound.on,
    })
}

/// The page's rows, under its title.
pub(crate) fn page(s: &State, b: &mut Builder<'_>) {
    let a = &s.ambience;
    b.note(key::PLAYER_ABOUT_AMBIENCE);
    // NONE ON THIS MACHINE: why, as the player says it — never an empty page.
    if a.sounds.is_empty() {
        let t = a.trouble.clone().unwrap_or(Trouble::NoAmbience);
        b.warning(&trouble_sentence(&t, b.texts));
        return;
    }
    // THE LONGEST WORD A ROW CAN SAY after its bar — its volume, off, held
    // — which every bar leaves room for: one page, one length of bar.
    let widest = [
        format!("{VOLUME_MOST}%"),
        crate::player_rows::on_off(false, b.texts),
        b.t(key::PLAYER_AMBIENCE_HELD),
    ]
    .iter()
    .map(|w| ronoctua_render::layout::width(w))
    .max()
    .unwrap_or(0);
    // BY GROUP, each under its heading (`player-ambience-group-<group>`), the
    // groups in the order the player lists their first sound — and each
    // sound keeps its place in the player's list, which its bar is read by.
    let mut groups: Vec<&str> = Vec::new();
    for sound in &a.sounds {
        if !groups.contains(&sound.group.as_str()) {
            groups.push(&sound.group);
        }
    }
    for (n, sound) in groups.iter().flat_map(|g| {
        a.sounds
            .iter()
            .enumerate()
            .filter(move |(_, x)| x.group == *g)
    }) {
        let first_of_its_group = a.sounds.iter().position(|x| x.group == sound.group) == Some(n);
        if first_of_its_group {
            b.heading_within(crate::player::ambience_group(&sound.group, b.texts));
        }
        let text = state_of(sound, a.paused, b.texts);
        b.marked(glyph_of(&sound.group), |b| {
            b.slider_pair_of(
                (tokens::space::PLAYER_SOUNDBAR, widest),
                crate::player::ambience_name(&sound.id, b.texts),
                share(f64::from(sound.volume_percent), f64::from(VOLUME_MOST)),
                &text,
                (
                    Target::Sound(sound.id.clone()),
                    &format!("sound-{}", sound.id),
                ),
                Slid::Ambience(n),
            );
        });
    }
    if a.sounds.iter().any(|x| x.on) {
        b.inert(crate::chrome::AIR);
        b.pair(
            b.t(key::PLAYER_AMBIENCE_HOLD),
            crate::player_rows::on_off(a.paused, b.texts),
            Target::Hold,
            "ambience-hold",
        );
        b.pair(
            b.t(key::PLAYER_AMBIENCE_SILENCE),
            String::new(),
            Target::Silence,
            "ambience-silence",
        );
    }
    b.inert(crate::chrome::AIR);
    let credits = PageId::AmbienceCredits;
    b.pair(
        b.t(credits.key()),
        String::new(),
        Target::Go(credits),
        credits.spot(),
    );
}

/// The credits' rows, under their title: for every sound, under its name,
/// who recorded it, under what licence, and where it was published — each a
/// sentence, wrapped and never cut: an author or an address cut to the width
/// of a column is no credit.
pub(crate) fn credits(s: &State, b: &mut Builder<'_>) {
    b.note(key::PLAYER_ABOUT_CREDITS);
    // FACTS, in a fact's tone.
    let (_, tone) = crate::chrome::Kind::of(false).tones(false);
    for sound in &s.ambience.sounds {
        b.heading_said(crate::player::ambience_name(&sound.id, b.texts));
        let page = sound.credit.page.as_str();
        let shown = page
            .strip_prefix("https://")
            .or_else(|| page.strip_prefix("http://"))
            .unwrap_or(page);
        let said = [
            b.texts
                .t_with(key::PLAYER_CREDIT_BY, &[("author", &sound.credit.author)]),
            b.t(match sound.credit.licence {
                Licence::Cc0 => key::PLAYER_LICENCE_CC0,
                Licence::PublicDomain => key::PLAYER_LICENCE_PUBLIC_DOMAIN,
            }),
            b.texts.t_with(key::PLAYER_CREDIT_PAGE, &[("page", shown)]),
        ];
        for sentence in said {
            for line in crate::chrome::note_lines(&sentence, b.room) {
                b.inert(vec![Builder::piece(b.left, line, tone)]);
            }
        }
    }
}

/// What the home's row of the ambient sounds says: nothing while none is on,
/// the one on by its name, how many — and that they are held.
#[must_use]
pub fn summary(s: &State, texts: &ronoctua_i18n::Texts) -> String {
    let on: Vec<&Sounding> = s.ambience.sounds.iter().filter(|x| x.on).collect();
    if on.is_empty() {
        return String::new();
    }
    if s.ambience.paused {
        return texts.t(key::PLAYER_AMBIENCE_HELD);
    }
    match on.as_slice() {
        [one] => crate::player::ambience_name(&one.id, texts),
        more => texts.t_args(
            key::PLAYER_AMBIENCE_COUNT,
            &[(
                "count",
                ronoctua_i18n::Arg::Number(i64::try_from(more.len()).unwrap_or(i64::MAX)),
            )],
        ),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::player::fixture::idle;
    use crate::player::{Press, View};
    use ronoctua_player::wire::{Ambience, Ask, Credit};

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB")
    }

    /// A sound of this machine, on or off.
    pub(crate) fn sounding(id: &str, group: &str, on: bool, volume_percent: u8) -> Sounding {
        Sounding {
            id: id.into(),
            group: group.into(),
            on,
            volume_percent,
            rounds: 0,
            credit: Credit {
                author: "Wildhorsemann".into(),
                page: format!("https://freesound.org/people/Wildhorsemann/sounds/{id}/"),
                licence: Licence::Cc0,
            },
        }
    }

    /// Nothing of the queue loaded, and the three sounds of the catalogue —
    /// the rain on at 60%.
    pub(crate) fn rain() -> State {
        State {
            ambience: Ambience {
                sounds: vec![
                    sounding("small-waterfall", "water", false, 100),
                    sounding("rain-and-thunder", "storm", true, 60),
                    sounding("thunderstorm", "storm", false, 40),
                ],
                at_once: 2,
                paused: false,
                trouble: None,
            },
            ..idle()
        }
    }

    fn on_the_page(s: &State) -> (View, Vec<crate::player_rows::Row>) {
        let t = texts();
        let mut v = View::blank(None);
        v.key(Press::Go(PageId::Ambience), Some(s), 80, &t);
        let rows = v.rows(Some(s), 80, &t);
        (v, rows)
    }

    fn row_of(rows: &[crate::player_rows::Row], target: &Target) -> usize {
        rows.iter()
            .position(|r| &r.target == target)
            .unwrap_or_else(|| panic!("no row for {target:?}"))
    }

    /// FAILS IF: a sound stops being its row — its group's glyph first, its
    /// name, its own volume as a bar a press sets, and whether it is on — or
    /// `RET` stops turning it on or off, the arrows its volume a step, or a
    /// press on its bar that volume there.
    #[test]
    fn every_sound_is_a_row_with_its_volume() {
        let t = texts();
        let s = rain();
        let (mut v, rows) = on_the_page(&s);
        let at = row_of(&rows, &Target::Sound("rain-and-thunder".into()));
        let r = &rows[at];
        assert_eq!(
            r.fields.first().map(|f| f.text.clone()),
            Some(tokens::icon::AMBIENCE_STORM.to_string())
        );
        assert_eq!(
            r.fields.get(1).map(|f| f.text.as_str()),
            Some("Rain and thunder")
        );
        let value = r.fields.get(2).map(|f| f.text.clone()).unwrap_or_default();
        assert!(value.ends_with(" 60%"), "{value}");
        let bar = r.slider.expect("the volume is a bar");
        assert_eq!(bar.of, Slid::Ambience(1));
        assert_eq!(
            crate::player_rows::slid(bar.of, 0.5, Some(&s)),
            Errand::Tell(Ask::AmbientVolume {
                id: "rain-and-thunder".into(),
                percent: 65
            })
        );
        v.clicked(at, &rows);
        assert_eq!(
            v.key(Press::Enter, Some(&s), 80, &t),
            Errand::Tell(Ask::Ambient {
                id: "rain-and-thunder".into(),
                on: false
            })
        );
        assert_eq!(
            v.key(Press::More, Some(&s), 80, &t),
            Errand::Tell(Ask::AmbientVolume {
                id: "rain-and-thunder".into(),
                percent: 65
            })
        );
        assert_eq!(
            v.key(Press::Less, Some(&s), 80, &t),
            Errand::Tell(Ask::AmbientVolume {
                id: "rain-and-thunder".into(),
                percent: 55
            })
        );
        let water = &rows[row_of(&rows, &Target::Sound("small-waterfall".into()))];
        assert_eq!(
            water.fields.first().map(|f| f.text.clone()),
            Some(tokens::icon::AMBIENCE_WATER.to_string())
        );
    }

    /// FAILS IF: the sounds stop being listed under their groups' headings,
    /// in either language, or the bars of one page are of different lengths
    /// — sized by each row's own word, `desligado` took a cell from its bar —
    /// or the groups' headings split the page into columns and a narrow door
    /// loses a bar altogether.
    #[test]
    fn sounds_are_grouped_and_their_bars_are_one_length() {
        for (language, columns) in [
            ("en-GB", 80),
            ("en-GB", 32),
            ("pt-BR", 36),
            ("pt-BR", 34),
            ("pt-BR", 32),
            ("pt-BR", 30),
            ("pt-BR", 26),
        ] {
            let t = ronoctua_i18n::Texts::fresh(language).expect("a language");
            // BOTH STORMS ON: their group says only numbers, and the water's
            // `off` is the longest word on the page.
            let mut s = rain();
            s.ambience.sounds[2].on = true;
            let mut v = View::blank(None);
            v.key(Press::Go(PageId::Ambience), Some(&s), columns, &t);
            let rows = v.rows(Some(&s), columns, &t);
            let said: Vec<String> = rows
                .iter()
                .filter(|r| {
                    r.heading
                        || r.fields
                            .iter()
                            .all(|f| f.color == tokens::color::TEXT_HEADING)
                })
                .filter_map(|r| r.fields.first().map(|f| f.text.clone()))
                .collect();
            for group in ["water", "storm"] {
                let heading = crate::player::ambience_group(group, &t);
                assert!(
                    said.contains(&heading),
                    "{language}: no heading {heading:?} in {said:?}"
                );
            }
            let bars: Vec<u16> = ["small-waterfall", "rain-and-thunder", "thunderstorm"]
                .iter()
                .map(|id| {
                    rows[row_of(&rows, &Target::Sound((*id).into()))]
                        .slider
                        .map_or(0, |b| b.cells)
                })
                .collect();
            assert!(
                bars[0] > 0,
                "{language} at {columns}: no bar at all: {bars:?}"
            );
            assert!(
                bars.windows(2).all(|w| w[0] == w[1]),
                "{language} at {columns}: bars of one page differ: {bars:?}"
            );
        }
    }

    /// FAILS IF: a sound past the machine's `at_once` is sent instead of
    /// refused in the player's own sentence — or turning one OFF at the
    /// ceiling is refused too.
    #[test]
    fn a_sound_past_the_ceiling_is_refused_before_it_is_sent() {
        let t = texts();
        let mut s = rain();
        s.ambience.sounds[0].on = true;
        assert!(matches!(
            turned("thunderstorm", &s, &t),
            Errand::Said(said) if said == trouble_sentence(
                &Trouble::TooMany(TooMany { what: Limit::Ambience, most: 2 }),
                &t,
            )
        ));
        assert_eq!(
            turned("small-waterfall", &s, &t),
            Errand::Tell(Ask::Ambient {
                id: "small-waterfall".into(),
                on: false
            })
        );
    }

    /// FAILS IF: while a sound is on, the page stops offering to hold them
    /// all and to silence them all — or offers either with none on — or the
    /// credits stop being a row of it.
    #[test]
    fn hold_and_silence_while_one_is_on_and_the_credits_always() {
        let t = texts();
        let s = rain();
        let (mut v, rows) = on_the_page(&s);
        let hold = row_of(&rows, &Target::Hold);
        v.clicked(hold, &rows);
        v.clicked(hold, &rows);
        assert_eq!(
            v.key(Press::Enter, Some(&s), 80, &t),
            Errand::Tell(Ask::AmbientToggle)
        );
        let silence = row_of(&rows, &Target::Silence);
        v.clicked(silence, &rows);
        v.clicked(silence, &rows);
        assert_eq!(
            v.key(Press::Enter, Some(&s), 80, &t),
            Errand::TellSaying(Ask::AmbientSilence, t.t(key::PLAYER_AMBIENCE_SILENCED))
        );
        row_of(&rows, &Target::Go(PageId::AmbienceCredits));
        let mut off = rain();
        off.ambience.sounds[1].on = false;
        let (_, rows) = on_the_page(&off);
        assert!(
            !rows
                .iter()
                .any(|r| matches!(r.target, Target::Hold | Target::Silence))
        );
        row_of(&rows, &Target::Go(PageId::AmbienceCredits));
    }

    /// FAILS IF: the credits stop saying who recorded each sound, under what
    /// licence in words, and where — the address whole, never cut, in every
    /// width.
    #[test]
    fn the_credits_say_who_where_and_under_what_licence() {
        let t = texts();
        let s = rain();
        for columns in [32, 80] {
            let mut v = View::blank(None);
            v.key(Press::Go(PageId::Ambience), Some(&s), columns, &t);
            v.key(Press::Go(PageId::AmbienceCredits), Some(&s), columns, &t);
            // EVERY LINE WHOLE, read without the spaces a wrap takes away.
            let said: String = v
                .rows(Some(&s), columns, &t)
                .iter()
                .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
                .collect::<String>()
                .replace(' ', "");
            let whole = |text: &str| text.replace(' ', "");
            assert!(!said.contains('…'), "{columns}: something is cut: {said}");
            for sentence in [
                t.t_with(key::PLAYER_CREDIT_BY, &[("author", "Wildhorsemann")]),
                t.t(key::PLAYER_LICENCE_CC0),
                t.t_with(
                    key::PLAYER_CREDIT_PAGE,
                    &[(
                        "page",
                        "freesound.org/people/Wildhorsemann/sounds/thunderstorm/",
                    )],
                ),
                "Small waterfall".to_owned(),
            ] {
                assert!(
                    said.contains(&whole(&sentence)),
                    "{columns}: {sentence}: {said}"
                );
            }
        }
    }

    /// FAILS IF: a machine with no ambient sounds shows an empty page instead
    /// of saying why, or a sound this door has no name for is drawn by its
    /// raw id.
    #[test]
    fn none_here_says_why_and_a_nameless_sound_is_said_as_one() {
        let t = texts();
        let (_, rows) = on_the_page(&idle());
        let said: String = rows
            .iter()
            .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
            .collect::<Vec<_>>()
            .join(" ");
        let why = t.t(key::PLAYER_TROUBLE_NO_AMBIENCE);
        assert!(
            said.contains(why.split_whitespace().next().unwrap_or_default()),
            "{said}"
        );
        let mut s = rain();
        s.ambience.sounds.push(sounding("wind", "air", false, 100));
        let (_, rows) = on_the_page(&s);
        let r = &rows[row_of(&rows, &Target::Sound("wind".into()))];
        assert_eq!(
            r.fields.first().map(|f| f.text.clone()),
            Some(tokens::icon::AMBIENCE_SOUND.to_string())
        );
        assert_eq!(
            r.fields.get(1).map(|f| f.text.clone()),
            Some(t.t_with(key::PLAYER_AMBIENCE_UNNAMED, &[("id", "wind")]))
        );
    }

    /// FAILS IF: the home's row of the ambient sounds stops saying what is on
    /// — nothing, the one by its name, how many — or that they are held.
    #[test]
    fn the_home_says_what_sounds() {
        let t = texts();
        let mut s = rain();
        assert_eq!(summary(&s, &t), "Rain and thunder");
        s.ambience.sounds[2].on = true;
        assert_eq!(summary(&s, &t), "2 sounds");
        s.ambience.paused = true;
        assert_eq!(summary(&s, &t), t.t(key::PLAYER_AMBIENCE_HELD));
        assert_eq!(summary(&idle(), &t), "");
    }
}
