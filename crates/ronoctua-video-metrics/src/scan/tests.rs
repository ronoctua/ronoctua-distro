//! The scanner over a fake YouTube: every decision a run makes, read back
//! from the catalogue it wrote.

use std::cell::RefCell;
use std::collections::HashMap;

use super::*;
use crate::db::tests::Scratch;
use crate::person::{NewChannel, Person};
use crate::read::{Catalog, Hidden};
use crate::{Rating, Rule, RuleKind, Weights};

/// A moment to scan at: 2026-09-30T00:00:00Z.
const NOW: i64 = 1_790_726_400;

fn channel(n: u8) -> ChannelId {
    ChannelId(format!("UC{}", char::from(b'a' + n).to_string().repeat(22)))
}

fn video(n: u32) -> VideoId {
    VideoId(format!("v{n:010}"))
}

/// Days since the epoch as a date (Howard Hinnant's `civil_from_days`).
fn civil(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

fn rfc3339(s: i64) -> String {
    let (y, m, d) = civil(s.div_euclid(DAY_S));
    let t = s.rem_euclid(DAY_S);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}+00:00",
        t / 3600,
        t % 3600 / 60,
        t % 60
    )
}

/// An Atom feed of these entries: (video, published at, a Short's link).
fn atom(channel: &ChannelId, entries: &[(VideoId, i64, bool)]) -> Vec<u8> {
    let mut xml = format!(
        r#"<?xml version="1.0"?><feed xmlns:yt="http://www.youtube.com/xml/schemas/2015"
             xmlns:media="http://search.yahoo.com/mrss/" xmlns="http://www.w3.org/2005/Atom">
           <yt:channelId>{}</yt:channelId><title>Channel</title>"#,
        channel.0
    );
    for (id, published, short) in entries {
        let url = if *short {
            format!("https://www.youtube.com/shorts/{}", id.0)
        } else {
            format!("https://www.youtube.com/watch?v={}", id.0)
        };
        xml.push_str(&format!(
            r#"<entry><yt:videoId>{id}</yt:videoId><yt:channelId>{ch}</yt:channelId>
               <title>Video {id}</title><link rel="alternate" href="{url}"/>
               <published>{p}</published>
               <media:group><media:description>About {id}</media:description>
               <media:community><media:statistics views="7"/></media:community></media:group>
               </entry>"#,
            id = id.0,
            ch = channel.0,
            p = rfc3339(*published),
        ));
    }
    xml.push_str("</feed>");
    xml.into_bytes()
}

/// A YouTube that answers what it was told to, and counts what it was asked.
#[derive(Default)]
struct Fake {
    feeds: HashMap<(Which, ChannelId), Result<Vec<u8>, Missed>>,
    tabs: HashMap<(ChannelId, Tab), Vec<TabEntry>>,
    details: HashMap<String, Result<Details, Missed>>,
    thumbs: bool,
    asked_feeds: RefCell<Vec<(Which, ChannelId)>>,
    asked_tabs: RefCell<Vec<(ChannelId, Tab)>>,
    asked_details: RefCell<Vec<String>>,
    pauses: RefCell<u32>,
}

impl Fake {
    fn feed(mut self, which: Which, channel: &ChannelId, answer: Result<Vec<u8>, Missed>) -> Self {
        self.feeds.insert((which, channel.clone()), answer);
        self
    }

    fn details(mut self, id: &VideoId, answer: Result<Details, Missed>) -> Self {
        self.details
            .insert(format!("https://www.youtube.com/watch?v={}", id.0), answer);
        self
    }
}

impl Reach for Fake {
    fn feed(&self, which: Which, channel: &ChannelId) -> Result<Vec<u8>, Missed> {
        self.asked_feeds.borrow_mut().push((which, channel.clone()));
        self.feeds
            .get(&(which, channel.clone()))
            .cloned()
            .unwrap_or(Err(Missed::NotFound))
    }

    fn tab(&self, channel: &ChannelId, tab: Tab, most: usize) -> Result<Vec<TabEntry>, Missed> {
        self.asked_tabs.borrow_mut().push((channel.clone(), tab));
        let mut entries = self
            .tabs
            .get(&(channel.clone(), tab))
            .cloned()
            .unwrap_or_default();
        entries.truncate(most);
        Ok(entries)
    }

    fn details(&self, url: &str) -> Result<Details, Missed> {
        self.asked_details.borrow_mut().push(url.to_owned());
        self.details
            .get(url)
            .cloned()
            .unwrap_or_else(|| Err(Missed::Failed("no answer".to_owned())))
    }

    fn thumb(&self, _id: &VideoId) -> Result<Vec<u8>, Missed> {
        if self.thumbs {
            let mut jpeg = vec![0xff, 0xd8, 0xff, 0xe0];
            jpeg.resize(600, 1);
            Ok(jpeg)
        } else {
            Err(Missed::Failed("no thumbnail".to_owned()))
        }
    }

    fn pause(&self) {
        *self.pauses.borrow_mut() += 1;
    }
}

/// A catalogue with these channels added.
fn catalogue(scratch: &Scratch, channels: &[ChannelId]) -> Person {
    let person = Person::open(&scratch.catalogue()).unwrap();
    for id in channels {
        person
            .add_channel(&NewChannel {
                id: id.clone(),
                handle: None,
                title: format!("Channel {}", id.0),
                url: format!("https://www.youtube.com/channel/{}", id.0),
                avatar_url: None,
            })
            .unwrap();
    }
    person
}

fn skipped(scratch: &Scratch) -> HashMap<String, String> {
    let conn = rusqlite::Connection::open(scratch.catalogue()).unwrap();
    let mut statement = conn.prepare("SELECT video, reason FROM skipped").unwrap();
    statement
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

fn catalogued(scratch: &Scratch) -> Vec<String> {
    let conn = rusqlite::Connection::open(scratch.catalogue()).unwrap();
    let mut statement = conn.prepare("SELECT id FROM videos ORDER BY id").unwrap();
    statement
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

fn feeds_only() -> Options {
    Options {
        details_most: 0,
        feeds_only: true,
    }
}

fn candidate(n: u32, published: Option<i64>) -> Candidate {
    Candidate {
        id: video(n),
        url: format!("https://www.youtube.com/watch?v={}", video(n).0),
        title: String::new(),
        description: String::new(),
        published_at_s: published,
        duration_s: None,
        short: None,
        live_status: None,
        thumb_url: None,
        views: None,
    }
}

#[test]
fn a_first_scan_of_forty_old_videos_takes_ten_and_tombstones_the_rest() {
    // Forty videos, one every twelve hours back from now: twenty-nine are
    // inside the fourteen days, eleven outside.
    let candidates: Vec<Candidate> = (0..40)
        .map(|k| candidate(k, Some(NOW - i64::from(k) * DAY_S / 2)))
        .collect();
    let decided = plan(
        None,
        candidates,
        &|_| false,
        &Settings::default(),
        false,
        NOW,
    );
    let taken: Vec<VideoId> = decided.take.iter().map(|c| c.id.clone()).collect();
    assert_eq!(
        taken,
        (0..10).map(video).collect::<Vec<_>>(),
        "the ten newest"
    );
    assert_eq!(decided.skip.len(), 30);
    assert!(decided.skip.iter().all(|(_, why)| *why == Skip::FirstScan));

    // All forty OLDER than the window: nothing is taken, all are tombstones.
    let old: Vec<Candidate> = (0..40)
        .map(|k| candidate(k, Some(NOW - 15 * DAY_S - i64::from(k) * DAY_S)))
        .collect();
    let decided = plan(None, old, &|_| false, &Settings::default(), false, NOW);
    assert!(decided.take.is_empty());
    assert_eq!(decided.skip.len(), 40);

    // Known ids are never decided again; undated ones wait for a date.
    let mixed = vec![
        candidate(1, Some(NOW)),
        candidate(2, None),
        candidate(3, Some(NOW)),
    ];
    let decided = plan(
        None,
        mixed,
        &|id| *id == video(1),
        &Settings::default(),
        false,
        NOW,
    );
    assert_eq!(decided.take.len(), 1);
    assert_eq!(decided.take[0].id, video(3));
    assert!(decided.skip.is_empty(), "the undated one is left for later");
}

#[test]
fn a_first_scan_through_the_feeds_catalogues_the_ten_newest() {
    let scratch = Scratch::new("first-scan");
    let a = channel(0);
    let person = catalogue(&scratch, std::slice::from_ref(&a));
    // Fifteen entries 1.2 days apart: twelve inside fourteen days.
    let entries: Vec<(VideoId, i64, bool)> = (0..15)
        .map(|k| (video(k), NOW - i64::from(k) * DAY_S * 6 / 5, false))
        .collect();
    let fake = Fake::default().feed(Which::Videos, &a, Ok(atom(&a, &entries)));
    let report = run_with(person.path(), &fake, &feeds_only(), NOW, None).unwrap();
    assert_eq!(
        (report.added, report.skipped, report.channels_failed),
        (10, 5, 0)
    );
    assert_eq!(
        catalogued(&scratch),
        (0..10).map(|k| video(k).0).collect::<Vec<_>>()
    );
    let tombs = skipped(&scratch);
    assert_eq!(tombs.len(), 5);
    assert!(tombs.values().all(|r| r == "first-scan"));
    // The Shorts feed was asked (and is not there), the backlog was not.
    assert!(fake.asked_feeds.borrow().contains(&(Which::Shorts, a)));
    assert!(
        fake.asked_tabs.borrow().is_empty(),
        "a first scan never lists the tab"
    );
    let reader = Catalog::open_ro(person.path()).unwrap();
    let channels = reader.channels().unwrap();
    assert_eq!(
        channels[0].reference_at_s,
        Some(NOW),
        "the reference is the scan's start"
    );
    assert_eq!(channels[0].videos, 10);
}

#[test]
fn a_later_scan_takes_what_is_new_since_the_reference_less_a_day() {
    let scratch = Scratch::new("later");
    let a = channel(0);
    let person = catalogue(&scratch, std::slice::from_ref(&a));
    let first = vec![(video(1), NOW - DAY_S, false)];
    let fake = Fake::default().feed(Which::Videos, &a, Ok(atom(&a, &first)));
    run_with(person.path(), &fake, &feeds_only(), NOW, None).unwrap();

    let later = NOW + 3 * DAY_S;
    let second = vec![
        (video(4), NOW + DAY_S, false),
        (video(3), NOW - DAY_S / 2, false),
        (video(2), NOW - 2 * DAY_S, false),
        (video(1), NOW - DAY_S, false),
    ];
    let fake = Fake::default().feed(Which::Videos, &a, Ok(atom(&a, &second)));
    let report = run_with(person.path(), &fake, &feeds_only(), later, None).unwrap();
    assert_eq!((report.added, report.skipped), (2, 1));
    assert_eq!(
        catalogued(&scratch),
        [video(1).0, video(3).0, video(4).0],
        "inside the day of grace, and after"
    );
    assert_eq!(
        skipped(&scratch).get(&video(2).0).map(String::as_str),
        Some("before-reference")
    );
    assert!(
        fake.asked_tabs.borrow().is_empty(),
        "a feed with known ids hides no backlog"
    );
}

#[test]
fn a_deleted_video_never_comes_back() {
    let scratch = Scratch::new("deleted");
    let a = channel(0);
    let person = catalogue(&scratch, std::slice::from_ref(&a));
    let entries = vec![
        (video(1), NOW - DAY_S, false),
        (video(2), NOW - DAY_S, false),
    ];
    let fake = Fake::default().feed(Which::Videos, &a, Ok(atom(&a, &entries)));
    run_with(person.path(), &fake, &feeds_only(), NOW, None).unwrap();
    let reader = Catalog::open_ro(person.path()).unwrap();
    reader.delete(&video(1)).unwrap();
    assert_eq!(reader.delete(&video(1)).unwrap_err().why, Why::NotFound);

    // Seen again by the next scan, inside the window: not taken.
    let fake = Fake::default().feed(Which::Videos, &a, Ok(atom(&a, &entries)));
    let report = run_with(person.path(), &fake, &feeds_only(), NOW + DAY_S, None).unwrap();
    assert_eq!(report.added, 0);
    assert_eq!(catalogued(&scratch), [video(2).0]);
    assert_eq!(
        skipped(&scratch).get(&video(1).0).map(String::as_str),
        Some("deleted")
    );

    // Nor after its channel is removed and added again (a first scan anew).
    person.remove_channel(&a).unwrap();
    assert!(catalogued(&scratch).is_empty());
    let person = catalogue(&scratch, std::slice::from_ref(&a));
    let fake = Fake::default().feed(Which::Videos, &a, Ok(atom(&a, &entries)));
    let report = run_with(person.path(), &fake, &feeds_only(), NOW + 2 * DAY_S, None).unwrap();
    assert_eq!(report.added, 0, "both are tombstones now");
    assert!(catalogued(&scratch).is_empty());
    // And the index hears what went.
    let gone = Catalog::open_ro(person.path())
        .unwrap()
        .gone_since(0)
        .unwrap();
    assert_eq!(gone, [video(1), video(2)]);
}

/// FAILS IF: the tombstones stop being counted by their reason — every
/// reason present, the zeros included, in the enum's order — or a reason this
/// build does not know is counted as another instead of refused.
#[test]
fn the_tombstones_are_counted_by_reason() {
    let scratch = Scratch::new("tombs");
    let a = channel(0);
    let person = catalogue(&scratch, std::slice::from_ref(&a));
    let entries: Vec<(VideoId, i64, bool)> = (0..15)
        .map(|k| (video(k), NOW - i64::from(k) * DAY_S * 6 / 5, false))
        .collect();
    let fake = Fake::default().feed(Which::Videos, &a, Ok(atom(&a, &entries)));
    run_with(person.path(), &fake, &feeds_only(), NOW, None).unwrap();
    let reader = Catalog::open_ro(person.path()).unwrap();
    reader.delete(&video(0)).unwrap();
    let counts = reader.skipped().unwrap();
    let order: Vec<Skip> = counts.iter().map(|(s, _)| *s).collect();
    assert_eq!(order, Skip::ALL, "every reason, in order");
    let of = |skip: Skip| counts.iter().find(|(s, _)| *s == skip).map(|(_, n)| *n);
    assert_eq!(of(Skip::FirstScan), Some(5));
    assert_eq!(of(Skip::Deleted), Some(1));
    assert_eq!(of(Skip::Unavailable), Some(0), "a zero is a count too");
    // A REASON NO BUILD WROTE is refused, never a plausible other one.
    let conn = rusqlite::Connection::open(person.path()).unwrap();
    let changed = conn
        .execute(
            "UPDATE skipped SET reason = 'melted' WHERE video = ?1",
            rusqlite::params![video(12).0],
        )
        .unwrap();
    assert_eq!(changed, 1, "the planted reason reached no tombstone");
    drop(conn);
    assert_eq!(reader.skipped().unwrap_err().why, Why::Unreadable);
}

#[test]
fn a_bot_check_stops_the_run_and_is_recorded() {
    let scratch = Scratch::new("bot-check");
    let a = channel(0);
    let person = catalogue(&scratch, std::slice::from_ref(&a));
    let entries: Vec<(VideoId, i64, bool)> = (1..=3)
        .map(|k| (video(k), NOW - i64::from(k) * 3600, false))
        .collect();
    let said = "ERROR: [youtube] v0000000002: Sign in to confirm you're not a bot";
    let fake = Fake::default()
        .feed(Which::Videos, &a, Ok(atom(&a, &entries)))
        .details(
            &video(1),
            Ok(Details {
                id: video(1).0,
                title: "One".to_owned(),
                duration_s: Some(600),
                ..Details::default()
            }),
        )
        .details(
            &video(2),
            Err(Missed::Stop(Stop::BotCheck, Some(said.to_owned()))),
        )
        .details(
            &video(3),
            Ok(Details {
                id: video(3).0,
                title: "Three".to_owned(),
                ..Details::default()
            }),
        );
    let report = run_with(person.path(), &fake, &Options::default(), NOW, None).unwrap();
    assert_eq!(report.stopped, Some(Stop::BotCheck));
    assert_eq!((report.detailed, report.pending), (1, 2));
    assert_eq!(
        fake.asked_details.borrow().len(),
        2,
        "nothing is asked after the bot check"
    );
    assert_eq!(
        *fake.pauses.borrow(),
        1,
        "one pause between the two it asked"
    );
    let last = Catalog::open_ro(person.path())
        .unwrap()
        .last_scan()
        .unwrap()
        .unwrap();
    assert_eq!(last.stopped, Some(Stop::BotCheck));
    assert_eq!(last.said.as_deref(), Some(said));
    assert!(last.ended_at_s.is_some());
}

#[test]
fn a_429_on_a_feed_stops_every_channel_after_it_and_keeps_the_reference() {
    let scratch = Scratch::new("429");
    let (a, b) = (channel(0), channel(1));
    let person = catalogue(&scratch, &[a.clone(), b.clone()]);
    let fake = Fake::default()
        .feed(
            Which::Videos,
            &a,
            Err(Missed::Stop(Stop::TooManyRequests, None)),
        )
        .feed(Which::Videos, &b, Ok(atom(&b, &[(video(1), NOW, false)])));
    let report = run_with(person.path(), &fake, &Options::default(), NOW, None).unwrap();
    assert_eq!(report.stopped, Some(Stop::TooManyRequests));
    assert_eq!((report.channels, report.channels_failed), (1, 1));
    assert!(
        !fake.asked_feeds.borrow().iter().any(|(_, c)| *c == b),
        "the second channel is not asked"
    );
    assert!(fake.asked_details.borrow().is_empty());
    let channels = Catalog::open_ro(person.path()).unwrap().channels().unwrap();
    assert!(channels.iter().all(|c| c.reference_at_s.is_none()));
}

#[test]
fn a_failed_feed_keeps_the_reference_and_says_why() {
    let scratch = Scratch::new("failed-feed");
    let a = channel(0);
    let person = catalogue(&scratch, std::slice::from_ref(&a));
    let fake = Fake::default()
        .feed(Which::Videos, &a, Ok(atom(&a, &[(video(1), NOW, false)])))
        .feed(
            Which::Shorts,
            &a,
            Err(Missed::Failed("HTTP 503".to_owned())),
        );
    let report = run_with(person.path(), &fake, &feeds_only(), NOW, None).unwrap();
    assert_eq!(
        (report.added, report.channels_failed),
        (1, 1),
        "what was read is kept"
    );
    let channel = &Catalog::open_ro(person.path()).unwrap().channels().unwrap()[0];
    assert_eq!(
        channel.reference_at_s, None,
        "and asked again from the same point"
    );
    assert_eq!(channel.last_error.as_deref(), Some("HTTP 503"));
    assert_eq!(channel.last_try_at_s, Some(NOW));
}

#[test]
fn a_full_feed_of_new_videos_lists_the_tab_for_the_backlog() {
    let scratch = Scratch::new("backlog");
    let a = channel(0);
    let person = catalogue(&scratch, std::slice::from_ref(&a));
    let fake = Fake::default().feed(Which::Videos, &a, Ok(atom(&a, &[(video(0), NOW, false)])));
    run_with(person.path(), &fake, &feeds_only(), NOW, None).unwrap();

    // Two days later, thirty new videos: the feed shows fifteen, the tab the
    // thirty (and one that is older than the day of grace).
    let later = NOW + 2 * DAY_S;
    let newest: Vec<(VideoId, i64, bool)> = (1..=15)
        .map(|k| (video(k), later - i64::from(k) * 3600, false))
        .collect();
    let mut fake = Fake::default().feed(Which::Videos, &a, Ok(atom(&a, &newest)));
    let tab: Vec<TabEntry> = (1..=31)
        .map(|k| TabEntry {
            id: video(k).0,
            url: format!("https://www.youtube.com/watch?v={}", video(k).0),
            title: String::new(),
            duration_s: Some(300),
            published_at_s: Some(if k == 31 {
                NOW - 3 * DAY_S
            } else {
                later - i64::from(k) * 3600
            }),
            live_status: None,
            views: None,
        })
        .collect();
    fake.tabs.insert((a.clone(), Tab::Videos), tab);
    let report = run_with(person.path(), &fake, &feeds_only(), later, None).unwrap();
    assert_eq!(fake.asked_tabs.borrow().as_slice(), [(a, Tab::Videos)]);
    assert_eq!((report.added, report.skipped), (30, 1));
    assert_eq!(catalogued(&scratch).len(), 31);
}

#[test]
fn blocked_shorts_are_never_catalogued_and_their_feed_is_not_asked() {
    let scratch = Scratch::new("blocked");
    let a = channel(0);
    let person = catalogue(&scratch, std::slice::from_ref(&a));
    person
        .set_settings(&Settings {
            block_shorts: true,
            ..Settings::default()
        })
        .unwrap();
    // NO VIDEOS PLAYLIST: the whole channel's feed, with a Short in it.
    let entries = vec![(video(1), NOW, false), (video(2), NOW, true)];
    let fake = Fake::default().feed(Which::Channel, &a, Ok(atom(&a, &entries)));
    let report = run_with(person.path(), &fake, &feeds_only(), NOW, None).unwrap();
    assert_eq!((report.added, report.skipped), (1, 1));
    assert_eq!(catalogued(&scratch), [video(1).0]);
    assert_eq!(
        skipped(&scratch).get(&video(2).0).map(String::as_str),
        Some("blocked-short")
    );
    assert!(
        !fake
            .asked_feeds
            .borrow()
            .contains(&(Which::Shorts, a.clone())),
        "a channel that blocks Shorts is not asked for them"
    );

    // A video its details reveal as a Short (short and upright) is hidden,
    // with the reason, and comes back when the channel lets its Shorts in.
    let fake = Fake::default().details(
        &video(1),
        Ok(Details {
            id: video(1).0,
            title: "Upright".to_owned(),
            duration_s: Some(40),
            size_px: Some((1080, 1920)),
            ..Details::default()
        }),
    );
    run_with(person.path(), &fake, &Options::default(), NOW + 60, None).unwrap();
    let reader = Catalog::open_ro(person.path()).unwrap();
    let all = Filter {
        hidden: Hidden::Any,
        ..Filter::default()
    };
    let row = &reader
        .ranked(
            &all,
            Page {
                number: 0,
                per_page: 10,
            },
        )
        .unwrap()[0];
    assert!(row.is_short && row.hidden);
    person.set_channel_block_shorts(&a, Some(false)).unwrap();
    let row = &reader
        .ranked(
            &all,
            Page {
                number: 0,
                per_page: 10,
            },
        )
        .unwrap()[0];
    assert!(row.is_short && !row.hidden);
}

#[test]
fn details_fill_a_video_a_gone_one_is_tombstoned_and_three_failures_end_the_asking() {
    let scratch = Scratch::new("details");
    let a = channel(0);
    let person = catalogue(&scratch, std::slice::from_ref(&a));
    let entries: Vec<(VideoId, i64, bool)> = (1..=3)
        .map(|k| (video(k), NOW - i64::from(k) * 60, false))
        .collect();
    let fake = Fake::default()
        .feed(Which::Videos, &a, Ok(atom(&a, &entries)))
        .details(
            &video(1),
            Ok(Details {
                id: video(1).0,
                title: "Real title".to_owned(),
                description: "Long text".to_owned(),
                duration_s: Some(1234),
                size_px: Some((1920, 1080)),
                tags: vec!["Rust".to_owned(), "Tutorial".to_owned()],
                chapters: vec![ronoctua_ytdlp::youtube::Chapter {
                    start_s: 0,
                    title: "Start".to_owned(),
                }],
                captions: vec!["en".to_owned()],
                language: Some("en".to_owned()),
                ..Details::default()
            }),
        )
        .details(&video(2), Err(Missed::Gone));
    let report = run_with(person.path(), &fake, &Options::default(), NOW, None).unwrap();
    assert_eq!((report.detailed, report.skipped, report.pending), (1, 1, 1));
    let reader = Catalog::open_ro(person.path()).unwrap();
    let one = reader.video(&video(1)).unwrap().unwrap();
    assert_eq!(one.row.title, "Real title");
    assert_eq!(one.row.duration_s, Some(1234));
    assert_eq!(one.tags, ["Rust", "Tutorial"]);
    assert_eq!(one.chapters.len(), 1);
    assert_eq!(one.size_px, Some((1920, 1080)));
    assert_eq!(one.languages.0.as_deref(), Some("en"));
    assert!(reader.video(&video(2)).unwrap().is_none());
    assert_eq!(
        skipped(&scratch).get(&video(2).0).map(String::as_str),
        Some("unavailable")
    );

    // The third failed once; two runs more and it is asked no more.
    for run in 1..=2 {
        let fake = Fake::default();
        let report = run_with(person.path(), &fake, &Options::default(), NOW + run, None).unwrap();
        assert_eq!(fake.asked_details.borrow().len(), 1);
        assert_eq!(report.pending, u32::from(run < 2));
    }
    let fake = Fake::default();
    run_with(person.path(), &fake, &Options::default(), NOW + 9, None).unwrap();
    assert!(fake.asked_details.borrow().is_empty());
}

/// One video's details as the catalogue keeps them: state, tries, title,
/// description, duration, published at.
fn kept(
    scratch: &Scratch,
    id: &VideoId,
) -> (String, i64, String, String, Option<u32>, Option<i64>) {
    let conn = rusqlite::Connection::open(scratch.catalogue()).unwrap();
    conn.query_row(
        "SELECT details_state, details_tries, title, description, duration_s, published_at
         FROM videos WHERE id = ?1",
        [&id.0],
        |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
            ))
        },
    )
    .unwrap()
}

fn url(id: &VideoId) -> String {
    format!("https://www.youtube.com/watch?v={}", id.0)
}

#[test]
fn an_answer_never_blanks_what_is_known_and_a_premiere_waits_for_its_time() {
    let scratch = Scratch::new("premiere");
    let a = channel(0);
    let person = catalogue(&scratch, std::slice::from_ref(&a));
    let entries: Vec<(VideoId, i64, bool)> = (1..=3)
        .map(|k| (video(k), NOW - i64::from(k) * 60, false))
        .collect();
    let premiere = NOW + DAY_S;
    let fake = Fake::default()
        .feed(Which::Videos, &a, Ok(atom(&a, &entries)))
        // What yt-dlp answers when it read nothing of the video: no title.
        .details(
            &video(1),
            Ok(Details {
                id: video(1).0,
                ..Details::default()
            }),
        )
        // A premiere: no duration, and a release still to come.
        .details(
            &video(2),
            Ok(Details {
                id: video(2).0,
                title: "Soon".to_owned(),
                live_status: Some(crate::edit::UPCOMING.to_owned()),
                published_at_s: Some(premiere),
                ..Details::default()
            }),
        )
        // Fields, but no description: the feed's stays.
        .details(
            &video(3),
            Ok(Details {
                id: video(3).0,
                title: "Three".to_owned(),
                duration_s: Some(300),
                ..Details::default()
            }),
        );
    let report = run_with(person.path(), &fake, &Options::default(), NOW, None).unwrap();
    assert_eq!((report.detailed, report.pending), (2, 2));
    let (one, two) = (video(1), video(2));
    assert_eq!(
        kept(&scratch, &one),
        (
            "pending".to_owned(),
            1,
            format!("Video {}", one.0),
            format!("About {}", one.0),
            None,
            Some(NOW - 60)
        ),
        "no title is a failed try, and nothing is blanked"
    );
    assert_eq!(
        kept(&scratch, &two),
        (
            "pending".to_owned(),
            0,
            "Soon".to_owned(),
            format!("About {}", two.0),
            None,
            Some(premiere)
        ),
        "a premiere waits, and waiting is no try"
    );
    let three = kept(&scratch, &video(3));
    assert_eq!(
        (three.0.as_str(), three.3, three.4),
        ("done", format!("About {}", video(3).0), Some(300))
    );

    // BEFORE ITS TIME the premiere is not asked.
    let fake = Fake::default();
    run_with(person.path(), &fake, &Options::default(), NOW + 1, None).unwrap();
    assert_eq!(*fake.asked_details.borrow(), [url(&one)]);

    // Its time come and still not out: asked, and counted.
    let still = Details {
        id: two.0.clone(),
        title: "Soon".to_owned(),
        live_status: Some(crate::edit::UPCOMING.to_owned()),
        published_at_s: Some(premiere),
        ..Details::default()
    };
    let fake = Fake::default().details(&two, Ok(still));
    run_with(
        person.path(),
        &fake,
        &Options::default(),
        premiere + 1,
        None,
    )
    .unwrap();
    assert_eq!(
        *fake.asked_details.borrow(),
        [url(&one), url(&two)],
        "the one that can answer first"
    );
    assert_eq!(kept(&scratch, &two).0, "pending");
    assert_eq!(kept(&scratch, &two).1, 1);

    // Out: its duration, and done.
    let out = Details {
        id: two.0.clone(),
        title: "Soon".to_owned(),
        duration_s: Some(3600),
        live_status: Some("was_live".to_owned()),
        published_at_s: Some(premiere),
        ..Details::default()
    };
    let fake = Fake::default().details(&two, Ok(out));
    run_with(
        person.path(),
        &fake,
        &Options::default(),
        premiere + 2,
        None,
    )
    .unwrap();
    let two_now = kept(&scratch, &two);
    assert_eq!((two_now.0.as_str(), two_now.4), ("done", Some(3600)));
}

#[test]
fn the_score_read_in_sql_is_the_score_of_the_rules() {
    let scratch = Scratch::new("score");
    let a = channel(0);
    let person = catalogue(&scratch, std::slice::from_ref(&a));
    person.set_channel_score(&a, 7000).unwrap();
    // Forty days of videos, one a day.
    let entries: Vec<(VideoId, i64, bool)> = (0..40)
        .map(|k| (video(k), NOW - i64::from(k) * DAY_S - 100, false))
        .collect();
    person
        .set_settings(&Settings {
            first_scan_days: 60,
            first_scan_most: 100,
            ..Settings::default()
        })
        .unwrap();
    let fake = Fake::default().feed(Which::Videos, &a, Ok(atom(&a, &entries)));
    run_with(person.path(), &fake, &feeds_only(), NOW, None).unwrap();
    // The starting rules: recency 2000 over 30 days.
    let reader = Catalog::open_ro(person.path()).unwrap();
    let rows = reader
        .ranked_at(
            &Filter::default(),
            Page {
                number: 0,
                per_page: 100,
            },
            Some(NOW),
        )
        .unwrap();
    assert_eq!(rows.len(), 40);
    for row in &rows {
        let k: u32 = row.id.0[1..].parse().unwrap();
        let age_days = u32::try_from((NOW - row.published_at_s.unwrap()) / DAY_S).unwrap();
        assert_eq!(age_days, k);
        assert_eq!(
            row.score,
            7000 + i64::from(crate::score::recency(2000, 30, age_days)),
            "{}",
            row.id.0
        );
    }
    // Best first, by the day.
    assert_eq!(rows[0].id, video(0));
    // Tomorrow's list is tomorrow's.
    let tomorrow = reader
        .ranked_at(
            &Filter::default(),
            Page {
                number: 0,
                per_page: 1,
            },
            Some(NOW + DAY_S),
        )
        .unwrap();
    assert_eq!(
        tomorrow[0].score,
        7000 + i64::from(crate::score::recency(2000, 30, 1))
    );
    // Pages are SQL's.
    let second = reader
        .ranked_at(
            &Filter::default(),
            Page {
                number: 1,
                per_page: 15,
            },
            Some(NOW),
        )
        .unwrap();
    assert_eq!(second[0].id, video(15));
    assert_eq!(reader.count(&Filter::default()).unwrap(), 40);
}

#[test]
fn rules_weights_and_the_second_fields_rescore_in_one_edit() {
    let scratch = Scratch::new("rescore");
    let (a, b) = (channel(0), channel(1));
    let person = catalogue(&scratch, &[a.clone(), b.clone()]);
    let fake = Fake::default()
        .feed(Which::Videos, &a, Ok(atom(&a, &[(video(1), NOW, false)])))
        .feed(Which::Videos, &b, Ok(atom(&b, &[(video(2), NOW, false)])));
    run_with(person.path(), &fake, &feeds_only(), NOW, None).unwrap();
    let reader = Catalog::open_ro(person.path()).unwrap();
    let points = |id: u32| reader.video(&video(id)).unwrap().unwrap().static_points;
    assert_eq!((points(1), points(2)), (0, 0));

    // A global term over the titles ("Video v0000000001").
    let term = person
        .put_rule(&Rule {
            id: None,
            channel: None,
            kind: RuleKind::Term {
                fields: crate::Fields::TITLE,
                pattern: "VIDEO".to_owned(),
                how: crate::TermMatch::Word,
            },
            points: 300,
            enabled: true,
        })
        .unwrap();
    assert_eq!((points(1), points(2)), (300, 300));
    // A channel's own term adds, for its videos only.
    person
        .put_rule(&Rule {
            id: None,
            channel: Some(a.clone()),
            kind: RuleKind::Term {
                fields: crate::Fields::SECOND_DESCRIPTION,
                pattern: "favourite".to_owned(),
                how: crate::TermMatch::Prefix,
            },
            points: 50,
            enabled: true,
        })
        .unwrap();
    assert_eq!(points(1), 300);
    person
        .set_second_description(&video(1), "One of my FAVOURITES")
        .unwrap();
    assert_eq!(points(1), 350, "the second description is read");
    // The rating's weight, then the weights edited.
    person
        .set_rating(&video(2), Some(Rating::VeryPositive))
        .unwrap();
    assert_eq!(points(2), 2300);
    person
        .set_weights(&Weights {
            very_positive: 10,
            ..Weights::default()
        })
        .unwrap();
    assert_eq!(points(2), 310);
    // Moved to the other channel, the term's points move with it.
    let mut moved = person
        .rules(Some(&a))
        .unwrap()
        .into_iter()
        .find(|r| matches!(&r.kind, RuleKind::Term { pattern, .. } if pattern == "favourite"))
        .unwrap();
    moved.channel = Some(b.clone());
    person.put_rule(&moved).unwrap();
    person
        .set_second_description(&video(2), "a favourite too")
        .unwrap();
    assert_eq!(
        (points(1), points(2)),
        (300, 360),
        "left the first, reached the second"
    );
    moved.channel = Some(a);
    person.put_rule(&moved).unwrap();
    assert_eq!((points(1), points(2)), (350, 310));
    // Removing the term takes its points away from both.
    person.remove_rule(term).unwrap();
    assert_eq!((points(1), points(2)), (50, 10));
    // A single rule new in a scope replaces the one there.
    let before = person.rules(None).unwrap().len();
    person
        .put_rule(&Rule {
            id: None,
            channel: None,
            kind: RuleKind::Recency { days: 7 },
            points: 100,
            enabled: true,
        })
        .unwrap();
    let global = person.rules(None).unwrap();
    assert_eq!(global.len(), before, "replaced, not added");
    assert!(
        global
            .iter()
            .any(|r| r.kind == RuleKind::Recency { days: 7 })
    );
    // A channel's version shows in its rules in place of the global one.
    person
        .put_rule(&Rule {
            id: None,
            channel: Some(b.clone()),
            kind: RuleKind::Short,
            points: 0,
            enabled: true,
        })
        .unwrap();
    let of_b = person.rules(Some(&b)).unwrap();
    let shorts: Vec<&Rule> = of_b.iter().filter(|r| r.kind == RuleKind::Short).collect();
    assert_eq!(shorts.len(), 1);
    assert_eq!(shorts[0].channel.as_ref(), Some(&b));
    // SWITCHED OFF THERE, it is still the channel's — the global one does not
    // come back in its place, so a door finds the switch it turned.
    let mut off = shorts[0].clone();
    off.enabled = false;
    person.put_rule(&off).unwrap();
    let of_b = person.rules(Some(&b)).unwrap();
    let shorts: Vec<&Rule> = of_b.iter().filter(|r| r.kind == RuleKind::Short).collect();
    assert_eq!(shorts.len(), 1, "the global one came back beside it");
    assert_eq!(
        (shorts[0].channel.as_ref(), shorts[0].enabled),
        (Some(&b), false)
    );
    // What the score cannot use is refused with a reason.
    let empty = Rule {
        id: None,
        channel: None,
        kind: RuleKind::Term {
            fields: crate::Fields::TITLE,
            pattern: " … ".to_owned(),
            how: crate::TermMatch::Word,
        },
        points: 1,
        enabled: true,
    };
    assert_eq!(person.put_rule(&empty).unwrap_err().why, Why::Invalid);
    let backwards = Rule {
        kind: RuleKind::Band {
            min_s: 10,
            max_s: 5,
        },
        ..empty
    };
    assert_eq!(person.put_rule(&backwards).unwrap_err().why, Why::Invalid);
    // A recency that would score as none — R below zero, or no days — is
    // refused, globally and for a channel, and nothing was switched off.
    for (channel, days, points) in [(None, 7, -40), (Some(b.clone()), 7, -1), (None, 0, 100)] {
        let recency = Rule {
            id: None,
            channel,
            kind: RuleKind::Recency { days },
            points,
            enabled: true,
        };
        assert_eq!(
            person.put_rule(&recency).unwrap_err().why,
            Why::Invalid,
            "{days} days, {points} points"
        );
    }
    assert!(
        person
            .rules(Some(&b))
            .unwrap()
            .iter()
            .any(|r| r.kind == RuleKind::Recency { days: 7 } && r.points == 100)
    );
    assert_eq!(person.rescore().unwrap(), 0, "nothing left to change");
}

#[test]
fn a_reader_sees_each_commit_and_searches_the_words() {
    let scratch = Scratch::new("reader");
    let a = channel(0);
    let person = catalogue(&scratch, std::slice::from_ref(&a));
    person
        .set_settings(&Settings {
            views_snapshots: true,
            ..Settings::default()
        })
        .unwrap();
    let reader = Catalog::open_ro(person.path()).unwrap();
    let before = reader.data_version().unwrap();
    let fake = Fake::default().feed(Which::Videos, &a, Ok(atom(&a, &[(video(1), NOW, false)])));
    run_with(person.path(), &fake, &feeds_only(), NOW, None).unwrap();
    assert_ne!(
        reader.data_version().unwrap(),
        before,
        "the scanner's commit is seen"
    );
    assert_eq!(reader.count(&Filter::default()).unwrap(), 1);

    // pt-BR: the accents are the subject — `ç` and `ã`, which the index folds.
    let tag = "Configuração";
    // pt-BR: the same word as a person types it, without them.
    let typed = "configuracao";
    person
        .set_second_tags(&video(1), &[tag.to_owned(), " ".to_owned()])
        .unwrap();
    let search = |text: &str| {
        reader
            .count(&Filter {
                text: Some(text.to_owned()),
                ..Filter::default()
            })
            .unwrap()
    };
    assert_eq!(search(typed), 1, "accents folded");
    assert_eq!(search("CONFIG"), 1, "the last word is a prefix");
    assert_eq!(search("about v0000000001"), 1, "the feed's description");
    assert_eq!(search("nothing"), 0);
    assert_eq!(search("  "), 1, "no words is no filter");
    assert_eq!(search("\"quoted"), 0, "a quote is a character, not syntax");

    // What changed since a generation, for the index.
    let (all, generation) = reader.all_since(0).unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].second_tags, [tag]);
    assert_eq!(
        all[0].views,
        Some(7),
        "the feed's count, kept as a snapshot"
    );
    let (none, same) = reader.all_since(generation).unwrap();
    assert!(none.is_empty() && same == generation);
    reader.hide(&video(1)).unwrap();
    assert_eq!(
        reader.count(&Filter::default()).unwrap(),
        0,
        "hidden is left out"
    );
    let (changed, _) = reader.all_since(generation).unwrap();
    assert!(changed[0].row.hidden);
    person.set_hidden(&video(1), false).unwrap();
    assert_eq!(reader.count(&Filter::default()).unwrap(), 1);
}

#[test]
fn thumbnails_are_fetched_for_the_new_and_the_folder_is_trimmed() {
    let scratch = Scratch::new("thumbs");
    let a = channel(0);
    let person = catalogue(&scratch, std::slice::from_ref(&a));
    let entries: Vec<(VideoId, i64, bool)> = (1..=3)
        .map(|k| (video(k), NOW - i64::from(k) * 60, false))
        .collect();
    let mut fake = Fake::default().feed(Which::Videos, &a, Ok(atom(&a, &entries)));
    fake.thumbs = true;
    // Room for two pictures of six hundred bytes.
    let thumbs = Thumbs::at(scratch.0.join("thumbs"), 1300);
    let options = Options {
        details_most: 0,
        feeds_only: false,
    };
    let report = run_with(person.path(), &fake, &options, NOW, Some(&thumbs)).unwrap();
    assert_eq!(report.thumbs, 3);
    let reader = Catalog::open_ro(person.path()).unwrap();
    let rows = reader
        .ranked(
            &Filter::default(),
            Page {
                number: 0,
                per_page: 10,
            },
        )
        .unwrap();
    let with: Vec<&RankedVideo> = rows.iter().filter(|r| r.thumb_file.is_some()).collect();
    assert_eq!(with.len(), 2, "the trimmed one is forgotten");
    assert!(
        with.iter()
            .all(|r| r.thumb_file.as_ref().unwrap().is_file())
    );
}

use crate::read::RankedVideo;

#[test]
fn a_removed_channel_takes_its_thumbnails_with_it() {
    let scratch = Scratch::new("remove-thumbs");
    let (a, b) = (channel(0), channel(1));
    let person = catalogue(&scratch, &[a.clone(), b.clone()]);
    let mut fake = Fake::default()
        .feed(
            Which::Videos,
            &a,
            Ok(atom(
                &a,
                &[(video(1), NOW - 60, false), (video(2), NOW - 120, false)],
            )),
        )
        .feed(
            Which::Videos,
            &b,
            Ok(atom(&b, &[(video(3), NOW - 60, false)])),
        );
    fake.thumbs = true;
    let thumbs = Thumbs::at(scratch.0.join(crate::thumbs::DIR), 1 << 20);
    let options = Options {
        details_most: 0,
        feeds_only: false,
    };
    let report = run_with(person.path(), &fake, &options, NOW, Some(&thumbs)).unwrap();
    assert_eq!(report.thumbs, 3);
    let files = |ids: &[u32]| -> Vec<bool> {
        ids.iter()
            .map(|k| {
                scratch
                    .0
                    .join(crate::thumbs::DIR)
                    .read_dir()
                    .unwrap()
                    .any(|e| {
                        e.unwrap()
                            .file_name()
                            .to_string_lossy()
                            .contains(&video(*k).0)
                    })
            })
            .collect()
    };
    assert_eq!(files(&[1, 2, 3]), [true, true, true]);
    person.remove_channel(&a).unwrap();
    assert_eq!(
        files(&[1, 2, 3]),
        [false, false, true],
        "its pictures went with it; the other channel's stayed"
    );
}

#[test]
fn two_scans_of_one_catalogue_do_not_run_at_once() {
    let scratch = Scratch::new("lock");
    let person = catalogue(&scratch, &[]);
    let held = hold(person.path()).unwrap();
    let refused = run_with(person.path(), &Fake::default(), &feeds_only(), NOW, None).unwrap_err();
    assert_eq!(refused.why, Why::Busy);
    drop(held);
    assert!(run_with(person.path(), &Fake::default(), &feeds_only(), NOW, None).is_ok());
}

/// FAILS IF: a reader cannot tell a scan running from a record a killed scan
/// left open — the lock is the scan's process, the record is not — or the
/// moment a scan announces itself comes before the lock says it runs.
#[test]
fn a_scan_running_is_its_lock_and_not_its_record() {
    let scratch = Scratch::new("running");
    let person = catalogue(&scratch, &[]);
    let path = person.path().to_owned();
    assert!(
        !running(&path),
        "no scan ever ran, and one was said running"
    );
    let mut said = None;
    run_held(
        &path,
        &Fake::default(),
        &feeds_only(),
        NOW,
        None,
        &mut || {
            let open = crate::read::Catalog::open_ro(&path)
                .unwrap()
                .last_scan()
                .unwrap()
                .is_some_and(|s| s.ended_at_s.is_none());
            said = Some((running(&path), open));
        },
    )
    .unwrap();
    assert_eq!(
        said,
        Some((true, true)),
        "announced before the lock was held, or before the record was open"
    );
    assert!(!running(&path), "a scan that ended is still said running");
    // A RECORD LEFT OPEN — the process killed between the two writes — with
    // nobody holding the lock is no scan running.
    {
        let conn = db::open_rw(&path).unwrap();
        conn.execute(
            "INSERT INTO scans (started_at) VALUES (?1)",
            rusqlite::params![NOW],
        )
        .unwrap();
    }
    let reader = crate::read::Catalog::open_ro(&path).unwrap();
    assert!(reader.last_scan().unwrap().unwrap().ended_at_s.is_none());
    assert!(
        !reader.scanning(),
        "a killed scan's record was taken for a scan"
    );
    let held = hold(&path).unwrap();
    assert!(
        reader.scanning(),
        "a scan holding the catalogue was not seen"
    );
    drop(held);
}

/// THE DESIGN'S NUMBER: a rule change recomputes about fifty thousand videos
/// in one transaction "in a few hundred ms". A time is no verdict a battery
/// can give on a shared machine, so this measures and prints; run it by hand
/// with `--ignored --nocapture`.
///
/// Its catalogue is built BESIDE THE TEST BINARY (the build directory), not in
/// the temporary directory: some hundreds of megabytes of rows, index and log
/// would fill a `/tmp` with a per-user quota, and every other program of the
/// person writing there would fail with it.
#[test]
#[ignore = "times a recompute of fifty thousand videos; run it by hand"]
fn a_recompute_of_fifty_thousand_videos_is_timed() {
    const VIDEOS: u32 = 50_000;
    let beside = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .join(format!(
            "ronoctua-video-metrics-{}-fifty-thousand",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&beside);
    std::fs::create_dir_all(&beside).unwrap();
    let scratch = Scratch(beside);
    let a = channel(0);
    let person = catalogue(&scratch, std::slice::from_ref(&a));
    {
        let conn = db::open_rw(person.path()).unwrap();
        let tx = db::immediate(&conn).unwrap();
        for k in 0..VIDEOS {
            let id = video(k);
            let url = format!("https://www.youtube.com/watch?v={}", id.0);
            let title = format!("Video {k}: a tutorial about the Rust language");
            let description = format!("Part {k} of a long series. Configuration, tools, tests.");
            let video = NewVideo {
                id: &id,
                channel: &a,
                url: &url,
                title: &title,
                description: &description,
                published_at_s: Some(NOW - i64::from(k) * 600),
                duration_s: Some(60 + k % 3600),
                short_evidence: None,
                live_status: None,
                thumb_url: None,
                views: None,
            };
            edit::insert_video(&tx, &video, false, 1, NOW).unwrap();
        }
        tx.commit().unwrap();
    }
    let rule = Rule {
        id: None,
        channel: None,
        kind: RuleKind::Term {
            fields: crate::Fields::TITLE,
            pattern: "rust".to_owned(),
            how: crate::TermMatch::Word,
        },
        points: 300,
        enabled: true,
    };
    let started = std::time::Instant::now();
    person.put_rule(&rule).unwrap();
    let elapsed = started.elapsed();
    println!("a term rule over {VIDEOS} videos, recomputed in one transaction: {elapsed:?}");
    let reader = Catalog::open_ro(person.path()).unwrap();
    assert_eq!(reader.count(&Filter::default()).unwrap(), u64::from(VIDEOS));
}
