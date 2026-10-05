//! The catalogue's ingests against answers written by hand: no test here
//! reaches a host, runs yt-dlp or reads the person's consent.

use std::sync::Mutex;

use ronoctua_broadcast::catalog::{ListQuery, Reader, Scope, Sources};
use ronoctua_broadcast::sources::Address;
use ronoctua_broadcast::{Fetch, Request};
use ronoctua_http::Reply;

use super::*;

/// Answers by address, each given as often as it is asked; every address
/// asked recorded.
#[derive(Default)]
struct Canned {
    answers: Vec<(String, u16, Vec<u8>)>,
    asked: Mutex<Vec<String>>,
}

impl Canned {
    fn new(answers: &[(&str, u16, &[u8])]) -> Self {
        Self {
            answers: answers
                .iter()
                .map(|(u, s, b)| ((*u).to_owned(), *s, b.to_vec()))
                .collect(),
            asked: Mutex::default(),
        }
    }

    fn answer(&self, url: &str) -> Reply {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(url.to_owned());
        let (status, body) = self
            .answers
            .iter()
            .find(|(u, _, _)| url.starts_with(u.as_str()))
            .map_or((404, Vec::new()), |(_, s, b)| (*s, b.clone()));
        Reply {
            status,
            headers: Vec::new(),
            body,
        }
    }

    fn asked(&self) -> Vec<String> {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl Web for Canned {
    fn get(&self, pass: &ronoctua_consent::Pass, fetch: &Fetch) -> Result<Reply, SourceRefused> {
        ask::covered(pass, &fetch.url)?;
        Ok(self.answer(&fetch.url))
    }

    fn get_own(&self, address: &Address, _most: u64) -> Result<Reply, SourceRefused> {
        Ok(self.answer(address.expose()))
    }
}

impl Net for Canned {
    fn send(&self, request: &Request) -> Result<Reply, ronoctua_broadcast::Why> {
        Ok(self.answer(&request.url))
    }
}

/// A directory of its own, empty, under the temporary directory.
fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "ronoctua-catalog-engine-{}-{tag}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn no_lives(_: &str, _: usize) -> Result<Vec<ronoctua_ytdlp::Live>, ronoctua_ytdlp::Trouble> {
    Err(ronoctua_ytdlp::Trouble::Absent)
}

fn no_listing(_: YouTubeList, _: &str) -> Result<(String, Harvest), SourceRefused> {
    Err(SourceRefused::NoYtDlp)
}

/// The tools of a test: `web` for every request, `store` the consent, no
/// cache and no pace.
fn tools<'a>(
    web: &'a Canned,
    pass: &'a dyn Fn(&str) -> Result<ronoctua_consent::Pass, SourceRefused>,
) -> Tools<'a> {
    Tools {
        web,
        net: web,
        pass,
        lives: &no_lives,
        listed: &no_listing,
        cache: None,
        pace: Duration::ZERO,
        now: SystemTime::UNIX_EPOCH + Duration::from_secs(1_790_000_000),
    }
}

/// The catalogue's files in a folder of their own.
fn stores(tag: &str) -> Stores {
    Stores {
        dir: Some(scratch(&format!("stores-{tag}"))),
        writer: Mutex::new(None),
    }
}

/// A person's source of `kind` at `url`.
fn person(id: &str, kind: Kind, url: &str) -> PersonSource {
    PersonSource {
        id: id.to_owned(),
        kind,
        address: Address::new(url),
        title: String::new(),
        adult: false,
        posters: false,
        added_s: 0,
    }
}

const LIST: &[u8] = b"#EXTM3U\n\
#EXTINF:-1 tvg-id=\"news.one\" group-title=\"Noticias\",News One HD\n\
http://iptv.example:8080/live/ana/hunter2/1.ts\n\
#EXTINF:-1 group-title=\"Filmes\",Casablanca (1942)\n\
http://iptv.example:8080/movie/ana/hunter2/2.mp4\n\
#EXTINF:-1 group-title=\"Adultos 18+\",Night\n\
http://iptv.example:8080/live/ana/hunter2/3.ts\n";

/// FAILS IF: a person's list is not ingested whole — its groups under their
/// themes, the adult group in the adult file — or the rows are not the new
/// ones after a second ingest; or a failed ingest takes the rows away.
#[test]
fn a_person_s_list_is_ingested_whole_and_a_failure_keeps_its_rows() {
    let stores = stores("person");
    let web = Canned::new(&[]);
    let pass = |_: &str| Err(SourceRefused::NotLet);
    let kit = tools(&web, &pass);
    let job = Job {
        what: What::Person(person(
            "p1",
            Kind::Playlist,
            "http://iptv.example:8080/get.php?username=ana&password=hunter2",
        )),
        posters: false,
        fetched: Some(LIST.to_vec()),
    };
    let outcome = ingest(&job, &stores, &kit, &AtomicBool::new(false));
    assert_eq!(
        outcome,
        Outcome::Written {
            items: 3,
            title: None
        }
    );
    assert!(
        web.asked().is_empty(),
        "the bytes fetched to add it are used"
    );
    let dir = stores.dir.clone().unwrap();
    let main = Reader::open(&dir, Store::Main).unwrap();
    let themes: Vec<String> = main
        .themes(&Sources::One("p1".into()))
        .unwrap()
        .into_iter()
        .map(|(t, _)| t)
        .collect();
    assert!(themes.contains(&"news".to_owned()), "{themes:?}");
    assert!(themes.contains(&"films".to_owned()), "{themes:?}");
    let page = main
        .list(&ListQuery {
            scope: Scope::AllCategories,
            sources: Sources::One("p1".into()),
            ..ListQuery::default()
        })
        .unwrap();
    assert_eq!(page.total, 2, "the adult group is not in the main file");
    let adult = Reader::open(&dir, Store::Adult).unwrap();
    assert_eq!(
        adult
            .list(&ListQuery {
                sources: Sources::One("p1".into()),
                ..ListQuery::default()
            })
            .unwrap()
            .total,
        1
    );
    // THE ADDRESS IS NOT IN THE SOURCE'S ROW: masked.
    let row = main.sources().unwrap().into_iter().next().unwrap();
    assert_eq!(row.shown, "iptv.example/…");

    // A FAILURE KEEPS THE ROWS, and says why.
    let failing = Job {
        fetched: None,
        ..job
    };
    let outcome = ingest(&failing, &stores, &kit, &AtomicBool::new(false));
    assert_eq!(outcome, Outcome::Refused(SourceRefused::NothingFound));
    let main = Reader::open(&dir, Store::Main).unwrap();
    let row = main.sources().unwrap().into_iter().next().unwrap();
    assert_eq!(row.last_error.as_deref(), Some("nothing-found"));
    assert_eq!(row.items, 2);
    let _ = std::fs::remove_dir_all(dir);
}

/// FAILS IF: a default source is asked without the person's consent for
/// every host it names — nothing may be sent.
#[test]
fn a_default_source_not_let_asks_nothing() {
    let stores = stores("not-let");
    let web = Canned::new(&[]);
    let pass = |_: &str| Err(SourceRefused::NotLet);
    let kit = tools(&web, &pass);
    for d in table::DEFAULT_SOURCES {
        let job = Job {
            what: What::Default(d),
            posters: false,
            fetched: None,
        };
        assert_eq!(
            ingest(&job, &stores, &kit, &AtomicBool::new(false)),
            Outcome::Refused(SourceRefused::NotLet),
            "{}",
            d.id
        );
    }
    assert!(web.asked().is_empty(), "{:?}", web.asked());
    let _ = std::fs::remove_dir_all(stores.dir.unwrap());
}

/// FAILS IF: a page that fails after others answered takes the whole ingest
/// down, or a host that asked to be asked less is asked again for the next
/// category — or an ingest that got nothing at all is not the first refusal.
#[test]
fn a_default_source_s_categories_fail_apart() {
    let nasa_source = DefaultSource::of("nasa").expect("the NASA source");
    let first = nasa_source.natives[0].ask;
    let page_of = |id: &str| {
        format!(
            r#"{{"collection":{{"items":[{{"href":"https://images-assets.nasa.gov/video/{id}/collection.json",
            "data":[{{"nasa_id":"{id}","title":"Video {id}","media_type":"video","date_created":"2020-01-01T00:00:00Z"}}],
            "links":[{{"href":"https://images-assets.nasa.gov/video/{id}/{id}~thumb.jpg","rel":"preview"}}]}}],
            "metadata":{{"total_hits":1}}}}}}"#
        )
    };
    let body = page_of("apollo11");
    let first_address = nasa::search_address(first, 1, nasa::PAGE_MOST).unwrap();
    let web = Canned::new(&[(&first_address, 200, body.as_bytes())]);
    // A CONSENT FILE OF THE TEST'S OWN, letting the source's hosts.
    let consented = scratch("consent-nasa");
    let store = ronoctua_consent::store_at(Some(consented.clone()));
    ronoctua_consent::let_hosts_in(&store, nasa_source.hosts, true).unwrap();
    let pass = |h: &str| ronoctua_consent::pass_in(&store, h).map_err(|_| SourceRefused::NotLet);
    let kit = tools(&web, &pass);
    let h = default_harvest(nasa_source, &kit).expect("one category answered");
    assert_eq!(h.items.len(), 1);
    assert!(h.cut, "the others failed: the harvest says it is not whole");
    // EVERY CATEGORY WAS ASKED: a 404 is no reason to stop.
    assert_eq!(web.asked().len(), nasa_source.natives.len());

    // A 429 STOPS THE ASKING at the category it came from.
    let busy = Canned::new(&[("https://images-api.nasa.gov/", 429, b"")]);
    let kit = tools(&busy, &pass);
    assert_eq!(
        default_harvest(nasa_source, &kit).unwrap_err(),
        SourceRefused::TooManyAsks
    );
    assert_eq!(busy.asked().len(), 1);
    let _ = std::fs::remove_dir_all(consented);
}

/// FAILS IF: paging does not stop where the source says it has no more, or
/// goes past the table's bound for one category.
#[test]
fn a_category_is_paged_to_its_end_and_never_past_the_bound() {
    let web = Canned::new(&[]);
    let pass = |_: &str| Err(SourceRefused::NotLet);
    let kit = tools(&web, &pass);
    let one = |n: usize| catalog::NewItem {
        ext_id: n.to_string(),
        kind: catalog::ItemKind::Video,
        title: format!("Item {n}"),
        also: Vec::new(),
        description: None,
        country: None,
        language: None,
        height: None,
        duration_s: None,
        year: None,
        licence: None,
        popularity: 0,
        published_s: None,
        poster_url: None,
        stream: ronoctua_player::own::Stream::typed(format!("https://x.example/{n}")),
        categories: Vec::new(),
    };
    // THREE PAGES, the third says it is the last.
    let mut h = Harvest::default();
    let mut asked = 0;
    paged(&kit, &mut h, |page, h| {
        asked += 1;
        h.items.push(one(page as usize));
        Ok(page < 3)
    })
    .unwrap();
    assert_eq!((asked, h.items.len(), h.cut), (3, 3, false));
    // A SOURCE THAT NEVER ENDS stops at the bound, and says it was cut.
    let mut h = Harvest::default();
    paged(&kit, &mut h, |_, h| {
        let n = h.items.len();
        for k in 0..table::PAGE_ROWS as usize {
            h.items.push(one(n + k));
        }
        Ok(true)
    })
    .unwrap();
    assert_eq!(h.items.len(), table::NATIVE_ITEMS_MOST);
    assert!(h.cut);
    // A PAGE THAT FAILS AFTER THE FIRST keeps what came, cut.
    let mut h = Harvest::default();
    paged(&kit, &mut h, |page, h| {
        if page == 2 {
            return Err(SourceRefused::Unreachable);
        }
        h.items.push(one(1));
        Ok(true)
    })
    .unwrap();
    assert_eq!((h.items.len(), h.cut), (1, true));
    // THE FIRST PAGE FAILING is the category's refusal.
    let mut h = Harvest::default();
    assert_eq!(
        paged(&kit, &mut h, |_, _| Err(SourceRefused::Declined)),
        Err(SourceRefused::Declined)
    );
}

/// FAILS IF: a pasted link is taken without being recognised, an address
/// that is no address reaches the network, or a person's `http` list is
/// refused instead of fetched (the owner's decision).
#[test]
fn a_pasted_link_is_recognised_before_it_is_kept() {
    let web = Canned::new(&[
        ("http://iptv.example/get.php", 200, LIST),
        (
            "https://feed.example/podcast.xml",
            200,
            b"<?xml version=\"1.0\"?><rss version=\"2.0\"><channel><title>A</title></channel></rss>",
        ),
        ("https://page.example/", 200, b"<!doctype html><html></html>"),
    ]);
    let (sniffed, bytes) = recognise(
        &web,
        &Address::new("http://iptv.example/get.php?username=ana&password=hunter2&type=m3u"),
    )
    .expect("an http list is taken");
    assert_eq!(sniffed, Sniffed::Playlist);
    assert_eq!(bytes.as_deref(), Some(LIST));
    assert_eq!(
        recognise(&web, &Address::new("https://feed.example/podcast.xml"))
            .unwrap()
            .0,
        Sniffed::Feed
    );
    assert_eq!(
        recognise(&web, &Address::new("https://page.example/")).unwrap_err(),
        SourceRefused::Unknown
    );
    // KNOWN BY ITS ADDRESS: nothing asked.
    let before = web.asked().len();
    assert_eq!(
        recognise(&web, &Address::new("https://archive.org/details/prelinger"))
            .unwrap()
            .0,
        Sniffed::Collection {
            identifier: "prelinger".into()
        }
    );
    assert_eq!(
        recognise(
            &web,
            &Address::new("https://www.youtube.com/watch?v=dQw4w9WgXcQ")
        )
        .unwrap_err(),
        SourceRefused::NotAList
    );
    // NOT AN ADDRESS: refused before anything is sent.
    for bad in [
        "iptv.example/list.m3u",
        "ftp://a.example/l.m3u",
        "http://ana/w:pw@b.example/l",
    ] {
        assert_eq!(
            recognise(&web, &Address::new(bad)).unwrap_err(),
            SourceRefused::NotAnAddress,
            "{bad}"
        );
    }
    assert_eq!(web.asked().len(), before, "{:?}", web.asked());
}

/// FAILS IF: the sources the state says are not every default then the
/// person's, a person's address is not masked, an `http` one is not said to
/// be in the clear, or what a status knew (busy, its items) is lost when the
/// book changes.
#[test]
fn every_source_is_stated_masked_and_its_status_kept() {
    let kept = Kept {
        sources: vec![
            person(
                "p1",
                Kind::Playlist,
                "http://iptv.example:8080/get.php?username=ana&password=hunter2",
            ),
            person("p2", Kind::Feed, "https://feed.example/podcast.xml"),
        ],
        default_posters: BTreeMap::from([("lives".to_owned(), true)]),
    };
    let first = statuses_of(&kept, &[]);
    assert_eq!(first.len(), table::DEFAULT_SOURCES.len() + 2);
    for (s, d) in first.iter().zip(table::DEFAULT_SOURCES) {
        assert_eq!(s.id.0, d.id);
        assert!(!s.plain && !s.adult);
    }
    let lives = first.iter().find(|s| s.id.0 == "lives").unwrap();
    assert!(lives.posters, "the person's switch, not the table's");
    let p1 = first.iter().find(|s| s.id.0 == "p1").unwrap();
    assert_eq!(p1.shown, "iptv.example/…");
    assert!(p1.plain);
    let printed = format!("{first:?}");
    assert!(
        !printed.contains("hunter2") && !printed.contains("ana"),
        "{printed}"
    );
    assert!(!first.iter().find(|s| s.id.0 == "p2").unwrap().plain);

    let mut before = first.clone();
    before[0].busy = true;
    before[0].items = 9;
    let again = statuses_of(&kept, &before);
    assert!(again[0].busy);
    assert_eq!(again[0].items, 9);
}

/// FAILS IF: a poster is asked of a host its source does not name, or a
/// default source's is asked in the clear — only a person's pasted list may
/// be (the owner's decision), and only of that list's own host: never of
/// another the list names, a machine on the person's network among them —
/// or a person's `http` poster from its list's host stops coming.
#[test]
fn a_poster_is_asked_where_its_source_says_and_in_the_clear_only_for_a_person() {
    let hosts = &Rule::Hosts(&["archive.org"]);
    assert!(poster_allowed(
        hosts,
        "https://archive.org/services/img/x",
        "archive.org"
    ));
    assert!(poster_allowed(
        hosts,
        "https://ia8.us.archive.org/x.jpg",
        "ia8.us.archive.org"
    ));
    assert!(!poster_allowed(
        hosts,
        "https://evil.example/archive.org.jpg",
        "evil.example"
    ));
    assert!(!poster_allowed(
        hosts,
        "https://notarchive.org/x.jpg",
        "notarchive.org"
    ));
    let default = &Rule::Anywhere { clear_from: None };
    assert!(poster_allowed(
        default,
        "https://i.imgur.com/logo.png",
        "i.imgur.com"
    ));
    assert!(!poster_allowed(
        default,
        "http://logos.example/one.png",
        "logos.example"
    ));
    // A PERSON'S `http` LIST: its own host in the clear, and no other — a
    // list that names a machine on the person's network gets nothing asked
    // of it in the clear.
    let person = &Rule::Anywhere {
        clear_from: plain_host("http://iptv.example:8080/get.php?username=a&password=b"),
    };
    assert!(poster_allowed(
        person,
        "http://iptv.example:8080/logos/one.png",
        "iptv.example"
    ));
    for (url, host) in [
        ("http://logos.example/one.png", "logos.example"),
        ("http://192.168.1.1/cgi-bin/reboot", "192.168.1.1"),
        ("http://localhost:8080/x.png", "localhost"),
    ] {
        assert!(!poster_allowed(person, url, host), "{url}");
    }
    assert!(poster_allowed(
        person,
        "https://logos.example/one.png",
        "logos.example"
    ));
    // AN `https` LIST'S posters are never asked in the clear.
    assert_eq!(plain_host("https://iptv.example/l.m3u"), None);
    let secure = &Rule::Anywhere {
        clear_from: plain_host("https://iptv.example/l.m3u"),
    };
    assert!(!poster_allowed(
        secure,
        "http://iptv.example/one.png",
        "iptv.example"
    ));
}

#[test]
fn only_a_picture_is_kept_as_a_poster() {
    assert!(looks_like_a_picture(b"\x89PNG\r\n\x1a\n...."));
    assert!(looks_like_a_picture(&[0xff, 0xd8, 0xff, 0xe0]));
    assert!(looks_like_a_picture(b"GIF89a...."));
    assert!(looks_like_a_picture(b"RIFF\0\0\0\0WEBPVP8 "));
    assert!(!looks_like_a_picture(b"<!doctype html>"));
    assert!(!looks_like_a_picture(b""));
}

/// FAILS IF: a temporary search is kept past the daemon that made it — the
/// writer removes it when it opens.
#[test]
fn a_temporary_search_is_gone_when_the_writer_opens() {
    let stores = stores("search");
    let dir = stores.dir.clone().unwrap();
    let mut h = Harvest::default();
    let key = format!("{SEARCH_PREFIX}robots");
    h.category(&key, "robots", None, false);
    h.items.push(catalog::NewItem {
        ext_id: "Robot_film".into(),
        kind: catalog::ItemKind::Video,
        title: "Robot film".into(),
        also: Vec::new(),
        description: None,
        country: None,
        language: None,
        height: None,
        duration_s: None,
        year: Some(1951),
        licence: None,
        popularity: 3,
        published_s: None,
        poster_url: None,
        stream: ronoctua_player::own::Stream::typed("https://archive.org/details/Robot_film"),
        categories: vec![key.clone()],
    });
    let job = Job {
        what: What::Search("robots".into()),
        posters: false,
        fetched: None,
    };
    stores
        .with_writer(|w| w.replace(&new_source(&job), &h, 1))
        .unwrap();
    let r = Reader::open(&dir, Store::Main).unwrap();
    assert!(r.sources().unwrap().iter().any(|s| s.id == SEARCHED));
    drop(r);
    // A NEW DAEMON'S WRITER.
    let fresh = Stores {
        dir: Some(dir.clone()),
        writer: Mutex::new(None),
    };
    fresh.with_writer(|_| Ok(())).unwrap();
    let r = Reader::open(&dir, Store::Main).unwrap();
    assert!(!r.sources().unwrap().iter().any(|s| s.id == SEARCHED));
    let _ = std::fs::remove_dir_all(dir);
}

/// A catalogue with no book and no folder: `kept`'s sources, stated.
fn a_catalogue(kept: Kept) -> Catalog {
    Catalog {
        stores: Arc::new(Stores {
            dir: None,
            writer: Mutex::new(None),
        }),
        book: None,
        statuses: statuses_of(&kept, &[]),
        kept,
        book_trouble: None,
        generation: 0,
        posters_generation: 0,
        workers: HashMap::new(),
        searched: None,
        posters: None,
        stop: Arc::new(AtomicBool::new(false)),
    }
}

/// A person's list that answers only when the test lets it: what it was
/// asked is told on `started`, and it waits on `go`.
struct Slow {
    started: Mutex<mpsc::Sender<()>>,
    go: Mutex<mpsc::Receiver<()>>,
}

impl Web for Slow {
    fn get(&self, _: &ronoctua_consent::Pass, _: &Fetch) -> Result<Reply, SourceRefused> {
        Err(SourceRefused::Unreachable)
    }

    fn get_own(&self, _: &Address, _most: u64) -> Result<Reply, SourceRefused> {
        let _ = self
            .started
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .send(());
        let _ = self
            .go
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .recv();
        Ok(Reply {
            status: 200,
            headers: Vec::new(),
            body: LIST.to_vec(),
        })
    }
}

/// FAILS IF: a source removed while its list is being read comes back —
/// the ingest under way writing its rows (the adult ones too) after the
/// removal deleted them, with no book entry left to ever clean them up.
#[test]
fn a_source_removed_while_it_is_read_leaves_no_row() {
    let stores = Arc::new(stores("withdrawn"));
    let dir = stores.dir.clone().unwrap();
    let address = "http://iptv.example:8080/get.php?username=ana&password=hunter2";
    let job = || Job {
        what: What::Person(person("p1", Kind::Playlist, address)),
        posters: false,
        fetched: None,
    };
    // READ ONCE: its rows are in both files.
    let canned = Canned::new(&[(address, 200, LIST)]);
    let no_pass = |_: &str| Err(SourceRefused::NotLet);
    let first = ingest(
        &job(),
        &stores,
        &tools(&canned, &no_pass),
        &AtomicBool::new(false),
    );
    assert!(matches!(first, Outcome::Written { .. }), "{first:?}");
    // READ AGAIN, slowly — and its thread is the catalogue's.
    let (started, started_here) = mpsc::channel();
    let (go_there, go) = mpsc::channel();
    let mut catalogue = a_catalogue(Kept::default());
    let flag = Arc::new(AtomicBool::new(false));
    let (tx, _rx) = mpsc::channel();
    catalogue.workers.insert(
        "p1".into(),
        Worker {
            tx,
            withdrawn: flag.clone(),
        },
    );
    let reading = std::thread::spawn({
        let stores = stores.clone();
        let flag = flag.clone();
        move || {
            let slow = Slow {
                started: Mutex::new(started),
                go: Mutex::new(go),
            };
            let none = Canned::new(&[]);
            let no_pass = |_: &str| Err(SourceRefused::NotLet);
            let kit = Tools {
                web: &slow,
                ..tools(&none, &no_pass)
            };
            ingest(&job(), &stores, &kit, &flag)
        }
    });
    started_here.recv().expect("the list was asked");
    // REMOVED MEANWHILE, as `remove_source` removes: the thread withdrawn,
    // then the rows.
    catalogue.withdraw("p1");
    assert!(flag.load(Ordering::SeqCst), "the thread's flag is up");
    assert!(catalogue.workers.is_empty(), "its mailbox is dropped");
    stores.with_writer(|w| w.remove("p1")).unwrap();
    go_there.send(()).unwrap();
    assert_eq!(reading.join().unwrap(), Outcome::Withdrawn);
    for store in Store::ALL {
        let r = Reader::open(&dir, store).unwrap();
        assert!(
            !r.sources().unwrap().iter().any(|s| s.id == "p1"),
            "{store:?} holds the removed source again"
        );
        assert_eq!(
            r.list(&ListQuery {
                sources: Sources::All,
                ..ListQuery::default()
            })
            .unwrap()
            .total,
            0,
            "{store:?}"
        );
    }
    let _ = std::fs::remove_dir_all(dir);
}

/// FAILS IF: the temporary search has no status — a door could not say it
/// is under way or why it failed, and drew either as "nothing matches" — or
/// a search for other words asked meanwhile is dropped, or an older one's
/// ending is said over the newest; or the book changing loses the status; or
/// its posters follow a rule of their own and not the archive's switch.
#[test]
fn a_search_has_a_status_and_only_the_newest_is_said() {
    let mut c = a_catalogue(Kept {
        sources: vec![person(
            "p1",
            Kind::Playlist,
            "http://iptv.example:8080/get.php?username=a&password=b",
        )],
        default_posters: BTreeMap::new(),
    });
    assert!(c.ask_search("robots"));
    let s = c.status_mut(SEARCHED).expect("a status of its own");
    assert!(s.busy && s.trouble.is_none());
    assert_eq!(s.title.as_deref(), Some("robots"));
    assert!(
        !c.ask_search("robots"),
        "the same search under way is that one"
    );
    assert!(c.ask_search("trains"), "another is read after it");
    assert!(
        !c.is_current(Some("robots")),
        "the older one's ending is not said"
    );
    assert!(c.is_current(Some("trains")) && c.is_current(None));
    // THE BOOK CHANGING keeps it, last.
    c.restate();
    let last = c.statuses.last().expect("statuses");
    assert_eq!(last.id.0, SEARCHED);
    assert!(last.busy);
    // ITS POSTERS ARE THE ARCHIVE'S: none while the archive's are off,
    // whatever its own status says; the archive's rule once they are on.
    if let Some(s) = c.status_mut(SEARCHED) {
        s.posters = true;
    }
    if let Some(p) = c.status_mut("p1") {
        p.posters = true;
    }
    if let Some(a) = c.status_mut("archive") {
        a.posters = false;
    }
    let rules = c.poster_rules();
    assert!(!rules.contains_key(SEARCHED), "{rules:?}");
    assert_eq!(
        rules.get("p1"),
        Some(&Rule::Anywhere {
            clear_from: Some("iptv.example".into())
        })
    );
    if let Some(a) = c.status_mut("archive") {
        a.posters = true;
    }
    let rules = c.poster_rules();
    assert_eq!(rules.get(SEARCHED), rules.get("archive"));
}
