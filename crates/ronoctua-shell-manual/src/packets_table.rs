//! THE TRANSPORT AND THE CAPTURE under the network: the buttons that play
//! it, the legend of the packets' colours, and every packet sent so far as a
//! capture lists it — its number, when it left, from where to where, its
//! protocol, the capture's line and its size. The packet of the moment is
//! marked; any row is a click that goes back to the moment it left.
//!
//! The capture's clock is the wire's, not the animation's: each packet
//! leaves when the one before it arrives, after the links' times along its
//! way (`Protocol::latency`).

use crate::packets_window::{Hit, Layout, Open, Paint, Rect, SPEEDS, grid, ink_of, said};
use ronoctua_i18n::{Arg, Book};
use ronoctua_packets::anatomy;
use ronoctua_packets::timeline::Pose;
use ronoctua_packets::{Class, Packet};
use ronoctua_tokens as tokens;

/// The capture's columns, in cells: the number, the time, the source, the
/// destination and the protocol; the line takes what is left, less the size.
const COLUMNS: [u16; 5] = [5, 10, 19, 19, 10];
/// The size's column, at the right.
const SIZE_CELLS: u16 = 10;

/// Paints the transport, the legend and the capture.
#[allow(clippy::too_many_lines)]
pub fn paint(
    paint: &mut Paint<'_, '_>,
    layout: &Layout,
    book: &Book,
    texts: &ronoctua_i18n::Texts,
    open: &Open,
    pose: &Pose,
) {
    let (cell, line) = grid();
    let p = open.protocol;
    let y = layout.controls_line;
    // THE TRANSPORT: back to the start, the step before, play or pause, the
    // next step, the speed — a glyph and a word each, every one a click.
    let play = if open.playing {
        tokens::icon::PAUSE
    } else {
        tokens::icon::PLAY
    };
    let buttons = [
        (
            tokens::icon::PROTOCOLS_RESTART,
            texts.t(crate::key::PROTOCOLS_RESTART),
            Hit::Restart,
        ),
        (
            tokens::icon::PROTOCOLS_PREVIOUS,
            texts.t(crate::key::PROTOCOLS_PREVIOUS),
            Hit::Previous,
        ),
        (play, texts.t(crate::key::PROTOCOLS_PLAY), Hit::Play),
        (
            tokens::icon::PROTOCOLS_NEXT,
            texts.t(crate::key::PROTOCOLS_NEXT),
            Hit::Next,
        ),
        (
            tokens::icon::PROTOCOLS_SPEED,
            book.t(SPEEDS[open.speed].1),
            Hit::Speed,
        ),
    ];
    let mut at = layout.left;
    for (glyph, word, hit) in buttons {
        let cells = ronoctua_render::layout::width(&word) + 3;
        let ink = if hit == Hit::Play {
            tokens::color::TEXT_HEADING
        } else {
            tokens::color::TEXT_SECONDARY
        };
        paint.takes(Rect::of_cells(at, y, cells, 1), hit);
        paint.canvas.spans.push(said(glyph.to_string(), at, y, ink));
        paint.canvas.spans.push(said(word, at + 2, y, ink));
        at += cells + 2;
    }
    // THE LEGEND, at the right: the classes this protocol sends.
    let mut classes: Vec<Class> = p.packets().map(|(_, pk)| pk.class).collect();
    classes.sort_unstable();
    classes.dedup();
    let legend: Vec<(Class, String)> = classes.iter().map(|c| (*c, book.t(c.name()))).collect();
    let wide: u16 = legend
        .iter()
        .map(|(_, n)| ronoctua_render::layout::width(n) + 4)
        .sum();
    if at + wide + 2 <= layout.right {
        let mut x = layout.right.saturating_sub(wide);
        for (class, name) in legend {
            paint.canvas.spans.push(said(
                tokens::icon::PROTOCOLS_CLASS.to_string(),
                x,
                y,
                ink_of(class),
            ));
            let w = ronoctua_render::layout::width(&name);
            paint
                .canvas
                .spans
                .push(said(name, x + 2, y, tokens::color::TEXT_DIM));
            x += w + 4;
        }
    }

    // THE CAPTURE: a rule over it, its heading, and the rows that fit — the
    // last sent, the packet of the moment among them.
    let (table_top, table_lines) = layout.table;
    if table_lines < 2 {
        return;
    }
    #[allow(clippy::cast_precision_loss)]
    let width = f32::from(layout.right + layout.left + 1) * cell;
    paint.canvas.under.push(crate::chrome::rule(
        width,
        (f32::from(table_top) - 0.5) * line,
    ));
    let info_at = layout.left + COLUMNS.iter().sum::<u16>();
    let size_at = layout.right.saturating_sub(SIZE_CELLS);
    let info_room = size_at.saturating_sub(info_at + 2);
    let heads = [
        ronoctua_packets::key::PACKETS_COL_NO,
        ronoctua_packets::key::PACKETS_COL_TIME,
        ronoctua_packets::key::PACKETS_COL_SOURCE,
        ronoctua_packets::key::PACKETS_COL_DESTINATION,
        ronoctua_packets::key::PACKETS_COL_PROTOCOL,
    ];
    let mut x = layout.left;
    for (k, head) in heads.iter().enumerate() {
        paint
            .canvas
            .spans
            .push(said(book.t(*head), x, table_top, tokens::color::TEXT_DIM));
        x += COLUMNS[k];
    }
    paint.canvas.spans.push(said(
        book.t(ronoctua_packets::key::PACKETS_COL_INFO),
        info_at,
        table_top,
        tokens::color::TEXT_DIM,
    ));
    paint.canvas.spans.push(said(
        book.t(ronoctua_packets::key::PACKETS_COL_SIZE),
        size_at,
        table_top,
        tokens::color::TEXT_DIM,
    ));
    let packets: Vec<&Packet> = p.packets().map(|(_, pk)| pk).collect();
    // THE WIRE'S CLOCK: when each packet left.
    let mut clock = Vec::with_capacity(packets.len());
    let mut t = 0_u32;
    for pk in &packets {
        clock.push(t);
        t += p.latency(pk.from, pk.to);
    }
    let room = usize::from(table_lines - 1);
    let shown_to = pose.sent;
    let from = shown_to.saturating_sub(room);
    let address = |d: usize| {
        let device = p.scene.devices[d];
        device.address.text().unwrap_or_else(|| book.t(device.name))
    };
    for (row, k) in (from..shown_to).enumerate() {
        let pk = packets[k];
        #[allow(clippy::cast_possible_truncation)]
        let y = table_top + 1 + row as u16;
        let here = pose.current == Some(k);
        let ink = if here {
            tokens::color::TEXT_PRIMARY
        } else {
            tokens::color::TEXT_SECONDARY
        };
        paint.takes(
            Rect::of_cells(layout.left, y, layout.right - layout.left, 1),
            Hit::Packet(k),
        );
        let time = book.t_args(
            ronoctua_packets::key::PACKETS_MILLISECONDS,
            &[("n", Arg::Number(i64::from(clock[k])))],
        );
        // WHAT A CAPTURE SHOWS: the addresses the packet says it is from and
        // to — its own (`Packet::between`), or its IP header's — and the
        // devices' only when it carries none (a frame, before any address).
        let (source, destination) = pk.addresses.or_else(|| ip_ends(pk)).map_or_else(
            || (address(pk.from), address(pk.to)),
            |(s, d)| (s.to_owned(), d.to_owned()),
        );
        let cells = [
            (
                format!("{}", k + 1),
                if here {
                    ink_of(pk.class)
                } else {
                    tokens::color::TEXT_DIM
                },
            ),
            (time, ink),
            (source, ink),
            (destination, ink),
            (pk.proto.to_owned(), ink_of(pk.class)),
        ];
        let mut x = layout.left;
        for (c, (text, color)) in cells.into_iter().enumerate() {
            let fits = ronoctua_render::layout::elide_end(&text, COLUMNS[c].saturating_sub(1));
            paint.canvas.spans.push(said(fits, x, y, color));
            x += COLUMNS[c];
        }
        paint.canvas.spans.push(said(
            ronoctua_render::layout::elide_end(pk.info, info_room),
            info_at,
            y,
            ink,
        ));
        paint.canvas.spans.push(said(
            crate::packets_panel::size_of(book, pk.bytes),
            size_at,
            y,
            ink,
        ));
    }
}

/// The source and destination its outermost IP header gives a packet.
fn ip_ends(pk: &Packet) -> Option<(&'static str, &'static str)> {
    let header = pk.headers.first()?;
    let (source, destination) = if std::ptr::eq(header.layout, &anatomy::IPV4) {
        (11, 12)
    } else if std::ptr::eq(header.layout, &anatomy::IPV6) {
        (6, 7)
    } else {
        return None;
    };
    let (s, d) = (header.values.get(source)?, header.values.get(destination)?);
    (!s.is_empty() && !d.is_empty()).then_some((*s, *d))
}
