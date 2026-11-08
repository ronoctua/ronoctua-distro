//! THE ANATOMY — the packet of the moment opened header by header, the
//! outermost first, each drawn the way its standard draws it: rows of 32
//! bits, a field a box as wide as its bits, its name and the value this
//! packet gives it. The fields the step talks about are lit; the field under
//! the pointer, or clicked, is explained in the panel. A text protocol's
//! header is its lines, a field each. The wheel scrolls it when it is longer
//! than the stage.

use crate::packets_window::{Hit, Layout, Open, Paint, Rect, grid, ink_of, placed, quad, wide};
use ronoctua_i18n::Book;
use ronoctua_packets::anatomy::{self, ROW_BITS};
use ronoctua_packets::timeline::Pose;
use ronoctua_tokens as tokens;

/// What the anatomy draws, line by line before it is cut to the stage: a
/// line of text, or a row of fields `rows` lines tall.
enum Part {
    /// Words: `(px in, text, ink)` each.
    Words(Vec<(f32, String, tokens::Color)>),
    /// A row of fields: the header, and its pieces.
    Fields(usize, Vec<anatomy::Piece>),
    /// A line of a text header: the header and the field.
    Text(usize, usize),
}

/// How many lines a row of fields takes.
const FIELD_LINES: f32 = 2.2;

/// Paints the anatomy.
#[allow(clippy::too_many_lines)]
pub fn paint(paint: &mut Paint<'_, '_>, layout: &Layout, book: &Book, open: &Open, pose: &Pose) {
    let stage = layout.stage_rect();
    let (cell, line) = grid();
    let p = open.protocol;
    let packets: Vec<_> = p.packets().map(|(_, pk)| pk).collect();
    let Some(k) = pose.current else {
        paint.canvas.spans.push(placed(
            book.t(ronoctua_packets::key::PACKETS_NO_PACKET),
            stage.x,
            stage.y + line / 2.0,
            tokens::color::TEXT_DIM,
        ));
        return;
    };
    let packet = packets[k];
    let mut parts: Vec<Part> = vec![Part::Words(vec![(
        0.0,
        book.t(ronoctua_packets::key::PACKETS_ANATOMY_OF),
        tokens::color::TEXT_DIM,
    )])];
    let tag = format!("{}  {}", packet.proto, packet.tag);
    parts.push(Part::Words(vec![
        (
            0.0,
            tokens::icon::PROTOCOLS_CLASS.to_string(),
            ink_of(packet.class),
        ),
        (cell * 2.0, tag, tokens::color::TEXT_PRIMARY),
    ]));
    if packet.headers.is_empty() {
        parts.push(Part::Words(Vec::new()));
        parts.push(Part::Words(vec![(
            0.0,
            book.t(ronoctua_packets::key::PACKETS_NO_HEADERS),
            tokens::color::TEXT_SECONDARY,
        )]));
    }
    for (h, header) in packet.headers.iter().enumerate() {
        parts.push(Part::Words(Vec::new()));
        let bits: u32 = header.layout.fields.iter().map(|f| u32::from(f.bits)).sum();
        let name = book.t(header.layout.name);
        let mut words = vec![(0.0, name.clone(), tokens::color::TEXT_HEADING)];
        if !header.layout.text {
            words.push((
                wide(&name) + cell * 2.0,
                book.t_args(
                    ronoctua_packets::key::PACKETS_BITS,
                    &[("n", ronoctua_i18n::Arg::Number(i64::from(bits)))],
                ),
                tokens::color::TEXT_DIM,
            ));
        }
        parts.push(Part::Words(words));
        if header.layout.text {
            for f in 0..header.layout.fields.len() {
                parts.push(Part::Text(h, f));
            }
        } else {
            // THE BITS' RULER over the rows.
            #[allow(clippy::cast_precision_loss)]
            let ruler = [0_u16, 8, 16, 24, 31]
                .iter()
                .map(|b| {
                    (
                        stage.w * f32::from(*b) / f32::from(ROW_BITS),
                        b.to_string(),
                        tokens::color::TEXT_DIM,
                    )
                })
                .collect();
            parts.push(Part::Words(ruler));
            for row in anatomy::rows(header.layout) {
                parts.push(Part::Fields(h, row));
            }
        }
    }
    // DRAWN from the stage's top, the wheel's rows skipped, as far as the
    // stage goes.
    let mut y = stage.y;
    let bottom = stage.y + stage.h;
    for part in parts.iter().skip(open.scrolled) {
        let tall = match part {
            Part::Fields(..) => line * FIELD_LINES,
            _ => line,
        };
        if y + tall > bottom {
            break;
        }
        match part {
            Part::Words(words) => {
                for (x, text, ink) in words {
                    paint.canvas.spans.push(placed(
                        text.clone(),
                        stage.x + x,
                        y + line / 2.0,
                        *ink,
                    ));
                }
            }
            Part::Text(h, f) => {
                let header = &packet.headers[*h];
                let field = &header.layout.fields[*f];
                let lit = header.lit.contains(f);
                let chosen = open.field == Some((*h, *f));
                let r = Rect {
                    x: stage.x,
                    y,
                    w: stage.w,
                    h: line,
                };
                if lit || chosen {
                    paint.canvas.under.push(quad(
                        r,
                        if chosen {
                            tokens::color::SURFACE_HOVER
                        } else {
                            tokens::color::SURFACE_SELECTION
                        },
                    ));
                }
                paint.takes(r, Hit::Field(*h, *f));
                let name = book.t(field.name);
                let label = cell * 18.0;
                let (label_ink, value_ink) = crate::chrome::Kind::Fact.tones(false);
                paint.canvas.spans.push(placed(
                    ronoctua_render::layout::elide_end(&name, 17),
                    stage.x + cell,
                    y + line / 2.0,
                    label_ink,
                ));
                let value = header.values.get(*f).copied().unwrap_or_default();
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let room = ((stage.w - label - cell) / cell).max(1.0) as u16;
                paint.canvas.spans.push(placed(
                    ronoctua_render::layout::elide_end(value, room),
                    stage.x + label,
                    y + line / 2.0,
                    value_ink,
                ));
            }
            Part::Fields(h, row) => {
                let header = &packet.headers[*h];
                for piece in row {
                    let field = &header.layout.fields[piece.field];
                    let lit = header.lit.contains(&piece.field);
                    let chosen = open.field == Some((*h, piece.field));
                    let x = stage.x + stage.w * f32::from(piece.from) / f32::from(ROW_BITS);
                    let w = stage.w * f32::from(piece.bits) / f32::from(ROW_BITS);
                    let r = Rect {
                        x: x + 1.0,
                        y: y + 1.0,
                        w: (w - 2.0).max(1.0),
                        h: line * FIELD_LINES - 2.0,
                    };
                    let ground = if chosen {
                        tokens::color::SURFACE_HOVER
                    } else if lit {
                        tokens::color::SURFACE_SELECTION
                    } else {
                        tokens::color::SURFACE_CONTAINER
                    };
                    paint.canvas.under.push(quad(r, ground));
                    if lit {
                        // THE LIT FIELD'S EDGE, in the packet's colour.
                        paint.canvas.over.push(quad(
                            Rect {
                                x: r.x,
                                y: r.y + r.h - 2.5,
                                w: r.w,
                                h: 2.5,
                            },
                            ink_of(packet.class),
                        ));
                    }
                    paint.hits.push((r, Hit::Field(*h, piece.field)));
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    let room = ((r.w / cell) - 1.0).max(0.0) as u16;
                    if room == 0 {
                        continue;
                    }
                    // A FIELD GOING ON from the row above says its name again,
                    // so the box is read as the same field.
                    if !piece.first {
                        let name = ronoctua_render::layout::elide_end(&book.t(field.name), room);
                        paint.canvas.spans.push(placed(
                            name.clone(),
                            r.x + (r.w - wide(&name)) / 2.0,
                            y + line * 1.1,
                            tokens::color::TEXT_DIM,
                        ));
                    }
                    // ITS NAME on the field's first piece, and its value.
                    if piece.first {
                        let name = ronoctua_render::layout::elide_end(&book.t(field.name), room);
                        paint.canvas.spans.push(placed(
                            name.clone(),
                            r.x + (r.w - wide(&name)) / 2.0,
                            y + line * 0.6,
                            tokens::color::TEXT_DIM,
                        ));
                        if let Some(value) =
                            header.values.get(piece.field).filter(|v| !v.is_empty())
                        {
                            let value = ronoctua_render::layout::elide_end(value, room);
                            paint.canvas.spans.push(placed(
                                value.clone(),
                                r.x + (r.w - wide(&value)) / 2.0,
                                y + line * 1.55,
                                tokens::color::TEXT_PRIMARY,
                            ));
                        }
                    }
                }
            }
        }
        y += tall;
    }
}
