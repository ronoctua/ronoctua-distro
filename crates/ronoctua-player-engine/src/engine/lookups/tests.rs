//! Every conversation against answers written by hand: no test here reaches
//! a host, and no key here is anybody's.

use std::sync::{Mutex, PoisonError};

use ronoctua_http::Reply;
use ronoctua_lookup::{
    Net, Refusal, Request, Secret, Service, Why,
    ask::Value,
    found::{Found, Take},
    keys::{Account, Key},
    lrclib::Record,
    subdl::Member,
};
use ronoctua_player::{
    find::Suggester,
    prefs::Preferences,
    tongues::{self, Tongue},
    wire::Trouble,
};
use ronoctua_tags::{Field, Kind};

use super::*;

/// Answers, each given once, to the first request whose address (without
/// its secret query) starts with the one named; every request recorded.
struct Canned {
    answers: Mutex<Vec<(String, u16, Vec<u8>)>>,
    asked: Mutex<Vec<Request>>,
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

    fn asked(&self) -> Vec<Request> {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn urls(&self) -> Vec<String> {
        self.asked().into_iter().map(|r| r.url).collect()
    }
}

impl Net for Canned {
    fn send(&self, request: &Request) -> Result<Reply, Why> {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(request.clone());
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

/// Keys that are nobody's, kept only where a test says.
#[derive(Default)]
struct Given {
    kept: Vec<(Key, &'static str)>,
    account: Option<(&'static str, &'static str)>,
}

impl Keys for Given {
    fn key(&self, key: Key) -> Result<Secret, Refusal> {
        self.kept
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, s)| Secret::new(*s))
            .ok_or_else(|| Refusal::new(key.service(), Why::NoKey))
    }

    fn account(&self) -> Option<Account> {
        self.account.map(|(user, password)| Account {
            user: user.to_owned(),
            password: Secret::new(password),
        })
    }
}

fn let_only(services: &[Service]) -> Preferences {
    let mut p = Preferences::default();
    for s in services {
        p.consent(*s);
    }
    p
}

fn song(tags: &[(&str, &str)]) -> About {
    let tags: Vec<(String, String)> = tags
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
    About {
        location: "/m/Band - Song.flac".into(),
        file: None,
        sheet: Some(ronoctua_tags::stream::sheet(
            "/m/Band - Song.flac",
            Kind::Music,
            &tags,
            Vec::new(),
        )),
        duration_seconds: Some(200.0),
    }
}

fn film(location: &str) -> About {
    About {
        location: location.into(),
        file: None,
        sheet: None,
        duration_seconds: None,
    }
}

const REC: &str = "11111111-1111-4111-8111-111111111111";
const REC2: &str = "33333333-3333-4333-8333-333333333333";
const REL: &str = "22222222-2222-4222-8222-222222222222";

fn search_answer() -> String {
    format!(
        r#"{{"recordings":[
          {{"id":"{REC}","score":100,"title":"Song","length":200000,
            "artist-credit":[{{"name":"Band","joinphrase":""}}],
            "releases":[{{"id":"{REL}","title":"Album","status":"Official"}}]}},
          {{"id":"{REC2}","score":80,"title":"Song (live)","length":260000,
            "artist-credit":[{{"name":"Band","joinphrase":""}}],"releases":[]}}
        ]}}"#
    )
}

fn recording_answer(id: &str) -> String {
    format!(
        r#"{{"id":"{id}","title":"Song","length":200000,
           "artist-credit":[{{"name":"Band","joinphrase":""}}],
           "isrcs":["GBAAA0500001"],
           "releases":[{{"id":"{REL}","title":"Album","status":"Official"}}]}}"#
    )
}

fn release_answer() -> String {
    format!(
        r#"{{"id":"{REL}","title":"Album","date":"2005-09-12",
           "artist-credit":[{{"name":"Band","joinphrase":""}}],
           "label-info":[{{"label":{{"name":"EMI"}}}}],
           "cover-art-archive":{{"front":true}},
           "media":[{{"position":1,"track-count":10,"tracks":[{{"position":3,"recording":{{"id":"{REC}"}}}}]}}]}}"#
    )
}

/// FAILS IF: a song stops being searched by its own tags — title, artist,
/// album and length —, or the first candidates stop being read whole (their
/// release: album, track of total, label, cover), or the rest are.
#[test]
fn a_song_is_searched_by_its_tags_and_the_first_few_are_read_whole() {
    let search = search_answer();
    let recording = recording_answer(REC);
    let recording2 = recording_answer(REC2);
    let release = release_answer();
    let net = Canned::new(&[
        (
            "https://musicbrainz.org/ws/2/recording?query=",
            200,
            search.as_bytes(),
        ),
        (
            &format!("https://musicbrainz.org/ws/2/recording/{REC}"),
            200,
            recording.as_bytes(),
        ),
        (
            &format!("https://musicbrainz.org/ws/2/release/{REL}"),
            200,
            release.as_bytes(),
        ),
        (
            &format!("https://musicbrainz.org/ws/2/recording/{REC2}"),
            200,
            recording2.as_bytes(),
        ),
        (
            &format!("https://musicbrainz.org/ws/2/release/{REL}"),
            200,
            release.as_bytes(),
        ),
    ]);
    let about = song(&[("title", "Song"), ("artist", "Band"), ("album", "Album")]);
    let mut steps = Vec::new();
    let asked = suggest(
        &net,
        &Given::default(),
        Suggester::MusicBrainz,
        None,
        &[],
        &about,
        &|| true,
        &mut |s| steps.push(s),
    )
    .expect("found");
    assert_eq!(asked.words, "Song");
    assert_eq!(asked.artist.as_deref(), Some("Band"));
    let urls = net.urls();
    let query = &urls[0];
    assert!(query.contains("recording%3A%22Song%22"), "{query}");
    assert!(query.contains("artist%3A%22Band%22"), "{query}");
    assert!(query.contains("release%3A%22Album%22"), "{query}");
    assert!(
        query.contains("dur%3A%5B198000%20TO%20202000%5D"),
        "{query}"
    );
    let Step::List(list, complete) = &steps[0] else {
        panic!("{steps:?}");
    };
    assert_eq!(list.len(), 2);
    assert!(!complete, "a search's candidates are read whole afterwards");
    let Step::Detail(0, first) = &steps[1] else {
        panic!("{steps:?}");
    };
    assert_eq!(first.value(Field::Track), Some("3/10"));
    assert_eq!(first.value(Field::Publisher), Some("EMI"));
    assert_eq!(first.value(Field::Isrc), Some("GBAAA0500001"));
    assert!(first.cover.is_some(), "the release's front cover");
    assert_eq!(steps.len(), 3, "two candidates, both read: {steps:?}");
    assert!(
        net.asked()
            .iter()
            .all(|r| r.host == ronoctua_lookup::Host::MusicBrainz),
        "one host, one pace"
    );
}

/// FAILS IF: a newer search does not stop the details of the one it
/// replaced — each is a MusicBrainz second somebody else is waiting for.
#[test]
fn a_replaced_search_reads_no_details() {
    let search = search_answer();
    let net = Canned::new(&[(
        "https://musicbrainz.org/ws/2/recording?query=",
        200,
        search.as_bytes(),
    )]);
    let about = song(&[("title", "Song")]);
    let mut steps = Vec::new();
    suggest(
        &net,
        &Given::default(),
        Suggester::MusicBrainz,
        None,
        &[],
        &about,
        &|| false,
        &mut |s| steps.push(s),
    )
    .expect("found");
    assert_eq!(steps.len(), 1, "{steps:?}");
    assert_eq!(net.asked().len(), 1);
}

/// FAILS IF: a file that names its MusicBrainz recording is searched
/// instead of asked by it — or a name that is not a recording (a release's
/// track id) is asked as one.
#[test]
fn a_file_that_names_its_recording_is_asked_by_it() {
    let recording = recording_answer(REC);
    let release = release_answer();
    let net = Canned::new(&[
        (
            &format!("https://musicbrainz.org/ws/2/recording/{REC}"),
            200,
            recording.as_bytes(),
        ),
        (
            &format!("https://musicbrainz.org/ws/2/release/{REL}"),
            200,
            release.as_bytes(),
        ),
    ]);
    let about = song(&[
        ("title", "Wrong title"),
        ("MUSICBRAINZ_TRACKID", &REC.to_uppercase()),
        ("MUSICBRAINZ_RELEASETRACKID", REC2),
    ]);
    let mut steps = Vec::new();
    let asked = suggest(
        &net,
        &Given::default(),
        Suggester::MusicBrainz,
        None,
        &[],
        &about,
        &|| true,
        &mut |s| steps.push(s),
    )
    .expect("found");
    assert!(asked.by_id);
    assert_eq!(net.asked().len(), 2, "{:?}", net.urls());
    assert!(
        net.urls().iter().all(|u| !u.contains("query=")),
        "never searched"
    );
    let Step::List(list, true) = &steps[0] else {
        panic!("{steps:?}");
    };
    assert_eq!(list[0].value(Field::Album), Some("Album"));
}

/// FAILS IF: TMDB is asked without the person's token — or anything at all
/// is sent before the refusal.
#[test]
fn a_keyed_service_without_its_key_sends_nothing() {
    let net = Canned::new(&[]);
    let refused = suggest(
        &net,
        &Given::default(),
        Suggester::Tmdb,
        Some("Amelie"),
        &[],
        &film("/v/Amelie.2001.mkv"),
        &|| true,
        &mut |_| {},
    )
    .unwrap_err();
    assert_eq!(
        refused,
        Trouble::Lookup(Refusal::new(Service::Tmdb, Why::NoKey))
    );
    assert!(net.asked().is_empty());
}

/// FAILS IF: a film's name stops being read for its title and year, or
/// Wikidata stops being asked in the person's language first.
#[test]
fn a_film_is_asked_by_what_its_name_says() {
    let net = Canned::new(&[
        (
            "https://www.wikidata.org/w/api.php",
            200,
            br#"{"search":[{"id":"Q484048","label":"Amelie"}]}"#,
        ),
        (
            "https://query.wikidata.org/",
            200,
            br#"{"results":{"bindings":[{"item":{"value":"http://www.wikidata.org/entity/Q484048"},
              "itemLabel":{"value":"O Fabuloso Destino de Amelie Poulain"},
              "kinds":{"value":"http://www.wikidata.org/entity/Q11424"},
              "date":{"value":"2001-04-25T00:00:00Z|11"}}]}}"#,
        ),
    ]);
    let br: Vec<&Tongue> = tongues::default_for("pt-BR");
    let mut steps = Vec::new();
    let asked = suggest(
        &net,
        &Given::default(),
        Suggester::Wikidata,
        None,
        &br,
        &film("/v/Amelie.2001.1080p.BluRay.mkv"),
        &|| true,
        &mut |s| steps.push(s),
    );
    let urls = net.urls();
    assert!(urls[0].contains("search=Amelie&"), "{urls:?}");
    assert!(urls[0].contains("language=pt-br"), "{urls:?}");
    // THE ANSWER'S SHAPE is the lookup crate's to read (and its tests'); here
    // what matters is what was asked, and that an answer became candidates or
    // a refusal — never a panic.
    match asked {
        Ok(a) => {
            assert_eq!(a.words, "Amelie");
            assert!(matches!(steps.first(), Some(Step::List(_, true))));
        }
        Err(t) => assert!(matches!(t, Trouble::Lookup(_)), "{t:?}"),
    }
}

fn os_search_answer(file_id: u64, language: &str) -> String {
    format!(
        r#"{{"data":[{{"attributes":{{"language":"{language}","download_count":10,
            "release":"Film.2001","moviehash_match":false,
            "feature_details":{{"title":"Film","year":2001}},
            "files":[{{"file_id":{file_id},"file_name":"film.srt"}}]}}}}]}}"#
    )
}

/// FAILS IF: a subtitle search asks a service the person did not let, or
/// stops saying so — naming its host — while it asks the one they did.
#[test]
fn subtitles_ask_every_service_let_and_say_the_others() {
    let answer = os_search_answer(7, "pt-BR");
    let net = Canned::new(&[(
        "https://api.opensubtitles.com/api/v1/subtitles?",
        200,
        answer.as_bytes(),
    )]);
    let keys = Given {
        kept: vec![(Key::OpenSubtitles, "os-key"), (Key::SubDl, "subdl-key")],
        account: None,
    };
    let prefs = let_only(&[Service::OpenSubtitles]);
    let session = Mutex::new(None);
    let asked_in = tongues::default_for("pt-BR");
    let found = subtitles(
        &net,
        &keys,
        &prefs,
        &session,
        &asked_in,
        None,
        &film("/v/Film.2001.mkv"),
    )
    .expect("one service answered");
    assert_eq!(found.found.len(), 1);
    assert_eq!(
        found.refused,
        [Trouble::Off {
            host: "api.subdl.com".into()
        }]
    );
    let urls = net.urls();
    assert_eq!(urls.len(), 1, "SubDL was not asked: {urls:?}");
    assert!(urls[0].contains("languages=en,pt-br"), "{urls:?}");
    assert!(urls[0].contains("query=film"), "{urls:?}");
    assert!(urls[0].contains("year=2001"), "{urls:?}");
    assert_eq!(found.asked.words, "Film");
}

/// FAILS IF: a subtitle search with no service let — or none with a key —
/// sends anything, or refuses without naming what is missing.
#[test]
fn subtitles_nobody_let_are_refused_with_the_host() {
    let net = Canned::new(&[]);
    let session = Mutex::new(None);
    let about = film("/v/Film.mkv");
    let nobody = subtitles(
        &net,
        &Given::default(),
        &Preferences::default(),
        &session,
        &[],
        None,
        &about,
    )
    .unwrap_err();
    assert_eq!(
        nobody,
        Trouble::Off {
            host: "api.opensubtitles.com".into()
        }
    );
    let no_key = subtitles(
        &net,
        &Given::default(),
        &let_only(&[Service::SubDl]),
        &session,
        &[],
        None,
        &about,
    )
    .unwrap_err();
    assert_eq!(
        no_key,
        Trouble::Lookup(Refusal::new(Service::SubDl, Why::NoKey)),
        "the service the person let, and why it could not be asked"
    );
    assert!(net.asked().is_empty());
}

/// FAILS IF: a kept account stops being logged into — once, and then its
/// token reused —, or the day's downloads left stop being said, or the key
/// alone stops searching when the login is refused.
#[test]
fn an_account_logs_in_once_and_says_the_downloads_left() {
    let answer = os_search_answer(7, "en");
    let net = Canned::new(&[
        (
            "https://api.opensubtitles.com/api/v1/login",
            200,
            br#"{"user":{"allowed_downloads":20},"token":"eyJtoken","status":200}"#,
        ),
        (
            "https://api.opensubtitles.com/api/v1/infos/user",
            200,
            br#"{"data":{"allowed_downloads":20,"remaining_downloads":18}}"#,
        ),
        (
            "https://api.opensubtitles.com/api/v1/subtitles?",
            200,
            answer.as_bytes(),
        ),
        (
            "https://api.opensubtitles.com/api/v1/subtitles?",
            200,
            answer.as_bytes(),
        ),
    ]);
    let keys = Given {
        kept: vec![(Key::OpenSubtitles, "os-key")],
        account: Some(("someone", "hunter2")),
    };
    let prefs = let_only(&[Service::OpenSubtitles]);
    let session = Mutex::new(None);
    let about = film("/v/Film.mkv");
    let first = subtitles(&net, &keys, &prefs, &session, &[], None, &about).expect("found");
    assert_eq!(first.remaining, Some(18));
    let again = subtitles(&net, &keys, &prefs, &session, &[], None, &about).expect("found");
    assert_eq!(again.found.len(), 1);
    let logins = net.urls().iter().filter(|u| u.ends_with("/login")).count();
    assert_eq!(logins, 1, "{:?}", net.urls());
    let searches: Vec<Request> = net
        .asked()
        .into_iter()
        .filter(|r| r.url.contains("/subtitles?"))
        .collect();
    assert!(
        searches.iter().all(|r| r
            .headers
            .iter()
            .any(|(n, v)| *n == "Authorization" && v.expose() == "Bearer eyJtoken")),
        "the token rides on every search"
    );
    assert!(
        net.asked()
            .iter()
            .all(|r| !format!("{r:?}").contains("hunter2") && !format!("{r:?}").contains("os-key")),
        "no secret prints"
    );
    // A LOGIN REFUSED: said, and the key alone still searches.
    let answer = os_search_answer(8, "en");
    let net = Canned::new(&[
        ("https://api.opensubtitles.com/api/v1/login", 401, b"{}"),
        (
            "https://api.opensubtitles.com/api/v1/subtitles?",
            200,
            answer.as_bytes(),
        ),
    ]);
    let session = Mutex::new(None);
    let alone = subtitles(&net, &keys, &prefs, &session, &[], None, &about).expect("found");
    assert_eq!(alone.found.len(), 1);
    assert_eq!(
        alone.refused,
        [
            Trouble::Lookup(Refusal::new(Service::OpenSubtitles, Why::KeyRefused)),
            Trouble::Off {
                host: "api.subdl.com".into()
            },
        ],
        "the login's refusal, and the service not let"
    );
}

/// FAILS IF: a token the service stopped taking is not logged in again —
/// once, and not in a loop.
#[test]
fn a_refused_token_logs_in_again_once() {
    let answer = os_search_answer(7, "en");
    let login = br#"{"user":{},"token":"eyJnew","status":200}"#;
    let net = Canned::new(&[
        (
            "https://api.opensubtitles.com/api/v1/subtitles?",
            401,
            b"{}",
        ),
        ("https://api.opensubtitles.com/api/v1/login", 200, login),
        ("https://api.opensubtitles.com/api/v1/infos/user", 500, b""),
        (
            "https://api.opensubtitles.com/api/v1/subtitles?",
            200,
            answer.as_bytes(),
        ),
    ]);
    let keys = Given {
        kept: vec![(Key::OpenSubtitles, "os-key")],
        account: Some(("someone", "pw")),
    };
    let session = Mutex::new(Some(LoggedIn {
        user: "someone".into(),
        session: ronoctua_lookup::opensubtitles::Session {
            token: Secret::new("eyJold"),
            host: "api.opensubtitles.com".into(),
            allowed_downloads: None,
        },
    }));
    let found = subtitles(
        &net,
        &keys,
        &let_only(&[Service::OpenSubtitles]),
        &session,
        &[],
        None,
        &film("/v/Film.mkv"),
    )
    .expect("found");
    assert_eq!(found.found.len(), 1);
    let urls = net.urls();
    assert_eq!(
        urls.iter().filter(|u| u.ends_with("/login")).count(),
        1,
        "{urls:?}"
    );
}

fn member(name: &str, bytes: &[u8]) -> Member {
    Member {
        name: name.into(),
        bytes: bytes.to_vec(),
    }
}

/// FAILS IF: a season pack stops giving the episode that plays, or a
/// VobSub index is saved without its pictures.
#[test]
fn a_season_pack_gives_this_episode() {
    let pack = vec![
        member("Show.S01E01.srt", b"one"),
        member("Show.S01E02.srt", b"two"),
        member("Show.S01E03.srt", b"three"),
    ];
    let got = chosen(pack.clone(), Some(1), Some(2)).expect("one");
    assert_eq!(got.name, "Show.S01E02.srt");
    let got = chosen(pack, None, None).expect("one");
    assert_eq!(got.name, "Show.S01E01.srt", "no episode known: the first");
    let vobsub = vec![
        member("film.sub", b"\x00\x00\x01\xba"),
        member("film.idx", b"# VobSub index file, v7"),
    ];
    let got = chosen(vobsub, None, None).expect("one");
    assert_eq!(got.name, "film.idx", "words before pictures");
    assert_eq!(got.companion.map(|(n, _)| n).as_deref(), Some("film.sub"));
    assert!(chosen(Vec::new(), None, None).is_none());
}

fn scratch(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("ronoctua-lookups-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a folder");
    dir
}

/// A SubRip file in Windows-1252, as a subtitle from 2008 often is.
const LATIN: &[u8] = b"1\r\n00:00:01,000 --> 00:00:02,000\r\nGr\xfc\xdfe\r\n";

/// FAILS IF: a fetched subtitle is not saved in UTF-8 as
/// `<film>.<language>.<ext>` beside its film — or a second one overwrites the
/// first, or the same one taken twice is saved twice.
#[test]
fn a_saved_subtitle_is_utf8_named_after_its_film_and_its_language() {
    let dir = scratch("beside");
    let movie = dir.join("Film (2001).mkv");
    std::fs::write(&movie, b"not a film").expect("a film");
    let about = About {
        location: movie.to_string_lossy().into_owned(),
        file: Some(movie.clone()),
        sheet: None,
        duration_seconds: None,
    };
    let fetched = Fetched {
        name: "whatever.SRT".into(),
        bytes: LATIN.to_vec(),
        companion: None,
    };
    let saved = save_subtitle(&about, "pt-br", &fetched, None).expect("saved");
    assert_eq!(saved, dir.join("Film (2001).pt-BR.srt"));
    let text = std::fs::read_to_string(&saved).expect("UTF-8");
    assert!(text.contains("Grüße"), "{text:?}");
    assert!(
        ronoctua_player::subtitles::named_after(&saved.to_string_lossy(), &movie.to_string_lossy()),
        "the film's own name: opening it puts it over the film"
    );
    let again = save_subtitle(&about, "pt-br", &fetched, None).expect("saved");
    assert_eq!(again, saved, "the same subtitle is that file");
    let other = Fetched {
        bytes: b"1\n00:00:01,000 --> 00:00:02,000\nother\n".to_vec(),
        ..fetched
    };
    let second = save_subtitle(&about, "pt-br", &other, None).expect("saved");
    assert_eq!(second, dir.join("Film (2001).pt-BR.2.srt"));
    assert!(
        std::fs::read_to_string(&saved)
            .expect("kept")
            .contains("Grüße"),
        "the first is never written over"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// FAILS IF: a film whose folder cannot be written loses its subtitle —
/// it goes to the player's own folder, named after the film all the same —
/// or a file that is no subtitle is saved as one.
#[test]
fn a_folder_that_takes_nothing_sends_the_subtitle_elsewhere() {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = scratch("readonly");
    let shelf = dir.join("shelf");
    std::fs::create_dir_all(&shelf).expect("a folder");
    let movie = shelf.join("film.mkv");
    std::fs::write(&movie, b"x").expect("a film");
    std::fs::set_permissions(&shelf, std::fs::Permissions::from_mode(0o555)).expect("read only");
    let about = About {
        location: movie.to_string_lossy().into_owned(),
        file: Some(movie.clone()),
        sheet: None,
        duration_seconds: None,
    };
    let fetched = Fetched {
        name: "a.srt".into(),
        bytes: b"1\n00:00:01,000 --> 00:00:02,000\nhi\n".to_vec(),
        companion: None,
    };
    let elsewhere = dir.join("data").join("0123456789abcdef");
    let saved = save_subtitle(&about, "en", &fetched, Some(elsewhere.clone()));
    // A PROCESS THAT WRITES WHERE IT PLEASES (root, in some sandboxes)
    // proves nothing about a folder that refuses: said, and not asserted.
    if std::fs::write(shelf.join("probe"), b"").is_ok() {
        eprintln!("skipped: this process writes into a read-only folder");
    } else {
        assert_eq!(saved.expect("saved"), elsewhere.join("film.en.srt"));
    }
    let junk = Fetched {
        name: "readme.txt".into(),
        bytes: b"this is not a subtitle at all".to_vec(),
        companion: None,
    };
    assert!(matches!(
        save_subtitle(&about, "en", &junk, Some(elsewhere)),
        Err(Trouble::Subtitle { .. })
    ));
    std::fs::set_permissions(&shelf, std::fs::Permissions::from_mode(0o755)).expect("back");
    let _ = std::fs::remove_dir_all(&dir);
}

/// FAILS IF: the player's own folder for subtitles is not one per film, or
/// not under the instance's data root.
#[test]
fn every_film_has_its_own_folder_of_kept_subtitles() {
    let a = kept_subtitles_dir("/v/a.mkv");
    let b = kept_subtitles_dir("/v/b.mkv");
    if let (Some(a), Some(b)) = (a, b) {
        assert_ne!(a, b);
        assert!(a.ends_with(a.file_name().expect("a name")));
        assert!(a.to_string_lossy().contains("/player/subtitles/"), "{a:?}");
        assert_eq!(
            kept_subtitles_dir("/v/a.mkv"),
            Some(a),
            "the same film, the same folder"
        );
    }
}

/// FAILS IF: a language tag the services write is not saved as this
/// system's code, or something that is no tag reaches a file's name.
#[test]
fn a_subtitles_language_is_this_systems_code() {
    assert_eq!(tongue_part("pt-br"), "pt-BR");
    assert_eq!(tongue_part("BR_PT"), "pt-BR");
    assert_eq!(tongue_part("EN"), "en");
    assert_eq!(tongue_part("x/../y"), "xy");
    assert_eq!(tongue_part(""), "und");
}

fn record(synced: Option<&str>, plain: Option<&str>) -> Record {
    Record {
        id: 1,
        track: "Song".into(),
        artist: "Band".into(),
        album: Some("Album".into()),
        duration_seconds: Some(125.4),
        instrumental: false,
        plain: plain.map(str::to_owned),
        synced: synced.map(str::to_owned),
    }
}

/// FAILS IF: a `.lrc` stops carrying its header and the synced words over
/// the plain ones — or an instrumental becomes an empty file.
#[test]
fn an_lrc_carries_its_header_and_the_synced_words() {
    let text = lrc_of(&record(
        Some("[00:01.00]one\n[00:02.00]two"),
        Some("one\ntwo"),
    ))
    .expect("words");
    assert_eq!(
        text,
        "[ar:Band]\n[ti:Song]\n[al:Album]\n[length:2:05]\n[00:01.00]one\n[00:02.00]two\n"
    );
    let lines = ronoctua_player::lyrics::parse(&text);
    assert_eq!(lines.len(), 2, "the header is not sung");
    let plain = lrc_of(&record(None, Some("one\ntwo"))).expect("words");
    assert!(plain.ends_with("one\ntwo\n"));
    let mut quiet = record(Some("x"), None);
    quiet.instrumental = true;
    assert_eq!(lrc_of(&quiet), None);
}

/// FAILS IF: a `.lrc` that is there is replaced without the person saying so
/// — or not replaced when they did.
#[test]
fn lyrics_beside_a_song_replace_only_when_the_person_said_so() {
    let dir = scratch("lyrics");
    let lrc = dir.join("song.lrc");
    save_lyrics(&lrc, "[00:01.00]first\n", false).expect("written");
    assert_eq!(
        save_lyrics(&lrc, "[00:01.00]second\n", false),
        Err(Trouble::Exists {
            path: lrc.to_string_lossy().into_owned()
        })
    );
    assert_eq!(
        std::fs::read_to_string(&lrc).expect("kept"),
        "[00:01.00]first\n"
    );
    save_lyrics(&lrc, "[00:01.00]second\n", true).expect("replaced");
    assert_eq!(
        std::fs::read_to_string(&lrc).expect("new"),
        "[00:01.00]second\n"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// FAILS IF: lyrics stop being searched by the song's own tags, or by the
/// person's words when they typed some.
#[test]
fn lyrics_are_asked_by_the_songs_tags_or_the_words() {
    let answer = br#"[{"id":1,"trackName":"Song","artistName":"Band","duration":200,
        "instrumental":false,"plainLyrics":"la","syncedLyrics":"[00:01.00]la"}]"#;
    let net = Canned::new(&[("https://lrclib.net/api/search?", 200, answer)]);
    let (asked, records) = lyrics(
        &net,
        None,
        None,
        None,
        &song(&[("title", "Song"), ("artist", "Band")]),
    )
    .expect("found");
    assert_eq!(asked.words, "Song");
    assert_eq!(records.len(), 1);
    assert!(
        net.urls()[0].contains("track_name=Song&artist_name=Band"),
        "{:?}",
        net.urls()
    );
    let net = Canned::new(&[("https://lrclib.net/api/search?", 200, b"[]")]);
    assert_eq!(
        lyrics(
            &net,
            Some("la la"),
            None,
            None,
            &film("https://radio.example/live")
        )
        .unwrap_err(),
        Trouble::Lookup(Refusal::new(Service::Lrclib, Why::NothingFound))
    );
    assert!(
        net.urls()[0].ends_with("/api/search?q=la%20la"),
        "{:?}",
        net.urls()
    );
}

/// FAILS IF: a key is tried without the person letting its service be
/// asked, or at the wrong address — or OpenSubtitles' account stops being
/// logged into, saying the downloads left.
#[test]
fn a_key_is_tried_where_the_service_says_and_only_when_let() {
    let keys = Given {
        kept: vec![
            (
                Key::Tmdb,
                "eyJhbGciOiJIUzI1NiJ9.eyJhdWQiOiJ4In0.c2lnbmF0dXJl",
            ),
            (Key::OpenSubtitles, "os-key"),
        ],
        account: Some(("someone", "pw")),
    };
    let session = Mutex::new(None);
    let net = Canned::new(&[]);
    assert_eq!(
        key_check(
            &net,
            &keys,
            &Preferences::default(),
            &session,
            Service::Tmdb
        ),
        Err(Trouble::Off {
            host: "api.themoviedb.org".into()
        })
    );
    assert!(net.asked().is_empty());
    let net = Canned::new(&[(
        "https://api.themoviedb.org/3/authentication",
        200,
        br#"{"success":true}"#,
    )]);
    assert_eq!(
        key_check(
            &net,
            &keys,
            &let_only(&[Service::Tmdb]),
            &session,
            Service::Tmdb
        ),
        Ok(None)
    );
    let net = Canned::new(&[
        (
            "https://api.opensubtitles.com/api/v1/infos/formats",
            200,
            br#"{"data":{}}"#,
        ),
        (
            "https://api.opensubtitles.com/api/v1/login",
            200,
            br#"{"user":{},"token":"eyJtoken","status":200}"#,
        ),
        (
            "https://api.opensubtitles.com/api/v1/infos/user",
            200,
            br#"{"data":{"remaining_downloads":5}}"#,
        ),
    ]);
    assert_eq!(
        key_check(
            &net,
            &keys,
            &let_only(&[Service::OpenSubtitles]),
            &session,
            Service::OpenSubtitles
        ),
        Ok(Some(5))
    );
    let net = Canned::new(&[(
        "https://api.opensubtitles.com/api/v1/infos/formats",
        403,
        b"{}",
    )]);
    assert_eq!(
        key_check(
            &net,
            &keys,
            &let_only(&[Service::OpenSubtitles]),
            &session,
            Service::OpenSubtitles
        ),
        Err(Trouble::Lookup(Refusal::new(
            Service::OpenSubtitles,
            Why::KeyRefused
        )))
    );
    assert_eq!(
        key_check(
            &net,
            &keys,
            &let_only(&[Service::Lrclib]),
            &session,
            Service::Lrclib
        ),
        Err(Trouble::Lookup(Refusal::new(
            Service::Lrclib,
            Why::NothingToAsk
        ))),
        "a keyless service has no key to try"
    );
}

/// FAILS IF: a subtitle is fetched without the person letting its service
/// be asked — or a dormant service is asked at all.
#[test]
fn a_subtitle_is_fetched_only_from_a_service_let() {
    let found = |source: Service, take: Take| Found {
        source,
        take,
        language: "en".into(),
        release: String::new(),
        file_name: None,
        downloads: None,
        hearing_impaired: false,
        fps: None,
        hash_match: false,
        machine_translated: false,
        title: None,
        year: None,
        season: None,
        episode: None,
    };
    let net = Canned::new(&[]);
    let session = Mutex::new(None);
    let about = film("/v/film.mkv");
    let subdl = found(
        Service::SubDl,
        Take::SubDl {
            path: "/subtitle/1-2.zip".into(),
        },
    );
    assert_eq!(
        fetch_subtitle(
            &net,
            &Given::default(),
            &Preferences::default(),
            &session,
            &subdl,
            &about
        )
        .unwrap_err(),
        Trouble::Off {
            host: "api.subdl.com".into()
        }
    );
    let asleep = found(Service::Podnapisi, Take::Podnapisi { id: "x".into() });
    let mut p = Preferences::default();
    p.consent(Service::Podnapisi);
    assert!(fetch_subtitle(&net, &Given::default(), &p, &session, &asleep, &about).is_err());
    assert!(net.asked().is_empty());
    // AND A DOWNLOAD SAYS HOW MANY ARE LEFT.
    let net = Canned::new(&[
        (
            "https://api.opensubtitles.com/api/v1/download",
            200,
            br#"{"link":"https://www.opensubtitles.com/download/x/film.srt","file_name":"film.srt","remaining":4}"#,
        ),
        (
            "https://www.opensubtitles.com/download/x/film.srt",
            200,
            b"1\n00:00:01,000 --> 00:00:02,000\nhi\n",
        ),
    ]);
    let os = found(Service::OpenSubtitles, Take::OpenSubtitles { file_id: 3 });
    let keys = Given {
        kept: vec![(Key::OpenSubtitles, "os-key")],
        account: None,
    };
    let (fetched, left) = fetch_subtitle(
        &net,
        &keys,
        &let_only(&[Service::OpenSubtitles]),
        &session,
        &os,
        &about,
    )
    .expect("fetched");
    assert_eq!(fetched.name, "film.srt");
    assert_eq!(left, Some(4));
    let link = &net.asked()[1];
    assert!(link.headers.is_empty(), "no key goes to the link: {link:?}");
    assert!(matches!(net.asked()[0].body, Some(Value::Plain(_))));
}

/// FAILS IF: a lookup refused for want of the person's pass is said as a
/// failure the person cannot act on, instead of the consent row naming the
/// host that service's row lets — or any other refusal is turned into one.
#[test]
fn a_lookup_not_let_is_its_consent_row() {
    assert_eq!(
        lookup(Refusal::new(Service::OpenSubtitles, Why::NotLet)),
        Trouble::Off {
            host: "api.opensubtitles.com".to_owned()
        }
    );
    assert_eq!(
        lookup(Refusal::new(Service::Lrclib, Why::NotLet)),
        Trouble::Off {
            host: ronoctua_player::prefs::LRCLIB_HOST.to_owned()
        }
    );
    let other = Refusal::new(Service::Lrclib, Why::Unreachable);
    assert_eq!(lookup(other), Trouble::Lookup(other));
}
