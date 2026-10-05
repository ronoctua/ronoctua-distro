//! The gate before fast-access depends on tantivy.
//!
//! One million synthetic file names (folders, dates, camelCase, digits, the
//! shapes a home holds), a tenth of them with a body of prose, indexed by the
//! same [`Writer`] the unit runs and asked through the same
//! [`Index::query`] the doors call — keystroke by keystroke, the way a person
//! types a query: `r`, `re`, `rep`, …, `report 2`, `report 20`, and a typo
//! for the fuzzy match.
//!
//! The gate: p99 ≤ 8 ms over the names (`/f`), and ≤ 25 ms for the default
//! scope, names and contents together. Past either the command fails: the
//! design falls back to an FST of our own rather than building on a number
//! that was never met.
//!
//! Usage: `ronoctua-bench-index [names] [one body every N] [folder]`.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use ronoctua_index::write::{Document, Writer};
use ronoctua_index::{Index, Kind, Query};

/// The gate, in milliseconds.
const NAMES_P99_MS: f64 = 8.0;
const CONTENT_P99_MS: f64 = 25.0;

/// Rows a query asks for, per kind (fast-access's sections).
const MOST: usize = 20;

/// SplitMix64: deterministic, so two runs measure the same index.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % (n as u64)).unwrap_or(0)
    }
}

/// A vocabulary of pseudo-words, and a Zipf sampler over it.
struct Words {
    words: Vec<String>,
    cumulative: Vec<f64>,
}

impl Words {
    fn new(rng: &mut Rng, size: usize) -> Self {
        const COMMON: [&str; 40] = [
            "report",
            "invoice",
            "photo",
            "final",
            "draft",
            "notes",
            "backup",
            "config",
            "main",
            "index",
            "readme",
            "test",
            "data",
            "image",
            "video",
            "music",
            "project",
            "budget",
            "letter",
            "resume",
            "screenshot",
            "download",
            "setup",
            "build",
            "source",
            "release",
            "meeting",
            "summary",
            "presentation",
            "contract",
            "travel",
            "family",
            "school",
            "recipe",
            "manual",
            "archive",
            "export",
            "import",
            "schedule",
            "thesis",
        ];
        const ONSETS: [&str; 20] = [
            "b", "c", "d", "f", "g", "l", "m", "n", "p", "r", "s", "t", "v", "br", "tr", "st",
            "ch", "pl", "gr", "qu",
        ];
        const VOWELS: [&str; 8] = ["a", "e", "i", "o", "u", "ai", "ou", "ea"];
        let mut words: Vec<String> = COMMON.iter().map(|w| (*w).to_owned()).collect();
        let mut seen: std::collections::HashSet<String> = words.iter().cloned().collect();
        while words.len() < size {
            let syllables = 1 + rng.below(4);
            let mut w = String::new();
            for _ in 0..syllables {
                w.push_str(ONSETS[rng.below(ONSETS.len())]);
                w.push_str(VOWELS[rng.below(VOWELS.len())]);
            }
            if rng.below(3) == 0 {
                w.push_str(ONSETS[rng.below(ONSETS.len())]);
            }
            if seen.insert(w.clone()) {
                words.push(w);
            }
        }
        let mut cumulative = Vec::with_capacity(words.len());
        let mut sum = 0.0;
        for rank in 0..words.len() {
            #[allow(clippy::cast_precision_loss)]
            let weight = 1.0 / (rank as f64 + 1.0);
            sum += weight;
            cumulative.push(sum);
        }
        Self { words, cumulative }
    }

    fn zipf(&self, rng: &mut Rng) -> &str {
        let total = *self.cumulative.last().unwrap_or(&1.0);
        #[allow(clippy::cast_precision_loss)]
        let x = (rng.next() >> 11) as f64 / (1u64 << 53) as f64 * total;
        let at = self.cumulative.partition_point(|&c| c < x);
        &self.words[at.min(self.words.len() - 1)]
    }

    fn uniform(&self, rng: &mut Rng) -> &str {
        &self.words[rng.below(self.words.len())]
    }
}

fn capital(w: &str) -> String {
    let mut c = w.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}

/// One synthetic file: its document, and the words a person would type to
/// find it.
fn file(rng: &mut Rng, words: &Words, now_s: i64, with_body: bool) -> (Document, Vec<String>) {
    const EXT: [&str; 12] = [
        "pdf", "txt", "jpg", "png", "docx", "odt", "rs", "md", "mp3", "mkv", "xlsx", "json",
    ];
    const TOP: [&str; 8] = [
        "Documents",
        "Pictures",
        "Music",
        "Videos",
        "Downloads",
        "projects",
        "src",
        "Desktop",
    ];
    let ext = EXT[rng.below(EXT.len())];
    let a = words.uniform(rng).to_owned();
    let b = words.uniform(rng).to_owned();
    let number = rng.below(10_000);
    let name = match rng.below(7) {
        0 => format!("{a}-{b}.{ext}"),
        1 => format!("{}{}.{ext}", capital(&a), capital(&b)),
        2 => format!(
            "IMG_{}{:02}{:02}_{:06}.jpg",
            2019 + rng.below(7),
            1 + rng.below(12),
            1 + rng.below(28),
            rng.below(1_000_000)
        ),
        3 => format!("{a}_{number}.{ext}"),
        4 => format!("{} {b} ({}).{ext}", capital(&a), 1 + rng.below(9)),
        5 => format!("{a}.{ext}"),
        _ => format!("{a}{number}.{ext}"),
    };
    let mut path = PathBuf::from("/home/ana");
    path.push(TOP[rng.below(TOP.len())]);
    for _ in 0..rng.below(5) {
        path.push(words.uniform(rng));
    }
    path.push(&name);
    let age_s = i64::try_from(rng.below(3 * 365 * 86_400)).unwrap_or(0);
    let kind = if rng.below(20) == 0 {
        Kind::Folder
    } else {
        Kind::File
    };
    let mut document = Document::of_path(&path, kind, None, now_s - age_s, rng.next() % 50_000_000);
    if with_body {
        let length = 150 + rng.below(150);
        let mut body = String::with_capacity(length * 8);
        for k in 0..length {
            if k > 0 {
                body.push(if k % 12 == 0 { '\n' } else { ' ' });
            }
            body.push_str(words.zipf(rng));
        }
        document.body = Some(body);
    }
    (document, vec![a, b])
}

/// The keystrokes of a query: every prefix of the first word, then the first
/// word and every prefix of the second.
fn keystrokes(first: &str, second: Option<&str>) -> Vec<String> {
    let mut out = Vec::new();
    let mut typed = String::new();
    for c in first.chars() {
        typed.push(c);
        out.push(typed.clone());
    }
    if let Some(second) = second {
        typed.push(' ');
        for c in second.chars() {
            typed.push(c);
            out.push(typed.clone());
        }
    }
    out
}

/// A word with two neighbouring letters swapped: what the fuzzy match is for.
fn typo(word: &str, rng: &mut Rng) -> Option<String> {
    let mut chars: Vec<char> = word.chars().collect();
    if chars.len() < 5 {
        return None;
    }
    let at = 1 + rng.below(chars.len() - 2);
    chars.swap(at, at + 1);
    Some(chars.into_iter().collect())
}

struct Timing {
    all: Vec<f64>,
    one_letter: Vec<f64>,
    hits: usize,
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    let at = ((sorted.len() as f64 * p) as usize).min(sorted.len() - 1);
    sorted[at]
}

fn measure(index: &Index, queries: &[String], names_only: bool) -> Result<Timing, String> {
    let ask = |text: &str| {
        let mut q = Query::of_text(text, MOST);
        q.contents = !names_only;
        q
    };
    // Warm-up: the first queries of an opened reader load what the segments
    // map lazily; a door pays that once per view.
    for text in queries.iter().take(50) {
        let _ = index.query(&ask(text));
    }
    let mut timing = Timing {
        all: Vec::with_capacity(queries.len()),
        one_letter: Vec::new(),
        hits: 0,
    };
    for text in queries {
        let q = ask(text);
        let t = Instant::now();
        let hits = index
            .query(&q)
            .map_err(|r| format!("the query refused: {:?}", r.why))?;
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        timing.hits += hits.len();
        timing.all.push(ms);
        if text.chars().count() == 1 {
            timing.one_letter.push(ms);
        }
    }
    timing.all.sort_by(f64::total_cmp);
    timing.one_letter.sort_by(f64::total_cmp);
    Ok(timing)
}

fn report(label: &str, t: &Timing) -> f64 {
    let p99 = percentile(&t.all, 0.99);
    println!(
        "{label}: {} queries  p50={:.3} ms  p95={:.3} ms  p99={p99:.3} ms  max={:.3} ms  (one letter: p99={:.3} ms over {})  hits={}",
        t.all.len(),
        percentile(&t.all, 0.50),
        percentile(&t.all, 0.95),
        t.all.last().copied().unwrap_or(0.0),
        percentile(&t.one_letter, 0.99),
        t.one_letter.len(),
        t.hits,
    );
    p99
}

fn build(
    dir: &Path,
    names: usize,
    body_every: usize,
    words: &Words,
    rng: &mut Rng,
) -> Result<Vec<(Vec<String>, bool)>, String> {
    let now_s = ronoctua_index::now_s();
    let t = Instant::now();
    let mut writer = Writer::open_with_threads(dir, 4, 1 << 30)
        .map_err(|r| format!("the writer refused: {:?}", r.why))?;
    let mut picked = Vec::new();
    let mut bodies = 0usize;
    for k in 0..names {
        let with_body = body_every > 0 && k % body_every == 0;
        bodies += usize::from(with_body);
        let (document, typed) = file(rng, words, now_s, with_body);
        if k % (names / 400).max(1) == 0 {
            picked.push((typed, with_body));
        }
        writer
            .add(&document)
            .map_err(|r| format!("add refused: {:?}", r.why))?;
    }
    writer
        .commit()
        .map_err(|r| format!("commit refused: {:?}", r.why))?;
    writer
        .finish()
        .map_err(|r| format!("finish refused: {:?}", r.why))?;
    println!(
        "indexed {names} names ({bodies} with a body) in {:.1} s; the folder holds {} MiB",
        t.elapsed().as_secs_f64(),
        ronoctua_index::write::size_of_dir(dir) >> 20
    );
    Ok(picked)
}

fn run(dir: &Path, names: usize, body_every: usize) -> Result<bool, String> {
    let mut rng = Rng(0x5EED_1DEA);
    let words = Words::new(&mut rng, 30_000);
    let picked = build(dir, names, body_every, &words, &mut rng)?;

    let mut name_queries = Vec::new();
    let mut content_queries = Vec::new();
    for (typed, _) in &picked {
        name_queries.extend(keystrokes(&typed[0], typed.get(1).map(String::as_str)));
        if let Some(t) = typo(&typed[0], &mut rng) {
            name_queries.push(t);
        }
    }
    // Contents: words a person remembers from a text — a common one and a
    // rarer one — typed key by key.
    for _ in 0..picked.len() {
        let first = words.zipf(&mut rng).to_owned();
        let second = words.uniform(&mut rng).to_owned();
        content_queries.extend(keystrokes(&first, Some(&second)));
    }

    let index = Index::open_ro(dir).map_err(|r| format!("the reader refused: {:?}", r.why))?;
    println!("the reader sees {} documents", index.documents());
    let names_t = measure(&index, &name_queries, true)?;
    let both_t = measure(&index, &content_queries, false)?;
    let names_default_t = measure(&index, &name_queries, false)?;
    println!();
    let names_p99 = report("names (/f)            ", &names_t);
    let both_p99 = report("names + contents      ", &both_t);
    let default_p99 = report("names typed, default  ", &names_default_t);
    let content_p99 = both_p99.max(default_p99);
    println!();
    println!(
        "gate: names p99 {names_p99:.3} ms ≤ {NAMES_P99_MS} ms; contents p99 {content_p99:.3} ms ≤ {CONTENT_P99_MS} ms"
    );
    Ok(names_p99 <= NAMES_P99_MS && content_p99 <= CONTENT_P99_MS)
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let names = args
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1_000_000);
    let body_every = args.next().and_then(|s| s.parse().ok()).unwrap_or(10);
    let dir = args.next().map_or_else(
        || {
            ronoctua_xdg::cache_root()
                .unwrap_or_else(std::env::temp_dir)
                .join(format!("ronoctua-bench-index.{}", std::process::id()))
        },
        PathBuf::from,
    );
    if dir.exists() {
        eprintln!(
            "{} exists; the bench builds in a folder of its own",
            dir.display()
        );
        return ExitCode::FAILURE;
    }
    let result = run(&dir, names, body_every);
    let _ = std::fs::remove_dir_all(&dir);
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => {
            eprintln!("ERROR: the index missed the gate; fast-access must not be built on it.");
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("ERROR: {e}");
            ExitCode::FAILURE
        }
    }
}
