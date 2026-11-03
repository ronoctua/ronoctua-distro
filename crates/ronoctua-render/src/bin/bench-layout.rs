//! Measures what section 9.3 calls the metrics INDEPENDENT OF THE GPU.
//!
//! # Why these, and not "how many frames per second"
//!
//! Section 9.3 is explicit: 5.2's frame targets are **never** measured under
//! software rasterization — they would measure llvmpipe, not the system. What
//! the CI can gate is the CPU work that happens before any pixel: building the
//! display list, resolving the layout, shaping the text.
//!
//! If those three overflow, the frame overflows on any GPU. If they fit, what
//! is left depends on the machine — and then it is `nix run .#bench` on it.

use std::time::Instant;

use ronoctua_render::layout::{Place, Segment, Shrink, Side, resolve};
use ronoctua_tokens as tokens;

/// The frame's budget, from section 5.2.
///
/// The whole layout has to fit in a FRACTION of that — it is only the first of
/// the frame's stages. A tenth is the ceiling I use here: above that, the rest
/// of the frame has no slack.
const FRAME_MS: f64 = 16.0;
const SLICE: f64 = 0.1;

fn segments(name: &str) -> Vec<Segment> {
    vec![
        Segment {
            id: "mode",
            text: "NORMAL".into(),
            priority: 1,
            min_cols: 3,
            side: Side::Left,
            shrink: Shrink::Cut,
            removable: false,
            place: None,
        },
        // The bell as the modeline draws it now: one glyph, no count, and
        // only there while something is unread.
        Segment {
            id: "notif",
            text: tokens::icon::NOTIFICATION_PENDING.to_string(),
            priority: 2,
            min_cols: 1,
            side: Side::Right,
            shrink: Shrink::Cut,
            removable: true,
            place: None,
        },
        Segment {
            id: "buffer",
            text: name.to_owned(),
            priority: 4,
            min_cols: 8,
            side: Side::Left,
            shrink: Shrink::InTheMiddle,
            removable: false,
            place: None,
        },
        Segment {
            id: "mode",
            text: "rust".into(),
            priority: 6,
            min_cols: 4,
            side: Side::Left,
            shrink: Shrink::Cut,
            removable: true,
            place: None,
        },
        Segment {
            id: "state",
            text: "search".into(),
            priority: 7,
            min_cols: 4,
            side: Side::Left,
            shrink: Shrink::Cut,
            removable: true,
            place: None,
        },
    ]
}

/// THE FULLEST RIGHT SIDE A MODELINE CARRIES: the line above, a deskful of
/// marks at the rungs they sit at, and NINE LLM accounts' sets — each its
/// lead and agents, touching at the rung far from the edge (`Place`) and
/// dropping at the brain's priority. The sets are the one scenario where
/// where a segment sits and when it goes are two answers, and the layout
/// asks both for every gap (`layout::resolve`).
fn nine_sets(name: &str) -> Vec<Segment> {
    let mut out = segments(name);
    let glyph = tokens::icon::NOTIFICATION_PENDING;
    // TWENTY MARKS of one glyph or a glyph and a number, from the edge's
    // rung to the numbers' — what a busy machine draws.
    for (n, id) in [
        "m-1", "m-2", "m-3", "m-4", "m-5", "m-6", "m-7", "m-8", "m-9", "m-10", "m-11", "m-12",
        "m-13", "m-14", "m-15", "m-16", "m-17", "m-18", "m-19", "m-20",
    ]
    .into_iter()
    .enumerate()
    {
        out.push(Segment {
            id,
            text: if n % 3 == 0 {
                format!("{glyph} {n}")
            } else {
                glyph.to_string()
            },
            priority: 1 + u8::try_from(n % 17).unwrap_or(0),
            min_cols: 1,
            side: Side::Right,
            shrink: Shrink::Cut,
            removable: true,
            place: None,
        });
    }
    // AND THE NINE SETS, pushed from the last: a pad, the lead, up to nine
    // agents a cell apart, a pad.
    for (n, id) in [
        "s-9", "s-8", "s-7", "s-6", "s-5", "s-4", "s-3", "s-2", "s-1",
    ]
    .into_iter()
    .enumerate()
    {
        let members = 1 + n;
        let text = format!(" {} ", vec![glyph.to_string(); members].join(" "));
        out.push(Segment {
            id,
            text,
            priority: 9,
            min_cols: 1,
            side: Side::Right,
            shrink: Shrink::Cut,
            removable: true,
            place: Some(Place {
                rung: 18,
                touching: true,
            }),
        });
    }
    out
}

fn main() -> std::process::ExitCode {
    let repeats = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(20_000);

    // A LONG name on purpose: a browser title goes past 200 characters,
    // and it is what makes the elision work.
    let name = "project/subfolder/deeper/still/one/a-rather-long-file-name.rs";

    // Widths that exercise the three regimes: slack, tight and phase B.
    let widths = [200_u16, 80, 40, 20];
    let mut worst = 0.0_f64;

    for (scenario, segs) in [("plain", segments(name)), ("nine sets", nine_sets(name))] {
        for width in widths {
            // Warm-up: the first pass pays for a cold cache.
            for _ in 0..100 {
                std::hint::black_box(resolve(width, &segs));
            }

            let mut samples = Vec::with_capacity(repeats);
            for _ in 0..repeats {
                let t = Instant::now();
                std::hint::black_box(resolve(width, &segs));
                samples.push(t.elapsed().as_secs_f64() * 1000.0);
            }
            samples.sort_by(f64::total_cmp);
            let p99 = samples[(samples.len() as f64 * 0.99) as usize];
            let p50 = samples[samples.len() / 2];
            println!("layout {scenario:>9}, {width:>4} columns: p50={p50:.4} ms  p99={p99:.4} ms");
            worst = worst.max(p99);
        }
    }

    let ceiling = FRAME_MS * SLICE;
    let pct = (SLICE * 100.0) as u32;
    println!();
    println!(
        "worst p99: {worst:.4} ms   ceiling: {ceiling:.1} ms ({pct}% of the {FRAME_MS} ms frame)"
    );

    if worst > ceiling {
        eprintln!(
            "\nERROR: layout on its own goes past {pct}% of the frame.\n\
             \n\
             It is the FIRST stage; what is left is for shaping text, bringing up\n\
             the atlas and drawing. See section 5.2."
        );
        return std::process::ExitCode::FAILURE;
    }
    println!("OK: the layout fits with room to spare");
    std::process::ExitCode::SUCCESS
}
