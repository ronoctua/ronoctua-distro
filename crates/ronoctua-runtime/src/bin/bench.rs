//! Measures instantiation and FAILS above the ceiling — section 5.2's gate.
//!
//! # Why a binary, and not a `#[bench]`
//!
//! The Nix check needs a command that exits non-zero. A benchmark that prints
//! pretty numbers and exits zero does not stop a regression — and section 9.3
//! is explicit in saying that the CI **gates** the GPU-independent metrics, and
//! instantiation is one of them.
//!
//! Usage: `ronoctua-bench-instance <component.wasm> [repetitions]`

use std::time::Instant;

use anyhow::{Context as _, Result, bail};
use ronoctua_runtime::{CEILING_MS, Runtime};

/// Measures one engine, giving back (p50, p99).
fn measure(rt: &Runtime, wasm: &[u8], repeats: usize) -> Result<(f64, f64)> {
    let cwasm = rt.precompile(wasm)?;
    let component = rt.load(&cwasm)?;

    // Warm-up. The first instance pays page faults the following ones do
    // not pay, and including that in the sample would measure the OS.
    for _ in 0..10 {
        drop(rt.instantiate(&component)?);
    }

    let mut samples = Vec::with_capacity(repeats);
    for _ in 0..repeats {
        let t = Instant::now();
        let store = rt.instantiate(&component)?;
        samples.push(t.elapsed().as_secs_f64() * 1000.0);
        drop(store);
    }
    samples.sort_by(f64::total_cmp);
    let p = |q: f64| samples[((samples.len() as f64 - 1.0) * q) as usize];
    Ok((p(0.50), p(0.99)))
}

fn main() -> Result<()> {
    let path = std::env::args()
        .nth(1)
        .context("usage: ronoctua-bench-instance <component.wasm> [repeats]")?;
    let repeats: usize = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(200);

    let wasm = std::fs::read(&path).with_context(|| format!("could not read {path}"))?;
    let rt = Runtime::fresh()?;

    println!("component: {} KiB", wasm.len() / 1024);

    let (p50, p99) = measure(&rt, &wasm, repeats)?;
    println!("with     memory_init_cow: p50={p50:.3} ms  p99={p99:.3} ms  (n={repeats})");

    // THE COMPARISON. Without it, "memory_init_cow is mandatory" would be
    // hearsay; with it, it is a number this component produces. It does not fail
    // the build — whoever gates is the production mode, right below.
    match Runtime::without_cow().and_then(|r| measure(&r, &wasm, repeats)) {
        Ok((s50, s99)) => {
            println!("without  memory_init_cow: p50={s50:.3} ms  p99={s99:.3} ms");
            if p99 > 0.0 {
                println!("the difference at p99: {:.1}x", s99 / p99);
            }
        }
        Err(e) => println!("(could not measure without cow: {e})"),
    }

    // The P99, not the average. The average hides exactly the case the user
    // notices — the buffer that took long.
    if p99 > CEILING_MS {
        bail!(
            "a p99 of {p99:.3} ms goes past section 5.2's ceiling of {CEILING_MS} ms.\n\
             \n\
             The first suspect is `memory_init_cow`: without it the memory image\n\
             is copied per instance. See `Runtime::fresh`."
        );
    }
    println!("OK: within the ceiling of {CEILING_MS} ms");
    Ok(())
}
