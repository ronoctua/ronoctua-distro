//! Why the FIRST shaping costs seconds, and what makes it cheap.
//!
//! # The finding that motivated this binary
//!
//! Measured on the footer on actual hardware: the first `prepare` cost 4,447 ms,
//! of which 4,447 are SHAPING and 3.7 are atlas. The following calls cost ~9 ms
//! with MORE text. The cost is all first-time, inside cosmic-text, and on its
//! own it blows section 5.2's 200 ms target twenty times over.
//!
//! # The two hypotheses, and why measure both
//!
//! 1. **The generic family.** We ask for `Family::Monospace`. cosmic-text
//!    0.19's `FontSystem::new_with_fonts` does
//!    `db.set_monospace_family("Noto Sans Mono")` — a font that is NOT in our
//!    database, because we load only our own face. The lookup fails and falls
//!    into the fallback machinery.
//! 2. **`Shaping::Advanced`.** It uses rustybuzz, which has to parse GSUB/GPOS.
//!    Iosevka has 57,289 glyphs and enormous stylistic sets; parsing that once
//!    may be the whole cost.
//!
//! Both are plausible and lead to DIFFERENT fixes. Guessing is how you find
//! out, three weeks later, that the fix fixed nothing.
//!
//! # The other half of the startup
//!
//! `wgpu::Instance::new` cost 780 measured ms, against 5.5 ms for
//! `request_adapter` and 4.4 ms for `request_device` — the cost is in
//! ENUMERATING backends, not in talking to the GPU. We ask for `VULKAN | GL`,
//! and initializing GL opens `libGL`/EGL and creates a context only to find out
//! we are not going to use it.
//!
//! Measured in a SEPARATE process (one argument per case): inside the same
//! process the second `Instance::new` finds the loaders warm and would measure
//! the second time, not the first.
//!
//! Usage:
//!   `ronoctua-bench-shaping`            the four shaping combinations
//!   `ronoctua-bench-shaping vulkan`     only `Instance::new` with Vulkan
//!   `ronoctua-bench-shaping vulkan+gl`  only `Instance::new` with Vulkan and GL

use std::time::Instant;

use glyphon::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, fontdb};
use ronoctua_render::text::EMBEDDED_FONT;
use ronoctua_tokens as tokens;

/// One arrangement to measure: label, how to build the database, and the family asked for.
type Case<'a> = (&'static str, fn() -> FontSystem, Family<'a>);

/// Text similar to what the modeline actually shows.
const SPANS: [&str; 5] = [
    "NORMAL",
    "*scratch*",
    "fundamental",
    "projeto/src/arquivo.rs",
    "12:34",
];

/// The database as cosmic-text builds it by default.
///
/// `new_with_fonts` points the three generics at fonts that are NOT in the
/// database (`Noto Sans Mono`, `Open Sans`, `DejaVu Serif`).
fn upstream_system() -> FontSystem {
    FontSystem::new_with_fonts([fontdb::Source::Binary(std::sync::Arc::new(EMBEDDED_FONT))])
}

/// The database as `Text::new` builds it: generics pointing at our face.
fn our_system() -> FontSystem {
    let mut db = fontdb::Database::new();
    db.load_font_source(fontdb::Source::Binary(std::sync::Arc::new(EMBEDDED_FONT)));
    db.set_monospace_family(tokens::font::FAMILY_MONO);
    db.set_sans_serif_family(tokens::font::FAMILY_UI);
    db.set_serif_family(tokens::font::FAMILY_UI);
    FontSystem::new_with_locale_and_db("en-GB".to_owned(), db)
}

/// Shapes the spans once, in a NEW `FontSystem`.
///
/// New on every measurement on purpose: what is measured is the FIRST
/// shaping, and reusing the system would measure the second.
fn measure(mut fs: FontSystem, family: Family<'_>, shaping: Shaping) -> f64 {
    let m = Metrics::new(tokens::font::SIZE_BASE, tokens::font::SIZE_BASE * 1.25);
    let attrs = Attrs::new().family(family);
    let t = Instant::now();
    for text in SPANS {
        let mut b = Buffer::new(&mut fs, m);
        b.set_size(Some(1920.0), Some(m.line_height));
        b.set_text(text, &attrs, shaping, None);
        b.shape_until_scroll(&mut fs, false);
        std::hint::black_box(&b);
    }
    t.elapsed().as_secs_f64() * 1000.0
}

/// Times `Instance::new` with a set of backends.
fn instance(backends: wgpu::Backends) -> f64 {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = backends;
    let t = Instant::now();
    let i = wgpu::Instance::new(desc);
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    std::hint::black_box(&i);
    ms
}

fn main() -> std::process::ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("vulkan") => {
            println!("{:.1}", instance(wgpu::Backends::VULKAN));
            return std::process::ExitCode::SUCCESS;
        }
        Some("vulkan+gl") => {
            println!(
                "{:.1}",
                instance(wgpu::Backends::VULKAN | wgpu::Backends::GL)
            );
            return std::process::ExitCode::SUCCESS;
        }
        _ => {}
    }

    // The FLOOR is ~120 ms: it is the cost of parsing Iosevka (15 MB, 57,289
    // glyphs) in order to shape for the first time, and there is no way of not
    // paying it with this font. The ceiling does not exist to tighten that — it
    // exists to catch the return of the regime of SECONDS, which is what the
    // generic family caused.
    const CEILING_MS: f64 = 300.0;

    println!(
        "primeira moldagem de {} trechos, sistema novo a cada vez\n",
        SPANS.len()
    );

    let name = Family::Name(tokens::font::FAMILY_MONO);
    let cases: [Case<'_>; 4] = [
        ("upstream + Monospace", upstream_system, Family::Monospace),
        ("upstream + nome exato", upstream_system, name),
        ("nosso    + Monospace", our_system, Family::Monospace),
        ("nosso    + nome exato  <- producao", our_system, name),
    ];

    let mut producao = f64::MAX;
    for (label, assemble, fam) in cases {
        let ms = measure(assemble(), fam, Shaping::Advanced);
        println!("  {label:36} {ms:9.1} ms");
        if label.contains("producao") {
            producao = ms;
        }
    }

    println!();
    println!("producao: {producao:.1} ms   teto {CEILING_MS:.0} ms");

    if producao > CEILING_MS {
        eprintln!(
            "\nERRO: a moldagem de producao passou de {CEILING_MS:.0} ms.\n\
             \n\
             O suspeito no 1 e a familia ter voltado a ser generica: o\n\
             cosmic-text aponta `Monospace` para `Noto Sans Mono`, que nao\n\
             esta no nosso banco, e a busca falha para dentro do fallback.\n\
             Ver `Texto::new` em crates/ronoctua-render/src/text.rs."
        );
        return std::process::ExitCode::FAILURE;
    }
    std::process::ExitCode::SUCCESS
}
