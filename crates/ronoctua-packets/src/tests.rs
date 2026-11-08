//! The manual's data, held to what the window assumes of it.

use crate::anatomy::{self, ROW_BITS};
use crate::timeline::{Pace, Timeline};
use crate::{Beat, Class, art, catalogue};

const PACE: Pace = Pace {
    camera: 700,
    hop: 600,
    key: 60,
    light: 500,
    rest: 800,
};

/// FAILS IF: two protocols share a word, a scene names a device it does not
/// have, a step frames one, or a packet is sent between devices no links
/// join — the window would fly it nowhere.
#[test]
fn every_scene_holds_what_its_steps_name() {
    let mut ids: Vec<&str> = catalogue::ALL.iter().map(|p| p.id).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(
        ids.len(),
        catalogue::ALL.len(),
        "two protocols share a word"
    );
    for p in catalogue::ALL {
        let n = p.scene.devices.len();
        assert!(n > 0, "{} has no devices", p.id);
        assert!(!p.steps.is_empty(), "{} has no steps", p.id);
        for l in p.scene.links {
            assert!(
                l.a < n && l.b < n && l.a != l.b,
                "{}: a link to nowhere",
                p.id
            );
        }
        for (s, step) in p.steps.iter().enumerate() {
            for d in step.focus {
                assert!(*d < n, "{} step {s} frames device {d}", p.id);
            }
            for beat in step.beats {
                match beat {
                    Beat::Send(packet) => {
                        assert!(
                            p.path(packet.from, packet.to).is_some(),
                            "{} step {s}: no links join {} and {}",
                            p.id,
                            packet.from,
                            packet.to
                        );
                        assert!(
                            !packet.tag.is_empty() && !packet.info.is_empty() && packet.bytes > 0
                        );
                        for h in packet.headers {
                            assert!(
                                h.values.len() <= h.layout.fields.len(),
                                "{} step {s}: more values than fields",
                                p.id
                            );
                            for lit in h.lit {
                                assert!(
                                    *lit < h.layout.fields.len(),
                                    "{} step {s}: lights field {lit}",
                                    p.id
                                );
                            }
                        }
                    }
                    Beat::Type { at, wire } => assert!(*at < n && !wire.is_empty()),
                    Beat::Light { at, .. } => assert!(*at < n),
                    Beat::Rest => {}
                }
            }
        }
    }
}

/// FAILS IF: a header's fixed fields do not fill whole bytes, or a row
/// holds more than 32 bits.
#[test]
fn every_header_falls_into_rows_of_whole_bytes() {
    for layout in anatomy::ALL {
        let bits: u32 = layout.fields.iter().map(|f| u32::from(f.bits)).sum();
        assert_eq!(bits % 8, 0, "a header of {bits} bits");
        for row in anatomy::rows(layout) {
            let used: u16 = row.iter().map(|p| p.bits).sum();
            assert!(used <= ROW_BITS, "a row of {used} bits");
        }
        // EVERY FIELD IS NAMED once, at its first piece.
        let named: Vec<usize> = anatomy::rows(layout)
            .iter()
            .flatten()
            .filter(|p| p.first)
            .map(|p| p.field)
            .collect();
        assert_eq!(named, (0..layout.fields.len()).collect::<Vec<_>>());
    }
}

/// FAILS IF: the timeline goes back, starts anywhere but the first step,
/// ends anywhere but the last, or loses a packet on the way.
#[test]
fn the_timeline_plays_every_step_and_sends_every_packet() {
    for p in catalogue::ALL {
        let line = Timeline::of(p, PACE);
        assert_eq!(line.steps.len(), p.steps.len());
        for w in line.steps.windows(2) {
            assert!(
                w[0].start < w[0].end && w[0].end == w[1].start,
                "{}: steps overlap",
                p.id
            );
        }
        assert_eq!(line.at(0).step, 0);
        assert_eq!(line.at(line.total).step, p.steps.len() - 1);
        let packets = p.packets().count();
        assert_eq!(
            line.at(line.total).sent,
            packets,
            "{}: a packet was not sent",
            p.id
        );
        let mut sent = 0;
        let mut t = 0;
        while t <= line.total {
            let pose = line.at(t);
            assert!(pose.sent >= sent, "{}: the capture lost a row at {t}", p.id);
            sent = pose.sent;
            for f in &pose.flights {
                assert!((0.0..=1.0).contains(&f.along));
            }
            t += 50;
        }
        for (k, _) in p.packets().enumerate() {
            let leaves = line.leaves(k).expect("every packet leaves");
            assert_eq!(line.at(leaves).sent, k + 1);
        }
    }
}

/// FAILS IF: a drawing names a colour of its own, or does not rasterise.
#[test]
fn every_drawing_is_inked_by_the_theme_and_rasterises() {
    let mut kinds = vec![
        art::Kind::Laptop,
        art::Kind::Phone,
        art::Kind::Desktop,
        art::Kind::Router,
        art::Kind::Switch,
        art::Kind::AccessPoint,
        art::Kind::Cloud,
        art::Kind::CoreRouter,
        art::Kind::Rack,
        art::Kind::Storage,
        art::Kind::Printer,
        art::Kind::Controller,
    ];
    for e in [
        art::Emblem::Globe,
        art::Emblem::Gear,
        art::Emblem::Envelope,
        art::Emblem::Clock,
        art::Emblem::Page,
        art::Emblem::Key,
        art::Emblem::Arrows,
        art::Emblem::Shield,
        art::Emblem::Hub,
        art::Emblem::Mirror,
        art::Emblem::Onion,
        art::Emblem::List,
    ] {
        kinds.push(art::Kind::Tower(e));
    }
    let mut drawings: Vec<art::Art> = kinds.into_iter().map(art::device).collect();
    for prop in [
        art::Prop::Lock,
        art::Prop::Key,
        art::Prop::Envelope,
        art::Prop::Certificate,
        art::Prop::Clock,
        art::Prop::Page,
        art::Prop::Onion,
        art::Prop::Shield,
        art::Prop::Check,
        art::Prop::Cross,
    ] {
        drawings.push(art::prop(prop));
    }
    drawings.push(art::pulse(30.0));
    drawings.push(art::pill(60.0, 14.0));
    for d in drawings {
        assert!(
            !d.svg.contains('#'),
            "a colour of its own: {}",
            &d.svg[..d.svg.len().min(200)]
        );
        assert!(d.svg.contains(art::GLOW) || d.svg.contains(art::INK));
        let svg = art::resolved(
            &d.svg,
            [200, 200, 200, 255],
            [120, 120, 120, 255],
            [80, 160, 255, 255],
        );
        let r = art::raster(&svg, 2.0).expect("it rasterises");
        assert!(r.pixels.chunks(4).any(|p| p[3] > 0), "nothing was drawn");
        assert!(
            d.anchor.0 >= 0.0
                && d.anchor.1 >= 0.0
                && d.anchor.0 <= d.width
                && d.anchor.1 <= d.height
        );
    }
}

/// FAILS IF: a class is missing from the legend, or two share a place.
#[test]
fn every_class_has_its_place_in_the_legend() {
    for (k, c) in Class::ALL.iter().enumerate() {
        assert_eq!(c.index(), k);
    }
}

/// FAILS IF: an address is not written as the wire writes it.
#[test]
fn addresses_are_written_as_the_wire_writes_them() {
    use crate::Address;
    assert_eq!(
        Address::V4([192, 168, 1, 10]).text().as_deref(),
        Some("192.168.1.10")
    );
    assert_eq!(
        Address::V6([0x2001, 0xdb8, 0, 0, 0, 0, 0, 1])
            .text()
            .as_deref(),
        Some("2001:db8::1")
    );
    assert_eq!(
        Address::Mac([0x52, 0x54, 0, 0x12, 0x34, 0x56])
            .text()
            .as_deref(),
        Some("52:54:00:12:34:56")
    );
    assert_eq!(Address::As(64500).text().as_deref(), Some("AS64500"));
    assert_eq!(Address::None.text(), None);
}

/// FAILS IF: the book cannot be opened in a declared language, or lacks a
/// key the manual names — the window would draw `[packets-…]`.
#[test]
fn the_book_holds_every_key_in_every_language() {
    for language in ronoctua_i18n::LANGUAGES {
        let book = ronoctua_i18n::Book::open(&crate::BOOK, language).expect("the book opens");
        for p in catalogue::ALL {
            for key in [p.name, p.title, p.tagline, p.overview] {
                assert!(!book.t(key).starts_with('['), "{language}: {}", key.name());
            }
        }
    }
}
