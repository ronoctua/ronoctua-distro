//! THE SUGGESTIONS for the tags of the file the Metadata section shows (plan
//! P8), and a suggestion's own page: what it would write, field by field,
//! beside what the file says now.
//!
//! # Who is asked
//!
//! A ring of three: MusicBrainz for a song (its covers are the Cover Art
//! Archive's, named with it), Wikidata for a film — keyless, and with a
//! poster only when Commons has one, which the page says — and TMDB, which
//! only the person's token opens: without one it says so and leads to the
//! Internet page, and nothing is asked. The file's kind picks the first.
//! The row that asks names the hosts, and pressing it is the consent
//! (`player_find::consented`).
//!
//! # A suggestion's page
//!
//! One row per field it has: a toggle at its head — on for a field the file
//! has blank, off for one it fills already —, the field, the SUGGESTED value,
//! and under it, in the quiet register, what the file says now; a field the
//! file already says the same is a fact that says `same`, nothing to write.
//! The suggested cover is drawn BESIDE the file's own — two pictures, one
//! geometry (`Builder::pictures`) — fetched by the daemon when the page opens,
//! at a door's size; taking it fetches the tag's. "Write n fields" asks the
//! daemon to take the candidate (`Ask::Take`), and the file's write is said
//! as every write of the Metadata is (`State::retag`).

use crate::player::{Errand, PageId, Target, View, lookup_refusal, service_key};
use crate::player_find::{ImportOf, ask_row, consented, default_tongues, film, hosts_of, status};
use crate::player_rows::Builder;
use ronoctua_i18n::{Arg, key};
use ronoctua_lookup::keys::Key;
use ronoctua_lookup::{Refusal, Service, Why};
use ronoctua_player::find::{
    CandidateId, Desk, Page, Query, SuggestedCover, Suggester, SuggestionCandidate,
};
use ronoctua_player::wire::{Ask, DeskKind, State};
use ronoctua_tags::Field;

impl View {
    /// Who suggests the tags of the file shown: the one the person turned
    /// the ring to, else by the file's kind — a film asks Wikidata, anything
    /// else MusicBrainz.
    #[must_use]
    pub(crate) fn source(&self, s: &State) -> Suggester {
        self.finding
            .source
            .unwrap_or_else(|| match self.meta.sheet(s).map(|sheet| sheet.kind) {
                Some(ronoctua_tags::Kind::Video) => Suggester::Wikidata,
                _ => Suggester::MusicBrainz,
            })
    }

    /// What `kind`'s search asks for what the view shows — the file of the
    /// Metadata, the film of the subtitles — with `words` in place of what
    /// the file says of itself.
    #[must_use]
    pub(crate) fn query_of(
        &self,
        kind: DeskKind,
        words: Option<String>,
        s: &State,
        texts: &ronoctua_i18n::Texts,
    ) -> Option<Query> {
        Some(match kind {
            DeskKind::Suggestions => Query::Suggest {
                location: self.meta.location(s)?,
                source: self.source(s),
                words,
                tongues: default_tongues(texts),
            },
            DeskKind::Subtitles => Query::Subtitles {
                location: film(&self.finding, s)?,
                tongues: self.finding.tongues(texts),
                words,
            },
            DeskKind::Lyrics => Query::Lyrics {
                location: self.meta.location(s)?,
                words,
            },
            DeskKind::Radio
            | DeskKind::Television
            | DeskKind::Channels
            | DeskKind::Archive
            | DeskKind::Lives => return None,
        })
    }

    /// The services a search asks: a subtitle search only those the person
    /// kept a key for — the daemon leaves the others out and says so.
    #[must_use]
    pub(crate) fn services_of(&self, query: &Query) -> Vec<Service> {
        match query {
            Query::Subtitles { .. } => crate::player_find::keyed_subtitle_services(&self.kept),
            other => other.services().to_vec(),
        }
    }

    /// `query` asked, after the press that asks it let its services.
    pub(crate) fn asking(&mut self, query: Query, texts: &ronoctua_i18n::Texts) -> Errand {
        let services = self.services_of(&query);
        let hosts = hosts_of(&services, texts);
        let ask = Errand::TellSaying(
            Ask::Find { query },
            texts.t_with(key::PLAYER_DESK_ASKING, &[("hosts", &hosts)]),
        );
        consented(&mut self.prefs, &services, ask)
    }

    /// Whether `of` of candidate `c` is to be written: as the person turned
    /// it, else as it started — a field the file has blank, and the cover
    /// of a file with none.
    #[must_use]
    pub(crate) fn imported(&self, c: &SuggestionCandidate, of: ImportOf, s: &State) -> bool {
        if let Some(on) = self.finding.imports.get(&(c.id, of)) {
            return *on;
        }
        let sheet = self.meta.sheet(s);
        match of {
            ImportOf::Field(f) => sheet
                .and_then(|sh| sh.value(f))
                .is_none_or(|v| v.trim().is_empty()),
            ImportOf::Cover => sheet.is_none_or(|sh| sh.front_cover().is_none()),
        }
    }

    /// What taking candidate `c` writes: the fields turned on whose value the
    /// file does not already say, and the cover when it is turned on.
    #[must_use]
    pub(crate) fn to_write(&self, c: &SuggestionCandidate, s: &State) -> (Vec<Field>, bool) {
        let sheet = self.meta.sheet(s);
        let fields = c
            .suggestion
            .fields
            .iter()
            .filter(|f| !same(sheet.and_then(|sh| sh.value(f.field)), &f.value))
            .filter(|f| self.imported(c, ImportOf::Field(f.field), s))
            .map(|f| f.field)
            .collect();
        let cover = c.suggestion.cover.is_some() && self.imported(c, ImportOf::Cover, s);
        (fields, cover)
    }
}

/// Whether the file's `now` says what is suggested.
fn same(now: Option<&str>, suggested: &str) -> bool {
    now.map(str::trim)
        .is_some_and(|n| !n.is_empty() && n == suggested.trim())
}

/// Who suggests, by its name — a proper name, kept as a key.
const fn suggester_key(s: Suggester) -> &'static str {
    match s {
        Suggester::MusicBrainz => key::PLAYER_SERVICE_MUSICBRAINZ,
        Suggester::Wikidata => key::PLAYER_SERVICE_WIKIDATA,
        Suggester::Tmdb => key::PLAYER_SERVICE_TMDB,
    }
}

/// The candidate `id` of the suggestions desk, while its page is about
/// `location`.
fn candidate(desk: Option<&Desk>, id: CandidateId) -> Option<&SuggestionCandidate> {
    match &desk?.page {
        Page::Suggestions { candidates, .. } => candidates.iter().find(|c| c.id == id),
        _ => None,
    }
}

/// A suggestion's name on its row: its title and its artist (data), else
/// the service's own id for it.
fn candidate_name(c: &SuggestionCandidate, texts: &ronoctua_i18n::Texts) -> String {
    let words: Vec<&str> = [Field::Title, Field::Artist, Field::Show]
        .into_iter()
        .filter_map(|f| c.suggestion.value(f))
        .map(str::trim)
        .filter(|w| !w.is_empty())
        .collect();
    if words.is_empty() {
        return c.suggestion.id.clone();
    }
    words.join(&texts.t(key::PLAYER_META_JOINER))
}

/// What a suggestion's row says of it: its album (or its season and
/// episode), its date — or that its details are still being read.
fn candidate_said(c: &SuggestionCandidate, texts: &ronoctua_i18n::Texts) -> String {
    if !c.complete {
        return texts.t(key::PLAYER_SUGGEST_READING);
    }
    [
        Field::Album,
        Field::Season,
        Field::Episode,
        Field::Date,
        Field::Director,
    ]
    .into_iter()
    .filter_map(|f| c.suggestion.value(f))
    .map(str::trim)
    .filter(|w| !w.is_empty())
    .collect::<Vec<_>>()
    .join(&texts.t(key::PLAYER_META_JOINER))
}

/// THE SUGGESTIONS' PAGE: what it is for, the file (a ring while several
/// play), who is asked — a ring — the row that asks them and the one that
/// asks with other words, what the daemon is doing or why it found nothing,
/// and the suggestions, each a row that opens its page.
pub(crate) fn page(v: &View, s: &State, b: &mut Builder<'_>) {
    b.note(key::PLAYER_ABOUT_SUGGESTIONS);
    let Some(location) = v.meta.location(s) else {
        return;
    };
    if crate::player_metadata::locations(s).len() > 1 {
        b.pair(
            b.t(key::PLAYER_META_FILE),
            crate::player::name_of(
                &ronoctua_player::queue::Name::Location(location.clone()),
                b.texts,
                crate::player::NAME_IN_A_SENTENCE,
            ),
            Target::MetaFile,
            "meta-file",
        );
    }
    let source = v.source(s);
    b.pair(
        b.t(key::PLAYER_SUGGEST_SOURCE),
        b.t(suggester_key(source)),
        Target::SuggestSource,
        "suggest-source",
    );
    // A POSTER ONLY WITH TMDB — which Wikidata almost never has: said before
    // anybody asks and wonders why no cover came.
    if source == Suggester::Wikidata {
        b.note(key::PLAYER_SUGGEST_POSTER_NEEDS_TOKEN);
    }
    b.heading(key::PLAYER_HEADING_RESULTS);
    // TMDB WITHOUT THE PERSON'S TOKEN asks nothing, and says where the token
    // goes.
    if source == Suggester::Tmdb && !v.kept.has(Key::Tmdb) {
        b.warning(&lookup_refusal(
            Refusal::new(Service::Tmdb, Why::NoKey),
            b.texts,
        ));
        let internet = PageId::Internet;
        b.pair(
            b.t(internet.key()),
            String::new(),
            Target::Go(internet),
            internet.spot(),
        );
        return;
    }
    ask_row(&v.prefs, source.services(), DeskKind::Suggestions, b);
    let desk = v.found.page_for(DeskKind::Suggestions, &location);
    status(
        s,
        DeskKind::Suggestions,
        desk,
        &hosts_of(source.services(), b.texts),
        b,
    );
    let Some(Page::Suggestions { candidates, .. }) = desk.map(|d| &d.page) else {
        return;
    };
    // FOUND BY ANOTHER than the ring now names: said, so the results are
    // not read as that one's.
    if let Some(Query::Suggest { source: by, .. }) = desk.and_then(|d| d.query.as_ref())
        && *by != source
        && !candidates.is_empty()
    {
        let said = b.texts.t_with(
            key::PLAYER_SUGGEST_CAME_FROM,
            &[("service", &b.t(suggester_key(*by)))],
        );
        crate::player_find::note_said(&said, b);
    }
    let busy = crate::player_find::summary(s, DeskKind::Suggestions).is_some_and(|d| d.busy);
    if candidates.is_empty() && desk.is_some_and(|d| d.trouble.is_none()) && !busy {
        b.note(key::PLAYER_DESK_NOTHING_FOUND);
    }
    if !candidates.is_empty() {
        let said = b.texts.t_args(
            key::PLAYER_DESK_FOUND,
            &[(
                "count",
                Arg::Number(i64::try_from(candidates.len()).unwrap_or(i64::MAX)),
            )],
        );
        crate::player_find::note_said(&said, b);
    }
    for c in candidates {
        b.pair_under(
            candidate_name(c, b.texts),
            String::new(),
            &candidate_said(c, b.texts),
            Target::Candidate(c.id),
            &format!("candidate-{}", c.id.0),
        );
    }
}

/// A SUGGESTION'S PAGE: who suggested it, the two covers side by side, a
/// row for each field it has — a toggle, the suggested value and what the
/// file says now — and the row that writes what is turned on, then how that
/// write goes.
pub(crate) fn candidate_page(v: &View, id: CandidateId, s: &State, b: &mut Builder<'_>) {
    let Some(location) = v.meta.location(s) else {
        return;
    };
    let desk = v.found.page_for(DeskKind::Suggestions, &location);
    let Some(c) = candidate(desk, id) else {
        b.note(key::PLAYER_TROUBLE_NO_SUCH_CANDIDATE);
        return;
    };
    b.note(key::PLAYER_ABOUT_CANDIDATE);
    b.pair(
        b.t(key::PLAYER_SUGGEST_SOURCE),
        b.t(service_key(c.suggestion.source)),
        Target::Nothing,
        "candidate-source",
    );
    if !c.complete {
        b.note(key::PLAYER_SUGGEST_READING);
    }
    covers(v, c, s, b);
    let sheet = v.meta.sheet(s);
    // THE COVER'S TOGGLE, when it suggests one.
    if c.suggestion.cover.is_some() {
        let on = v.imported(c, ImportOf::Cover, s);
        let now = sheet.and_then(|sh| sh.front_cover()).map(|cover| {
            b.texts.t_with(
                key::PLAYER_SUGGEST_NOW,
                &[(
                    "value",
                    &match (cover.width, cover.height) {
                        (Some(w), Some(h)) => b.texts.t_args(
                            key::PLAYER_META_COVER_SIZE,
                            &[
                                ("width", Arg::Number(i64::from(w))),
                                ("height", Arg::Number(i64::from(h))),
                            ],
                        ),
                        _ => b.t(key::PLAYER_META_COVER_SOME),
                    },
                )],
            )
        });
        b.pair_with_toggle(
            Some(on),
            b.t(key::PLAYER_SUGGEST_COVER),
            b.t(if on {
                key::PLAYER_SUGGEST_TAKEN
            } else {
                key::PLAYER_SUGGEST_LEFT
            }),
            now,
            Target::Import(c.id, ImportOf::Cover),
            "import-cover",
        );
    }
    for f in &c.suggestion.fields {
        let now = sheet.and_then(|sh| sh.value(f.field));
        let label = b.t(crate::player_metadata::field_key(f.field));
        let value = crate::player_metadata::field_value(Some(&f.value), b.texts);
        let spot = format!("import-{}", crate::player_metadata::field_key(f.field));
        if same(now, &f.value) {
            b.pair_with_toggle(
                None,
                label,
                value,
                Some(b.t(key::PLAYER_SUGGEST_SAME)),
                Target::Nothing,
                &spot,
            );
            continue;
        }
        let now = now.map(str::trim).filter(|n| !n.is_empty()).map(|n| {
            b.texts.t_with(
                key::PLAYER_SUGGEST_NOW,
                &[(
                    "value",
                    &crate::player_metadata::field_value(Some(n), b.texts),
                )],
            )
        });
        let on = v.imported(c, ImportOf::Field(f.field), s);
        b.pair_with_toggle(
            Some(on),
            label,
            value,
            now,
            Target::Import(c.id, ImportOf::Field(f.field)),
            &spot,
        );
    }
    b.inert(crate::chrome::AIR);
    let (fields, cover) = v.to_write(c, s);
    let count = Arg::Number(i64::try_from(fields.len()).unwrap_or(i64::MAX));
    if fields.is_empty() && !cover {
        b.pair(
            b.t(key::PLAYER_SUGGEST_NOTHING_CHOSEN),
            String::new(),
            Target::Nothing,
            "write",
        );
    } else {
        b.pair(
            b.texts.t_args(
                if cover {
                    key::PLAYER_SUGGEST_WRITE_COVER
                } else {
                    key::PLAYER_SUGGEST_WRITE
                },
                &[("count", count)],
            ),
            String::new(),
            Target::Write(c.id),
            "write",
        );
    }
    if let Some(desk) = desk
        && let Some(taken) = desk.taken.as_ref().filter(|t| t.candidate == id)
        && let ronoctua_player::find::TakeStatus::Refused(t) = &taken.status
    {
        b.warning(&crate::player::trouble_sentence(t, b.texts));
    }
    if let Some(sheet) = sheet {
        crate::player_metadata::retag_lines(s, sheet, &location, b);
    }
}

/// The two covers, side by side in one geometry: the suggested one — being
/// fetched, refused, or drawn — and the file's own.
fn covers(v: &View, c: &SuggestionCandidate, s: &State, b: &mut Builder<'_>) {
    let current = v
        .meta
        .cover(s)
        .map(|(k, rgba)| (k.clone(), (rgba.width, rgba.height)));
    let mut pictures: Vec<crate::player_rows::Pictured> = Vec::new();
    let mut refused = None;
    if c.suggestion.cover.is_some() {
        let (picture, caption) = match &c.seen {
            Some(SuggestedCover::Kept { path }) => (
                v.found
                    .picture(path)
                    .map(|(k, rgba)| (k.clone(), (rgba.width, rgba.height))),
                key::PLAYER_SUGGEST_PICTURE_SUGGESTED,
            ),
            Some(SuggestedCover::Refused(t)) => {
                refused = Some(t.clone());
                (None, key::PLAYER_SUGGEST_PICTURE_NONE)
            }
            // ASKED AND REFUSED at once — no such candidate any more, a
            // host not let: said, never "fetching" for good.
            Some(SuggestedCover::Fetching) | None => match v.found.cover_refused(c.id) {
                Some(t) => {
                    refused = Some(t.clone());
                    (None, key::PLAYER_SUGGEST_PICTURE_NONE)
                }
                None => (None, key::PLAYER_SUGGEST_FETCHING),
            },
        };
        pictures.push((picture, b.t(caption)));
    }
    if let Some(current) = current {
        pictures.push((Some(current), b.t(key::PLAYER_SUGGEST_PICTURE_CURRENT)));
    }
    if pictures.is_empty() {
        return;
    }
    b.pictures(&pictures);
    if let Some(t) = refused {
        b.warning(&crate::player::trouble_sentence(&t, b.texts));
    }
}

/// What a press on a row of the suggestions does — `None` for a target that
/// is none of theirs.
pub(crate) fn turn(
    v: &mut View,
    target: &Target,
    press: crate::player::Press,
    s: &State,
    texts: &ronoctua_i18n::Texts,
) -> Option<Errand> {
    use crate::player::Press;
    match (target, press) {
        (Target::SuggestSource, Press::Enter | Press::Less | Press::More) => {
            let all = Suggester::ALL;
            let now = all.iter().position(|x| *x == v.source(s)).unwrap_or(0);
            let n = all.len();
            let to = if press == Press::Less {
                (now + n - 1) % n
            } else {
                (now + 1) % n
            };
            v.finding.source = Some(all[to]);
            Some(Errand::Nothing)
        }
        (Target::Import(id, of), Press::Enter | Press::Less | Press::More) => {
            let location = v.meta.location(s)?;
            let c = candidate(v.found.page_for(DeskKind::Suggestions, &location), *id)?.clone();
            let on = v.imported(&c, *of, s);
            v.finding.imports.insert((*id, *of), !on);
            Some(Errand::Nothing)
        }
        (Target::Write(id), Press::Enter) => {
            let location = v.meta.location(s)?;
            let c = candidate(v.found.page_for(DeskKind::Suggestions, &location), *id)?;
            let (fields, cover) = v.to_write(c, s);
            if fields.is_empty() && !cover {
                return Some(Errand::Nothing);
            }
            let name = crate::player::name_of(
                &ronoctua_player::queue::Name::Location(location),
                texts,
                crate::player::NAME_IN_A_SENTENCE,
            );
            Some(Errand::TellSaying(
                Ask::Take {
                    desk: DeskKind::Suggestions,
                    candidate: *id,
                    fields,
                    cover,
                    to: ronoctua_player::find::LyricsTo::Beside,
                    replace: false,
                },
                texts.t_with(key::PLAYER_SUGGEST_WRITING, &[("name", &name)]),
            ))
        }
        _ => None,
    }
}

/// What the Metadata section's row of the suggestions says: what the
/// daemon is doing for the file shown, how many it found, or nothing yet.
#[must_use]
pub fn summary(v: &View, s: &State, texts: &ronoctua_i18n::Texts) -> String {
    let Some(location) = v.meta.location(s) else {
        return String::new();
    };
    if crate::player_find::summary(s, DeskKind::Suggestions).is_some_and(|d| d.busy) {
        return texts.t(key::PLAYER_SUGGEST_BUSY);
    }
    match v
        .found
        .page_for(DeskKind::Suggestions, &location)
        .map(|d| &d.page)
    {
        Some(Page::Suggestions { candidates, .. }) if !candidates.is_empty() => texts.t_args(
            key::PLAYER_DESK_FOUND,
            &[(
                "count",
                Arg::Number(i64::try_from(candidates.len()).unwrap_or(i64::MAX)),
            )],
        ),
        _ => String::new(),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::player::fixture::{alone, track};
    use crate::player::{Press, Typing};
    use crate::player_metadata::Got;
    use ronoctua_lookup::suggestion::{Cover, Suggested, Suggestion};
    use ronoctua_player::find::{Asked, SuggestionCandidate, TakeStatus, Taken};
    use ronoctua_player::wire::{DeskSummary, TrackKind as Kind};
    use ronoctua_tokens as tokens;

    fn texts(language: &str) -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh(language).expect(language)
    }

    /// The song the Metadata's fixture sheet is of, playing.
    pub(crate) fn a_song(location: &str) -> State {
        let mut s = alone(location, vec![track(1, Kind::Audio, None, true)]);
        s.desks = vec![DeskSummary {
            kind: DeskKind::Suggestions,
            generation: 2,
            busy: false,
            trouble: None,
        }];
        s
    }

    /// A view that holds the song's sheet — "Harbour Lights" by Marta
    /// Quintela, blanks for the rest, a cover of its own.
    fn holding(s: &State) -> View {
        let location = crate::player_metadata::locations(s)[0].clone();
        let mut v = View::blank(None);
        v.meta.given(
            &location,
            s.sheet_generation,
            Got::Sheet(Box::new(crate::player_metadata::tests::song_sheet(
                &location,
            ))),
        );
        v
    }

    /// The suggestions desk as the daemon answers it for `location`: two
    /// candidates of MusicBrainz — the first read whole, its cover named
    /// (`seen`), the second still being read.
    pub(crate) fn desk_for(location: &str, seen: Option<SuggestedCover>) -> Desk {
        let field = |field, value: &str| Suggested {
            field,
            value: value.into(),
        };
        let mut first = Suggestion::new(Service::MusicBrainz, "9a0b-first".into());
        first.fields = vec![
            field(Field::Title, "Harbour Lights"),
            field(Field::Artist, "Marta Quintela & Coro"),
            field(Field::Album, "Tidewater"),
            field(Field::Date, "2024-05-17"),
        ];
        first.cover = Some(Cover::CoverArt {
            release: "b10bbbfc-cf9e-42e0-be17-e2c3e1d2600d".into(),
        });
        let mut second = Suggestion::new(Service::MusicBrainz, "9a0b-second".into());
        second.fields = vec![field(Field::Title, "Harbour Lights (live)")];
        Desk {
            generation: 2,
            query: Some(Query::Suggest {
                location: location.into(),
                source: Suggester::MusicBrainz,
                words: None,
                tongues: vec!["en".into()],
            }),
            asked: Some(Asked {
                words: "Harbour Lights".into(),
                artist: Some("Marta Quintela".into()),
                ..Asked::default()
            }),
            page: Page::Suggestions {
                location: location.into(),
                candidates: vec![
                    SuggestionCandidate {
                        id: CandidateId(1),
                        suggestion: first,
                        complete: true,
                        seen,
                    },
                    SuggestionCandidate {
                        id: CandidateId(2),
                        suggestion: second,
                        complete: false,
                        seen: None,
                    },
                ],
            },
            ..Desk::empty(DeskKind::Suggestions)
        }
    }

    fn row_of<'a>(
        rows: &'a [crate::player_rows::Row],
        target: &Target,
    ) -> Option<(usize, &'a crate::player_rows::Row)> {
        rows.iter().enumerate().find(|(_, r)| &r.target == target)
    }

    /// Every word on the page, a wrapped sentence whole again.
    fn said(rows: &[crate::player_rows::Row]) -> String {
        rows.iter()
            .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// FAILS IF: the search of suggestions asks anybody but through its row —
    /// which names the hosts, and whose press lets them before asking, once —
    /// or a film stops asking Wikidata first, or TMDB without the person's
    /// token asks at all instead of saying where the token goes.
    #[test]
    fn suggestions_are_asked_through_a_row_that_names_the_hosts() {
        let t = texts("en-GB");
        let location = "/music/harbour.flac";
        let s = a_song(location);
        let mut v = holding(&s);
        v.key(Press::Go(PageId::Suggestions), Some(&s), 80, &t);
        assert_eq!(v.page(), PageId::Suggestions);
        let rows = v.rows(Some(&s), 80, &t);
        let (at, ask) = row_of(&rows, &Target::Search(DeskKind::Suggestions)).expect("the ask");
        let label = &ask.fields[0].text;
        assert!(
            label.contains("musicbrainz.org") && label.contains("coverartarchive.org"),
            "{label}"
        );
        v.grabbed(at);
        let query = Query::Suggest {
            location: location.into(),
            source: Suggester::MusicBrainz,
            words: None,
            tongues: vec!["en".into()],
        };
        match v.key(Press::Enter, Some(&s), 80, &t) {
            Errand::Let {
                services,
                directories,
                on: true,
                then,
            } => {
                assert_eq!(services, [Service::MusicBrainz, Service::CoverArtArchive]);
                assert!(directories.is_empty());
                assert!(
                    matches!(*then, Errand::TellSaying(Ask::Find { query: ref q }, _) if *q == query),
                    "{then:?}"
                );
            }
            other => panic!("the first press asked {other:?}"),
        }
        // LET NOW: the second press only asks.
        assert!(matches!(
            v.key(Press::Enter, Some(&s), 80, &t),
            Errand::TellSaying(Ask::Find { .. }, _)
        ));
        // OTHER WORDS, typed.
        let rows = v.rows(Some(&s), 80, &t);
        let (at, _) = row_of(&rows, &Target::Words(DeskKind::Suggestions)).expect("words");
        v.grabbed(at);
        let crate::player::Errand::Ask(q) = v.key(Press::Enter, Some(&s), 80, &t) else {
            panic!("other words asked nothing");
        };
        assert_eq!(q.id, crate::player_question::FIND_WORDS);
        assert_eq!(v.typing, Some(Typing::Words(DeskKind::Suggestions)));
        let mut pairing = crate::player_machines::Pairing::default();
        match v.answered(q.id, "tidewater", &mut pairing, Some(&s), 80, &t) {
            Errand::TellSaying(
                Ask::Find {
                    query: Query::Suggest { words, .. },
                },
                _,
            ) => assert_eq!(words.as_deref(), Some("tidewater")),
            other => panic!("the words asked {other:?}"),
        }
        // THE RING: Wikidata, then TMDB — which, with no token, asks nothing
        // and leads to where the token goes.
        let rows = v.rows(Some(&s), 80, &t);
        let (at, _) = row_of(&rows, &Target::SuggestSource).expect("the ring");
        v.grabbed(at);
        v.key(Press::More, Some(&s), 80, &t);
        assert_eq!(v.source(&s), Suggester::Wikidata);
        v.key(Press::More, Some(&s), 80, &t);
        assert_eq!(v.source(&s), Suggester::Tmdb);
        let rows = v.rows(Some(&s), 80, &t);
        assert!(row_of(&rows, &Target::Search(DeskKind::Suggestions)).is_none());
        assert!(row_of(&rows, &Target::Go(PageId::Internet)).is_some());
        assert!(
            said(&rows).contains(&lookup_refusal(Refusal::new(Service::Tmdb, Why::NoKey), &t)),
            "{}",
            said(&rows)
        );
        v.kept = crate::player_internet::Kept::given(vec![Key::Tmdb]);
        let rows = v.rows(Some(&s), 80, &t);
        let (_, ask) = row_of(&rows, &Target::Search(DeskKind::Suggestions)).expect("the ask");
        assert!(ask.fields[0].text.contains("api.themoviedb.org"));
        // A FILM asks Wikidata first.
        let film = alone(
            "/films/harbour.mkv",
            vec![track(1, Kind::Video, None, true)],
        );
        let mut sheet = crate::player_metadata::tests::song_sheet("/films/harbour.mkv");
        sheet.kind = ronoctua_tags::Kind::Video;
        let mut v = View::blank(None);
        v.meta
            .given("/films/harbour.mkv", 0, Got::Sheet(Box::new(sheet)));
        assert_eq!(v.source(&film), Suggester::Wikidata);
    }

    /// FAILS IF: the suggestions of another file are shown as this one's,
    /// or a suggestion stops saying it is still being read, or the
    /// Metadata's row stops saying how many were found.
    #[test]
    fn a_page_shows_the_suggestions_of_its_own_file() {
        let t = texts("pt-BR");
        let s = a_song("/music/harbour.flac");
        let mut v = holding(&s);
        v.found.given(desk_for("/music/other.flac", None));
        v.key(Press::Go(PageId::Suggestions), Some(&s), 80, &t);
        let rows = v.rows(Some(&s), 80, &t);
        assert!(
            !rows
                .iter()
                .any(|r| matches!(r.target, Target::Candidate(_))),
            "another file's suggestions are shown"
        );
        v.found.given(desk_for("/music/harbour.flac", None));
        let rows = v.rows(Some(&s), 80, &t);
        let (_, first) = row_of(&rows, &Target::Candidate(CandidateId(1))).expect("the first");
        assert!(
            first
                .fields
                .iter()
                .any(|f| f.text.contains("Marta Quintela & Coro"))
        );
        // WHAT IS SAID OF IT, under its name: still being read.
        let (at, _) = row_of(&rows, &Target::Candidate(CandidateId(2))).expect("the second");
        assert!(
            rows[at + 1].continues
                && rows[at + 1].target == Target::Candidate(CandidateId(2))
                && rows[at + 1].fields[0].text == t.t(key::PLAYER_SUGGEST_READING),
            "{:?}",
            rows[at + 1]
        );
        assert!(
            said(&rows).contains(&t.t_with(
                key::PLAYER_DESK_SEARCHED_FOR,
                &[("words", "Harbour Lights")]
            )),
            "{}",
            said(&rows)
        );
        v.back(Some(&s), 80, &t);
        let rows = v.rows(Some(&s), 80, &t);
        let (_, row) = row_of(&rows, &Target::Go(PageId::Suggestions)).expect("the metadata's");
        assert_eq!(
            row.fields.last().map(|f| f.text.clone()),
            Some(t.t_args(key::PLAYER_DESK_FOUND, &[("count", Arg::Number(2))]))
        );
    }

    /// FAILS IF: a suggestion's page stops putting each field beside what
    /// the file says now — the same value a fact that says so, a blank one
    /// turned on and a filled one off — or its toggle stops turning on the
    /// click at its head, or "Write" stops taking exactly what is turned on.
    #[test]
    fn a_suggestion_is_written_field_by_field_as_chosen() {
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            let location = "/music/harbour.flac";
            let s = a_song(location);
            let mut v = holding(&s);
            v.found.given(desk_for(location, None));
            v.key(Press::Go(PageId::Suggestions), Some(&s), 80, &t);
            let rows = v.rows(Some(&s), 80, &t);
            let (at, _) = row_of(&rows, &Target::Candidate(CandidateId(1))).expect("the first");
            v.clicked(at, &rows);
            v.clicked(at, &rows);
            assert_eq!(v.key(Press::Enter, Some(&s), 80, &t), Errand::Nothing);
            assert_eq!(v.page(), PageId::Candidate(CandidateId(1)), "{language}");
            let rows = v.rows(Some(&s), 80, &t);
            let text = said(&rows);
            // THE SAME TITLE: a fact, saying so.
            assert!(
                !rows
                    .iter()
                    .any(|r| matches!(r.target, Target::Import(_, ImportOf::Field(Field::Title)))),
                "{language}: the same title is a toggle"
            );
            assert!(text.contains(&t.t(key::PLAYER_SUGGEST_SAME)), "{text}");
            // THE ARTIST the file says otherwise: off, and what it says now
            // under it.
            let artist = Target::Import(CandidateId(1), ImportOf::Field(Field::Artist));
            let (at, row) = row_of(&rows, &artist).expect("the artist");
            assert_eq!(
                row.fields[0].text,
                tokens::icon::PLAYER_IMPORT_OFF.to_string(),
                "{language}"
            );
            let now = t.t_with(key::PLAYER_SUGGEST_NOW, &[("value", "Marta Quintela")]);
            assert!(
                rows.get(at + 1)
                    .is_some_and(|r| r.continues && r.fields.iter().any(|f| f.text == now)),
                "{language}: `{now}` is not under the value"
            );
            // THE ALBUM the file has blank: on.
            let album = Target::Import(CandidateId(1), ImportOf::Field(Field::Album));
            let (_, row) = row_of(&rows, &album).expect("the album");
            assert_eq!(
                row.fields[0].text,
                tokens::icon::PLAYER_IMPORT_ON.to_string()
            );
            // THE COVER: the file has one, so off.
            let cover = Target::Import(CandidateId(1), ImportOf::Cover);
            let (_, row) = row_of(&rows, &cover).expect("the cover");
            assert_eq!(
                row.fields[0].text,
                tokens::icon::PLAYER_IMPORT_OFF.to_string()
            );
            // THE ARTIST turned on BY A CLICK ON ITS TOGGLE — the control at
            // the row's head acts at once.
            let from = row
                .cells
                .iter()
                .find(|c| c.control)
                .map(|c| c.from)
                .expect("a toggle");
            let (at, _) = row_of(&rows, &artist).expect("the artist");
            assert!(
                v.clicked_at(at, from, &rows),
                "{language}: the toggle waited"
            );
            assert_eq!(v.key(Press::Enter, Some(&s), 80, &t), Errand::Nothing);
            let rows = v.rows(Some(&s), 80, &t);
            let (at, write) = row_of(&rows, &Target::Write(CandidateId(1))).expect("write");
            assert_eq!(
                write.fields[0].text,
                t.t_args(key::PLAYER_SUGGEST_WRITE, &[("count", Arg::Number(2))]),
                "{language}: the artist turned on and the blank album — not the date the \
                 file already has"
            );
            v.grabbed(at);
            match v.key(Press::Enter, Some(&s), 80, &t) {
                Errand::TellSaying(
                    Ask::Take {
                        desk: DeskKind::Suggestions,
                        candidate: CandidateId(1),
                        fields,
                        cover: false,
                        ..
                    },
                    _,
                ) => assert_eq!(fields, vec![Field::Artist, Field::Album]),
                other => panic!("{language}: write asked {other:?}"),
            }
        }
    }

    /// Writes a PNG of `w × h` pixels to `path`.
    fn png(path: &std::path::Path, w: u32, h: u32) {
        image::RgbaImage::from_pixel(w, h, image::Rgba([200, 40, 40, 255]))
            .save(path)
            .expect("a picture");
    }

    /// FAILS IF: the suggested cover stops standing BESIDE the file's own —
    /// two boxes of one size on the same lines, each picture found by the
    /// key its row names — or a cover still being fetched stops keeping its
    /// box and saying so.
    #[test]
    fn the_suggested_cover_stands_beside_the_file_s_own() {
        let t = texts("en-GB");
        let dir = std::env::temp_dir().join(format!("ronoctua-suggested-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a folder");
        let (own, suggested) = (dir.join("own.png"), dir.join("preview-1.png"));
        png(&own, 60, 60);
        png(&suggested, 50, 50);
        let location = "/music/harbour.flac";
        let s = a_song(location);
        let mut sheet = crate::player_metadata::tests::song_sheet(location);
        sheet.cover_file = Some(own.clone());
        let mut v = View::blank(None);
        v.meta.given(location, 0, Got::Sheet(Box::new(sheet)));
        v.found.given(desk_for(
            location,
            Some(SuggestedCover::Kept {
                path: suggested.to_string_lossy().into_owned(),
            }),
        ));
        v.push(PageId::Candidate(CandidateId(1)), None);
        let rows = v.rows(Some(&s), 80, &t);
        let lines: Vec<&crate::player_rows::Row> =
            rows.iter().filter(|r| !r.covers.is_empty()).collect();
        assert_eq!(
            lines.len(),
            usize::from(tokens::space::PLAYER_COVERLINES),
            "the pictures' lines"
        );
        let [a, b] = lines[0].covers.as_slice() else {
            panic!("not two pictures side by side: {:?}", lines[0].covers);
        };
        assert_eq!((a.cells, a.lines), (b.cells, b.lines), "two geometries");
        assert_eq!(
            b.left,
            a.left + a.cells + tokens::space::PLAYER_COVERGAP,
            "not side by side"
        );
        assert!(v.picture(&s, &a.key).is_some(), "the suggested one");
        assert!(v.picture(&s, &b.key).is_some(), "the file's own");
        assert_ne!(a.key, b.key);
        let second_box = b.left;
        // IN A NARROW DOOR the boxes are as tall as the pictures in them —
        // no lines of air between the pictures and what they are.
        let narrow = v.rows(Some(&s), 32, &t);
        let tall = narrow.iter().filter(|r| !r.covers.is_empty()).count();
        assert!(
            tall < usize::from(tokens::space::PLAYER_COVERLINES),
            "{tall} lines for two small squares"
        );
        let first = narrow
            .iter()
            .find_map(|r| r.covers.first())
            .expect("a picture");
        let size = tokens::text::size();
        let (cell, line) = (
            tokens::metrics::cell_width(size),
            tokens::metrics::line_height(size),
        );
        let (area, crop) = crate::player_metadata::cover_place(
            (first.cells, first.lines),
            (cell, line),
            first.size,
            (0, first.lines),
        )
        .expect("drawn");
        assert_eq!(crop, (0.0, 1.0), "the picture is cut by its own box");
        assert!(
            (area.width - f32::from(first.cells) * cell).abs() < 0.5,
            "the picture does not fill its box's width: {area:?}"
        );
        let text = said(&rows);
        for k in [
            key::PLAYER_SUGGEST_PICTURE_SUGGESTED,
            key::PLAYER_SUGGEST_PICTURE_CURRENT,
        ] {
            assert!(text.contains(&t.t(k)), "{k}");
        }
        // BEING FETCHED: its box kept, empty, and said.
        let mut v = View::blank(None);
        let mut sheet = crate::player_metadata::tests::song_sheet(location);
        sheet.cover_file = Some(own);
        v.meta.given(location, 0, Got::Sheet(Box::new(sheet)));
        v.found
            .given(desk_for(location, Some(SuggestedCover::Fetching)));
        v.push(PageId::Candidate(CandidateId(1)), None);
        let rows = v.rows(Some(&s), 80, &t);
        let line = rows
            .iter()
            .find(|r| !r.covers.is_empty())
            .expect("a picture");
        assert_eq!(line.covers.len(), 1, "only the file's own is drawn");
        assert_eq!(
            line.covers[0].left, second_box,
            "the file's own slid into the empty box"
        );
        assert!(said(&rows).contains(&t.t(key::PLAYER_SUGGEST_FETCHING)));
        // REFUSED AT ONCE — asked, and the player would not fetch it: said,
        // never "fetching" for good.
        let off = ronoctua_player::wire::Trouble::Off {
            host: "coverartarchive.org".into(),
        };
        v.found.cover_refused_given(CandidateId(1), off.clone());
        let text = said(&v.rows(Some(&s), 80, &t));
        assert!(!text.contains(&t.t(key::PLAYER_SUGGEST_FETCHING)), "{text}");
        assert!(
            text.contains(&crate::player::trouble_sentence(&off, &t)),
            "the refusal is not said: {text}"
        );
        // THE SAME FILE NAME, written again by a player started again: another
        // texture's key — one is never taken down by its key.
        let mut again = View::blank(None);
        let desk = || {
            desk_for(
                location,
                Some(SuggestedCover::Kept {
                    path: suggested.to_string_lossy().into_owned(),
                }),
            )
        };
        again.found.given(desk());
        let first = again
            .found
            .picture(&suggested.to_string_lossy())
            .map(|(k, _)| k.clone())
            .expect("decoded");
        png(&suggested, 70, 40);
        let mut later = View::blank(None);
        later.found.given(desk());
        let second = later
            .found
            .picture(&suggested.to_string_lossy())
            .map(|(k, _)| k.clone())
            .expect("decoded");
        assert_ne!(first, second, "a new picture under the old picture's key");
        std::fs::remove_dir_all(&dir).expect("cleaned");
    }

    /// FAILS IF: a suggestion a newer search replaced is still drawn, or its
    /// take refused is not said on its page.
    #[test]
    fn a_suggestion_gone_or_refused_says_so() {
        let t = texts("en-GB");
        let location = "/music/harbour.flac";
        let s = a_song(location);
        let mut v = holding(&s);
        let mut desk = desk_for(location, None);
        desk.taken = Some(Taken {
            candidate: CandidateId(1),
            status: TakeStatus::Refused(ronoctua_player::wire::Trouble::Off {
                host: "coverartarchive.org".into(),
            }),
        });
        v.found.given(desk);
        v.push(PageId::Candidate(CandidateId(1)), None);
        let text = said(&v.rows(Some(&s), 80, &t));
        assert!(
            text.contains(&t.t_with(key::PLAYER_TROUBLE_OFF, &[("host", "coverartarchive.org")])),
            "{text}"
        );
        v.push(PageId::Candidate(CandidateId(9)), None);
        assert!(
            said(&v.rows(Some(&s), 80, &t)).contains(&t.t(key::PLAYER_TROUBLE_NO_SUCH_CANDIDATE))
        );
    }
}
