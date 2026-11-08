//! THE PANEL beside the network — what the protocol is, and what the step of
//! the moment does — and the line of its title and its three views above
//! them.
//!
//! Before it plays, the panel is the protocol's OVERVIEW: what it is for, its
//! facts and its steps. While it plays, it is the STEP: which of how many,
//! what it does and why, its facts, and the card of the packet it is about —
//! what that packet says, in words. The steps are listed under either, the
//! step of the moment marked, each a click that goes to it. In the anatomy,
//! the field under the pointer is explained on top.

use crate::packets_window::{Hit, Layout, Open, Paint, Rect, View, grid, ink_of, said};
use ronoctua_i18n::{Arg, Book};
use ronoctua_packets::timeline::Pose;
use ronoctua_packets::{Fact, Packet, Value};
use ronoctua_tokens as tokens;

/// A line of the panel: its pieces, each `(cells in, text, ink)`, and what a
/// click on it takes.
type Line = (Vec<(u16, String, tokens::Color)>, Option<Hit>);

/// A paragraph, wrapped to `room`, in one ink, from `at`.
fn paragraph(out: &mut Vec<Line>, text: &str, at: u16, room: u16, ink: tokens::Color) {
    for l in crate::chrome::note_lines(text, room.saturating_sub(at).max(1)) {
        out.push((vec![(at, l, ink)], None));
    }
}

/// A fact's value, as it is written.
fn value_of(book: &Book, v: Value) -> String {
    match v {
        Value::Said(k) => book.t(k),
        Value::Wire(w) => w.to_owned(),
    }
}

/// Facts, label beside value, the values aligned and wrapped.
fn facts(out: &mut Vec<Line>, book: &Book, facts: &[(String, String)], room: u16) {
    let label = facts
        .iter()
        .map(|(l, _)| ronoctua_render::layout::width(l))
        .max()
        .unwrap_or(0)
        .min(room / 2);
    let at = label + 2;
    let _ = book;
    let (label_ink, value_ink) = crate::chrome::Kind::Fact.tones(false);
    for (l, v) in facts {
        let wrapped = crate::chrome::note_lines(v, room.saturating_sub(at).max(1));
        for (k, piece) in wrapped.into_iter().enumerate() {
            let mut pieces = Vec::new();
            if k == 0 {
                pieces.push((0, ronoctua_render::layout::elide_end(l, label), label_ink));
            }
            pieces.push((at, piece, value_ink));
            out.push((pieces, None));
        }
    }
}

/// A size, as the book writes it: bytes, or kilobytes to a tenth.
#[must_use]
pub fn size_of(book: &Book, bytes: u32) -> String {
    if bytes < 1024 {
        book.t_args(
            ronoctua_packets::key::PACKETS_BYTES,
            &[("n", Arg::Number(i64::from(bytes)))],
        )
    } else {
        book.t_args(
            ronoctua_packets::key::PACKETS_KILOBYTES,
            &[("n", Arg::Fraction(f64::from(bytes) / 1024.0))],
        )
    }
}

/// The line over the stage: the protocol's name, what it stands for and what
/// it does, and its views' tabs at its right.
pub fn top_line(paint: &mut Paint<'_, '_>, layout: &Layout, book: &Book, open: &Open) {
    let (cell, line) = grid();
    let y = layout.title_line;
    let p = open.protocol;
    // THE TABS, from the right edge: a glyph and a name each.
    let mut right = layout.right;
    let mut tabs = Vec::new();
    for view in View::ALL.iter().rev() {
        let name = book.t(view.name());
        let cells = ronoctua_render::layout::width(&name) + 2;
        right = right.saturating_sub(cells + 2);
        tabs.push((*view, right, name, cells));
    }
    let tabs_left = right;
    for (view, at, name, cells) in tabs {
        let ink = if view == open.view {
            tokens::color::TEXT_HEADING
        } else {
            tokens::color::TEXT_DIM
        };
        let r = Rect::of_cells(at, y, cells + 1, 1);
        paint.takes(r, Hit::View(view));
        paint
            .canvas
            .spans
            .push(said(view.glyph().to_string(), at, y, ink));
        paint.canvas.spans.push(said(name, at + 2, y, ink));
    }
    // THE NAME, what it stands for, and what it does.
    let name = book.t(p.name);
    let title = book.t(p.title);
    let tagline = book.t(p.tagline);
    let mut at = layout.left;
    let room = tabs_left.saturating_sub(2);
    for (text, ink) in [
        (name, tokens::color::TEXT_HEADING),
        (title, tokens::color::TEXT_SECONDARY),
        (tagline, tokens::color::TEXT_DIM),
    ] {
        if at + 4 >= room {
            break;
        }
        let text = ronoctua_render::layout::elide_end(&text, room - at);
        let w = ronoctua_render::layout::width(&text);
        paint.canvas.spans.push(said(text, at, y, ink));
        at += w + 3;
    }
    // AND A RULE UNDER IT, the window's width.
    #[allow(clippy::cast_precision_loss)]
    let width = f32::from(layout.right + layout.left + 1) * cell;
    paint
        .canvas
        .under
        .push(crate::chrome::rule(width, (f32::from(y) + 1.5) * line));
}

/// Paints the panel.
#[allow(clippy::too_many_lines)]
pub fn paint(
    paint: &mut Paint<'_, '_>,
    layout: &Layout,
    book: &Book,
    texts: &ronoctua_i18n::Texts,
    open: &Open,
    pose: &Pose,
) {
    let _ = texts;
    let (_, _, room, height) = layout.panel;
    let p = open.protocol;
    let mut out: Vec<Line> = Vec::new();
    let air = |out: &mut Vec<Line>| out.push((Vec::new(), None));
    let packets: Vec<&Packet> = p.packets().map(|(_, pk)| pk).collect();
    let overview = open.t == 0 && !open.playing;

    // IN THE ANATOMY, the field under the pointer first.
    if let (View::Anatomy, Some((h, f)), Some(packet)) = (
        open.view,
        open.field,
        pose.current.and_then(|k| packets.get(k)),
    ) && let Some(field) = packet.headers.get(h).and_then(|hd| hd.layout.fields.get(f))
    {
        let header = &packet.headers[h];
        let bits = if field.bits == 0 {
            book.t(ronoctua_packets::key::PACKETS_ANY_LENGTH)
        } else {
            book.t_args(
                ronoctua_packets::key::PACKETS_BITS,
                &[("n", Arg::Number(i64::from(field.bits)))],
            )
        };
        out.push((
            vec![(0, book.t(header.layout.name), tokens::color::TEXT_DIM)],
            None,
        ));
        let name = book.t(field.name);
        let w = ronoctua_render::layout::width(&name);
        out.push((
            vec![
                (0, name, tokens::color::TEXT_HEADING),
                (w + 2, bits, tokens::color::TEXT_DIM),
            ],
            None,
        ));
        if let Some(v) = header.values.get(f).filter(|v| !v.is_empty()) {
            facts(
                &mut out,
                book,
                &[(
                    book.t(ronoctua_packets::key::PACKETS_VALUE),
                    (*v).to_owned(),
                )],
                room,
            );
        }
        paragraph(
            &mut out,
            &book.t(field.about),
            0,
            room,
            tokens::color::TEXT_SECONDARY,
        );
        air(&mut out);
    }

    if overview {
        out.push((
            vec![(
                0,
                book.t(ronoctua_packets::key::PACKETS_OVERVIEW),
                tokens::color::TEXT_HEADING,
            )],
            None,
        ));
        paragraph(
            &mut out,
            &book.t(p.overview),
            0,
            room,
            tokens::color::TEXT_SECONDARY,
        );
        air(&mut out);
        let mut all = vec![
            (
                book.t(ronoctua_packets::key::PACKETS_FACT_LAYER),
                book.t(p.layer.name()),
            ),
            (
                book.t(ronoctua_packets::key::PACKETS_FACT_GROUP),
                book.t(p.group.name()),
            ),
        ];
        all.extend(
            p.facts
                .iter()
                .map(|f: &Fact| (book.t(f.label), value_of(book, f.value))),
        );
        facts(&mut out, book, &all, room);
    } else {
        let step = &p.steps[pose.step];
        #[allow(clippy::cast_possible_wrap)]
        let of = book.t_args(
            ronoctua_packets::key::PACKETS_STEP_OF,
            &[
                ("n", Arg::Number(pose.step as i64 + 1)),
                ("total", Arg::Number(p.steps.len() as i64)),
            ],
        );
        out.push((vec![(0, of, tokens::color::TEXT_DIM)], None));
        out.push((
            vec![(0, book.t(step.title), tokens::color::TEXT_HEADING)],
            None,
        ));
        paragraph(
            &mut out,
            &book.t(step.sub),
            0,
            room,
            tokens::color::TEXT_SECONDARY,
        );
        air(&mut out);
        paragraph(
            &mut out,
            &book.t(step.body),
            0,
            room,
            tokens::color::TEXT_PRIMARY,
        );
        if !step.facts.is_empty() {
            air(&mut out);
            let all: Vec<(String, String)> = step
                .facts
                .iter()
                .map(|f| (book.t(f.label), value_of(book, f.value)))
                .collect();
            facts(&mut out, book, &all, room);
        }
        // THE PACKET OF THE MOMENT: its class's dot, its protocol and tag,
        // which of how many, its size, and what it says in words.
        if let Some(k) = pose.current
            && let Some(packet) = packets.get(k)
        {
            air(&mut out);
            let dot = tokens::icon::PROTOCOLS_CLASS.to_string();
            let tag = format!("{}  {}", packet.proto, packet.tag);
            out.push((
                vec![
                    (0, dot, ink_of(packet.class)),
                    (
                        2,
                        ronoctua_render::layout::elide_end(&tag, room.saturating_sub(2)),
                        tokens::color::TEXT_PRIMARY,
                    ),
                ],
                Some(Hit::Packet(k)),
            ));
            #[allow(clippy::cast_possible_wrap)]
            let of = book.t_args(
                ronoctua_packets::key::PACKETS_PACKET_OF,
                &[
                    ("n", Arg::Number(k as i64 + 1)),
                    ("total", Arg::Number(packets.len() as i64)),
                ],
            );
            let class = book.t(packet.class.name());
            out.push((
                vec![(
                    2,
                    format!("{of} · {class} · {}", size_of(book, packet.bytes)),
                    tokens::color::TEXT_DIM,
                )],
                None,
            ));
            if let Some(says) = packet.says {
                paragraph(
                    &mut out,
                    &format!("“{}”", book.t(says)),
                    2,
                    room,
                    tokens::color::TEXT_SECONDARY,
                );
            }
        }
    }

    // THE STEPS, each a click; the one of the moment marked — when the
    // panel holds its title and a step at least.
    if out.len() + 3 > usize::from(height) {
        return draw(paint, layout, out);
    }
    air(&mut out);
    out.push((
        vec![(
            0,
            book.t(ronoctua_packets::key::PACKETS_STEPS),
            tokens::color::TEXT_HEADING,
        )],
        None,
    ));
    for (s, step) in p.steps.iter().enumerate() {
        let here = !overview && s == pose.step;
        let ink = if here {
            tokens::color::TEXT_HERE_READ
        } else {
            tokens::color::TEXT_SECONDARY
        };
        let number = format!("{:>2}", s + 1);
        let text = format!("{} — {}", book.t(step.title), book.t(step.sub));
        out.push((
            vec![
                (
                    0,
                    number,
                    if here {
                        tokens::color::TEXT_HEADING
                    } else {
                        tokens::color::TEXT_DIM
                    },
                ),
                (
                    4,
                    ronoctua_render::layout::elide_end(&text, room.saturating_sub(4)),
                    ink,
                ),
            ],
            Some(Hit::Step(s)),
        ));
    }
    if overview {
        air(&mut out);
        paragraph(
            &mut out,
            &book.t(ronoctua_packets::key::PACKETS_READY),
            0,
            room,
            tokens::color::TEXT_DIM,
        );
    }

    draw(paint, layout, out);
}

/// The panel's lines, DRAWN from its top, as many as it holds; the divider
/// beside it when it stands beside the stage.
fn draw(paint: &mut Paint<'_, '_>, layout: &Layout, out: Vec<Line>) {
    let (column, top, room, height) = layout.panel;
    let (cell, _) = grid();
    for (k, (pieces, hit)) in out.into_iter().take(usize::from(height)).enumerate() {
        #[allow(clippy::cast_possible_truncation)]
        let y = top + k as u16;
        if let Some(hit) = hit {
            paint.takes(Rect::of_cells(column, y, room, 1), hit);
        }
        for (at, text, ink) in pieces {
            paint.canvas.spans.push(said(text, column + at, y, ink));
        }
    }
    if layout.panel.1 == layout.stage.1 {
        let panel = Rect::of_cells(column, top, room, height);
        paint.canvas.under.push(crate::chrome::divider(
            panel.x - 1.5 * cell,
            panel.y,
            panel.y + panel.h,
        ));
    }
}
