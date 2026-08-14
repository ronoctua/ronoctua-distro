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
//!   diferencial <corpus-dir> <golden-dir>            compares
//!   diferencial <corpus-dir> <golden-dir> --write_out   re-records

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
/// A list of ONE today. Adding Ghostty here is the only step for the actual
/// differential mode to come back.
fn engines() -> Vec<Box<dyn Engine>> {
    vec![Box::new(Alacritty::fresh(COLUMNS, ROWS))]
}

fn main() -> Result<()> {
    let corpus = std::env::args()
        .nth(1)
        .context("uso: diferencial <corpus> <golden> [--gravar]")?;
    let golden = std::env::args()
        .nth(2)
        .context("falta o diretorio do golden")?;
    let write_out = std::env::args().any(|a| a == "--gravar");

    let corpus = Path::new(&corpus);
    let golden = Path::new(&golden);
    if write_out {
        std::fs::create_dir_all(golden)?;
    }

    let mut cases: Vec<_> = std::fs::read_dir(corpus)
        .with_context(|| format!("nao consegui ler {}", corpus.display()))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "vt"))
        .collect();
    // A stable order: the failure report has to be the same between runs.
    cases.sort();
    if cases.is_empty() {
        bail!("nenhum caso `.vt` em {}", corpus.display());
    }

    let n_motores = engines().len();
    println!("{} casos, {n_motores} motor(es)", cases.len());
    if n_motores < 2 {
        println!(
            "MODO GOLDEN-FILE: so ha um motor, entao isto pega regressao, nao\n\
             divergencia de interpretacao. Ver o risco R1 no ARCHITECTURE.md."
        );
    }

    let mut failures = Vec::new();
    for caso in &cases {
        let name = caso
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let bytes = std::fs::read(caso)?;

        let grades: Vec<(String, _)> = engines()
            .into_iter()
            .map(|mut m| {
                let name = m.name().to_owned();
                m.feed(&bytes);
                (name, m.grid())
            })
            .collect();

        // Strong mode: everyone against the first.
        let (nome_ref, grade_ref) = &grades[0];
        for (otherwise, g) in &grades[1..] {
            let d = grade_ref.diff(g);
            if !d.is_empty() {
                failures.push(format!(
                    "{name}: {nome_ref} x {otherwise} divergem:\n  {}",
                    d.join("\n  ")
                ));
            }
        }

        // Golden mode.
        let file = golden.join(format!("{name}.txt"));
        let current = grade_ref.canonical_f();
        if write_out {
            std::fs::write(&file, &current)?;
            continue;
        }
        let expected = std::fs::read_to_string(&file).with_context(|| {
            format!(
                "falta o golden {}.\n\
                 \n\
                 Gere com: diferencial <corpus> <golden> --gravar\n\
                 e CONFIRA o resultado antes de versionar — um golden errado\n\
                 congela o bug em vez de pegar.",
                file.display()
            )
        })?;
        if current != expected {
            failures.push(format!(
                "{name}: a grade mudou.\n--- gravado ---\n{expected}\n--- agora ---\n{current}"
            ));
        }
    }

    if write_out {
        println!(
            "OK: {} goldens gravados em {}",
            cases.len(),
            golden.display()
        );
        return Ok(());
    }
    if !failures.is_empty() {
        bail!(
            "{} caso(s) com diferenca:\n\n{}",
            failures.len(),
            failures.join("\n\n")
        );
    }
    println!("OK: diff de grade ZERO em {} casos", cases.len());
    Ok(())
}
