use std::os::unix::fs::MetadataExt as _;
use std::time::Instant;

use ronoctua_playlist::own::{Origin, Stream};

use super::*;
use crate::catalog::{NewCategory, fold};

const MIB: u64 = 1 << 20;

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "ronoctua-broadcast-catalog-{}-{tag}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn source(id: &str, adult: bool) -> NewSource {
    NewSource {
        id: id.to_owned(),
        kind: SourceKind::Playlist,
        title: "My provider".to_owned(),
        shown: "iptv.example/…".to_owned(),
        adult,
        posters_on: false,
    }
}

fn category(key: &str, theme: Option<&str>, adult: bool) -> NewCategory {
    NewCategory {
        key: key.to_owned(),
        title: key.to_owned(),
        theme: theme.map(ToOwned::to_owned),
        adult,
    }
}

fn item(ext_id: &str, title: &str, categories: &[&str]) -> NewItem {
    NewItem {
        ext_id: ext_id.to_owned(),
        kind: ItemKind::Live,
        title: title.to_owned(),
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
        stream: Stream {
            url: format!("https://s.example/{ext_id}.m3u8"),
            title: Some(title.to_owned()),
            referrer: None,
            user_agent: None,
            alternates: Vec::new(),
            origin: Origin::Typed,
            live: true,
        },
        categories: categories.iter().map(|c| (*c).to_owned()).collect(),
    }
}

/// A list in the shape a person's IPTV provider gives: news, films, an adult
/// group, a channel in two groups, one channel written twice.
fn provider() -> Harvest {
    let mut bbc = item("bbc", "BBC News", &["News"]);
    bbc.country = Some("GB".to_owned());
    bbc.language = Some("English".to_owned());
    bbc.height = Some(1080);
    bbc.popularity = 50;
    let mut bbc_backup = item("bbc", "BBC News backup", &["News"]);
    bbc_backup.stream.url = "https://backup.example/bbc.m3u8".to_owned();
    // pt-BR: a title and a description whose accents are the subject.
    let mut accented = item("accented", "Ação Total", &["Filmes"]);
    accented.kind = ItemKind::Video;
    accented.description = Some("Um filme de música ao vivo".to_owned());
    accented.year = Some(1968);
    accented.published_s = Some(-50_000_000);
    let mut nandu = item("nandu", "Ñandú TV", &["Filmes"]);
    nandu.popularity = 10;
    nandu.height = Some(480);
    nandu.published_s = Some(1_700_000_000);
    Harvest {
        categories: vec![
            category("News", Some("news"), false),
            category("Filmes", Some("films"), false),
            category("Adultos 18+", None, true),
        ],
        items: vec![
            bbc,
            item("emile", "Émile's Show", &["News"]),
            nandu,
            accented,
            item("24h", "24 Horas", &["News"]),
            item("late", "Late", &["Adultos 18+"]),
            item("both", "Both", &["News", "Adultos 18+"]),
            bbc_backup,
        ],
        cut: false,
    }
}

fn titles(page: &ListPage) -> Vec<&str> {
    page.rows.iter().map(|r| r.title.as_str()).collect()
}

fn in_category(source: &str, key: &str) -> ListQuery {
    ListQuery {
        scope: Scope::Category(CategoryId::of(source, key)),
        ..ListQuery::default()
    }
}

/// FAILS IF: a listing stops answering what a door asks — a category by
/// title, a letter (É with E, Ñ its own), a theme, words with and without
/// their accents, a country, a height, an order, a page — or an adult group
/// shows in the main file.
#[test]
fn a_source_is_written_whole_and_every_filter_answers() {
    let dir = scratch("filters");
    let mut w = Writer::open(&dir, 64 * MIB, 1_000).unwrap();
    let kept = w.replace(&source("p1", false), &provider(), 2_000).unwrap();
    assert_eq!(
        kept,
        Replaced { main: 5, adult: 2 },
        "bbc twice is one; `both` is adult"
    );

    let r = Reader::open(&dir, Store::Main).unwrap();
    assert_eq!(
        r.themes(&Sources::One("p1".to_owned())).unwrap(),
        [("films".to_owned(), 2), ("news".to_owned(), 3)]
    );
    let sources = r.sources().unwrap();
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].items, 5);
    assert_eq!(sources[0].last_ingest_s, Some(2_000));
    assert_eq!(sources[0].shown, "iptv.example/…");
    let categories = r.categories(&Sources::All).unwrap();
    let names: Vec<&str> = categories.iter().map(|c| c.key.as_str()).collect();
    assert_eq!(names, ["Filmes", "News"], "the adult group is not here");
    assert_eq!(categories[1].items, 3);

    // A CATEGORY, BY TITLE: É sorts with E, a digit first.
    let news = r.list(&in_category("p1", "News")).unwrap();
    assert_eq!(titles(&news), ["24 Horas", "BBC News", "Émile's Show"]);
    assert_eq!((news.total, news.page, news.pages), (3, 1, 1));
    assert_eq!(
        r.letters(&in_category("p1", "News")).unwrap(),
        ['#', 'B', 'E']
    );
    let e = r
        .list(&ListQuery {
            letter: Some('E'),
            ..in_category("p1", "News")
        })
        .unwrap();
    assert_eq!(titles(&e), ["Émile's Show"]);

    // EVERY ITEM: Ñ is a letter of its own, and sorts with N.
    let all = r.list(&ListQuery::default()).unwrap();
    assert_eq!(
        titles(&all),
        [
            "24 Horas",
            "Ação Total",
            "BBC News",
            "Émile's Show",
            "Ñandú TV"
        ]
    );
    assert_eq!(
        r.letters(&ListQuery::default()).unwrap(),
        ['#', 'A', 'B', 'E', 'Ñ']
    );

    // A THEME.
    let films = r
        .list(&ListQuery {
            scope: Scope::Theme("films".to_owned()),
            ..ListQuery::default()
        })
        .unwrap();
    assert_eq!(titles(&films), ["Ação Total", "Ñandú TV"]);
    let themes = r.themes(&Sources::All).unwrap();
    assert!(themes.contains(&("news".to_owned(), 3)));
    assert!(themes.contains(&("films".to_owned(), 2)));

    // WORDS, with and without their accents, in the title or the description.
    // pt-BR: words with and without their accents are the subject.
    for words in ["musica", "música", "MÚSICA", "acao", "ação total", "tot"] {
        let found = r
            .list(&ListQuery {
                words: Some(words.to_owned()),
                ..ListQuery::default()
            })
            .unwrap();
        assert_eq!(titles(&found), ["Ação Total"], "{words}");
    }
    let nandu = r
        .list(&ListQuery {
            words: Some("nandu".to_owned()),
            ..ListQuery::default()
        })
        .unwrap();
    assert_eq!(titles(&nandu), ["Ñandú TV"]);
    let none = r
        .list(&ListQuery {
            words: Some("\"NEAR(".to_owned()),
            ..ListQuery::default()
        })
        .unwrap();
    assert_eq!(none.total, 0, "syntax is a word, and finds nothing");

    // A COUNTRY, A HEIGHT, AN ORDER.
    assert_eq!(
        r.countries(&ListQuery::default()).unwrap(),
        [("GB".to_owned(), 1)]
    );
    assert_eq!(
        r.languages(&ListQuery::default()).unwrap(),
        [("English".to_owned(), 1)]
    );
    let tall = r
        .list(&ListQuery {
            min_height: Some(720),
            ..ListQuery::default()
        })
        .unwrap();
    assert_eq!(titles(&tall), ["BBC News"]);
    let popular = r
        .list(&ListQuery {
            sort: Sort::Popular,
            ..ListQuery::default()
        })
        .unwrap();
    assert_eq!(titles(&popular)[..2], ["BBC News", "Ñandú TV"]);
    let newest = r
        .list(&ListQuery {
            sort: Sort::Newest,
            ..ListQuery::default()
        })
        .unwrap();
    assert_eq!(titles(&newest)[..2], ["Ñandú TV", "Ação Total"]);
    let oldest = r
        .list(&ListQuery {
            sort: Sort::Oldest,
            ..ListQuery::default()
        })
        .unwrap();
    assert_eq!(titles(&oldest)[0], "Ação Total", "the only year");

    // A PAGE PAST THE LAST is the last.
    let last = r
        .list(&ListQuery {
            per_page: 2,
            page: 9,
            ..ListQuery::default()
        })
        .unwrap();
    assert_eq!((last.page, last.pages, last.total), (3, 3, 5));
    assert_eq!(titles(&last), ["Ñandú TV"]);

    // AN ITEM, WHOLE: the second line of a channel is its alternate.
    let bbc = r.item(ItemId::of("p1", "bbc")).unwrap().unwrap();
    assert_eq!(bbc.row.title, "BBC News", "the first line's title");
    assert_eq!(bbc.stream.alternates, ["https://backup.example/bbc.m3u8"]);
    assert_eq!(bbc.categories, [CategoryId::of("p1", "News")]);
    assert_eq!(bbc.row.letter, 'B');
    let accented = r.item(ItemId::of("p1", "accented")).unwrap().unwrap();
    // pt-BR: the description kept is the one written.
    assert_eq!(
        accented.description.as_deref(),
        Some("Um filme de música ao vivo")
    );
    assert_eq!(accented.row.kind, ItemKind::Video);
    assert_eq!(r.item(ItemId(12345)).unwrap(), None);

    // AND THE ADULT FILE holds the adult group, and the channel in both.
    let adult = Reader::open(&dir, Store::Adult).unwrap();
    let a = adult.list(&ListQuery::default()).unwrap();
    assert_eq!(titles(&a), ["Both", "Late"]);
    let adult_categories = adult.categories(&Sources::All).unwrap();
    assert_eq!(adult_categories.len(), 1);
    assert_eq!(adult_categories[0].items, 2);
    assert_eq!(adult.sources().unwrap()[0].items, 2);
    let _ = std::fs::remove_dir_all(dir);
}

/// FAILS IF: an ingest leaves the old rows beside the new, moves an id, or a
/// failure takes the rows away.
#[test]
fn a_source_is_replaced_whole_and_a_failure_keeps_its_rows() {
    let dir = scratch("replace");
    let mut w = Writer::open(&dir, 64 * MIB, 1_000).unwrap();
    w.replace(&source("p1", false), &provider(), 2_000).unwrap();
    let r = Reader::open(&dir, Store::Main).unwrap();
    let before = r.generation().unwrap();
    let smaller = Harvest {
        categories: vec![category("News", Some("news"), false)],
        items: vec![item("bbc", "BBC News", &["News"])],
        cut: true,
    };
    w.replace(&source("p1", false), &smaller, 3_000).unwrap();
    assert!(
        r.generation().unwrap() > before,
        "the reader sees the commit"
    );
    let all = r.list(&ListQuery::default()).unwrap();
    assert_eq!(titles(&all), ["BBC News"]);
    assert_eq!(all.rows[0].id, ItemId::of("p1", "bbc"), "the same id");
    assert!(r.sources().unwrap()[0].cut);
    assert_eq!(r.categories(&Sources::All).unwrap().len(), 1);
    // THE ADULT ROWS OF IT are gone too, and so is its row there.
    let adult = Reader::open(&dir, Store::Adult).unwrap();
    assert_eq!(adult.list(&ListQuery::default()).unwrap().total, 0);
    assert!(adult.sources().unwrap().is_empty());

    w.failed(&source("p1", false), "unreachable").unwrap();
    let s = &r.sources().unwrap()[0];
    assert_eq!(s.last_error.as_deref(), Some("unreachable"));
    assert_eq!(s.last_ingest_s, Some(3_000), "the last success stays");
    assert_eq!(r.list(&ListQuery::default()).unwrap().total, 1, "rows stay");

    // A SECOND SOURCE is apart, and removing one leaves the other; a theme
    // across the sources holds both, and each source's holds its own.
    w.replace(&source("p2", false), &provider(), 4_000).unwrap();
    assert_eq!(r.list(&ListQuery::default()).unwrap().total, 6);
    assert_eq!(
        r.themes(&Sources::All).unwrap(),
        [("films".to_owned(), 2), ("news".to_owned(), 4)]
    );
    let news = |sources: Sources| ListQuery {
        scope: Scope::Theme("news".to_owned()),
        sources,
        ..ListQuery::default()
    };
    assert_eq!(r.list(&news(Sources::All)).unwrap().total, 4);
    assert_eq!(
        r.list(&news(Sources::One("p1".to_owned()))).unwrap().total,
        1
    );
    // A CATEGORY ASKED OF ANOTHER SOURCE holds nothing.
    let of_p1 = in_category("p1", "News");
    assert_eq!(r.list(&of_p1).unwrap().total, 1);
    assert_eq!(
        r.list(&ListQuery {
            sources: Sources::One("p2".to_owned()),
            ..of_p1
        })
        .unwrap()
        .total,
        0
    );
    let only_p2 = r
        .list(&ListQuery {
            sources: Sources::One("p2".to_owned()),
            ..ListQuery::default()
        })
        .unwrap();
    assert_eq!(only_p2.total, 5);
    w.remove("p1").unwrap();
    assert_eq!(r.sources().unwrap().len(), 1);
    assert_eq!(r.list(&ListQuery::default()).unwrap().total, 5);
    assert_eq!(
        r.themes(&Sources::All).unwrap(),
        [("films".to_owned(), 2), ("news".to_owned(), 3)],
        "the theme across the sources lost p1's"
    );
    assert!(r.themes(&Sources::One("p1".to_owned())).unwrap().is_empty());
    w.set_posters("p2", true).unwrap();
    assert!(r.sources().unwrap()[0].posters_on);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_source_marked_adult_is_only_in_the_adult_file() {
    let dir = scratch("adult");
    let mut w = Writer::open(&dir, 64 * MIB, 1_000).unwrap();
    let kept = w.replace(&source("p9", true), &provider(), 2_000).unwrap();
    assert_eq!(kept, Replaced { main: 0, adult: 7 });
    let main = Reader::open(&dir, Store::Main).unwrap();
    assert!(main.sources().unwrap().is_empty(), "not even its name");
    assert_eq!(main.list(&ListQuery::default()).unwrap().total, 0);
    let adult = Reader::open(&dir, Store::Adult).unwrap();
    assert_eq!(adult.categories(&Sources::All).unwrap().len(), 3);
    let _ = std::fs::remove_dir_all(dir);
}

/// FAILS IF: the catalogue — whose items carry a provider's stream addresses
/// with the person's account in them — is readable by anybody else, or a
/// door's reader can write.
#[test]
fn the_files_are_private_and_a_reader_cannot_write() {
    let dir = scratch("private");
    let mut w = Writer::open(&dir, 64 * MIB, 1_000).unwrap();
    w.replace(&source("p1", false), &provider(), 2_000).unwrap();
    let mode = |p: &Path| std::fs::metadata(p).unwrap().mode() & 0o777;
    assert_eq!(mode(&dir), 0o700);
    for file in [
        "catalog.sqlite",
        "catalog.sqlite-wal",
        "catalog.sqlite-shm",
        "catalog-adult.sqlite",
    ] {
        assert_eq!(mode(&dir.join(file)), 0o600, "{file}");
    }
    let r = Reader::open(&dir, Store::Main).unwrap();
    assert!(r.conn.execute("DELETE FROM items", []).is_err());
    assert!(
        r.conn
            .execute("INSERT INTO meta(key, value) VALUES ('x', 1)", [])
            .is_err()
    );
    assert_eq!(r.list(&ListQuery::default()).unwrap().total, 5);
    let _ = std::fs::remove_dir_all(dir);
}

/// Plants `sql` as the whole of `path`: the files a writer finds there left
/// by another build.
fn plant(path: &Path, sql: &str) {
    forget_files(path);
    let conn = Connection::open(path).unwrap();
    conn.execute_batch(sql).unwrap();
}

/// FAILS IF: a file the writer finds that is not this build's — another
/// layout, tables with no `meta`, bytes that are not a database — stops it
/// instead of being made again (the catalogue is a cache, rebuilt and never
/// migrated), or a door's reader of the old file never learns it was.
#[test]
fn a_file_of_another_layout_or_none_is_made_again() {
    let dir = scratch("schema");
    assert_eq!(
        Reader::open(&dir, Store::Main).unwrap_err(),
        Trouble::Absent
    );
    drop(Writer::open(&dir, 64 * MIB, 1_000).unwrap());
    let main = dir.join(Store::Main.file());
    let adult = dir.join(Store::Adult.file());
    // A FILE THAT HOLDS NOTHING YET is not a catalogue, and not another.
    plant(&main, "");
    assert_eq!(
        Reader::open(&dir, Store::Main).unwrap_err(),
        Trouble::Absent
    );

    // ANOTHER BUILD'S LAYOUT: its own `items`, whose columns this build's
    // indexes name none of, and a `meta` that says 0.
    plant(
        &main,
        "CREATE TABLE meta(key TEXT PRIMARY KEY, value INTEGER NOT NULL) WITHOUT ROWID;
         INSERT INTO meta VALUES ('schema', 0), ('generation', 7);
         CREATE TABLE items(id INTEGER PRIMARY KEY, name TEXT);
         INSERT INTO items VALUES (1, 'left by another build');",
    );
    // TABLES AND NO `meta` saying which layout.
    plant(
        &adult,
        "CREATE TABLE items(id INTEGER PRIMARY KEY, name TEXT);
         INSERT INTO items VALUES (1, 'x');",
    );
    for store in Store::ALL {
        assert_eq!(
            Reader::open(&dir, store).unwrap_err(),
            Trouble::OtherSchema,
            "{store:?}"
        );
    }
    let mut w = Writer::open(&dir, 64 * MIB, 5_000).unwrap();
    w.replace(&source("p1", false), &provider(), 6_000).unwrap();
    let r = Reader::open(&dir, Store::Main).unwrap();
    assert_eq!(
        r.list(&ListQuery::default()).unwrap().total,
        5,
        "made again, and written"
    );
    assert!(
        r.generation().unwrap() >= 5_000,
        "a number never seen before"
    );
    assert_eq!(
        Reader::open(&dir, Store::Adult)
            .unwrap()
            .list(&ListQuery::default())
            .unwrap()
            .total,
        2
    );
    assert!(!r.replaced());
    drop(w);

    // NOT A DATABASE AT ALL: bytes over the file.
    forget_files(&main);
    std::fs::write(
        &main,
        b"not a database, and long enough for a header ".repeat(40),
    )
    .unwrap();
    assert_eq!(
        Reader::open(&dir, Store::Main).unwrap_err(),
        Trouble::OtherSchema
    );
    assert!(r.replaced(), "the reader holds a file no longer there");
    let w = Writer::open(&dir, 64 * MIB, 7_000).unwrap();
    let again = Reader::open(&dir, Store::Main).unwrap();
    assert_eq!(
        again.list(&ListQuery::default()).unwrap().total,
        0,
        "made again, empty"
    );
    assert!(again.generation().unwrap() >= 7_000);
    assert!(!again.replaced());

    // AND A FILE OF OURS MADE AGAIN UNDER A READER: the reader is told.
    drop(w);
    let conn = Connection::open(&main).unwrap();
    conn.execute("UPDATE meta SET value = 99 WHERE key = 'schema'", [])
        .unwrap();
    drop(conn);
    let _w = Writer::open(&dir, 64 * MIB, 8_000).unwrap();
    assert!(again.replaced(), "made again: a door opens a new reader");
    assert!(!Reader::open(&dir, Store::Main).unwrap().replaced());
    let _ = std::fs::remove_dir_all(dir);
}

/// FAILS IF: a row this build did not write — a kind it does not know, a
/// letter that is not one character, a number out of its type — is shown as
/// a plausible guess instead of refused.
#[test]
fn a_row_this_build_did_not_write_is_refused() {
    let dir = scratch("foreign");
    let mut w = Writer::open(&dir, 64 * MIB, 1_000).unwrap();
    w.replace(&source("p1", false), &provider(), 2_000).unwrap();
    let r = Reader::open(&dir, Store::Main).unwrap();
    let bbc = ItemId::of("p1", "bbc");
    for (sql, what) in [
        (
            "UPDATE items SET kind = 'hologram' WHERE id = ?1",
            "a kind this build does not know",
        ),
        ("UPDATE items SET letter = '' WHERE id = ?1", "no letter"),
        (
            "UPDATE items SET letter = 'AB' WHERE id = ?1",
            "two letters",
        ),
        (
            "UPDATE items SET quality = 70000 WHERE id = ?1",
            "a height past its type",
        ),
        (
            "UPDATE items SET year = 9999999999 WHERE id = ?1",
            "a year past its type",
        ),
        (
            "UPDATE items SET popularity = -1 WHERE id = ?1",
            "a negative popularity",
        ),
    ] {
        assert_eq!(
            r.list(&ListQuery::default()).unwrap().total,
            5,
            "{what}: before"
        );
        w.main.execute(sql, [bbc.0]).unwrap();
        assert_eq!(
            r.list(&ListQuery::default()).unwrap_err(),
            Trouble::OtherSchema,
            "{what}"
        );
        assert_eq!(r.item(bbc).unwrap_err(), Trouble::OtherSchema, "{what}");
        // THE NEXT INGEST writes the row again, whole.
        w.replace(&source("p1", false), &provider(), 3_000).unwrap();
        assert_eq!(r.item(bbc).unwrap().unwrap().row.kind, ItemKind::Live);
    }
    w.main
        .execute("UPDATE sources SET kind = 'hologram' WHERE id = 'p1'", [])
        .unwrap();
    assert_eq!(r.sources().unwrap_err(), Trouble::OtherSchema);
    let _ = std::fs::remove_dir_all(dir);
}

/// FAILS IF: the layout changes and `SCHEMA` does not — a reader of an older
/// build would take a newer file for its own. When it fails, raise `SCHEMA`
/// and pin the new digest.
#[test]
fn the_layout_has_the_number_it_says() {
    let mut parts: Vec<&str> = vec![SCHEMA_SQL];
    parts.extend(ItemKind::ALL.map(ItemKind::word));
    parts.extend(SourceKind::ALL.map(SourceKind::id));
    let digest = format!("{:016x}", super::super::fnv(&parts));
    assert_eq!(
        (SCHEMA, digest.as_str()),
        (1, "b3c2a0d148a848ee"),
        "the layout changed: raise SCHEMA and pin the new digest"
    );
}

/// FAILS IF: the catalogue can grow past `playerCatalogMib` — an ingest that
/// would pass it must change nothing.
#[test]
fn an_ingest_past_the_ceiling_is_refused_and_changes_nothing() {
    let dir = scratch("ceiling");
    let mut w = Writer::open(&dir, 2 * MIB, 1_000).unwrap();
    w.replace(&source("p1", false), &provider(), 2_000).unwrap();
    let big = Harvest {
        categories: vec![category("All", None, false)],
        items: (0..20_000)
            .map(|n| {
                let mut i = item(&format!("n{n}"), &format!("Channel {n}"), &["All"]);
                i.description = Some("a long description ".repeat(20));
                i
            })
            .collect(),
        cut: false,
    };
    assert_eq!(
        w.replace(&source("p2", false), &big, 3_000),
        Err(Trouble::TooLarge)
    );
    let r = Reader::open(&dir, Store::Main).unwrap();
    assert_eq!(r.list(&ListQuery::default()).unwrap().total, 5, "as it was");
    assert!(w.bytes().unwrap() <= 2 * MIB);

    // A HARVEST WHOSE ADULT HALF FITS AND WHOSE MAIN HALF DOES NOT changes
    // neither file: the two are committed together or not at all.
    let mut both = big.clone();
    both.categories.push(category("Adultos 18+", None, true));
    both.items
        .extend((0..50).map(|n| item(&format!("a{n}"), &format!("Late {n}"), &["Adultos 18+"])));
    assert_eq!(
        w.replace(&source("p2", false), &both, 3_500),
        Err(Trouble::TooLarge)
    );
    let adult = Reader::open(&dir, Store::Adult).unwrap();
    assert_eq!(
        adult.list(&ListQuery::default()).unwrap().total,
        2,
        "p1's two adult rows, and none of p2's"
    );
    assert!(
        adult.sources().unwrap().iter().all(|s| s.id != "p2"),
        "not even p2's name"
    );
    // AND A CEILING LOWERED UNDER WHAT IS THERE empties the catalogue.
    drop(w);
    let w = Writer::open(&dir, 1024, 4_000).unwrap();
    assert_eq!(
        Reader::open(&dir, Store::Main)
            .unwrap()
            .list(&ListQuery::default())
            .unwrap()
            .total,
        0
    );
    drop(w);
    let _ = std::fs::remove_dir_all(dir);
}

/// The steps SQLite chose for `sql`.
fn explain(r: &Reader, sql: &str, args: Vec<Value>) -> Vec<String> {
    let mut st = r
        .conn
        .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
        .unwrap();
    st.query_map(rusqlite::params_from_iter(args), |row| {
        row.get::<_, String>(3)
    })
    .unwrap()
    .collect::<rusqlite::Result<Vec<_>>>()
    .unwrap()
}

/// The steps SQLite chose to walk a listing's page, from its start or from
/// its end.
fn plan(r: &Reader, q: &ListQuery, from_end: bool) -> Vec<String> {
    let (sql, mut args) = walk(q, from_end);
    args.push(Value::Integer(50));
    args.push(Value::Integer(0));
    explain(r, &sql, args)
}

#[test]
fn a_page_is_walked_from_the_nearer_end() {
    // (from_end, limit, offset)
    assert_eq!(placed(0, 1, 50), (false, 0, 0), "nothing");
    assert_eq!(placed(120, 1, 50), (false, 50, 0), "the first");
    assert_eq!(placed(120, 3, 50), (true, 20, 0), "the last, short");
    assert_eq!(placed(120, 2, 50), (true, 50, 20), "nearer the end");
    assert_eq!(
        placed(150, 2, 50),
        (false, 50, 50),
        "the middle, from the start"
    );
    assert_eq!(placed(1000, 20, 50), (true, 50, 0));
    assert_eq!(placed(1000, 11, 50), (true, 50, 450));
    assert_eq!(placed(1000, 10, 50), (false, 50, 450), "the middle");
}

/// FAILS IF: a page walked from the end is not the page walked from the
/// start — the reverse order must be the order, reversed, ties and all.
#[test]
fn a_page_from_the_end_is_the_same_page() {
    let dir = scratch("from-the-end");
    let mut w = Writer::open(&dir, 64 * MIB, 1_000).unwrap();
    let mut harvest = Harvest {
        categories: vec![category("All", Some("news"), false)],
        items: Vec::new(),
        cut: false,
    };
    for n in 0..230 {
        // TIES on every key: a tenth of the titles, popularities and dates
        // repeat, so only the id tells the rows apart.
        let mut i = item(&format!("i{n}"), &format!("Title {}", n % 23), &["All"]);
        i.popularity = n % 7;
        i.year = (n % 3 == 0).then_some(1990 + (n % 5) as i32);
        i.country = Some(["BR", "GB"][n as usize % 2].to_owned());
        harvest.items.push(i);
    }
    w.replace(&source("p1", false), &harvest, 2_000).unwrap();
    let r = Reader::open(&dir, Store::Main).unwrap();
    for scope in [
        Scope::AllCategories,
        Scope::Category(CategoryId::of("p1", "All")),
        Scope::Theme("news".to_owned()),
    ] {
        for sort in Sort::ALL {
            for country in [None, Some("BR".to_owned())] {
                let q = ListQuery {
                    scope: scope.clone(),
                    sort,
                    country: country.clone(),
                    per_page: 500,
                    ..ListQuery::default()
                };
                let whole = r.list(&q).unwrap();
                let ids: Vec<ItemId> = whole.rows.iter().map(|r| r.id).collect();
                let mut paged = Vec::new();
                for page in 1..=whole.total.div_ceil(17) as u32 {
                    let one = r
                        .list(&ListQuery {
                            per_page: 17,
                            page,
                            ..q.clone()
                        })
                        .unwrap();
                    paged.extend(one.rows.iter().map(|r| r.id));
                }
                assert_eq!(paged, ids, "{scope:?} {sort:?} {country:?}");
            }
        }
    }
    let _ = std::fs::remove_dir_all(dir);
}

/// How many items the big synthetic source has.
const FIFTY_THOUSAND: usize = 50_000;

/// The theme the big synthetic source's catch-all group is filed under.
const BIG_THEME: &str = "films";

/// Writes the synthetic catalogue the listing's timing is measured on, and
/// gives back how long the big source's ingest took.
///
/// Fifty thousand items in one source (`big`) — 40 categories under the
/// table's themes, each item in two of them, and one group (`everything`)
/// holding every item, filed under [`BIG_THEME`], the way a provider's list
/// has an "all" group — beside a second source (`small`) of two thousand;
/// titles in several scripts, a third with a description, a country, a
/// language, a height, and a year, a publication, both or neither.
fn fifty_thousand(w: &mut Writer) -> std::time::Duration {
    const WORDS: [&str; 16] = [
        "Canal",
        "Notícias",
        "Ñandú",
        "Élan",
        "Música",
        "Sport",
        "Кино",
        "Новости",
        "東京",
        "Ação",
        "Zebra",
        "Óculos",
        "Quiz",
        "Yoga",
        "Weather",
        "Jazz",
    ];
    let themes: Vec<&str> = crate::table::THEMES.iter().map(|t| t.id).collect();
    assert!(themes.contains(&BIG_THEME));
    let mut categories: Vec<NewCategory> = (0..40)
        .map(|n| category(&format!("g{n}"), Some(themes[n % themes.len()]), false))
        .collect();
    categories.push(category("everything", Some(BIG_THEME), false));
    let mut seed: u64 = 7;
    let mut next = move || {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        seed >> 33
    };
    let mut make = |n: usize, prefix: &str, groups: &[String]| {
        let a = WORDS[(next() % 16) as usize];
        let b = WORDS[(next() % 16) as usize];
        let chosen: Vec<&str> = groups.iter().map(String::as_str).collect();
        let mut i = item(&format!("{prefix}{n}"), &format!("{a} {b} {n}"), &chosen);
        i.popularity = next() % 100_000;
        i.country = Some(["BR", "GB", "US", "JP", "RU"][(next() % 5) as usize].to_owned());
        i.language = Some(["Portuguese", "English", "Japanese"][(next() % 3) as usize].to_owned());
        i.height = Some([480, 720, 1080][(next() % 3) as usize]);
        match next() % 4 {
            0 => i.year = Some(1900 + (next() % 120) as i32),
            1 => i.published_s = Some(1_000_000_000 + (next() % 700_000_000) as i64),
            2 => {
                i.year = Some(1900 + (next() % 120) as i32);
                i.published_s = Some(1_000_000_000 + (next() % 700_000_000) as i64);
            }
            _ => {}
        }
        i.description = n
            .is_multiple_of(3)
            .then(|| "um programa ao vivo de música".to_owned());
        i
    };
    let items: Vec<NewItem> = (0..FIFTY_THOUSAND)
        .map(|n| {
            let groups = [
                format!("g{}", n % 40),
                format!("g{}", (n * 7 + 3) % 40),
                "everything".to_owned(),
            ];
            make(n, "i", &groups)
        })
        .collect();
    let harvest = Harvest {
        categories,
        items,
        cut: false,
    };
    let started = Instant::now();
    w.replace(&source("big", false), &harvest, 2_000).unwrap();
    let ingest = started.elapsed();
    let small = Harvest {
        categories: (0..5)
            .map(|n| category(&format!("s{n}"), Some(themes[n]), false))
            .collect(),
        items: (0..2_000)
            .map(|n| make(n, "s", &[format!("s{}", n % 5)]))
            .collect(),
        cut: false,
    };
    w.replace(&source("small", false), &small, 2_000).unwrap();
    ingest
}

/// FAILS IF: a listing over a provider-sized catalogue stops answering in
/// under 20 ms — the door queries on a worker and draws what comes, and a
/// slow answer is a list that lags every key — or a listing is walked by
/// sorting it whole (the 100–160 ms the last page cost before every order
/// had its index).
///
/// The catalogue is [`fifty_thousand`]'s. Every scope a door asks (every
/// item, a source, a big and a small category, a big and a small theme,
/// across the sources and in one) in every order at its first, middle and
/// last page, and with a letter, with words (rare ones, and one half the
/// items have), with a country and a height, with a language. Each shape is
/// timed five times, the first (cold) included, and the MEDIAN must be under
/// 20 ms — the worst is reported too; a machine busy with other work
/// stretches a run, not the plan. What cannot be stretched is asserted as
/// well: every walk, from either end, reads a covering index and never
/// sorts.
#[test]
fn a_listing_over_fifty_thousand_items_answers_in_under_20_ms() {
    let dir = scratch("fifty-thousand");
    let mut w = Writer::open(&dir, 512 * MIB, 1_000).unwrap();
    let ingest = fifty_thousand(&mut w);
    let themes: Vec<&str> = crate::table::THEMES.iter().map(|t| t.id).collect();
    let big_theme = BIG_THEME;
    let r = Reader::open(&dir, Store::Main).unwrap();
    let big = Sources::One("big".to_owned());
    let everything = Scope::Category(CategoryId::of("big", "everything"));
    let scopes: Vec<(&str, Scope, Sources)> = vec![
        ("every item", Scope::AllCategories, Sources::All),
        ("a source", Scope::AllCategories, big.clone()),
        ("a category of every item", everything.clone(), Sources::All),
        ("the same, of its source", everything.clone(), big.clone()),
        (
            "a small category",
            Scope::Category(CategoryId::of("big", "g7")),
            Sources::All,
        ),
        (
            "a theme across sources",
            Scope::Theme(big_theme.to_owned()),
            Sources::All,
        ),
        ("a source's theme", Scope::Theme(big_theme.to_owned()), big),
        (
            "a small theme",
            Scope::Theme(themes[3].to_owned()),
            Sources::All,
        ),
    ];
    let mut shapes: Vec<(String, ListQuery)> = Vec::new();
    for (name, scope, sources) in &scopes {
        let base = ListQuery {
            scope: scope.clone(),
            sources: sources.clone(),
            ..ListQuery::default()
        };
        let middle = r.list(&base).unwrap().pages / 2 + 1;
        for sort in Sort::ALL {
            for (page, at) in [(1, "first"), (middle, "middle"), (u32::MAX, "last")] {
                shapes.push((
                    format!("{name}, {}, {at} page", sort.id()),
                    ListQuery {
                        sort,
                        page,
                        ..base.clone()
                    },
                ));
            }
        }
        let filtered: [(&str, ListQuery); 5] = [
            (
                "letter N",
                ListQuery {
                    letter: Some('N'),
                    ..base.clone()
                },
            ),
            (
                "letter Ñ, popular",
                ListQuery {
                    letter: Some('Ñ'),
                    sort: Sort::Popular,
                    ..base.clone()
                },
            ),
            (
                "rare words, newest",
                ListQuery {
                    words: Some("zebra jazz".to_owned()),
                    sort: Sort::Newest,
                    ..base.clone()
                },
            ),
            (
                "a word half the items have, title",
                ListQuery {
                    words: Some("musica".to_owned()),
                    ..base.clone()
                },
            ),
            (
                "a country and a height, popular",
                ListQuery {
                    country: Some("BR".to_owned()),
                    min_height: Some(720),
                    sort: Sort::Popular,
                    ..base.clone()
                },
            ),
        ];
        let language = (
            "a language, oldest",
            ListQuery {
                language: Some("Japanese".to_owned()),
                sort: Sort::Oldest,
                ..base.clone()
            },
        );
        for (what, q) in filtered.into_iter().chain([language]) {
            let middle = r.list(&q).unwrap().pages / 2 + 1;
            for (page, at) in [(1, "first"), (middle, "middle"), (u32::MAX, "last")] {
                shapes.push((
                    format!("{name}, {what}, {at} page"),
                    ListQuery { page, ..q.clone() },
                ));
            }
        }
    }
    // THE PLANS FIRST: a plan that sorts a listing whole, or searches once
    // per row, fails here in a moment rather than in the timing, in minutes.
    for (name, q) in &shapes {
        for from_end in [false, true] {
            let steps = plan(&r, q, from_end);
            assert!(
                steps.iter().all(|s| !s.contains("TEMP B-TREE"))
                    && steps.iter().any(|s| s.contains("COVERING INDEX")),
                "{name} (from the end: {from_end}) is not walked on an index: {steps:?}"
            );
        }
        if q.words.is_some() {
            // WORDS ARE COUNTED BY THE TEXT INDEX: the search is the outer
            // loop — the only one, with no other filter — and each item it
            // finds is looked up by its key.
            for filtered in [false, true] {
                let asked = ListQuery {
                    min_height: filtered.then_some(720),
                    ..q.clone()
                };
                let (sql, args) = count_query(&asked);
                let steps = explain(&r, &sql, args);
                assert!(
                    steps.first().is_some_and(|s| s.contains("VIRTUAL TABLE"))
                        && steps[1..].iter().all(|s| s.starts_with("SEARCH"))
                        && (filtered || steps.len() == 1),
                    "{name} (a filter: {filtered}) is not counted from what its words find: {steps:?}"
                );
            }
        }
    }
    let mut report = Vec::new();
    let mut slow = Vec::new();
    for (name, q) in &shapes {
        let mut runs = Vec::with_capacity(5);
        let mut total = 0;
        for _ in 0..5 {
            let t = Instant::now();
            let page = r.list(q).unwrap();
            runs.push(t.elapsed());
            total = page.total;
            assert!(!page.rows.is_empty(), "{name} found nothing");
        }
        runs.sort();
        let (median, worst) = (runs[2], runs[4]);
        report.push(format!(
            "{name}: median {median:?}, worst {worst:?} ({total} items)"
        ));
        if median.as_millis() >= 20 {
            slow.push(name.clone());
        }
    }
    println!(
        "ingest of {FIFTY_THOUSAND} items: {ingest:?}; catalogue {} KiB; {} shapes\n{}",
        w.bytes().unwrap() / 1024,
        shapes.len(),
        report.join("\n")
    );
    assert!(slow.is_empty(), "slower than 20 ms: {slow:?}");
    let t = Instant::now();
    let letters = r
        .letters(&ListQuery {
            scope: everything,
            ..ListQuery::default()
        })
        .unwrap();
    report.push(format!(
        "letters of the big category: {:?} ({} letters)",
        t.elapsed(),
        letters.len()
    ));
    assert!(letters.contains(&'Ñ') && letters.contains(&'К') && letters.contains(&'東'));
    assert!(letters.iter().position(|l| *l == 'Ñ') > letters.iter().position(|l| *l == 'N'));
    assert_eq!(fold("Ñ"), "n");
    println!("{}", report.last().unwrap());
    let _ = std::fs::remove_dir_all(dir);
}
