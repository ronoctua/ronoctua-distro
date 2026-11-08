//! THE SEQUENCE — the same packets as a sequence diagram: a lane down from
//! each device, and every packet an arrow from its sender's lane to its
//! receiver's, in the order they left, under the title of the step that
//! sends them. The packet in the air grows its arrow as it flies; the packet
//! of the moment is lit; any arrow is a click that goes back to the moment it
//! left. The wheel scrolls it when it is longer than the stage.

use crate::packets_window::{Hit, Layout, Open, Paint, Rect, grid, ink_of, placed, quad, wide};
use ronoctua_i18n::Book;
use ronoctua_packets::Packet;
use ronoctua_packets::timeline::Pose;
use ronoctua_tokens as tokens;

/// How far apart two arrows are, in lines.
const ROW: f32 = 1.6;

/// Paints the sequence.
#[allow(clippy::too_many_lines)]
pub fn paint(paint: &mut Paint<'_, '_>, layout: &Layout, book: &Book, open: &Open, pose: &Pose) {
    let stage = layout.stage_rect();
    let (_, line) = grid();
    let p = open.protocol;
    // THE LANES, left to right as the devices stand.
    let mut lanes: Vec<usize> = (0..p.scene.devices.len()).collect();
    lanes.sort_by_key(|d| (p.scene.devices[*d].at.col, p.scene.devices[*d].at.row));
    #[allow(clippy::cast_precision_loss)]
    let lane_x = |d: usize| {
        let k = lanes.iter().position(|l| *l == d).unwrap_or(0);
        stage.x + stage.w * (k as f32 + 0.5) / lanes.len() as f32
    };
    // THEIR HEADS: each device's name and address.
    for &d in &lanes {
        let device = p.scene.devices[d];
        let x = lane_x(d);
        let name = book.t(device.name);
        paint.canvas.spans.push(placed(
            name.clone(),
            x - wide(&name) / 2.0,
            stage.y + line * 0.5,
            tokens::color::TEXT_PRIMARY,
        ));
        if let Some(address) = device.address.text() {
            paint.canvas.spans.push(placed(
                address.clone(),
                x - wide(&address) / 2.0,
                stage.y + line * 1.5,
                tokens::color::TEXT_DIM,
            ));
        }
        paint.canvas.under.push(crate::chrome::divider(
            x,
            stage.y + line * 2.2,
            stage.y + stage.h,
        ));
    }
    // THE ROWS: a step's title before its first packet, then its packets.
    let packets: Vec<(usize, &Packet)> = p.packets().collect();
    let mut rows: Vec<(Option<usize>, usize)> = Vec::new();
    let mut step_of = None;
    for (k, (s, _)) in packets.iter().enumerate() {
        if step_of != Some(*s) {
            step_of = Some(*s);
            rows.push((None, *s));
        }
        rows.push((Some(k), *s));
    }
    let body_top = stage.y + line * 2.8;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let fits = (((stage.y + stage.h - body_top) / (line * ROW))
        .floor()
        .max(1.0)) as usize;
    // WHAT IS SHOWN keeps the packet of the moment in sight, and the wheel
    // moves it from there.
    let last_shown = rows
        .iter()
        .position(|(k, _)| {
            *k == pose.current.or(if pose.sent > 0 {
                Some(pose.sent - 1)
            } else {
                None
            })
        })
        .unwrap_or(0);
    let first = (last_shown + 1)
        .saturating_sub(fits)
        .saturating_add(open.scrolled)
        .min(rows.len().saturating_sub(1));
    let flights: Vec<(usize, f32)> = pose
        .flights
        .iter()
        .filter_map(|f| {
            let pk = packets.get(f.packet)?.1;
            let path = p.path(pk.from, pk.to)?;
            let hops = path.len().saturating_sub(1).max(1);
            let hop = path.iter().position(|d| *d == f.between.0).unwrap_or(0);
            #[allow(clippy::cast_precision_loss)]
            Some((f.packet, (hop as f32 + f.along) / hops as f32))
        })
        .collect();
    for (row, (k, s)) in rows.iter().skip(first).take(fits).enumerate() {
        #[allow(clippy::cast_precision_loss)]
        let y = body_top + line * ROW * (row as f32 + 0.5);
        let Some(k) = *k else {
            let title = format!("{} · {}", s + 1, book.t(p.steps[*s].title));
            paint
                .canvas
                .spans
                .push(placed(title, stage.x, y, tokens::color::TEXT_DIM));
            continue;
        };
        let flying = flights.iter().find(|(f, _)| *f == k).map(|(_, u)| *u);
        if k >= pose.sent && flying.is_none() {
            continue;
        }
        let pk = packets[k].1;
        let (a, b) = (lane_x(pk.from), lane_x(pk.to));
        let reach = flying.unwrap_or(1.0);
        let tip = a + (b - a) * reach;
        let ink = ink_of(pk.class);
        let (x0, x1) = if a < tip { (a, tip) } else { (tip, a) };
        let thick = if pose.current == Some(k) { 2.5 } else { 1.5 };
        paint.canvas.over.push(quad(
            Rect {
                x: x0,
                y: y + line * 0.25,
                w: (x1 - x0).max(1.0),
                h: thick,
            },
            ink,
        ));
        // THE ARROWHEAD, a triangle of thin columns pointing where it goes.
        let toward = if b >= a { 1.0 } else { -1.0 };
        for i in 0..6 {
            #[allow(clippy::cast_precision_loss)]
            let i = i as f32;
            let half = (5.0 - i) * 0.9;
            paint.canvas.over.push(quad(
                Rect {
                    x: tip - toward * (6.0 - i) - 0.5,
                    y: y + line * 0.25 + thick / 2.0 - half,
                    w: 1.2,
                    h: half * 2.0,
                },
                ink,
            ));
        }
        // ITS TAG over it, and the click that takes it.
        let ink_text = if pose.current == Some(k) {
            tokens::color::TEXT_PRIMARY
        } else {
            tokens::color::TEXT_SECONDARY
        };
        let w = wide(pk.tag);
        paint.canvas.spans.push(placed(
            pk.tag.to_owned(),
            (a + b) / 2.0 - w / 2.0,
            y - line * 0.3,
            ink_text,
        ));
        paint.takes(
            Rect {
                x: x0.min(b),
                y: y - line * 0.75,
                w: (a - b).abs().max(w),
                h: line * ROW,
            },
            Hit::Packet(k),
        );
    }
}
