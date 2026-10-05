//! Every listing against answers written by hand: no test here reaches a
//! directory, runs yt-dlp or reads the machine's atlas.

use std::{
    path::PathBuf,
    sync::{Mutex, PoisonError},
};

use ronoctua_broadcast::{Request, iptv::Area, picks::Kind};
use ronoctua_http::Reply;
use ronoctua_player::{find::Directory, queue::EntryId};

use super::*;

/// Answers, each given once, to the first request whose address starts with
/// the one named; every request recorded.
struct Canned {
    answers: Mutex<Vec<(String, u16, Vec<u8>)>>,
    asked: Mutex<Vec<String>>,
}

impl Canned {
    fn new(answers: &[(&str, u16, &[u8])]) -> Self {
        Self {
            answers: Mutex::new(
                answers
                    .iter()
                    .map(|(u, s, b)| ((*u).to_owned(), *s, b.to_vec()))
                    .collect(),
            ),
            asked: Mutex::new(Vec::new()),
        }
    }

    fn urls(&self) -> Vec<String> {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl Net for Canned {
    fn send(&self, request: &Request) -> Result<Reply, Why> {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(request.url.clone());
        let mut answers = self.answers.lock().unwrap_or_else(PoisonError::into_inner);
        let at = answers
            .iter()
            .position(|(u, _, _)| request.url.starts_with(u.as_str()))
            .ok_or(Why::Unreachable)?;
        let (_, status, body) = answers.remove(at);
        Ok(Reply {
            status,
            headers: Vec::new(),
            body,
        })
    }
}

/// A map of a few countries, and of places by name.
#[derive(Default)]
struct Drawn {
    anchors: Vec<(Code, Spot)>,
    places: Vec<(Code, &'static str, Spot)>,
}

impl Map for Drawn {
    fn anchor(&self, country: Code) -> Option<Spot> {
        self.anchors
            .iter()
            .find(|(c, _)| *c == country)
            .map(|(_, s)| *s)
    }

    fn place(&self, country: Code, name: &str) -> Option<Spot> {
        self.places
            .iter()
            .find(|(c, n, _)| *c == country && *n == name)
            .map(|(_, _, s)| *s)
    }
}

const BR: Code = Code(*b"BR");
const GB: Code = Code(*b"GB");
const BR_ANCHOR: Spot = Spot {
    lat: -10.8,
    lon: -53.1,
};
const GB_ANCHOR: Spot = Spot {
    lat: 54.4,
    lon: -2.1,
};

fn map() -> Drawn {
    Drawn {
        anchors: vec![(BR, BR_ANCHOR), (GB, GB_ANCHOR)],
        places: Vec::new(),
    }
}

/// A cache of its own, empty, under the system's temporary directory.
fn cache(name: &str) -> Cache {
    let dir: PathBuf = std::env::temp_dir().join(format!(
        "ronoctua-directory-test-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    Cache::at(dir, 64 * MIB)
}

/// Every directory let.
fn all_let() -> Preferences {
    let mut p = Preferences::default();
    for d in Directory::ALL {
        p.let_directory(d);
    }
    p
}

/// Every page a listing posted, with what gave nothing beside it.
type Posted = Vec<(Page, Vec<Trouble>)>;

/// A listing asked the way a desk's thread asks it, and every page posted.
fn looked(
    query: &Query,
    prefs: &Preferences,
    net: &Canned,
    cache: Option<&Cache>,
    now: SystemTime,
) -> (Result<(), Trouble>, Posted) {
    let drawn = map();
    let radio = Mutex::new(None);
    let index = Mutex::new(());
    let tools = Tools {
        net,
        cache,
        map: &drawn,
        radio: &radio,
        index: &index,
        seed: 0,
        now,
    };
    let mut pages = Vec::new();
    let outcome = look(query, prefs, &tools, &mut |page, refused| {
        pages.push((page, refused));
    });
    (outcome, pages)
}

fn station(uuid: &str, name: &str, country: Option<Code>, at: Option<Spot>) -> Station {
    Station {
        uuid: uuid.to_owned(),
        name: name.to_owned(),
        url: format!("https://radio.example/{name}"),
        alternate: None,
        homepage: None,
        country,
        tags: Vec::new(),
        languages: Vec::new(),
        codec: None,
        bitrate_kbps: 0,
        hls: false,
        votes: 0,
        clicks: 0,
        at,
    }
}

const UUID_A: &str = "7ada8a81-5ae1-418c-8f18-51d2f38d86a4";
const UUID_B: &str = "901c92bd-a757-449a-b8ce-652b079fd38a";
const UUID_C: &str = "11111111-2222-3333-4444-555555555555";
const UUID_D: &str = "11111111-2222-3333-4444-555555555556";

/// FAILS IF: a station that does not say where it is disappears from the
/// globe, or is drawn anywhere but its country's anchor — or a station that
/// does say is not drawn where it says, or one with no country the map
/// knows is not counted as having no place.
#[test]
fn a_station_without_a_coordinate_lands_on_its_countrys_anchor() {
    let sao_paulo = Spot {
        lat: -23.55,
        lon: -46.63,
    };
    let (rows, globe) = stations_globe(
        vec![
            station(UUID_A, "Bossa", Some(BR), None),
            station(UUID_B, "Mix", Some(BR), Some(sao_paulo)),
            station(UUID_C, "Samba", Some(BR), None),
            station(UUID_D, "Nowhere", None, None),
        ],
        &map(),
    );
    assert_eq!(rows.len(), 4, "every station is a row");
    let anchor = globe
        .marks
        .iter()
        .find(|m| m.kind == MarkKind::Country)
        .expect("the country's mark");
    assert_eq!(anchor.at, BR_ANCHOR);
    assert_eq!(anchor.country, Some(BR));
    assert_eq!(anchor.weight, 2, "both stations without a coordinate");
    assert_eq!(rows[0].mark, Some(anchor.id));
    assert_eq!(rows[2].mark, Some(anchor.id));
    let own = globe
        .marks
        .iter()
        .find(|m| m.kind == MarkKind::Station)
        .expect("the station's own mark");
    assert_eq!(
        (own.at, own.weight, own.label.as_str()),
        (sao_paulo, 1, "Mix")
    );
    assert_eq!(rows[1].mark, Some(own.id));
    assert_eq!(rows[3].mark, None);
    assert_eq!(globe.placeless, 1);
    // THE STREAM IS READY TO PUT: live, and where it was found.
    assert!(rows[0].stream.live);
    assert_eq!(
        rows[0].stream.origin,
        Origin::RadioBrowser {
            uuid: UUID_A.to_owned()
        }
    );
}

const CATEGORIES_JSON: &[u8] = br#"[{"id":"news","name":"News"},{"id":"movies","name":"Movies"}]"#;
const CHANNELS_JSON: &[u8] = br#"[
    {"id":"News24.uk","name":"News 24","country":"UK","categories":["news"]},
    {"id":"Films.br","name":"Filmes","country":"BR","categories":["movies"]},
    {"id":"Lost.zz","name":"Lost","country":"ZZ","categories":["news"]}
]"#;
const STREAMS_JSON: &[u8] = br#"[
    {"channel":"News24.uk","url":"https://tv.example/hd.m3u8","quality":"1080p","user_agent":"Mozilla/5.0 (X11; Linux, like Gecko)"},
    {"channel":"News24.uk","url":"https://tv.example/sd.m3u8","quality":"576i","user_agent":"Mozilla/5.0 (X11; Linux, like Gecko)"},
    {"channel":"Films.br","url":"https://tv.example/f.m3u8"},
    {"channel":"Lost.zz","url":"https://tv.example/l.m3u8"}
]"#;
const FEEDS_JSON: &[u8] =
    br#"[{"channel":"News24.uk","id":"HD","is_main":true,"broadcast_area":["ct/GBLON"]}]"#;
const CITIES_JSON: &[u8] = br#"[{"country":"UK","name":"London","code":"GBLON"}]"#;

fn iptv_answers() -> Canned {
    Canned::new(&[
        (
            "https://iptv-org.github.io/api/categories.json",
            200,
            CATEGORIES_JSON,
        ),
        ("https://iptv-org.github.io/api/blocklist.json", 200, b"[]"),
        (
            "https://iptv-org.github.io/api/channels.json",
            200,
            CHANNELS_JSON,
        ),
        (
            "https://iptv-org.github.io/api/streams.json",
            200,
            STREAMS_JSON,
        ),
        ("https://iptv-org.github.io/api/feeds.json", 200, FEEDS_JSON),
        (
            "https://iptv-org.github.io/api/cities.json",
            200,
            CITIES_JSON,
        ),
        (
            "https://iptv-org.github.io/api/subdivisions.json",
            200,
            b"[]",
        ),
    ])
}

/// FAILS IF: British TV — which iptv-org writes `UK` — is not Great
/// Britain's on the globe, or a channel whose country names none is not said
/// to have no place.
#[test]
fn the_united_kingdom_is_gb_on_the_tv_globe() {
    let net = iptv_answers();
    let (outcome, pages) = looked(
        &Query::Tv(TvList::Countries),
        &all_let(),
        &net,
        None,
        SystemTime::now(),
    );
    assert_eq!(outcome, Ok(()));
    let Some((
        Page::TvCountries {
            countries, globe, ..
        },
        _,
    )) = pages.last()
    else {
        panic!("{pages:?}");
    };
    let gb = countries
        .iter()
        .find(|c| c.country == GB)
        .expect("the United Kingdom, as GB");
    assert_eq!(gb.count, 1);
    let mark = &globe.marks[gb.mark.expect("a mark") as usize];
    assert_eq!((mark.at, mark.label.as_str()), (GB_ANCHOR, "GB"));
    assert_eq!(globe.placeless, 1, "the channel of `ZZ`");
    assert!(
        !countries.iter().any(|c| c.country.as_str() == "UK"),
        "{countries:?}"
    );
    // AND THE ONES WITH NO PLACE ARE A LIST TOO: the count, open.
    let net = iptv_answers();
    let (outcome, pages) = looked(
        &Query::Tv(TvList::Placeless),
        &all_let(),
        &net,
        None,
        SystemTime::now(),
    );
    assert_eq!(outcome, Ok(()));
    let Some((
        Page::Channels {
            channels, total, ..
        },
        _,
    )) = pages.last()
    else {
        panic!("{pages:?}");
    };
    assert_eq!(*total, 1, "{channels:?}");
    assert!(channels.iter().all(|c| c.country.is_none_or(|c| c != GB)));
}

/// FAILS IF: a channel's city is not said on its row, or — while the map
/// cannot find it — the channel is not drawn on its country's anchor; or,
/// when the map can, not drawn at the city.
#[test]
fn a_channels_city_is_drawn_where_the_map_finds_it_else_on_its_country() {
    let index = iptv::build(
        &iptv::Raw {
            channels: CHANNELS_JSON,
            streams: STREAMS_JSON,
            categories: CATEGORIES_JSON,
            blocklist: b"[]",
            feeds: Some(FEEDS_JSON),
            cities: Some(CITIES_JSON),
            subdivisions: None,
        },
        &["xxx"],
        0,
    )
    .expect("an index");
    let british = index.in_country(GB);
    let (rows, globe) = channels_globe(&british, &index, &map());
    assert_eq!(rows[0].areas, ["London"]);
    assert_eq!(globe.marks[0].kind, MarkKind::Country);
    assert_eq!(globe.marks[0].at, GB_ANCHOR);
    let london = Spot {
        lat: 51.5,
        lon: -0.1,
    };
    let mut knows = map();
    knows.places.push((GB, "London", london));
    let (rows, globe) = channels_globe(&british, &index, &knows);
    let mark = &globe.marks[rows[0].mark.expect("a mark") as usize];
    assert_eq!(
        (mark.kind, mark.at, mark.label.as_str()),
        (MarkKind::Place, london, "London")
    );
    assert!(index.place(&Area::City("GBLON".into())).is_some());
}

/// FAILS IF: a stream whose first address fails does not try the next, in
/// the order the directory gave — or goes on after the last, instead of
/// being said down.
#[test]
fn a_failed_stream_tries_its_next_address_before_saying_so() {
    let index = iptv::build(
        &iptv::Raw {
            channels: CHANNELS_JSON,
            streams: STREAMS_JSON,
            categories: CATEGORIES_JSON,
            blocklist: b"[]",
            feeds: None,
            cities: None,
            subdivisions: None,
        },
        &["xxx"],
        0,
    )
    .expect("an index");
    let (rows, _) = channels_globe(&index.in_country(GB), &index, &map());
    let stream = &rows[0].stream;
    let addresses: Vec<&str> = stream.addresses().collect();
    assert_eq!(
        addresses,
        ["https://tv.example/hd.m3u8", "https://tv.example/sd.m3u8"],
        "the best picture first, the other asked with the same headers next"
    );
    let first = super::super::queue::after_failure(stream, 0).expect("a next address");
    assert_eq!(addresses[first], "https://tv.example/sd.m3u8");
    assert_eq!(
        super::super::queue::after_failure(stream, first),
        None,
        "every address tried: the stream is down"
    );
    let alone = Stream::typed("https://radio.example/one");
    assert_eq!(super::super::queue::after_failure(&alone, 0), None);
}

/// FAILS IF: radio-browser is told a station played before its sound
/// started, more than once for one play, while the radio is not let — or a
/// start after a stop is not a start again; or a directory's stream is not
/// kept among the recently played.
#[test]
fn the_click_is_sent_once_after_the_stream_starts() {
    let mut d = Directories::open();
    let row = station(UUID_A, "Bossa", Some(BR), None).stream();
    let clicks = |owed: &[Fetch]| {
        owed.iter()
            .filter(|f| matches!(f, Fetch::Click { uuid, .. } if uuid == UUID_A))
            .count()
    };
    let recents = |owed: &[Fetch]| {
        owed.iter()
            .filter(|f| matches!(f, Fetch::Recent { pick } if pick.kind == Kind::Radio && pick.id == UUID_A))
            .count()
    };
    let prefs = all_let();
    // PUTTING IT CLICKS NOTHING: only a start is counted, and a start is
    // the `FILE_LOADED` that calls this.
    let owed = d.started(EntryId(7), &row, &prefs, 100);
    assert_eq!((clicks(&owed), recents(&owed)), (1, 1));
    // A RELAY TRIED, a pause, the same entry loaded again: the same play.
    assert!(d.started(EntryId(7), &row, &prefs, 101).is_empty());
    // STOPPED AND PLAYED AGAIN: a start again.
    d.forget_start();
    assert_eq!(clicks(&d.started(EntryId(7), &row, &prefs, 102)), 1);
    // THE RADIO NOT LET: nothing is asked of it; the recent is this
    // machine's own file.
    d.forget_start();
    let owed = d.started(EntryId(7), &row, &Preferences::default(), 103);
    assert_eq!((clicks(&owed), recents(&owed)), (0, 1));
    // AN ADDRESS A PERSON TYPED is owed nothing.
    d.forget_start();
    assert!(
        d.started(
            EntryId(8),
            &Stream::typed("https://x.example/"),
            &prefs,
            104
        )
        .is_empty()
    );
}

/// FAILS IF: anything is asked of a directory the person did not let — the
/// refusal must come before a request, naming the host — or the lives are
/// asked without yt-dlp.
#[test]
fn nothing_is_asked_while_a_switch_is_off() {
    let queries = [
        Query::Radio(RadioList::Countries),
        Query::Radio(RadioList::Country {
            country: BR,
            page: 1,
        }),
        Query::Radio(RadioList::Search {
            words: "jazz".into(),
            page: 1,
        }),
        Query::Tv(TvList::Countries),
    ];
    for query in &queries {
        let net = Canned::new(&[]);
        let (outcome, pages) = looked(
            query,
            &Preferences::default(),
            &net,
            None,
            SystemTime::now(),
        );
        let host = query.directory().expect("a directory").host().to_owned();
        assert_eq!(outcome, Err(Trouble::Off { host }), "{query:?}");
        assert!(net.urls().is_empty(), "{query:?} asked {:?}", net.urls());
        assert!(pages.is_empty(), "{query:?} posted a page");
        // AND THE DESK REFUSES IT AT ONCE, before a thread is asked.
        assert!(matches!(
            admitted(&Preferences::default(), query),
            Err(Trouble::Off { .. })
        ));
    }
    assert!(matches!(
        admitted(
            &all_let(),
            &Query::Radio(RadioList::Search {
                words: "  ".into(),
                page: 1,
            }),
        ),
        Err(Trouble::Directory(_))
    ));
}

const COUNTRIES_JSON: &[u8] = br#"[{"name":"Brazil","iso_3166_1":"BR","stationcount":10},
    {"name":"The United Kingdom","iso_3166_1":"UK","stationcount":3},
    {"name":"Nowhere","iso_3166_1":"ZZ","stationcount":9}]"#;

/// FAILS IF: a kept list is not shown at once, a stale one is not shown
/// while the fresh one is asked — or does not stay, with the refusal, when
/// the directory does not answer; or a fresh one is asked again.
#[test]
fn a_stale_list_is_shown_while_a_fresh_one_is_asked() {
    let cache = cache("stale");
    let query = Query::Radio(RadioList::Countries);
    // NOTHING KEPT: asked — the servers, then the list — and kept.
    let net = Canned::new(&[
        (
            "https://all.api.radio-browser.info/json/servers",
            200,
            br#"[{"name":"de1.api.radio-browser.info"}]"#,
        ),
        (
            "https://de1.api.radio-browser.info/json/countries",
            200,
            COUNTRIES_JSON,
        ),
    ]);
    let (outcome, pages) = looked(&query, &all_let(), &net, Some(&cache), SystemTime::now());
    assert_eq!(outcome, Ok(()));
    assert_eq!(pages.len(), 1);
    let Page::RadioCountries {
        countries,
        globe,
        listed,
    } = &pages[0].0
    else {
        panic!("{pages:?}");
    };
    assert!(!listed.stale);
    assert_eq!(countries[0].country, BR);
    assert_eq!(
        countries.iter().find(|c| c.country == GB).map(|c| c.count),
        Some(3),
        "UK is GB"
    );
    assert_eq!(globe.marks.len(), 2);
    // FRESH: shown at once, and nothing asked.
    let silent = Canned::new(&[]);
    let (outcome, pages) = looked(&query, &all_let(), &silent, Some(&cache), SystemTime::now());
    assert_eq!(outcome, Ok(()));
    assert_eq!(pages.len(), 1);
    assert!(silent.urls().is_empty(), "{:?}", silent.urls());
    // A DAY LATER, and the directory gone: the stale list is shown, and
    // stays, with the refusal.
    let later = SystemTime::now() + Duration::from_secs(24 * 3600);
    let gone = Canned::new(&[]);
    let (outcome, pages) = looked(&query, &all_let(), &gone, Some(&cache), later);
    assert_eq!(
        outcome,
        Err(Trouble::Directory(Refusal::new(
            Source::Radio,
            Why::Unreachable
        )))
    );
    assert_eq!(pages.len(), 1);
    assert!(matches!(&pages[0].0, Page::RadioCountries { listed, .. } if listed.stale));
    assert!(!gone.urls().is_empty(), "a fresh one was asked");
    let _ = std::fs::remove_dir_all(cache.dir());
}

/// FAILS IF: a header with a comma — 741 of iptv-org's user agents have one
/// — is cut at the comma on its way into mpv's per-file options.
#[test]
fn a_header_with_a_comma_survives_mpvs_option_list() {
    let ua = "Mozilla/5.0 (X11; Linux x86_64, like Gecko)".to_owned();
    let text = super::super::options_text(&[
        ("user-agent", ua.clone()),
        ("network-timeout", "12".to_owned()),
    ]);
    assert_eq!(
        text,
        format!("user-agent=%{}%{ua},network-timeout=%2%12", ua.len())
    );
}

/// FAILS IF: a star pressed on a row stops keeping that row's stream, or
/// finds a station by the id of a channel.
#[test]
fn a_favourite_is_the_row_it_was_pressed_on() {
    let mut d = Directories::open();
    let (stations, globe) = stations_globe(vec![station(UUID_A, "Bossa", Some(BR), None)], &map());
    if let Some(slot) = d.slot(DeskKind::Radio) {
        slot.desk.page = Page::Stations {
            stations,
            globe,
            listed: Listed::default(),
        };
    }
    let (title, country, stream) = d.row(Kind::Radio, UUID_A).expect("the row");
    assert_eq!((title.as_str(), country), ("Bossa", Some(BR)));
    assert_eq!(stream.url, "https://radio.example/Bossa");
    assert_eq!(d.row(Kind::Tv, UUID_A), None);
    // A RECENT of a film of the archive played through one of its files is
    // kept WHOLE — its page — so playing it again opens every part.
    let file = Stream {
        url: "https://archive.org/download/Night/a.mp4".into(),
        title: Some("Night — a.mp4".into()),
        referrer: None,
        user_agent: None,
        alternates: Vec::new(),
        origin: Origin::Archive {
            identifier: "Night".into(),
        },
        live: false,
    };
    let pick = d.recent_of(&file, 5).expect("a recent");
    assert_eq!(pick.kind, Kind::Archive);
    assert_eq!(pick.stream.url, "https://archive.org/details/Night");
    assert_eq!(archive::whole_item(&pick.stream), Some("Night"));
}

/// FAILS IF: the state stops listing every desk — the lookups' three, then
/// the directories' five, in `DeskKind`'s order — a door would not know
/// there is a page to ask for.
#[test]
fn the_state_lists_every_desk_in_order() {
    let every: Vec<DeskKind> = super::super::desk::KINDS
        .iter()
        .chain(KINDS.iter())
        .copied()
        .collect();
    assert_eq!(
        every,
        [
            DeskKind::Suggestions,
            DeskKind::Subtitles,
            DeskKind::Lyrics,
            DeskKind::Radio,
            DeskKind::Television,
        ]
    );
    let d = Directories::open();
    let kinds: Vec<DeskKind> = d.summaries().iter().map(|s| s.kind).collect();
    assert_eq!(kinds, KINDS);
    assert!(d.summaries().iter().all(|s| s.generation == 0 && !s.busy));
    assert_eq!(
        d.page(DeskKind::Lyrics),
        None,
        "a lookup's desk is not ours"
    );
}

/// FAILS IF: a language of radio is named by a guess — `pt` is Brazil's and
/// Portugal's, and radio-browser's `portuguese` is neither: it is named as
/// the language, with no region — or `brazilian portuguese` stops being
/// Brazil's, or a language the table has once stops being named by it.
#[test]
fn a_language_of_radio_is_named_only_when_the_table_is_sure() {
    assert_eq!(
        tongue_of("portuguese", Some("pt")).as_deref(),
        Some("pt"),
        "two rows share it: the bare language"
    );
    assert_eq!(
        tongue_of("brazilian portuguese", Some("pt")).as_deref(),
        Some("pt-BR"),
        "its words say the region"
    );
    assert_eq!(
        tongue_of("brazilian portuguese", None).as_deref(),
        Some("pt-BR")
    );
    assert_eq!(tongue_of("english", Some("en")).as_deref(), Some("en"));
    assert_eq!(tongue_of("japanese", Some(" JA ")).as_deref(), Some("ja"));
    assert_eq!(tongue_of("klingon", Some("xx")), None);
    assert_eq!(tongue_of("nothing", Some("")), None);
    assert_eq!(tongue_of("nothing", None), None);
}

/// FAILS IF: a directory refused for want of the person's pass is said as a
/// failure, instead of the consent row whose host is the one that
/// directory's row lets (so its press lets it) — or another refusal is.
#[test]
fn a_directory_not_let_is_its_consent_row() {
    for d in [Directory::Radio, Directory::Iptv, Directory::Archive] {
        let source = match d {
            Directory::Radio => Source::Radio,
            Directory::Iptv => Source::Iptv,
            _ => Source::Archive,
        };
        let t = refused(Refusal::new(source, Why::NotLet));
        assert_eq!(
            t,
            Trouble::Off {
                host: d.host().to_owned()
            }
        );
        let Trouble::Off { host } = t else {
            unreachable!()
        };
        assert_eq!(Directory::of_host(&host), Some(d), "its row lets it");
    }
    let other = Refusal::new(Source::Radio, Why::Unreachable);
    assert_eq!(refused(other), Trouble::Directory(other));
}
