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

use ronoctua_render::layout::{Segment, Shrink, Side, resolve};
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
        },
        Segment {
            id: "notif",
            text: format!("{} 3", tokens::icon::NOTIFICATION),
            priority: 2,
            min_cols: 3,
            side: Side::Right,
            shrink: Shrink::Cut,
            removable: false,
        },
        Segment {
            id: "buffer",
            text: name.to_owned(),
            priority: 4,
            min_cols: 8,
            side: Side::Left,
            shrink: Shrink::InTheMiddle,
            removable: false,
        },
        Segment {
            id: "mode",
            text: "rust".into(),
            priority: 6,
            min_cols: 4,
            side: Side::Left,
            shrink: Shrink::Cut,
            removable: true,
        },
        Segment {
            id: "state",
            text: "search".into(),
            priority: 7,
            min_cols: 4,
            side: Side::Left,
            shrink: Shrink::Cut,
            removable: true,
        },
    ]
}

fn main() -> std::process::ExitCode {
    let repeats = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(20_000);

    // A LONG name on purpose: a browser title goes past 200 characters,
    // and it is what makes the elision work.
    let name = "projeto/subpasta/mais/fundo/ainda/um/arquivo-de-nome-bem-longo.rs";
    let segs = segments(name);

    // Widths that exercise the three regimes: slack, tight and phase B.
    let widths = [200_u16, 80, 40, 20];
    let mut worst = 0.0_f64;

    for width in widths {
        // Aquecimento: a primeira passada paga cache frio.
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
        println!("layout {width:>4} colunas: p50={p50:.4} ms  p99={p99:.4} ms");
        worst = worst.max(p99);
    }

    let ceiling = FRAME_MS * SLICE;
    let pct = (SLICE * 100.0) as u32;
    println!();
    println!("pior p99: {worst:.4} ms   teto: {ceiling:.1} ms ({pct}% do frame de {FRAME_MS} ms)");

    if worst > ceiling {
        eprintln!(
            "\nERRO: o layout sozinho passa de {pct}% do frame.\n\
             \n\
             Ele e a PRIMEIRA etapa; o que sobra e para moldar texto, subir\n\
             atlas e desenhar. Ver a secao 5.2."
        );
        return std::process::ExitCode::FAILURE;
    }
    println!("OK: o layout cabe com folga");
    std::process::ExitCode::SUCCESS
}
