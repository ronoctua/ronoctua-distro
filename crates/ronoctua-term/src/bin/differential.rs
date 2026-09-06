//! Differential test of VT engines — section 7.10, milestone 7's criterion.
//!
//! # What it does when there are TWO engines
//!
//! It feeds the same corpus into both and compares the grids. A zero diff is the
//! criterion; any differing cell is a divergence of interpretation, which is
//! the most expensive bug to find later — it shows up as "the terminal goes
//! odd sometimes".
//!
//! # What it does when there is ONE
//!
//! It compares against the RECORDED grid (a golden file). It is what risk R1 prescribes
//! as degradation, and it is worth saying what is lost: a golden file catches
//! **regression** (I changed something and the output changed), not **divergence** (two
//! engines disagree). When `libghostty-vt` arrives, the mode goes back to the
//! strong one without touching this file — just by adding the engine to the list.
//!
//! Usage:
//!   ronoctua-term-diff <corpus-dir> <golden-dir>            compares
//!   ronoctua-term-diff <corpus-dir> <golden-dir> --write_out   re-records

use std::path::Path;

use anyhow::{Context as _, Result, bail};
use ronoctua_term::{Engine, alacritty::Alacritty};

/// The corpus grid's size.
///
/// Narrow on purpose: 20 columns make line wrapping happen in
/// case 07 with few bytes, and wrapping is where engines diverge.
const COLUMNS: u16 = 20;
const ROWS: u16 = 6;

/// The available engines.
///
/// TWO when `libghostty-vt` was built, one otherwise — and the count is printed,
/// because "the differential passed" means something different in each case. The
/// `cfg` comes from `build.rs`: outside the Nix build the library is absent, and
/// a crate that failed to compile over it would take the whole workspace with it
/// for a test harness.
///
/// A Ghostty that refuses to start is a WARNING and not a failure. It is a
/// second opinion: without it the comparison degrades to the golden file, which
/// is exactly the documented single-engine mode. Taking the run down would turn
/// a lost opinion into no result at all.
fn engines() -> Vec<Box<dyn Engine>> {
    let mut out: Vec<Box<dyn Engine>> = vec![Box::new(Alacritty::fresh(COLUMNS, ROWS))];
    #[cfg(ghostty_vt)]
    match ronoctua_term::ghostty::Ghostty::fresh(COLUMNS, ROWS) {
        Ok(engine) => out.push(Box::new(engine)),
        Err(e) => eprintln!(
            "WARNING: the ghostty engine is unavailable ({e}); comparing against the golden"
        ),
    }
    out
}

fn main() -> Result<()> {
    let corpus = std::env::args()
        .nth(1)
        .context("usage: differential <corpus> <golden> [--record]")?;
    let golden = std::env::args()
        .nth(2)
        .context("the golden's directory is missing")?;
    let write_out = std::env::args().any(|a| a == "--record");

    let corpus = Path::new(&corpus);
    let golden = Path::new(&golden);
    if write_out {
        std::fs::create_dir_all(golden)?;
    }

    let mut cases: Vec<_> = std::fs::read_dir(corpus)
        .with_context(|| format!("could not read {}", corpus.display()))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "vt"))
        .collect();
    // A stable order: the failure report has to be the same between runs.
    cases.sort();
    if cases.is_empty() {
        bail!("no `.vt` case in {}", corpus.display());
    }

    let n_engines = engines().len();
    println!("{} cases, {n_engines} engine(s)", cases.len());
    if n_engines < 2 {
        println!(
            "GOLDEN-FILE MODE: there is only one engine, so this catches a regression, not\n\
             a divergence of interpretation. See risk R1 in ARCHITECTURE.md."
        );
    }

    let mut failures = Vec::new();
    for case in &cases {
        let name = case
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let bytes = std::fs::read(case)?;

        let grids: Vec<(String, _)> = engines()
            .into_iter()
            .map(|mut m| {
                let name = m.name().to_owned();
                m.feed(&bytes);
                (name, m.grid())
            })
            .collect();

        // Strong mode: everyone against the first.
        let (name_ref, grid_ref) = &grids[0];
        for (otherwise, g) in &grids[1..] {
            let d = grid_ref.diff(g);
            if !d.is_empty() {
                failures.push(format!(
                    "{name}: {name_ref} x {otherwise} divergem:\n  {}",
                    d.join("\n  ")
                ));
            }
        }

        // Golden mode.
        let file = golden.join(format!("{name}.txt"));
        let current = grid_ref.canonical_f();
        if write_out {
            std::fs::write(&file, &current)?;
            continue;
        }
        let expected = std::fs::read_to_string(&file).with_context(|| {
            format!(
                "the golden {} is missing.\n\
                 \n\
                 Make it with: differential <corpus> <golden> --record\n\
                 and CHECK the result before versioning it — a wrong golden\n\
                 freezes the bug instead of catching it.",
                file.display()
            )
        })?;
        if current != expected {
            failures.push(format!(
                "{name}: the grid changed.\n--- recorded ---\n{expected}\n--- now ---\n{current}"
            ));
        }
    }

    if write_out {
        println!(
            "OK: {} goldens written into {}",
            cases.len(),
            golden.display()
        );
        return Ok(());
    }
    if !failures.is_empty() {
        bail!(
            "{} case(s) with a difference:\n\n{}",
            failures.len(),
            failures.join("\n\n")
        );
    }
    println!("OK: ZERO grid diff in {} cases", cases.len());
    Ok(())
}
