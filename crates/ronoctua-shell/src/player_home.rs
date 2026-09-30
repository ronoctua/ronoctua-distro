//! The player's HOME: the bottom of the page stack (`crate::player_page`),
//! and the first thing every door shows.
//!
//! # State lines, and nothing that explains
//!
//! The home is what is TRUE now — what plays, where, how loud, what comes
//! next — and one row for each page that does more. What a page is for is
//! said on the page: a sentence on the home is a sentence read a hundred
//! times by a person who needed it once (`home_has_no_about_text`).
//!
//! In order:
//!
//! - **Now** — troubles, what plays, its state and position, the volume, the
//!   speed, repeat and shuffle; in the footer the swap's glyph on its title
//!   while no media panel carries it.
//! - **Media in the file** — only when the file offers a choice, and a row
//!   for each kind there is one of: its sounds, pictures and subtitles, each
//!   a page (`crate::player_tracks`).
//! - **Lyrics** — only when there are some, and a row saying where they came
//!   from.
//! - **Queue** — the outline the door keeps, as a tree whose groups fold
//!   (`crate::player_queue`), and the glyph that empties it on its title.
//! - **Listen and watch** — the files, the world radio, the world TV and the
//!   channels by theme (`crate::player_directory`), and the ambient sounds
//!   with what sounds now (`crate::player_ambience`).
//! - **More features** — playing together, the timeline, how the pictures
//!   share a screen (`crate::player_layout`) and what plays on the whole of
//!   every screen, subtitles and lyrics from the internet
//!   (`crate::player_find`), saving what plays as a set and the queue as a file,
//!   opening one, renaming what plays, the internet, the other machines, and
//!   closing the player everywhere.
//! - **Metadata** — last: which file of what plays, the suggestions
//!   (`crate::player_suggest`), the
//!   cover, every common field blanks included, the technical facts, and the
//!   rows of the streams, the chapters and every tag
//!   (`crate::player_metadata`).

use crate::player::{
    LYRICS_AROUND, OF_THE_VIEW, PageId, Purpose, Target, TrackKind, VOLUME_MOST, View, clock,
    loaded, node_of, now_playing, older, trouble_sentence,
};
use crate::player_queue::{lane_glyph, lane_name};
use crate::player_rows::{Builder, Slid, on_off, share, trim_float};
use ronoctua_i18n::key;
use ronoctua_player::queue::{NodeKind, Outline};
use ronoctua_player::wire::{LaneStatus, Repeat, State};
use ronoctua_tokens as tokens;

impl View {
    /// The home's rows, in reading order.
    pub(crate) fn home(&self, state: Option<&State>, b: &mut Builder<'_>) {
        self.now_heading(b);
        let Some(s) = state else {
            b.note(key::PLAYER_NOT_RUNNING);
            listen_and_watch(b, None);
            various(b, None, None);
            return;
        };
        // AN OLDER PLAYER, which may not understand a word this door says:
        // said first, with the row that starts it again.
        if older(s) {
            b.warning(&b.t(key::PLAYER_OLD_DAEMON));
            b.pair(
                b.t(key::PLAYER_RESTART),
                String::new(),
                Target::Restart,
                "restart",
            );
        }
        // NO ROOM FOR THE PICTURE, in the footer: said first under "Now",
        // with the key that shows it alone.
        if self.no_room && loaded(s) {
            no_room(b);
        }
        let outline = self.outline.as_deref();
        now(s, outline, self.deck_in_now, b);
        media_in_the_file(s, b);
        lyrics(s, b);
        crate::player_queue::queue(self, s, outline, b);
        listen_and_watch(b, Some(s));
        various(b, Some(s), outline);
        // THE METADATA, LAST (plan P7): what the file shown says of itself.
        crate::player_metadata::section(self, s, b);
    }

    /// The "Now" title — with the swap's glyph beside it in the footer while
    /// no media panel stands beside the rows to carry it in its transport.
    fn now_heading(&self, b: &mut Builder<'_>) {
        if self.swap && !self.panel {
            b.heading_with(
                key::PLAYER_HEADING_NOW,
                tokens::icon::PLAYER_WINDOW,
                Target::Swap,
            );
        } else {
            b.heading(key::PLAYER_HEADING_NOW);
        }
    }
}

/// A row that opens `page`: its label is the page's own key, the one it is
/// titled by.
fn opens(b: &mut Builder<'_>, page: PageId, value: String) {
    b.pair(b.t(page.key()), value, Target::Go(page), page.spot());
}

/// NOW: what plays, and how — and, in the footer, a SET's lanes on one
/// clock where its position would be: the deck as rows (`Builder::deck`), the
/// footer having no block under a transport to draw it in.
fn now(s: &State, outline: Option<&Outline>, deck: bool, b: &mut Builder<'_>) {
    for t in &s.troubles {
        b.warning(&trouble_sentence(t, b.texts));
    }
    if !loaded(s) {
        b.note(key::PLAYER_NOTHING);
    } else {
        if let Some(name) = now_playing(s, outline, b.texts) {
            b.pair(b.t(key::PLAYER_PLAYING), name, Target::Nothing, "title");
        }
        b.pair(
            b.t(key::PLAYER_STATE),
            b.t(if s.playing {
                key::PLAYER_STATE_PLAYING
            } else {
                key::PLAYER_STATE_PAUSED
            }),
            Target::Playing,
            "playing",
        );
        // A SET'S LANES, in the footer: the ruler is the position, and a
        // row a lane.
        let set = crate::player_timeline::set_lanes(s, outline);
        // THE POSITION IS A BAR when the item has a length: a press seeks
        // there, as on the window's time bar. A stream has none, and says
        // only where it is.
        match s.duration.filter(|d| *d > 0.0) {
            _ if deck && !set.is_empty() => b.deck(set, s),
            Some(d) => b.slider_pair(
                b.t(key::PLAYER_POSITION),
                share(s.position, d),
                &format!("{} / {}", clock(s.position), clock(d)),
                Target::Position,
                "position",
                Slid::Position,
            ),
            None => b.pair(
                b.t(key::PLAYER_POSITION),
                clock(s.position),
                Target::Position,
                "position",
            ),
        }
    }
    // THE VOLUME IS A BAR up to the loudest the ring goes, the window's
    // track's length.
    b.slider_pair(
        b.t(key::PLAYER_VOLUME),
        share(f64::from(s.volume), f64::from(VOLUME_MOST)),
        &if s.muted {
            b.t(key::PLAYER_MUTED)
        } else {
            format!("{}%", s.volume)
        },
        Target::Volume,
        "volume",
        Slid::Volume,
    );
    b.pair(
        b.t(key::PLAYER_SPEED),
        format!("{}×", trim_float(s.speed)),
        Target::Speed,
        "speed",
    );
    b.pair(
        b.t(key::PLAYER_REPEAT),
        b.t(match s.repeat {
            Repeat::Nothing => key::PLAYER_REPEAT_NOTHING,
            Repeat::Queue => key::PLAYER_REPEAT_QUEUE,
            Repeat::One => key::PLAYER_REPEAT_ONE,
        }),
        Target::Repeat,
        "repeat",
    );
    b.pair(
        b.t(key::PLAYER_SHUFFLE),
        on_off(s.shuffle, b.texts),
        Target::Shuffle,
        "shuffle",
    );
}

/// The footer's refusal: the region is too narrow for the picture beside the
/// rows, and `o` — read off the view's own table — shows it alone.
fn no_room(b: &mut Builder<'_>) {
    let keys = OF_THE_VIEW
        .iter()
        .find(|(_, id, _)| *id == "player-only")
        .map_or("", |(k, _, _)| *k);
    let said = b.texts.t_with(key::PLAYER_PANEL_NO_ROOM, &[("keys", keys)]);
    for line in crate::chrome::note_lines(&said, b.room) {
        b.inert(vec![Builder::piece(b.left, line, tokens::color::TEXT_DIM)]);
    }
}

/// MEDIA IN THE FILE: a row for each kind the file carries more than one of —
/// two sounds, two pictures, a subtitle — each the page of its kind, its
/// value the one in use; and the whole section only when there is one.
fn media_in_the_file(s: &State, b: &mut Builder<'_>) {
    let kinds: Vec<TrackKind> = TrackKind::with_a_choice(s).collect();
    if kinds.is_empty() {
        return;
    }
    // A SET OF SEVERAL FILES is no one file: its title says the files.
    let mut sources: Vec<&str> = s.lanes.iter().map(|l| l.source.as_str()).collect();
    sources.sort_unstable();
    sources.dedup();
    b.heading(if sources.len() > 1 {
        key::PLAYER_HEADING_MEDIA_SET
    } else {
        key::PLAYER_HEADING_MEDIA
    });
    for kind in kinds {
        // THE SUBTITLES are lanes over the picture, several at once: what is
        // shown there, and not one track of the file.
        let now = if kind == TrackKind::Subtitles {
            crate::player_subtitles::summary(s, b.texts)
        } else {
            crate::player_tracks::in_use_of(kind, s, b.texts)
        };
        opens(b, PageId::Tracks(kind), now);
    }
}

/// LYRICS: the line being sung in the accent, two on each side, and a row
/// saying where they came from.
fn lyrics(s: &State, b: &mut Builder<'_>) {
    if s.lyrics.is_empty() {
        return;
    }
    b.heading(key::PLAYER_HEADING_LYRICS);
    let here = ronoctua_player::lyrics::current(&s.lyrics, s.position);
    let synced = s.lyrics.iter().any(|l| l.at.is_some());
    let (from, to) = match (here, synced) {
        (Some(i), true) => (
            i.saturating_sub(LYRICS_AROUND),
            (i + LYRICS_AROUND + 1).min(s.lyrics.len()),
        ),
        _ => (0, (2 * LYRICS_AROUND + 1).min(s.lyrics.len())),
    };
    for (i, l) in s.lyrics.iter().enumerate().take(to).skip(from) {
        let colour = if Some(i) == here && synced {
            tokens::color::STATE_ACCENT
        } else {
            tokens::color::TEXT_DIM
        };
        for line in crate::chrome::note_lines(&l.text, b.room) {
            b.inert(vec![Builder::piece(b.left, line, colour)]);
        }
    }
    // WHERE THEY CAME FROM is a fact about them, a row of its own after air:
    // drawn dim under the lines, it read as the song's last line.
    if let Some(said) = s
        .lyrics_from
        .as_deref()
        .and_then(|from| lyrics_source(from, b.texts))
    {
        b.inert(crate::chrome::AIR);
        b.pair(
            b.t(key::PLAYER_LYRICS_SOURCE),
            said,
            Target::Nothing,
            "lyrics-source",
        );
    }
}

/// Where the lyrics came from, in words — the value of their source's row:
/// the `.lrc` beside the file, the file's own tags, or the host they were
/// asked of. A source the player
/// names and this door does not know is NOT drawn — never shown as one of
/// the three it is not.
fn lyrics_source(from: &str, texts: &ronoctua_i18n::Texts) -> Option<String> {
    match from {
        "file" => Some(texts.t(key::PLAYER_LYRICS_FROM_FILE)),
        "tag" => Some(texts.t(key::PLAYER_LYRICS_FROM_TAG)),
        ronoctua_player::prefs::LRCLIB_HOST => Some(texts.t_with(
            key::PLAYER_LYRICS_FROM_HOST,
            &[("host", ronoctua_player::prefs::LRCLIB_HOST)],
        )),
        _ => None,
    }
}

/// LISTEN AND WATCH: where something to play is found — the same place in
/// every state, so a person always finds it: the files, the world's radio
/// and TV, the channels by theme (`crate::player_directory`) — and the
/// ambient sounds, whose row says what sounds, while a player answers to say
/// which there are.
fn listen_and_watch(b: &mut Builder<'_>, s: Option<&State>) {
    b.heading(key::PLAYER_HEADING_LISTEN);
    opens(b, PageId::Files(Purpose::Queue), String::new());
    for page in [PageId::Radio, PageId::Tv, PageId::Channels] {
        opens(b, page, String::new());
    }
    if let Some(s) = s {
        opens(
            b,
            PageId::Ambience,
            crate::player_ambience::summary(s, b.texts),
        );
    }
}

/// VARIOUS FEATURES: a row for each page that does more — playing together,
/// saving and opening the player's own files, renaming what plays — and
/// closing the player everywhere, in every state: with no daemon behind it,
/// it still closes every door that is up.
fn various(b: &mut Builder<'_>, s: Option<&State>, outline: Option<&Outline>) {
    b.heading(key::PLAYER_HEADING_FEATURES);
    // TOGETHER WITH WHAT PLAYS, which has to be there.
    if let Some(s) =
        s.filter(|s| PageId::Together.subject(Some(s)) == crate::player_page::Subject::Here)
    {
        // HOW MANY LANES PLAY TOGETHER, once something was added to what
        // plays — a set: the page's one state that matters from here. A file
        // alone is its own lanes, and says nothing.
        let set = s
            .current
            .zip(outline)
            .and_then(|(id, o)| node_of(o, id))
            .is_some_and(|n| n.kind == NodeKind::Set);
        let lanes = if set {
            b.texts.t_args(
                key::PLAYER_TOGETHER_LANES,
                &[(
                    "count",
                    ronoctua_i18n::Arg::Number(i64::try_from(s.lanes.len()).unwrap_or(i64::MAX)),
                )],
            )
        } else {
            String::new()
        };
        opens(b, PageId::Together, lanes);
        // AND THE SAME LANES ON ONE CLOCK: when each begins, dragged or
        // typed.
        opens(b, PageId::Timeline, String::new());
        // HOW ITS PICTURES SHARE A SCREEN — the form, said — and what plays
        // on the whole of every screen it goes to: both while it has a
        // picture to arrange.
        if !crate::player_layout::candidates(s).is_empty() {
            let form = crate::player_layout::form_of(s);
            opens(b, PageId::Layout, b.t(crate::player_layout::form_key(form)));
            b.pair(
                b.t(key::PLAYER_FEATURE_EVERY_SCREEN),
                String::new(),
                Target::EveryScreen,
                "every-screen",
            );
        }
    }
    // FROM THE INTERNET, FOR WHAT PLAYS: subtitles for a film, and lyrics —
    // each a page that asks only when a row naming its hosts is pressed.
    if let Some(s) = s.filter(|s| loaded(s)) {
        if crate::player::has_a_picture(s) {
            opens(b, PageId::FindSubtitles, String::new());
        }
        opens(b, PageId::FindLyrics, String::new());
    }
    // THE PLAYER'S OWN FILES: what plays saved as a set, the queue saved,
    // and either opened — each while it has something to act on; opening,
    // always, since a file can be opened with nothing playing.
    if s.is_some_and(loaded) {
        b.pair(
            b.t(key::PLAYER_FEATURE_SAVE_SET),
            String::new(),
            Target::SaveSet,
            "save-set",
        );
    }
    if outline.is_some_and(|o| !o.entries.is_empty()) {
        b.pair(
            b.t(key::PLAYER_FEATURE_SAVE_QUEUE),
            String::new(),
            Target::SaveQueue,
            "save-queue",
        );
    }
    opens(b, PageId::Files(Purpose::Open), String::new());
    if s.is_some_and(|s| s.current.is_some()) {
        b.pair(
            b.t(key::PLAYER_FEATURE_RENAME),
            String::new(),
            Target::Rename,
            "rename",
        );
    }
    // THE SWITCHES ARE THE MACHINE'S, there with no player running.
    opens(b, PageId::Internet, String::new());
    if let Some(s) = s {
        // THE MACHINE JOINED, when one is: the one state the page holds that
        // matters from here.
        let joined = s
            .joined
            .as_ref()
            .map(|j| j.host.clone())
            .unwrap_or_default();
        opens(b, PageId::Machines, joined);
    }
    b.pair(
        b.t(key::PLAYER_QUIT_EVERYWHERE),
        String::new(),
        Target::Quit,
        "quit",
    );
}

/// The lanes of what plays — each its kind's glyph and its name, and what it
/// is doing: how far from the clock it is while it plays — and the row that
/// picks something else to play together, under the page's title.
pub(crate) fn together(s: &State, b: &mut Builder<'_>) {
    b.note(key::PLAYER_ABOUT_TOGETHER);
    for lane in &s.lanes {
        // WHAT KIND IT IS, in front of its name: the picture and the sound of
        // one film are two lanes with one file's name.
        let name = lane_name(lane, &s.lanes, b.texts, b.room);
        let now = s.lane_states.iter().find(|l| l.lane == lane.id);
        let value = match now.map(|l| &l.status) {
            None => b.t(key::PLAYER_LANE_SWITCHED_OFF),
            Some(LaneStatus::Pending) => b.t(key::PLAYER_LANE_PENDING),
            Some(LaneStatus::Ended) => b.t(key::PLAYER_LANE_ENDED),
            Some(LaneStatus::Preparing) => b.t(key::PLAYER_LANE_PREPARING),
            // REFUSED: its sentence, under its row — a value is one word.
            Some(LaneStatus::Refused(_)) => b.t(key::PLAYER_LANE_REFUSED),
            Some(LaneStatus::Playing) => b.texts.t_args(
                key::PLAYER_LANE_APART,
                &[(
                    "ms",
                    ronoctua_i18n::Arg::Number(now.map_or(0, |l| l.drift_ms)),
                )],
            ),
        };
        b.pair_with_mark(
            lane_glyph(lane.kind),
            name,
            value,
            Target::Lane(lane.id),
            &format!("lane-{}", lane.id.0),
        );
        if let Some(LaneStatus::Refused(t)) = now.map(|l| &l.status) {
            b.warning(&trouble_sentence(t, b.texts));
        }
    }
    // THE SAME PICKER as the files, for another purpose: the file chosen
    // plays together with what plays, and a path or an address can be typed
    // there too.
    opens(b, PageId::Files(Purpose::Lane), String::new());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::fixture::{alone, idle, track};
    use crate::player::{Errand, Press};
    use ronoctua_player::wire::{LyricLine, TrackKind as Kind};

    fn playing() -> State {
        State {
            position: 75.0,
            duration: Some(3725.0),
            volume: 80,
            album: Some("An album".into()),
            tailnet: Some("here".into()),
            lyrics: vec![LyricLine {
                at: Some(70.0),
                text: "a line".into(),
            }],
            lyrics_from: Some("file".into()),
            ..alone("/music/one.flac", vec![track(1, Kind::Audio, None, true)])
        }
    }

    /// Every word on the rows, joined.
    fn said(rows: &[crate::player_rows::Row]) -> String {
        rows.iter()
            .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// FAILS IF: a sentence that EXPLAINS — what a page is for, why a
    /// machine cannot pair, what playing in step is — is drawn on the home,
    /// in either language, in any state: the home says what is true, and
    /// each page says what it is for.
    #[test]
    fn home_has_no_about_text() {
        let about = [
            key::PLAYER_ABOUT_INTERNET,
            key::PLAYER_ABOUT_SUGGESTIONS,
            key::PLAYER_ABOUT_FIND_SUBTITLES,
            key::PLAYER_LYRICS_ONE_SOURCE,
            key::PLAYER_ABOUT_MACHINES,
            key::PLAYER_ABOUT_TOGETHER,
            key::PLAYER_NO_TAILNET,
        ];
        let mut off = playing();
        off.tailnet = None;
        let states = [None, Some(playing()), Some(off), Some(idle())];
        // THE FOOTER TOO WHERE ITS REGION HAS NO ROOM FOR THE PICTURE: the
        // one sentence the home may carry is that refusal — a refusal is a
        // sentence, never an absence, and it is what is true of this door
        // now, not what a page is for.
        for language in ["en-GB", "pt-BR"] {
            let t = ronoctua_i18n::Texts::fresh(language).expect("a language");
            let refusal = t.t_with(key::PLAYER_PANEL_NO_ROOM, &[("keys", "o")]);
            for s in &states {
                let mut refused = View::blank(None).in_the_footer();
                refused.picture_refused(true);
                for door in [
                    View::blank(None),
                    View::blank(None).in_the_footer(),
                    refused,
                ] {
                    let home = said(&door.rows(s.as_ref(), 80, &t));
                    for k in about {
                        let sentence = t.t(k);
                        assert!(
                            !home.contains(&sentence),
                            "{language}: the home says `{k}`:\n{home}"
                        );
                    }
                    // AND THE ONLY DIM LINES ARE STATE: lyrics, the four
                    // states a line says — the file's sheet being read among
                    // them — and the footer's refusal.
                    let states_said = [
                        t.t(key::PLAYER_NOT_RUNNING),
                        t.t(key::PLAYER_NOTHING),
                        t.t(key::PLAYER_QUEUE_EMPTY),
                        t.t(key::PLAYER_META_READING),
                    ];
                    for r in door.rows(s.as_ref(), 80, &t) {
                        for f in r
                            .fields
                            .iter()
                            .filter(|f| f.color == tokens::color::TEXT_DIM)
                        {
                            let lyric = s.as_ref().is_some_and(|s| {
                                s.lyrics.iter().any(|l| l.text.contains(f.text.trim()))
                            });
                            let state = states_said.iter().any(|w| w.contains(f.text.trim()));
                            let numbered = f.text.trim_end_matches('.').parse::<usize>().is_ok();
                            let refused = refusal.contains(f.text.trim());
                            assert!(
                                lyric || state || numbered || refused,
                                "{language}: a dim line on the home that is no state: {:?}",
                                f.text
                            );
                        }
                    }
                }
            }
        }
    }

    /// FAILS IF: the home's sections leave their order, a section with
    /// nothing to show is drawn, or the album — a fact of the metadata — is
    /// back among what plays.
    #[test]
    fn the_home_is_its_sections_in_order() {
        let t = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let v = View::blank(None);
        let headings = |s: Option<&State>| -> Vec<String> {
            v.rows(s, 80, &t)
                .iter()
                .filter(|r| {
                    r.heading
                        && r.fields
                            .first()
                            .is_some_and(|f| f.color == tokens::color::TEXT_HEADING)
                })
                .filter_map(|r| r.fields.first().map(|f| f.text.clone()))
                .collect()
        };
        let mut s = alone(
            "/music/one.flac",
            vec![
                track(1, Kind::Audio, None, true),
                track(1, Kind::Sub, Some("en"), false),
            ],
        );
        s.lyrics = playing().lyrics;
        s.album = playing().album;
        let named = |ks: &[&str]| ks.iter().map(|k| t.t(k)).collect::<Vec<_>>();
        assert_eq!(
            headings(Some(&s)),
            named(&[
                key::PLAYER_HEADING_NOW,
                key::PLAYER_HEADING_MEDIA,
                key::PLAYER_HEADING_LYRICS,
                key::PLAYER_HEADING_QUEUE,
                key::PLAYER_HEADING_LISTEN,
                key::PLAYER_HEADING_FEATURES,
                key::PLAYER_HEADING_METADATA,
            ])
        );
        // NO CHOICE IN THE FILE, NO LYRICS: neither section.
        s = playing();
        s.lyrics.clear();
        assert_eq!(
            headings(Some(&s)),
            named(&[
                key::PLAYER_HEADING_NOW,
                key::PLAYER_HEADING_QUEUE,
                key::PLAYER_HEADING_LISTEN,
                key::PLAYER_HEADING_FEATURES,
                key::PLAYER_HEADING_METADATA,
            ])
        );
        assert!(!said(&v.rows(Some(&s), 80, &t)).contains("An album"));
        // NO PLAYER: what plays is not there, the rest that works without
        // one is.
        assert_eq!(
            headings(None),
            named(&[
                key::PLAYER_HEADING_NOW,
                key::PLAYER_HEADING_LISTEN,
                key::PLAYER_HEADING_FEATURES,
            ])
        );
    }

    /// FAILS IF: the footer's home stops saying why there is no picture
    /// beside the rows when the region is too narrow for one — and which key
    /// shows it alone — or says it with nothing loaded, or in a door that
    /// was not told so.
    #[test]
    fn the_footer_says_why_the_picture_has_no_room() {
        let t = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let refusal = t.t_with(key::PLAYER_PANEL_NO_ROOM, &[("keys", "o")]);
        let mut v = View::blank(None).in_the_footer();
        assert!(!said(&v.rows(Some(&playing()), 200, &t)).contains(&refusal));
        assert!(v.picture_refused(true), "the change is not reported");
        assert!(!v.picture_refused(true), "no change reported as one");
        let home = said(&v.rows(Some(&playing()), 200, &t));
        assert!(home.contains(&refusal), "{home}");
        assert!(
            !said(&v.rows(Some(&idle()), 200, &t)).contains(&refusal),
            "said with nothing loaded"
        );
    }

    /// FAILS IF: where the lyrics came from is drawn as the player's raw
    /// word (`file`, `tag`) instead of a sentence in the language, or a
    /// source nobody named is drawn as one of the three.
    #[test]
    fn every_lyrics_source_is_said_in_words() {
        for language in ["en-GB", "pt-BR"] {
            let t = ronoctua_i18n::Texts::fresh(language).expect("a language");
            let file = lyrics_source("file", &t).expect("the .lrc beside it");
            let tag = lyrics_source("tag", &t).expect("its own tags");
            let online = lyrics_source(ronoctua_player::prefs::LRCLIB_HOST, &t).expect("the host");
            assert_ne!(file, tag);
            assert!(!file.contains("file") || language == "en-GB", "{file}");
            assert!(online.contains(ronoctua_player::prefs::LRCLIB_HOST));
            assert_eq!(lyrics_source("somewhere", &t), None);
        }
    }

    /// FAILS IF: playing together stops being a page of its own — every
    /// lane of what plays, and the picker — or a lane on it stops being taken
    /// off with `d`; or the home's row stops saying how many lanes a set
    /// plays, or says it of a file that plays alone.
    #[test]
    fn playing_together_is_a_page_with_its_lanes_and_the_picker() {
        use ronoctua_player::own::{Lane, LaneId, LaneKind};
        use ronoctua_player::queue::{EntryId, Name, Node, NodeKind, Outline};
        let t = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let mut film = alone(
            "/v/a.mkv",
            vec![
                track(1, Kind::Video, None, true),
                track(1, Kind::Audio, None, true),
            ],
        );
        let mut v = View::blank(None);
        let value_of_the_row = |v: &View, s: &State| {
            v.rows(Some(s), 80, &t)
                .iter()
                .find(|r| r.target == Target::Go(PageId::Together))
                .map(|r| r.fields.get(1).map(|f| f.text.clone()).unwrap_or_default())
                .expect("a row for playing together")
        };
        assert_eq!(value_of_the_row(&v, &film), "", "a file alone says nothing");
        // A SET: the file and a commentary, one entry of the queue.
        let mut commentary = Lane::new(LaneId(4), LaneKind::Audio, "/v/b.flac");
        commentary.title = Some("Commentary".into());
        film.lanes.push(commentary);
        v.outline_arrived(Some(Box::new(Outline {
            revision: 2,
            entries: vec![Node {
                id: EntryId(1),
                name: Name::Together(vec!["a.mkv".into(), "Commentary".into()]),
                kind: NodeKind::Set,
                children: Vec::new(),
                lanes: 3,
            }],
        })));
        assert_eq!(value_of_the_row(&v, &film), "3 lanes");
        v.settle_in(Some(&film), 80, &t);
        let rows = v.rows(Some(&film), 80, &t);
        let at = rows
            .iter()
            .position(|r| r.target == Target::Go(PageId::Together))
            .expect("a row for playing together");
        v.clicked(at, &rows);
        assert_eq!(v.key(Press::Enter, Some(&film), 80, &t), Errand::Nothing);
        let rows = v.rows(Some(&film), 80, &t);
        assert!(
            rows.iter()
                .any(|r| r.target == Target::Go(PageId::Files(Purpose::Lane)))
        );
        let lanes: Vec<&Target> = rows
            .iter()
            .map(|r| &r.target)
            .filter(|t| matches!(t, Target::Lane(_)))
            .collect();
        assert_eq!(lanes.len(), 3, "{lanes:?}");
        // THE PICTURE AND THE SOUND OF ONE FILE read apart: one name, and
        // what kind each is, its glyph, in front of it — and while each plays
        // how far it is from the clock.
        let said_of = |lane: u32| {
            rows.iter()
                .find(|r| r.target == Target::Lane(LaneId(lane)))
                .map(|r| r.fields.iter().map(|f| f.text.clone()).collect::<Vec<_>>())
                .expect("a lane's row")
        };
        assert_ne!(said_of(1), said_of(2));
        // THE MARK, the name, the value — the mark in front of the label.
        let glyph = |kind: LaneKind| crate::player_queue::lane_glyph(kind).to_string();
        assert_eq!(said_of(1)[0], glyph(LaneKind::Video), "{:?}", said_of(1));
        assert_eq!(said_of(2)[0], glyph(LaneKind::Audio), "{:?}", said_of(2));
        assert_eq!(
            said_of(1)[2],
            t.t_args(
                key::PLAYER_LANE_APART,
                &[("ms", ronoctua_i18n::Arg::Number(0))]
            )
        );
        let at = rows
            .iter()
            .position(|r| r.target == Target::Lane(LaneId(4)))
            .expect("the commentary is a row");
        assert_eq!(
            said_of(4)[..2],
            [glyph(LaneKind::Audio), "Commentary".into()]
        );
        // THE LABEL IS SET IN after its mark, and so is what it wraps onto.
        assert!(rows[at].fields[1].at > rows[at].fields[0].at);
        // OFF, it says so, in the lane's own word.
        assert_eq!(said_of(4)[2], t.t(key::PLAYER_LANE_SWITCHED_OFF));
        v.clicked(at, &rows);
        assert_eq!(
            v.key(Press::Remove, Some(&film), 80, &t),
            Errand::Tell(ronoctua_player::wire::Ask::Lane {
                lane: LaneId(4),
                change: ronoctua_player::wire::LaneChange::Remove,
            })
        );
        // AND WHEN NOTHING PLAYS the page stays, and says so.
        let said_now = said(&v.rows(Some(&idle()), 80, &t));
        assert!(said_now.contains(&t.t(key::PLAYER_PAGE_GONE)), "{said_now}");
    }

    /// FAILS IF: the footer's "Now" stops drawing a SET's lanes as the
    /// deck's rows in the position's place — every lane a row carrying its
    /// line — or draws them for a file alone, or a window's "Now" (whose
    /// block under the transport is the deck) draws them twice.
    #[test]
    fn the_footer_s_now_is_a_set_s_deck_and_a_window_s_keeps_its_position() {
        use ronoctua_player::own::{Lane, LaneId, LaneKind};
        let t = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let (set, outline) = crate::player::fixture::set(
            vec![
                Lane::new(LaneId(1), LaneKind::Video, "/films/one.mkv"),
                Lane::new(LaneId(2), LaneKind::Audio, "/films/dub.flac"),
            ],
            &[Some(60.0), Some(50.0)],
            Some(60.0),
            12.0,
        );
        let decked = |v: &View, s: &State| {
            v.rows(Some(s), 80, &t)
                .iter()
                .filter(|r| r.deck.is_some())
                .count()
        };
        let position =
            |v: &View, s: &State| v.rows(Some(s), 80, &t).iter().any(|r| r.spot == "position");
        let mut footer = View::blank(None).in_the_footer();
        footer.outline_arrived(Some(outline.clone()));
        assert_eq!(decked(&footer, &set), 3, "the ruler and a row a lane");
        assert!(!position(&footer, &set), "the ruler is the position");
        let mut window = View::blank(None);
        window.outline_arrived(Some(outline));
        assert_eq!(decked(&window, &set), 0);
        assert!(position(&window, &set));
        // A FILE ALONE: its position, in both.
        let alone = playing();
        let mut footer = View::blank(None).in_the_footer();
        footer.outline_arrived(Some(crate::player::fixture::outline(vec![
            ronoctua_player::queue::Name::Location("/music/one.flac".into()),
        ])));
        assert_eq!(decked(&footer, &alone), 0);
        assert!(position(&footer, &alone));
    }
}
