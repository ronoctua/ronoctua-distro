//! The index end to end: written by the writer, read by a reader in the same
//! shape a door has, crawled from a planted tree.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::crawl::{Crawler, Limits, Rules, shed_target};
use crate::serve::{Config, Sources, Unit};
use crate::videos::{Found, Videos};
use crate::write::{Document, Writer};
use crate::{History, Hit, Index, Kind, Query, Settings, Why};

/// A folder of its own for one test, empty.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ronoctua-index-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn file(id: &str, mtime_s: i64) -> Document {
    Document::of_path(Path::new(id), Kind::File, Some("text/plain"), mtime_s, 10)
}

fn ids(hits: &[Hit]) -> Vec<&str> {
    hits.iter().map(|h| h.id.as_str()).collect()
}

fn ask(index: &Index, text: &str) -> Vec<Hit> {
    index.query(&Query::of_text(text, 20)).unwrap()
}

fn names_only(index: &Index, text: &str) -> Vec<Hit> {
    let mut q = Query::of_text(text, 20);
    q.contents = false;
    index.query(&q).unwrap()
}

#[test]
fn the_reader_sees_the_commit_after_a_reload_and_not_before() {
    let dir = scratch("commit");
    let mut writer = Writer::open(&dir, 32 << 20).unwrap();
    writer.add(&file("/home/ana/first.txt", 0)).unwrap();
    writer.commit().unwrap();

    let mut index = Index::open_ro(&dir).unwrap();
    assert_eq!(ids(&ask(&index, "first")), ["/home/ana/first.txt"]);

    writer.add(&file("/home/ana/second.txt", 0)).unwrap();
    writer.commit().unwrap();
    // No clock: the reader sees what it saw until it is told to reload.
    assert!(ask(&index, "second").is_empty());
    index.reload().unwrap();
    assert_eq!(ids(&ask(&index, "second")), ["/home/ana/second.txt"]);
    assert_eq!(index.documents(), 2);
    assert_eq!(index.meta_path(), dir.join("meta.json"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_name_is_found_by_its_prefix_its_parts_and_a_typo() {
    let dir = scratch("names");
    let mut writer = Writer::open(&dir, 32 << 20).unwrap();
    for id in [
        "/home/ana/Documents/reportFinal-v2.docx",
        "/home/ana/Documents/notes.txt",
        "/home/ana/Pictures/IMG_20230415_101010.jpg",
        "/home/ana/Café/Résumé Crème.odt",
    ] {
        writer.add(&file(id, 0)).unwrap();
    }
    writer.commit().unwrap();
    let index = Index::open_ro(&dir).unwrap();

    let report = ["/home/ana/Documents/reportFinal-v2.docx"];
    assert_eq!(ids(&names_only(&index, "rep")), report);
    assert_eq!(ids(&names_only(&index, "fin")), report, "a camelCase part");
    assert_eq!(
        ids(&names_only(&index, "reportfinal")),
        report,
        "the whole chunk"
    );
    assert_eq!(
        ids(&names_only(&index, "reprot")),
        report,
        "a transposition"
    );
    assert_eq!(ids(&names_only(&index, "report v2")), report);
    assert!(
        names_only(&index, "report zebra").is_empty(),
        "every word must match"
    );
    assert_eq!(
        ids(&names_only(&index, "resume creme")),
        ["/home/ana/Café/Résumé Crème.odt"],
        "accents fold"
    );
    assert_eq!(
        ids(&names_only(&index, "cafe res")),
        ["/home/ana/Café/Résumé Crème.odt"],
        "a folder above counts"
    );
    assert_eq!(
        ids(&names_only(&index, "2023")),
        ["/home/ana/Pictures/IMG_20230415_101010.jpg"]
    );
    assert!(names_only(&index, "").is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_name_outranks_a_folder_and_a_newer_file_an_older_one() {
    let dir = scratch("rank");
    let now = crate::now_s();
    let mut writer = Writer::open(&dir, 32 << 20).unwrap();
    writer
        .add(&file("/home/ana/budget/table.ods", now))
        .unwrap();
    writer
        .add(&file("/home/ana/old/budget.ods", now - 400 * 86_400))
        .unwrap();
    writer.add(&file("/home/ana/new/budget.ods", now)).unwrap();
    writer.commit().unwrap();
    let index = Index::open_ro(&dir).unwrap();
    let hits = names_only(&index, "budget");
    assert_eq!(
        ids(&hits),
        [
            "/home/ana/new/budget.ods",
            "/home/ana/old/budget.ods",
            "/home/ana/budget/table.ods"
        ]
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn contents_are_their_own_kind_and_names_only_leaves_them_out() {
    let dir = scratch("contents");
    let mut writer = Writer::open(&dir, 32 << 20).unwrap();
    let mut letter = file("/home/ana/letter.txt", 0);
    letter.body = Some("Dear Ana, the invoice is attached".to_owned());
    writer.add(&letter).unwrap();
    let mut named = file("/home/ana/invoice.txt", 0);
    named.body = Some("invoice number 12".to_owned());
    writer.add(&named).unwrap();
    let mut page = Document {
        id: "man:ls.1".to_owned(),
        kind: Kind::Man,
        name: "ls".to_owned(),
        also: Vec::new(),
        path_terms: Vec::new(),
        body: Some("list directory contents invoice".to_owned()),
        note: Some("list directory contents".to_owned()),
        mime: None,
        mtime_s: 0,
        size_bytes: 0,
    };
    writer.add(&page).unwrap();
    page.id = "man:invoice.1".to_owned();
    page.name = "invoice".to_owned();
    writer.add(&page).unwrap();
    writer.commit().unwrap();
    let index = Index::open_ro(&dir).unwrap();

    let hits = ask(&index, "invoice");
    let kinds: Vec<(Kind, &str)> = hits.iter().map(|h| (h.kind, h.id.as_str())).collect();
    assert!(
        kinds.contains(&(Kind::File, "/home/ana/invoice.txt")),
        "{kinds:?}"
    );
    assert!(
        kinds.contains(&(Kind::Content, "/home/ana/letter.txt")),
        "{kinds:?}"
    );
    assert!(
        !kinds.contains(&(Kind::Content, "/home/ana/invoice.txt")),
        "found by name, not also by content"
    );
    assert!(kinds.contains(&(Kind::Man, "man:ls.1")), "{kinds:?}");
    // The page NAMED invoice outranks the one that only says it.
    let man: Vec<&str> = hits
        .iter()
        .filter(|h| h.kind == Kind::Man)
        .map(|h| h.id.as_str())
        .collect();
    assert_eq!(man, ["man:invoice.1", "man:ls.1"]);
    let note = hits
        .iter()
        .find(|h| h.id == "man:ls.1")
        .and_then(|h| h.note.clone());
    assert_eq!(note.as_deref(), Some("list directory contents"));
    // Sections come in fast-access's order.
    let order: Vec<Kind> = hits.iter().map(|h| h.kind).collect();
    let mut sorted = order.clone();
    sorted.sort_by_key(|k| Kind::ALL.iter().position(|x| x == k));
    assert_eq!(order, sorted);

    let names = names_only(&index, "invoice");
    assert!(names.iter().all(|h| h.kind != Kind::Content));
    assert!(names.iter().any(|h| h.id == "man:invoice.1"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn kinds_and_types_filter_and_each_kind_keeps_its_own_most() {
    let dir = scratch("filters");
    let mut writer = Writer::open(&dir, 32 << 20).unwrap();
    for k in 0..30 {
        let mut song = file(&format!("/m/song{k}.flac"), 0);
        song.mime = Some("audio/flac".to_owned());
        writer.add(&song).unwrap();
    }
    let mut film = file("/v/song-of-the-sea.mkv", 0);
    film.mime = Some("video/x-matroska".to_owned());
    writer.add(&film).unwrap();
    writer
        .add(&Document::of_path(
            Path::new("/m/songs"),
            Kind::Folder,
            Some("inode/directory"),
            0,
            0,
        ))
        .unwrap();
    writer.commit().unwrap();
    let index = Index::open_ro(&dir).unwrap();

    let mut q = Query::of_text("song", 5);
    let hits = index.query(&q).unwrap();
    assert_eq!(
        hits.iter().filter(|h| h.kind == Kind::File).count(),
        5,
        "five files"
    );
    assert_eq!(
        hits.iter().filter(|h| h.kind == Kind::Folder).count(),
        1,
        "and the folder"
    );

    q.mime_prefixes = vec!["video/".to_owned()];
    assert_eq!(ids(&index.query(&q).unwrap()), ["/v/song-of-the-sea.mkv"]);
    q.mime_prefixes = vec!["image/".to_owned()];
    assert!(
        index.query(&q).unwrap().is_empty(),
        "no such type: nothing, not everything"
    );

    let folders = Query {
        kinds: vec![Kind::Folder],
        ..Query::of_text("song", 5)
    };
    assert_eq!(ids(&index.query(&folders).unwrap()), ["/m/songs"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_history_lifts_a_hit_even_from_outside_the_window() {
    let dir = scratch("history");
    let now = crate::now_s();
    let mut writer = Writer::open(&dir, 32 << 20).unwrap();
    // Names of one shape (`plan`, a number, `txt`): only the dates differ.
    for k in 0..40 {
        writer
            .add(&file(&format!("/d/plan-{k}.txt"), now - k))
            .unwrap();
    }
    // The oldest by far: last without a history.
    writer.add(&file("/d/plan-old.txt", 1)).unwrap();
    writer.commit().unwrap();
    let index = Index::open_ro(&dir).unwrap();
    let q = Query::of_text("plan", 3);
    assert!(!ids(&index.query(&q).unwrap()).contains(&"/d/plan-old.txt"));
    let history = History::new().with("/d/plan-old.txt", 1.0);
    let hits = index.query_with(&q, &history).unwrap();
    assert!(ids(&hits).contains(&"/d/plan-old.txt"), "{:?}", ids(&hits));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn documents_are_replaced_by_id_and_stamps_answer_the_last_commit() {
    let dir = scratch("stamps");
    let mut writer = Writer::open(&dir, 32 << 20).unwrap();
    writer.add(&file("/a/x.txt", 5)).unwrap();
    writer.add(&file("/a/y.txt", 6)).unwrap();
    writer.add(&file("/b/z.txt", 7)).unwrap();
    writer.commit().unwrap();
    let stamp = writer.stamp("/a/x.txt").unwrap();
    assert_eq!(
        (stamp.kind, stamp.mtime_s, stamp.size_bytes),
        (Kind::File, 5, 10)
    );
    assert!(writer.stamp("/a/none.txt").is_none());

    writer.replace(&file("/a/x.txt", 9)).unwrap();
    writer.delete("/a/y.txt").unwrap();
    writer.commit().unwrap();
    assert_eq!(writer.stamp("/a/x.txt").map(|s| s.mtime_s), Some(9));
    assert!(writer.stamp("/a/y.txt").is_none());
    assert_eq!(writer.documents(), 2);

    let mut under_a = Vec::new();
    writer.each_id("/a/", |id| under_a.push(id.to_owned()));
    assert_eq!(under_a, ["/a/x.txt"]);
    let mut all = Vec::new();
    writer.each_id("", |id| all.push(id.to_owned()));
    all.sort();
    assert_eq!(all, ["/a/x.txt", "/b/z.txt"]);

    writer.delete_kind(Kind::File);
    writer.commit().unwrap();
    assert_eq!(writer.documents(), 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_index_of_another_format_is_refused_and_a_second_writer_is_busy() {
    let dir = scratch("format");
    {
        let mut writer = Writer::open(&dir, 32 << 20).unwrap();
        assert_eq!(Writer::open(&dir, 32 << 20).unwrap_err().why, Why::Busy);
        writer.add(&file("/x", 0)).unwrap();
        writer.state_mut().format = "ronoctua-index/0".to_owned();
        writer.commit().unwrap();
    }
    assert_eq!(Index::open_ro(&dir).unwrap_err().why, Why::OtherFormat);
    assert_eq!(
        Writer::open(&dir, 32 << 20).unwrap_err().why,
        Why::OtherFormat
    );
    // The unit wipes it and starts again.
    let writer = crate::serve::open_writer(&dir, 32 << 20).unwrap();
    assert_eq!(writer.documents(), 0);
    let _ = std::fs::remove_dir_all(&dir);
}

fn limits(ceiling_bytes: u64) -> Limits {
    Limits {
        ceiling_bytes,
        content_file_bytes: 1 << 20,
        content_bytes: 64 << 10,
        skip: vec![".git".to_owned(), "target".to_owned()],
    }
}

/// A planted tree: text files, a skipped folder, a hidden one, a symbolic link.
fn plant(root: &Path) {
    std::fs::create_dir_all(root.join("docs/deep")).unwrap();
    std::fs::create_dir_all(root.join(".git/objects")).unwrap();
    std::fs::create_dir_all(root.join("project/target/debug")).unwrap();
    std::fs::create_dir_all(root.join(".hidden")).unwrap();
    std::fs::write(root.join("docs/recipe.txt"), "flour eggs crème brûlée").unwrap();
    std::fs::write(
        root.join("docs/deep/letter.md"),
        "a letter about the harbour",
    )
    .unwrap();
    std::fs::write(root.join(".git/objects/abc"), "blob").unwrap();
    std::fs::write(root.join("project/target/debug/build.txt"), "artefact").unwrap();
    std::fs::write(root.join(".hidden/secret.txt"), "hidden words").unwrap();
    std::fs::write(
        root.join("project/main.rs"),
        "fn main() { println!(\"harbour\"); }",
    )
    .unwrap();
    std::os::unix::fs::symlink(root.join("docs"), root.join("docs-link")).unwrap();
}

#[test]
fn a_walk_indexes_what_is_there_and_leaves_out_what_is_skipped() {
    let base = scratch("walk");
    let root = base.join("home");
    plant(&root);
    let rules = Rules::new(&limits(1 << 30), &Settings::default());
    let mut crawler = Crawler::new(
        Writer::open(&base.join("index"), 32 << 20).unwrap(),
        rules,
        1 << 30,
    );
    let mut seen = HashSet::new();
    let mut folders = Vec::new();
    let walked = crawler
        .walk(&root, &mut seen, false, &mut |f| {
            folders.push(f.to_path_buf())
        })
        .unwrap();
    crawler.commit().unwrap();
    assert_eq!(walked.indexed, walked.entries);
    let index = Index::open_ro(&base.join("index")).unwrap();

    assert!(!ask(&index, "recipe").is_empty());
    let flour = ask(&index, "flour");
    assert_eq!(
        flour.iter().map(|h| h.kind).collect::<Vec<_>>(),
        [Kind::Content]
    );
    assert_eq!(
        flour[0].path.as_deref(),
        Some(root.join("docs/recipe.txt").as_path())
    );
    assert!(
        ask(&index, "brulee")
            .iter()
            .any(|h| h.kind == Kind::Content),
        "contents fold too"
    );
    for gone in ["abc", "objects", "debug", "artefact", "secret", "hidden"] {
        assert!(
            ask(&index, gone).is_empty(),
            "{gone} should have been skipped"
        );
    }
    let link = names_only(&index, "docs link");
    assert_eq!(link.len(), 1, "the link is a file of its own");
    assert_eq!(link[0].kind, Kind::File);
    assert!(folders.contains(&root.join("docs/deep")));
    assert!(
        !folders.iter().any(|f| f.ends_with("docs-link")),
        "a link to a folder is never entered"
    );
    assert_eq!(
        ask(&index, "harbour")
            .iter()
            .filter(|h| h.kind == Kind::Content)
            .count(),
        2
    );

    // Walking again with nothing changed writes nothing.
    let mut seen = HashSet::new();
    let again = crawler.walk(&root, &mut seen, false, &mut |_| {}).unwrap();
    assert_eq!(again.indexed, 0);

    // A file removed, a folder removed: the prune deletes what the walk did
    // not see; a changed file is read again.
    std::fs::remove_file(root.join("docs/recipe.txt")).unwrap();
    std::fs::remove_dir_all(root.join("docs/deep")).unwrap();
    std::fs::write(
        root.join("project/main.rs"),
        "fn main() { println!(\"lighthouse\"); }",
    )
    .unwrap();
    let mut seen = HashSet::new();
    let third = crawler.walk(&root, &mut seen, false, &mut |_| {}).unwrap();
    // main.rs changed size; a folder whose time moved within the same second
    // is not seen by a stamp in seconds (inotify is what follows those).
    assert!(third.indexed >= 1, "{third:?}");
    let pruned = crawler.prune(std::slice::from_ref(&root), &seen).unwrap();
    assert_eq!(pruned, 3, "recipe.txt, deep, letter.md");
    crawler.commit().unwrap();
    let mut index = index;
    index.reload().unwrap();
    assert!(ask(&index, "recipe").is_empty());
    assert!(ask(&index, "letter").is_empty());
    assert!(
        ask(&index, "lighthouse")
            .iter()
            .any(|h| h.kind == Kind::Content)
    );

    // A root the person removed takes its documents with it.
    let gone = crawler
        .prune(&[base.join("elsewhere")], &HashSet::new())
        .unwrap();
    assert!(gone > 0);
    crawler.commit().unwrap();
    index.reload().unwrap();
    assert_eq!(index.documents(), 0);
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn one_path_and_one_tree_are_rechecked_against_the_disk() {
    let base = scratch("recheck");
    let root = base.join("home");
    std::fs::create_dir_all(&root).unwrap();
    let rules = Rules::new(&limits(1 << 30), &Settings::default());
    let mut crawler = Crawler::new(
        Writer::open(&base.join("index"), 32 << 20).unwrap(),
        rules,
        1 << 30,
    );

    std::fs::write(root.join("todo.txt"), "milk").unwrap();
    crawler.recheck(&root, &root.join("todo.txt")).unwrap();
    std::fs::create_dir_all(root.join("album/disc1")).unwrap();
    std::fs::write(root.join("album/disc1/track01.flac"), "").unwrap();
    crawler
        .recheck_tree(&root, &root.join("album"), &mut |_| {})
        .unwrap();
    // Inside a skipped folder, an event is not a document.
    std::fs::create_dir_all(root.join(".git")).unwrap();
    std::fs::write(root.join(".git/HEAD"), "ref").unwrap();
    crawler.recheck(&root, &root.join(".git/HEAD")).unwrap();
    crawler.commit().unwrap();
    let mut index = Index::open_ro(&base.join("index")).unwrap();
    assert_eq!(names_only(&index, "todo").len(), 1);
    assert_eq!(names_only(&index, "track01").len(), 1);
    assert!(names_only(&index, "head").is_empty());

    std::fs::remove_file(root.join("todo.txt")).unwrap();
    crawler.recheck(&root, &root.join("todo.txt")).unwrap();
    std::fs::rename(root.join("album"), root.join("records")).unwrap();
    crawler
        .recheck_tree(&root, &root.join("album"), &mut |_| {})
        .unwrap();
    crawler
        .recheck_tree(&root, &root.join("records"), &mut |_| {})
        .unwrap();
    crawler.commit().unwrap();
    index.reload().unwrap();
    assert!(names_only(&index, "todo").is_empty());
    let track = names_only(&index, "track01");
    assert_eq!(
        ids(&track),
        [root.join("records/disc1/track01.flac").to_str().unwrap()]
    );
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn past_the_ceiling_the_oldest_contents_go_first_and_names_stay() {
    let base = scratch("ceiling");
    let root = base.join("home");
    std::fs::create_dir_all(&root).unwrap();
    let words: Vec<String> = (0..4000).map(|k| format!("w{k:04}x")).collect();
    for k in 0..40 {
        let text: String = words
            .iter()
            .skip(k * 50)
            .take(2000)
            .cloned()
            .collect::<Vec<_>>()
            .join(" ");
        std::fs::write(root.join(format!("note{k:02}.txt")), text).unwrap();
    }
    let rules = Rules::new(&limits(1 << 30), &Settings::default());
    let dir = base.join("index");
    let mut crawler = Crawler::new(
        Writer::open(&dir, 32 << 20).unwrap(),
        rules.clone(),
        1 << 30,
    );
    crawler
        .walk(&root, &mut HashSet::new(), false, &mut |_| {})
        .unwrap();
    crawler.commit().unwrap();
    let full = crawler.writer.size_bytes();
    drop(crawler);

    // The same index under a ceiling below what it holds.
    let ceiling = full * 2 / 3;
    let mut crawler = Crawler::new(Writer::open(&dir, 32 << 20).unwrap(), rules, ceiling);
    crawler.commit().unwrap();
    let size = crawler.writer.size_bytes();
    assert!(size <= ceiling, "{size} > {ceiling} (was {full})");
    assert!(!crawler.is_full());
    let index = Index::open_ro(&dir).unwrap();
    for k in 0..40 {
        // The name first; its neighbours at one letter's distance after it.
        let hits = names_only(&index, &format!("note{k:02}"));
        let path = root.join(format!("note{k:02}.txt"));
        assert_eq!(
            hits.first().map(|h| Path::new(&h.id)),
            Some(path.as_path()),
            "every name stays"
        );
    }
    let emptied = (0..40)
        .filter(|k| {
            crawler
                .writer
                .stamp(root.join(format!("note{k:02}.txt")).to_str().unwrap())
                .is_some_and(|s| s.body_bytes == 0)
        })
        .count();
    assert!(emptied > 0 && emptied < 40, "{emptied} emptied");
    let _ = std::fs::remove_dir_all(&base);
}

/// A video source that answers from a list.
struct Fake {
    videos: Vec<Found>,
    generation: u64,
    asked: Vec<u64>,
}

impl Videos for Fake {
    fn changed_since(&mut self, generation: u64) -> Result<(Vec<Found>, u64), String> {
        self.asked.push(generation);
        Ok((self.videos.clone(), self.generation))
    }

    fn shown(&mut self) -> Result<u64, String> {
        Ok(self.videos.iter().filter(|v| !v.hidden).count() as u64)
    }

    fn watched(&self) -> Option<PathBuf> {
        None
    }
}

fn video(id: &str, title: &str, hidden: bool) -> Found {
    Found {
        id: id.to_owned(),
        title: title.to_owned(),
        channel: "Harbour Channel".to_owned(),
        text: "a film about lighthouses".to_owned(),
        published_at_s: 1_700_000_000,
        hidden,
    }
}

fn config(base: &Path, sources: Sources) -> Config {
    Config {
        dir: base.join("index"),
        limits: limits(1 << 30),
        memory_bytes: 32 << 20,
        roots: Some(vec![base.join("home")]),
        sources,
        home: None,
        settings_dir: None,
        man_candidates: vec![base.join("man")],
        packages: Some(base.join("index.tsv")),
        videos: None,
        system_triggers: Vec::new(),
    }
}

#[test]
fn a_pass_takes_every_source_and_keeps_what_it_took_in_the_commit() {
    let base = scratch("pass");
    std::fs::create_dir_all(base.join("home")).unwrap();
    std::fs::write(base.join("home/lighthouse.txt"), "").unwrap();
    std::fs::create_dir_all(base.join("man/man1")).unwrap();
    std::fs::write(
        base.join("man/man1/beacon.1"),
        ".SH NAME\nbeacon \\- light a lighthouse\n",
    )
    .unwrap();
    std::fs::write(
        base.join("index.tsv"),
        format!(
            "{}\t{}\t1\nlighthouse-keeper\tkeeper\t1\t\tkeeper\tMIT\t\tKeeps a lighthouse lit\n",
            ronoctua_software::nix_index::HEADER,
            ronoctua_software::nix_index::FORMAT
        ),
    )
    .unwrap();
    let mut first = config(&base, Sources::ALL);
    first.videos = Some(Box::new(Fake {
        videos: vec![
            video("abc", "Lighthouses of the north", false),
            video("def", "hidden lighthouse", true),
        ],
        generation: 7,
        asked: Vec::new(),
    }));
    let mut unit = Unit::new(first).unwrap();
    let pass = unit.pass(None).unwrap();
    assert_eq!((pass.manuals, pass.packages, pass.videos), (1, 1, 1));
    let index = Index::open_ro(&base.join("index")).unwrap();
    let hits = ask(&index, "lighthouse");
    let got: HashSet<(Kind, &str)> = hits.iter().map(|h| (h.kind, h.id.as_str())).collect();
    for want in [
        (Kind::Man, "man:beacon.1"),
        (Kind::Package, "pkg:lighthouse-keeper"),
        (Kind::Video, "yt:abc"),
    ] {
        assert!(got.contains(&want), "{want:?} in {got:?}");
    }
    assert!(
        !got.iter().any(|(_, id)| *id == "yt:def"),
        "a hidden video is not offered"
    );
    let video_hit = hits.iter().find(|h| h.id == "yt:abc").unwrap();
    assert_eq!(video_hit.note.as_deref(), Some("Harbour Channel"));
    let file_hit = hits.iter().find(|h| h.kind == Kind::File).unwrap();
    assert!(file_hit.id.ends_with("lighthouse.txt"));

    // The state went with the commit: a second unit sees the same signatures
    // and relists nothing.
    let writer = unit.into_writer();
    let state = writer.state().clone();
    assert_eq!(state.videos, Some(7));
    assert!(state.packages.is_some() && state.manuals.is_some());
    assert_eq!(state.contents, Some(true));
    drop(writer);
    let mut unit = Unit::new(config(&base, Sources::parse("manuals,packages").unwrap())).unwrap();
    let again = unit.pass(None).unwrap();
    assert_eq!((again.manuals, again.packages), (0, 0));
    let _ = std::fs::remove_dir_all(&base);
}

/// Waits (the door's way: `meta.json` watched, a reload per commit) until
/// `words` find `wanted`, or `patience` runs out.
fn seen_by_a_reader(
    dir: &Path,
    words: &str,
    wanted: impl Fn(&[Hit]) -> bool,
    patience: Duration,
) -> bool {
    let (told, heard) = std::sync::mpsc::channel::<()>();
    let _watch = crate::on_commit(dir, move || told.send(()).is_ok()).unwrap();
    let deadline = std::time::Instant::now() + patience;
    let mut index: Option<Index> = None;
    loop {
        match index.as_mut() {
            None => index = Index::open_ro(dir).ok(),
            Some(i) => i.reload().unwrap(),
        }
        if index
            .as_ref()
            .is_some_and(|i| wanted(&names_only(i, words)))
        {
            return true;
        }
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        if left.is_zero() || heard.recv_timeout(left).is_err() {
            return false;
        }
    }
}

#[test]
fn the_unit_follows_the_kernel_and_a_door_sees_each_commit() {
    let base = scratch("serve");
    let root = base.join("home");
    std::fs::create_dir_all(root.join("docs")).unwrap();
    std::fs::create_dir_all(base.join("index")).unwrap();
    let config = config(&base, Sources::parse("files").unwrap());
    std::thread::spawn(move || {
        let mut unit = Unit::new(config).unwrap();
        let _ = unit.serve();
    });
    let dir = base.join("index");
    let patience = Duration::from_secs(20);
    let has = |id: PathBuf| move |hits: &[Hit]| hits.iter().any(|h| Path::new(&h.id) == id);
    let lacks = |id: PathBuf| move |hits: &[Hit]| !hits.iter().any(|h| Path::new(&h.id) == id);

    // The first pass commits an index that knows `docs`.
    assert!(seen_by_a_reader(
        &dir,
        "docs",
        has(root.join("docs")),
        patience
    ));
    // AND IT SAID SO: the walk began from scratch, ended, and its process is
    // the one that wrote it (the test's own, here — not the unit's program,
    // so `alive` answers no and the brain would not mark it).
    // (Read until the pass's own line is there: a reader may see the commit
    // a moment before the unit writes that it landed.)
    let until = std::time::Instant::now() + patience;
    let said = loop {
        let said = crate::status::read(&dir).expect("the unit wrote its status");
        if said.phase == crate::Phase::Following || std::time::Instant::now() > until {
            break said;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(said.phase, crate::Phase::Following);
    assert!(said.scratch, "an empty index is a walk from scratch");
    assert_eq!(said.pid, std::process::id());
    assert!(said.documents > 0 && said.committed_s.is_some(), "{said:?}");
    // A file written after it.
    std::fs::write(root.join("docs/planted-lighthouse.txt"), "hello").unwrap();
    assert!(seen_by_a_reader(
        &dir,
        "planted lighthouse",
        has(root.join("docs/planted-lighthouse.txt")),
        patience
    ));
    // A folder made with something inside, in one go.
    std::fs::create_dir_all(root.join("fresh/inner")).unwrap();
    std::fs::write(root.join("fresh/inner/buoy.txt"), "").unwrap();
    assert!(seen_by_a_reader(
        &dir,
        "buoy",
        has(root.join("fresh/inner/buoy.txt")),
        patience
    ));
    // And a file inside that new folder, later: its watch was added.
    std::fs::write(root.join("fresh/inner/anchor.txt"), "").unwrap();
    assert!(seen_by_a_reader(
        &dir,
        "anchor",
        has(root.join("fresh/inner/anchor.txt")),
        patience
    ));
    // A folder moved: the old paths leave, the new ones come.
    std::fs::rename(root.join("fresh"), root.join("moved")).unwrap();
    assert!(seen_by_a_reader(
        &dir,
        "buoy",
        has(root.join("moved/inner/buoy.txt")),
        patience
    ));
    assert!(seen_by_a_reader(
        &dir,
        "buoy",
        lacks(root.join("fresh/inner/buoy.txt")),
        patience
    ));
    // A file removed.
    std::fs::remove_file(root.join("docs/planted-lighthouse.txt")).unwrap();
    assert!(seen_by_a_reader(
        &dir,
        "planted",
        lacks(root.join("docs/planted-lighthouse.txt")),
        patience
    ));
    let _ = std::fs::remove_dir_all(&base);
}

/// Words drawn from a vocabulary of `vocab` six-letter words, `count` of them,
/// by a fixed sequence (a test's text is the same at every run).
fn words_of(seed: u64, vocab: u64, count: usize) -> String {
    let mut x = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
    let mut out = String::new();
    for _ in 0..count {
        x = x
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let mut w = (x >> 33) % vocab;
        if !out.is_empty() {
            out.push(' ');
        }
        for _ in 0..6 {
            out.push(char::from(b'a' + u8::try_from(w % 26).unwrap()));
            w /= 26;
        }
    }
    out
}

#[test]
fn edits_at_the_ceiling_find_room_and_do_not_rewrite_the_index_each_time() {
    let base = scratch("room");
    let root = base.join("home");
    std::fs::create_dir_all(&root).unwrap();
    for k in 0..40u64 {
        std::fs::write(
            root.join(format!("planted{k:02}.txt")),
            words_of(k, 30_000, 1500),
        )
        .unwrap();
    }
    // What the tree weighs with no ceiling in reach.
    let free = {
        let dir = base.join("free");
        let mut crawler = Crawler::new(
            Writer::open(&dir, 32 << 20).unwrap(),
            Rules::new(&limits(1 << 30), &Settings::default()),
            1 << 30,
        );
        crawler
            .walk(&root, &mut HashSet::new(), false, &mut |_| {})
            .unwrap();
        crawler.commit().unwrap();
        crawler.writer.committed_bytes()
    };
    let ceiling = free * 2 / 3;
    let mut crawler = Crawler::new(
        Writer::open(&base.join("index"), 32 << 20).unwrap(),
        Rules::new(&limits(ceiling), &Settings::default()),
        ceiling,
    );
    crawler
        .walk(&root, &mut HashSet::new(), false, &mut |_| {})
        .unwrap();
    crawler.commit().unwrap();
    let built = crawler.held();
    assert_eq!(built.sheds, 1, "the build crossed the ceiling once");
    // It shed down to the target, not to the ceiling: room for what comes.
    let after = crawler.writer.committed_bytes();
    assert!(
        after <= shed_target(ceiling),
        "{after} > {} (ceiling {ceiling})",
        shed_target(ceiling)
    );

    // Twelve saves, each a new file a fifth the size of a planted one,
    // applied as the unit applies a batch: the path rechecked, a commit.
    let mut largest = 0;
    for k in 0..12u64 {
        let path = root.join(format!("saved{k:02}.txt"));
        std::fs::write(&path, words_of(100 + k, 30_000, 300)).unwrap();
        crawler.recheck(&root, &path).unwrap();
        crawler.commit().unwrap();
        // AT REST: a merge still in flight under a loaded machine holds its
        // inputs and its output at once (`Writer::committed_bytes` says so),
        // and reading the folder in that instant measured the load.
        largest = largest.max(crawler.writer.settled_size_bytes().unwrap());
    }
    let held = crawler.held();
    let sheds = held.sheds - built.sheds;
    let rewritten = held.rewritten_bytes - built.rewritten_bytes;
    assert!(
        sheds <= 2,
        "{sheds} sheds in twelve saves: each one rewrites most of the index"
    );
    assert!(
        rewritten <= 3 * ceiling,
        "twelve saves rewrote {rewritten} bytes of a {ceiling}-byte ceiling"
    );
    // And the folder, as the disk counts it once its merges end, stayed
    // under the ceiling.
    assert!(largest <= ceiling, "the folder held {largest} > {ceiling}");
    let _ = std::fs::remove_dir_all(&base);
}

/// A video catalogue as the real one answers: every video carries the
/// generation it last changed in, and a deleted one is gone — never answered
/// again, by `changed_since` or by the count.
#[derive(Default)]
struct Shelf {
    videos: BTreeMap<String, (Found, u64)>,
    generation: u64,
    whole_asks: u32,
    /// What was deleted, and in which generation — the tombstones a catalogue
    /// that says what left it answers (`Videos::gone_since`).
    gone: Vec<(String, u64)>,
}

impl Shelf {
    fn put(&mut self, video: Found) {
        self.generation += 1;
        self.videos
            .insert(video.id.clone(), (video, self.generation));
    }

    fn delete(&mut self, id: &str) {
        self.generation += 1;
        self.videos.remove(id);
        self.gone.push((id.to_owned(), self.generation));
    }
}

/// The same catalogue, telling what left it — as the video catalogue's does.
struct Telling(Arc<Mutex<Shelf>>);

impl Videos for Telling {
    fn changed_since(&mut self, generation: u64) -> Result<(Vec<Found>, u64), String> {
        Shared(Arc::clone(&self.0)).changed_since(generation)
    }

    fn shown(&mut self) -> Result<u64, String> {
        Shared(Arc::clone(&self.0)).shown()
    }

    fn watched(&self) -> Option<PathBuf> {
        None
    }

    fn gone_since(&mut self, generation: u64) -> Result<Vec<String>, String> {
        let shelf = self.0.lock().unwrap();
        Ok(shelf
            .gone
            .iter()
            .filter(|(_, g)| *g > generation)
            .map(|(id, _)| id.clone())
            .collect())
    }
}

/// FAILS IF: a video the catalogue says left it stays in the index, or its
/// leaving costs a whole take of the catalogue.
#[test]
fn a_video_the_catalogue_says_is_gone_leaves_by_its_id() {
    let base = scratch("videos-gone");
    std::fs::create_dir_all(base.join("home")).unwrap();
    let shelf = Arc::new(Mutex::new(Shelf::default()));
    {
        let mut s = shelf.lock().unwrap();
        s.put(video("aaa", "Harbour at dawn", false));
        s.put(video("bbb", "Harbour at noon", false));
    }
    let mut config = config(&base, Sources::parse("videos").unwrap());
    config.videos = Some(Box::new(Telling(Arc::clone(&shelf))));
    let dir = config.dir.clone();
    let mut unit = Unit::new(config).unwrap();
    unit.pass(None).unwrap();
    assert_eq!(videos_found(&dir, "harbour"), ["yt:aaa", "yt:bbb"]);
    shelf.lock().unwrap().delete("aaa");
    unit.follow_videos().unwrap();
    assert_eq!(videos_found(&dir, "harbour"), ["yt:bbb"]);
    assert_eq!(
        shelf.lock().unwrap().whole_asks,
        1,
        "the one whole take is the unit's start"
    );
    let _ = std::fs::remove_dir_all(&base);
}

struct Shared(Arc<Mutex<Shelf>>);

impl Videos for Shared {
    fn changed_since(&mut self, generation: u64) -> Result<(Vec<Found>, u64), String> {
        let mut shelf = self.0.lock().unwrap();
        if generation == 0 {
            shelf.whole_asks += 1;
        }
        let changed = shelf
            .videos
            .values()
            .filter(|(_, g)| *g > generation)
            .map(|(v, _)| v.clone())
            .collect();
        Ok((changed, shelf.generation))
    }

    fn shown(&mut self) -> Result<u64, String> {
        let shelf = self.0.lock().unwrap();
        Ok(shelf.videos.values().filter(|(v, _)| !v.hidden).count() as u64)
    }

    fn watched(&self) -> Option<PathBuf> {
        None
    }
}

/// The videos a reader finds by `words`.
fn videos_found(dir: &Path, words: &str) -> Vec<String> {
    let index = Index::open_ro(dir).unwrap();
    let mut found: Vec<String> = ask(&index, words)
        .into_iter()
        .filter(|h| h.kind == Kind::Video)
        .map(|h| h.id)
        .collect();
    found.sort();
    found
}

#[test]
fn a_video_deleted_while_the_unit_runs_leaves_the_index() {
    let base = scratch("videos");
    std::fs::create_dir_all(base.join("home")).unwrap();
    let shelf = Arc::new(Mutex::new(Shelf::default()));
    {
        let mut s = shelf.lock().unwrap();
        s.put(video("aaa", "Lighthouse at dawn", false));
        s.put(video("bbb", "Lighthouse at noon", false));
    }
    let mut config = config(&base, Sources::parse("videos").unwrap());
    config.videos = Some(Box::new(Shared(Arc::clone(&shelf))));
    let dir = config.dir.clone();
    let mut unit = Unit::new(config).unwrap();
    unit.pass(None).unwrap();
    assert_eq!(videos_found(&dir, "lighthouse"), ["yt:aaa", "yt:bbb"]);
    let whole = |shelf: &Arc<Mutex<Shelf>>| shelf.lock().unwrap().whole_asks;
    assert_eq!(whole(&shelf), 1, "the unit starts with a whole take");

    // An addition is taken by itself, with no whole take.
    shelf
        .lock()
        .unwrap()
        .put(video("ccc", "Lighthouse at dusk", false));
    unit.follow_videos().unwrap();
    assert_eq!(
        videos_found(&dir, "lighthouse"),
        ["yt:aaa", "yt:bbb", "yt:ccc"]
    );
    assert_eq!(whole(&shelf), 1);

    // A deletion is never answered: the count tells it, and it leaves.
    shelf.lock().unwrap().delete("bbb");
    unit.follow_videos().unwrap();
    assert_eq!(videos_found(&dir, "lighthouse"), ["yt:aaa", "yt:ccc"]);
    assert_eq!(whole(&shelf), 2, "one whole take, for the deletion");

    // Hiding is answered as a change, and needs no whole take.
    shelf
        .lock()
        .unwrap()
        .put(video("aaa", "Lighthouse at dawn", true));
    unit.follow_videos().unwrap();
    assert_eq!(videos_found(&dir, "lighthouse"), ["yt:ccc"]);
    assert_eq!(whole(&shelf), 2);
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn a_root_that_is_a_link_is_walked_as_the_folder_it_names() {
    let base = scratch("root-link");
    std::fs::create_dir_all(base.join("disk/docs")).unwrap();
    std::fs::write(base.join("disk/docs/ledger.txt"), "").unwrap();
    let root = base.join("dropbox");
    std::os::unix::fs::symlink(base.join("disk"), &root).unwrap();
    let mut crawler = Crawler::new(
        Writer::open(&base.join("index"), 32 << 20).unwrap(),
        Rules::new(&limits(1 << 30), &Settings::default()),
        1 << 30,
    );
    let mut folders = Vec::new();
    crawler
        .walk(&root, &mut HashSet::new(), false, &mut |f| {
            folders.push(f.to_path_buf())
        })
        .unwrap();
    crawler.commit().unwrap();
    let index = Index::open_ro(&base.join("index")).unwrap();
    // Under the root's own name, as the person sees it.
    assert_eq!(
        ids(&names_only(&index, "ledger")),
        [root.join("docs/ledger.txt").to_str().unwrap()]
    );
    assert_eq!(folders, [root.clone(), root.join("docs")]);
    // And a tree rechecked from the root keeps it (followed there too).
    crawler.recheck_tree(&root, &root, &mut |_| {}).unwrap();
    crawler.commit().unwrap();
    let mut index = index;
    index.reload().unwrap();
    assert_eq!(names_only(&index, "ledger").len(), 1);
    drop(crawler);

    // The unit watches it: a file written straight under the root is seen.
    let mut config = config(&base, Sources::parse("files").unwrap());
    config.roots = Some(vec![root.clone()]);
    config.dir = base.join("index-live");
    let dir = config.dir.clone();
    std::thread::spawn(move || {
        let mut unit = Unit::new(config).unwrap();
        let _ = unit.serve();
    });
    let patience = Duration::from_secs(20);
    assert!(seen_by_a_reader(
        &dir,
        "ledger",
        |h| !h.is_empty(),
        patience
    ));
    std::fs::write(root.join("compass.txt"), "").unwrap();
    let wanted = root.join("compass.txt");
    assert!(seen_by_a_reader(
        &dir,
        "compass",
        |hits| hits.iter().any(|h| Path::new(&h.id) == wanted),
        patience
    ));
    let _ = std::fs::remove_dir_all(&base);
}

/// Counts the commits `dir` makes from now on (a door's watch).
fn commits_of(dir: &Path) -> (ronoctua_files::notice::Notice, Arc<Mutex<u32>>) {
    let count = Arc::new(Mutex::new(0u32));
    let counted = Arc::clone(&count);
    let watch = crate::on_commit(dir, move || {
        *counted.lock().unwrap() += 1;
        true
    })
    .unwrap();
    (watch, count)
}

#[test]
fn the_index_folder_is_never_walked_whatever_the_roots() {
    let base = scratch("own");
    let home = base.join("home");
    std::fs::create_dir_all(home.join("notes")).unwrap();
    std::fs::write(home.join("notes/tide.txt"), "").unwrap();
    // The index inside a root, where nothing hidden or skipped keeps it out,
    // and a second root that reaches it through a link.
    let dir = home.join("cache/ronoctua/index");
    std::os::unix::fs::symlink(home.join("cache/ronoctua"), base.join("alias")).unwrap();
    let mut first = config(&base, Sources::parse("files").unwrap());
    first.dir = dir.clone();
    first.roots = Some(vec![home.clone(), base.join("alias")]);
    let mut unit = Unit::new(first).unwrap();
    unit.pass(None).unwrap();
    let writer = unit.into_writer();
    let mut all = Vec::new();
    writer.each_id("/", |id| all.push(id.to_owned()));
    assert!(all.iter().any(|id| id.ends_with("tide.txt")));
    for id in &all {
        assert!(
            !Path::new(id).starts_with(&dir)
                && !Path::new(id).starts_with(base.join("alias/index")),
            "{id} is the index's own"
        );
    }
    drop(writer);

    // And the unit following that tree makes one commit, not one a second
    // (from an index of its own: the reader waits for ITS first commit).
    std::fs::remove_dir_all(&dir).unwrap();
    let mut live = config(&base, Sources::parse("files").unwrap());
    live.dir = dir.clone();
    live.roots = Some(vec![home]);
    std::thread::spawn(move || {
        let mut unit = Unit::new(live).unwrap();
        let _ = unit.serve();
    });
    let patience = Duration::from_secs(20);
    assert!(seen_by_a_reader(&dir, "tide", |h| !h.is_empty(), patience));
    let (_watch, commits) = commits_of(&dir);
    std::thread::sleep(Duration::from_secs(4));
    assert_eq!(
        *commits.lock().unwrap(),
        0,
        "a commit fed the next batch: the index's folder was walked"
    );
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn a_file_held_open_in_the_home_is_not_a_batch_a_second() {
    let base = scratch("held");
    let home = base.join("home");
    std::fs::create_dir_all(home.join("docs")).unwrap();
    // The home is walked AND holds a trigger (the profile link), as it does
    // for the person.
    let mut config = config(&base, Sources::parse("files,manuals").unwrap());
    config.system_triggers = vec![home.join(".nix-profile")];
    let dir = config.dir.clone();
    std::thread::spawn(move || {
        let mut unit = Unit::new(config).unwrap();
        let _ = unit.serve();
    });
    let patience = Duration::from_secs(20);
    assert!(seen_by_a_reader(&dir, "docs", |h| !h.is_empty(), patience));

    use std::io::Write as _;
    let mut log = std::fs::File::create(home.join("session.log")).unwrap();
    assert!(seen_by_a_reader(
        &dir,
        "session",
        |h| !h.is_empty(),
        patience
    ));
    let (_watch, commits) = commits_of(&dir);
    // Written and flushed, never closed, a little slower than the unit's
    // quiet second.
    for line in 0..3 {
        writeln!(log, "line {line}").unwrap();
        log.sync_data().unwrap();
        std::thread::sleep(Duration::from_millis(1300));
    }
    std::thread::sleep(Duration::from_millis(700));
    assert_eq!(
        *commits.lock().unwrap(),
        0,
        "a write to an open file in the home made a batch"
    );
    // Let go of it: that is a change, and it is heard.
    drop(log);
    let deadline = std::time::Instant::now() + patience;
    while *commits.lock().unwrap() == 0 && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(
        *commits.lock().unwrap() >= 1,
        "closing the file was not heard"
    );
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn a_settings_folder_made_after_the_start_is_watched_from_then_on() {
    let base = scratch("late-settings");
    let home = base.join("home");
    std::fs::create_dir_all(home.join("docs")).unwrap();
    std::fs::write(home.join("docs/lantern.txt"), "").unwrap();
    std::fs::create_dir_all(base.join("config")).unwrap();
    let settings_dir = base.join("config/ronoctua");
    let mut config = config(&base, Sources::parse("files").unwrap());
    config.roots = None;
    config.home = Some(home.clone());
    config.settings_dir = Some(settings_dir.clone());
    let dir = config.dir.clone();
    std::thread::spawn(move || {
        let mut unit = Unit::new(config).unwrap();
        let _ = unit.serve();
    });
    let patience = Duration::from_secs(20);
    let lantern = home.join("docs/lantern.txt");
    let has = |hits: &[Hit]| hits.iter().any(|h| Path::new(&h.id) == lantern);
    assert!(seen_by_a_reader(&dir, "lantern", has, patience));
    assert!(!settings_dir.exists(), "the unit made the person's folder");

    // The person saves for the first time: the folder is made, the file
    // written into it.
    let store: ronoctua_store::Store<Settings> =
        ronoctua_store::Store::at(Some(settings_dir), crate::settings::FILE);
    let skipping = Settings {
        skip: vec!["docs".to_owned()],
        ..Settings::default()
    };
    store.write(&skipping).unwrap();
    assert!(seen_by_a_reader(&dir, "lantern", |h| !has(h), patience));

    // And saves again, into the folder that now exists.
    store.write(&Settings::default()).unwrap();
    assert!(
        seen_by_a_reader(&dir, "lantern", has, patience),
        "the second save of index.json was not heard"
    );
    let _ = std::fs::remove_dir_all(&base);
}
