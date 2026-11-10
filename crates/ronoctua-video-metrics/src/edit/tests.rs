//! The person's marks on a video — set aside, kept — a channel's keep, and an
//! imported list staged, decided and registered: each written here and read
//! back through the readers' own door.

use super::*;
use crate::db::tests::Scratch;
use crate::person::Person;
use crate::read::{Catalog, Filter, Forever, Hidden, Order, Page, SetAside};

/// A moment to write at: 2026-09-30T00:00:00Z.
const NOW: i64 = 1_790_726_400;

/// A day, in seconds.
const DAY: i64 = 86_400;

fn channel(n: u8) -> ChannelId {
    ChannelId(format!("UC{}", char::from(b'a' + n).to_string().repeat(22)))
}

fn video(n: u32) -> VideoId {
    VideoId(format!("v{n:010}"))
}

fn new_channel(id: &ChannelId) -> NewChannel {
    NewChannel {
        id: id.clone(),
        handle: None,
        title: format!("Channel {}", id.0),
        url: format!("https://www.youtube.com/channel/{}", id.0),
        avatar_url: None,
    }
}

/// A catalogue with these channels, each with these videos (its number,
/// published this many days before [`NOW`]), catalogued at [`NOW`].
fn catalogue(scratch: &Scratch, channels: &[(ChannelId, &[(u32, i64)])]) -> Person {
    let person = Person::open(&scratch.catalogue()).unwrap();
    let conn = db::open_rw(person.path()).unwrap();
    for (id, videos) in channels {
        add_channel(&conn, &new_channel(id), NOW).unwrap();
        let tx = db::immediate(&conn).unwrap();
        for (n, days) in *videos {
            let vid = video(*n);
            let url = format!("https://www.youtube.com/watch?v={}", vid.0);
            let title = format!("Video {n}");
            let new = NewVideo {
                id: &vid,
                channel: id,
                url: &url,
                title: &title,
                description: "",
                published_at_s: Some(NOW - days * DAY),
                duration_s: Some(600),
                short_evidence: None,
                live_status: None,
                thumb_url: None,
                views: None,
            };
            assert!(insert_video(&tx, &new, false, 1, NOW).unwrap());
        }
        tx.commit().unwrap();
    }
    person
}

fn page() -> Page {
    Page {
        number: 0,
        per_page: 100,
    }
}

fn ids(rows: &[crate::read::RankedVideo]) -> Vec<VideoId> {
    rows.iter().map(|r| r.id.clone()).collect()
}

fn subscription(n: u8, title: &str) -> Subscription {
    Subscription {
        id: channel(n),
        title: title.to_owned(),
    }
}

/// FAILS IF: the set-aside list is not the last set aside first, setting a
/// video aside again moves it, or the filter and the count disagree.
#[test]
fn the_set_aside_list_is_newest_first_and_keeps_its_first_moment() {
    let scratch = Scratch::new("set-aside");
    let a = channel(0);
    let person = catalogue(&scratch, &[(a, &[(1, 1), (2, 2), (3, 3)])]);
    let conn = db::open_rw(person.path()).unwrap();
    set_aside(&conn, &video(1), true, NOW + 10).unwrap();
    set_aside(&conn, &video(2), true, NOW + 20).unwrap();
    set_aside(&conn, &video(1), true, NOW + 30).unwrap();
    let reader = Catalog::open_ro(person.path()).unwrap();
    let only = Filter {
        set_aside: SetAside::Only,
        hidden: Hidden::Any,
        order: Order::NewestSetAside,
        ..Filter::default()
    };
    let rows = reader.ranked(&only, page()).unwrap();
    assert_eq!(ids(&rows), [video(2), video(1)], "the last set aside first");
    assert_eq!(
        rows.iter().map(|r| r.set_aside_at_s).collect::<Vec<_>>(),
        [Some(NOW + 20), Some(NOW + 10)],
        "set aside again, a video keeps its first moment"
    );
    let without = Filter {
        set_aside: SetAside::Without,
        ..Filter::default()
    };
    assert_eq!(ids(&reader.ranked(&without, page()).unwrap()), [video(3)]);
    assert_eq!(
        (
            reader.count(&only).unwrap(),
            reader.count(&without).unwrap(),
            reader.count(&Filter::default()).unwrap()
        ),
        (2, 1, 3)
    );
    // EVERY VIDEO IN THAT ORDER: the set aside first, then the rest by score.
    let all = Filter {
        order: Order::NewestSetAside,
        ..Filter::default()
    };
    assert_eq!(
        ids(&reader.ranked(&all, page()).unwrap()),
        [video(2), video(1), video(3)]
    );
    set_aside(&conn, &video(1), false, NOW + 40).unwrap();
    assert_eq!(ids(&reader.ranked(&only, page()).unwrap()), [video(2)]);
    // Set aside anew, it is the newest.
    set_aside(&conn, &video(1), true, NOW + 50).unwrap();
    assert_eq!(
        ids(&reader.ranked(&only, page()).unwrap()),
        [video(1), video(2)]
    );
}

/// FAILS IF: a ranked row does not carry the rating, the moment it was set
/// aside and its ∞ — the video's own mark over its channel's for ever — or
/// a number this build does not know is read as none.
#[test]
fn a_ranked_row_carries_its_rating_set_aside_and_forever() {
    let scratch = Scratch::new("ranked-marks");
    let (a, b) = (channel(0), channel(1));
    let person = catalogue(&scratch, &[(a.clone(), &[(1, 1)]), (b, &[(2, 1)])]);
    person.set_channel_keep(&a, Some(Keep::Forever)).unwrap();
    person
        .set_rating(&video(2), Some(Rating::Positive))
        .unwrap();
    person.set_aside(&video(2), true).unwrap();
    let reader = Catalog::open_ro(person.path()).unwrap();
    let row = |id: VideoId| {
        reader
            .ranked(&Filter::default(), page())
            .unwrap()
            .into_iter()
            .find(|r| r.id == id)
            .unwrap()
    };
    assert_eq!(row(video(1)).forever, Forever::ByChannel);
    assert_eq!(row(video(2)).forever, Forever::No);
    assert_eq!(row(video(2)).rating, Some(Rating::Positive));
    assert!(row(video(2)).set_aside_at_s.is_some());
    assert_eq!(row(video(1)).set_aside_at_s, None);
    person.set_keep(&video(1), true).unwrap();
    assert_eq!(
        row(video(1)).forever,
        Forever::ByVideo,
        "the video's own mark wins"
    );
    // AND IT STANDS when the channel stops keeping for ever.
    person.set_channel_keep(&a, None).unwrap();
    assert_eq!(row(video(1)).forever, Forever::ByVideo);
    person.set_keep(&video(1), false).unwrap();
    assert_eq!(row(video(1)).forever, Forever::No);
    let whole = reader.video(&video(2)).unwrap().unwrap();
    assert_eq!(whole.rating, whole.row.rating, "one rating, read twice");
    // A RATING NO BUILD WROTE is refused, never read as no rating.
    let conn = db::open_rw(person.path()).unwrap();
    conn.execute(
        "UPDATE videos SET rating = 7 WHERE id = ?1",
        params![video(2).0],
    )
    .unwrap();
    assert_eq!(
        reader.ranked(&Filter::default(), page()).unwrap_err().why,
        Why::Unreadable
    );
}

/// FAILS IF: a mark on what is not there is taken for done.
#[test]
fn a_mark_on_a_missing_video_or_channel_is_not_found() {
    let scratch = Scratch::new("missing");
    let a = channel(0);
    let person = catalogue(&scratch, &[(a.clone(), &[(1, 1)])]);
    let missing = video(9);
    for refused in [
        person.set_aside(&missing, true),
        person.set_keep(&missing, true),
        person.set_channel_keep(&channel(5), Some(Keep::Forever)),
        person.decide(&channel(5), Decision::Keep),
        person.decide(&a, Decision::Keep),
    ] {
        assert_eq!(refused.unwrap_err().why, Why::NotFound);
    }
}

/// FAILS IF: a keep of no days — which would take everything out at the next
/// scan — or past a hundred years is kept, or a keep does not come back as
/// it was set.
#[test]
fn a_keep_of_no_days_is_invalid_and_a_keep_round_trips() {
    let scratch = Scratch::new("keep");
    let a = channel(0);
    let person = catalogue(&scratch, &[(a.clone(), &[])]);
    for wrong in [Keep::Days(0), Keep::Days(KEEP_DAYS_MOST + 1)] {
        assert_eq!(
            person.set_channel_keep(&a, Some(wrong)).unwrap_err().why,
            Why::Invalid
        );
        assert_eq!(
            person
                .set_settings(&Settings {
                    keep: wrong,
                    ..Settings::default()
                })
                .unwrap_err()
                .why,
            Why::Invalid
        );
    }
    let reader = Catalog::open_ro(person.path()).unwrap();
    assert_eq!(reader.channels().unwrap()[0].keep, None, "as everywhere");
    for keep in [Some(Keep::Days(30)), Some(Keep::Forever), None] {
        person.set_channel_keep(&a, keep).unwrap();
        assert_eq!(reader.channels().unwrap()[0].keep, keep);
    }
    person
        .set_settings(&Settings {
            keep: Keep::Days(30),
            ..Settings::default()
        })
        .unwrap();
    assert_eq!(reader.settings().unwrap().keep, Keep::Days(30));
}

/// FAILS IF: staging a list stages a channel already registered, or a staged
/// channel is among the catalogue's channels before it is registered.
#[test]
fn staging_answers_what_is_registered_and_keeps_the_rest_apart() {
    let scratch = Scratch::new("stage");
    let a = channel(0);
    let person = catalogue(&scratch, &[(a, &[])]);
    let staging = person
        .stage(&[
            subscription(0, "Another name for it"),
            subscription(1, "Two"),
            subscription(2, "three"),
            subscription(1, "Two again"),
        ])
        .unwrap();
    assert_eq!(staging.staged, 2, "each channel once");
    assert_eq!(
        staging.already,
        [(channel(0), "Channel ".to_owned() + &channel(0).0)]
    );
    let reader = Catalog::open_ro(person.path()).unwrap();
    assert_eq!(
        reader
            .channels()
            .unwrap()
            .iter()
            .map(|c| c.id.clone())
            .collect::<Vec<_>>(),
        [channel(0)],
        "a staged channel is no channel of the catalogue yet"
    );
    let staged = reader.staged().unwrap();
    assert_eq!(
        staged
            .iter()
            .map(|s| (s.channel.title.as_str(), s.decision))
            .collect::<Vec<_>>(),
        [("three", Decision::Undecided), ("Two", Decision::Undecided)],
        "by title, case folded"
    );
    assert_eq!(person.staged().unwrap(), staged);
    assert_eq!(
        staged[1].channel.url,
        format!("https://www.youtube.com/channel/{}", channel(1).0)
    );
    // An id that is not a channel's refuses the whole list.
    assert_eq!(
        person
            .stage(&[
                subscription(3, "Four"),
                Subscription {
                    id: ChannelId("UCshort".to_owned()),
                    title: "Short".to_owned(),
                }
            ])
            .unwrap_err()
            .why,
        Why::Invalid
    );
    assert_eq!(reader.staged().unwrap().len(), 2);
}

/// FAILS IF: the staged list sorts a title begun with an accented capital
/// anywhere but where its letter is — SQLite's `NOCASE` folds ASCII only and
/// put it after every unaccented title.
#[test]
fn the_staged_list_folds_accents_as_it_sorts() {
    let scratch = Scratch::new("stage-order");
    let person = catalogue(&scratch, &[]);
    // pt-BR: titles begun with accented capitals, the subject of the order.
    let titles = ["Zebra", "Ética", "banana", "Árvore"];
    person
        .stage(
            &titles
                .iter()
                .zip(1_u8..)
                .map(|(title, n)| subscription(n, title))
                .collect::<Vec<_>>(),
        )
        .unwrap();
    assert_eq!(
        person
            .staged()
            .unwrap()
            .iter()
            .map(|s| s.channel.title.as_str())
            .collect::<Vec<_>>(),
        // pt-BR: the same titles, in the order of their letters.
        ["Árvore", "banana", "Ética", "Zebra"]
    );
}

/// FAILS IF: a staged channel cannot be given, before it is registered, what
/// its page sets on any channel — a rule, a score, a keep.
#[test]
fn a_staged_channel_takes_a_rule_a_score_and_a_keep() {
    let scratch = Scratch::new("staged-edits");
    let person = catalogue(&scratch, &[]);
    person.stage(&[subscription(1, "One")]).unwrap();
    let one = channel(1);
    person
        .put_rule(&Rule {
            id: None,
            channel: Some(one.clone()),
            kind: RuleKind::Short,
            points: -50,
            enabled: true,
        })
        .unwrap();
    person.set_channel_score(&one, 9000).unwrap();
    person.set_channel_keep(&one, Some(Keep::Days(7))).unwrap();
    let staged = Catalog::open_ro(person.path()).unwrap().staged().unwrap();
    assert_eq!(
        (staged[0].channel.score, staged[0].channel.keep),
        (9000, Some(Keep::Days(7)))
    );
    assert!(
        person
            .rules(Some(&one))
            .unwrap()
            .iter()
            .any(|r| r.channel.as_ref() == Some(&one) && r.points == -50)
    );
}

/// FAILS IF: a list imported again forgets what was decided, or keeps the
/// old title.
#[test]
fn staging_again_keeps_the_decision_and_takes_the_new_title() {
    let scratch = Scratch::new("stage-again");
    let person = catalogue(&scratch, &[]);
    person.stage(&[subscription(1, "Old name")]).unwrap();
    person.decide(&channel(1), Decision::Discard).unwrap();
    let again = person.stage(&[subscription(1, "New name")]).unwrap();
    assert_eq!((again.staged, again.already.len()), (1, 0));
    let staged = person.staged().unwrap();
    assert_eq!(
        (staged[0].channel.title.as_str(), staged[0].decision),
        ("New name", Decision::Discard)
    );
}

/// FAILS IF: registering does anything but this — the kept join the
/// catalogue, added then; the discarded go, with their rules and with no
/// tombstone; the undecided stay to decide.
#[test]
fn registering_keeps_the_kept_drops_the_discarded_and_leaves_the_undecided() {
    let scratch = Scratch::new("register");
    let person = catalogue(&scratch, &[]);
    person
        .stage(&[
            subscription(1, "Kept"),
            subscription(2, "Dropped"),
            subscription(3, "Undecided"),
        ])
        .unwrap();
    person.decide(&channel(1), Decision::Keep).unwrap();
    person.decide(&channel(2), Decision::Discard).unwrap();
    person
        .put_rule(&Rule {
            id: None,
            channel: Some(channel(2)),
            kind: RuleKind::Live,
            points: 10,
            enabled: true,
        })
        .unwrap();
    let conn = db::open_rw(person.path()).unwrap();
    let registered = register_staged(&conn, NOW + 100).unwrap();
    assert_eq!(
        registered,
        Registered {
            kept: 1,
            discarded: 1,
            left: 1
        }
    );
    let reader = Catalog::open_ro(person.path()).unwrap();
    let channels = reader.channels().unwrap();
    assert_eq!(
        channels
            .iter()
            .map(|c| (c.id.clone(), c.added_at_s))
            .collect::<Vec<_>>(),
        [(channel(1), NOW + 100)],
        "kept, and added when registered"
    );
    let staged = reader.staged().unwrap();
    assert_eq!(staged.len(), 1);
    assert_eq!(staged[0].channel.id, channel(3));
    let (rules, tombs): (i64, i64) = conn
        .query_row(
            "SELECT (SELECT count(*) FROM rules WHERE channel = ?1),
               (SELECT count(*) FROM skipped)",
            params![channel(2).0],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(
        (rules, tombs),
        (0, 0),
        "dropped with its rules, no tombstone"
    );
    // Nothing decided, nothing to do.
    assert_eq!(
        register_staged(&conn, NOW + 200).unwrap(),
        Registered {
            kept: 0,
            discarded: 0,
            left: 1
        }
    );
}

/// FAILS IF: "all of them" leaves a staged channel as it was, or reaches a
/// channel of the catalogue.
#[test]
fn deciding_all_turns_every_staged_channel() {
    let scratch = Scratch::new("decide-all");
    let a = channel(0);
    let person = catalogue(&scratch, &[(a, &[])]);
    person
        .stage(&[subscription(1, "One"), subscription(2, "Two")])
        .unwrap();
    person.decide(&channel(1), Decision::Discard).unwrap();
    assert_eq!(person.decide_all(Decision::Keep).unwrap(), 2);
    assert!(
        person
            .staged()
            .unwrap()
            .iter()
            .all(|s| s.decision == Decision::Keep)
    );
    assert_eq!(
        Catalog::open_ro(person.path())
            .unwrap()
            .channels()
            .unwrap()
            .len(),
        1,
        "the catalogue's channel is not staged by it"
    );
}

/// FAILS IF: a staged channel the person adds by hand stays staged — and so
/// unscanned — or loses what was set on it while it waited.
#[test]
fn adding_a_staged_channel_by_hand_registers_it() {
    let scratch = Scratch::new("add-staged");
    let person = catalogue(&scratch, &[]);
    person.stage(&[subscription(1, "One")]).unwrap();
    person.set_channel_score(&channel(1), 8000).unwrap();
    let conn = db::open_rw(person.path()).unwrap();
    add_channel(&conn, &new_channel(&channel(1)), NOW + 5).unwrap();
    let reader = Catalog::open_ro(person.path()).unwrap();
    assert!(reader.staged().unwrap().is_empty());
    let channels = reader.channels().unwrap();
    assert_eq!(
        (channels[0].score, channels[0].added_at_s),
        (8000, NOW + 5),
        "registered now, with its score"
    );
}
