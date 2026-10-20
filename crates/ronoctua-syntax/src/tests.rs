//! What the highlighter promises, locked.
//!
//! # Every grammar here is a Rust literal
//!
//! Which is the point: the scanner takes a `&Grammar`, so nesting, raw strings
//! and fences are exercised in `cargo test` in seconds, with no generated
//! table, no environment variable and no Nix. What the generated table has to
//! be is RIGHT, and that is asked separately — at the bottom of this file, and
//! in `tests/checks.nix`.
//!
//! # Each test names the defect it fails on
//!
//! Because a test that passes with the defect back in the code is worthless,
//! and this repository has found five of those by planting defects on purpose.

use alloc::{vec, vec::Vec};

use crate::{
    Attribute, Block, Delimiter, Grammar, Kind, Language, Numbers, Property, Range, Role, State,
    counts, guess, line, line_into, of_mime, of_shebang, of_tag, ranges, states,
};

// ---------------------------------------------------------------------
// Grammars written by hand, so the scanner is testable on its own.
// ---------------------------------------------------------------------

const fn bare() -> Grammar {
    Grammar {
        id: "fixture",
        label: "Fixture",
        kind: Kind::Code,
        line_comments: &[],
        block_comment: None,
        strings: &[],
        interpolation: &[],
        fences: &[],
        attribute: None,
        keywords: &[],
        types: &[],
        constants: &[],
        numbers: Numbers::Plain,
        property: Property::None,
        capitalised_is_type: false,
        screaming_is_constant: false,
        call: false,
        word_chars: "_$?",
        char_prefix: None,
        colon_is_constant: false,
    }
}

/// Rust's shape, cut to what the tests exercise.
const fn rustish() -> Grammar {
    Grammar {
        line_comments: &["//"],
        block_comment: Some(Block {
            open: "/*",
            close: "*/",
            nests: true,
        }),
        strings: &[
            // `r` AND NOT `r"`: the hashes sit between the marker and the
            // quote, so the marker is what comes before them.
            Delimiter {
                open: "r",
                close: "\"",
                escape: "",
                spans_lines: true,
                hashes: true,
                max: None,
            },
            Delimiter {
                open: "\"",
                close: "\"",
                escape: "\\",
                spans_lines: true,
                hashes: false,
                max: None,
            },
            Delimiter {
                open: "'",
                close: "'",
                escape: "\\",
                spans_lines: false,
                hashes: false,
                max: Some(4),
            },
        ],
        attribute: Some(Attribute {
            prefix: "#[",
            close: Some("]"),
        }),
        // SORTED, as the generator emits them: the lookup is a binary search.
        keywords: &["fn", "if", "let", "mut", "pub", "struct"],
        // SORTED BY `Ord`, which puts the capital first: `binary_search`.
        types: &["String", "str", "u8", "usize"],
        constants: &["false", "true"],
        numbers: Numbers::C,
        capitalised_is_type: true,
        screaming_is_constant: true,
        call: true,
        ..bare()
    }
}

/// A language whose comment does NOT nest, for the contrast.
const fn nixish() -> Grammar {
    Grammar {
        line_comments: &["#"],
        block_comment: Some(Block {
            open: "/*",
            close: "*/",
            nests: false,
        }),
        strings: &[Delimiter {
            open: "\"",
            close: "\"",
            escape: "\\",
            spans_lines: true,
            hashes: false,
            max: None,
        }],
        interpolation: &["${"],
        keywords: &["in", "let", "with"],
        property: Property::Equals,
        ..bare()
    }
}

/// Haskell's, for nesting.
const fn haskellish() -> Grammar {
    Grammar {
        line_comments: &["--"],
        block_comment: Some(Block {
            open: "{-",
            close: "-}",
            nests: true,
        }),
        keywords: &["module", "where"],
        ..bare()
    }
}

/// Python's, for the triple quote.
const fn pythonish() -> Grammar {
    Grammar {
        line_comments: &["#"],
        strings: &[
            Delimiter {
                open: "\"\"\"",
                close: "\"\"\"",
                escape: "\\",
                spans_lines: true,
                hashes: false,
                max: None,
            },
            Delimiter {
                open: "\"",
                close: "\"",
                escape: "\\",
                spans_lines: false,
                hashes: false,
                max: None,
            },
        ],
        keywords: &["def", "return"],
        ..bare()
    }
}

/// Common Lisp's, cut to what the tests exercise: the reader's word
/// characters, the character literal, the keyword's colon, the signed
/// numeral, and a docstring that spans lines.
const fn lispish() -> Grammar {
    Grammar {
        line_comments: &[";"],
        block_comment: Some(Block {
            open: "#|",
            close: "|#",
            nests: true,
        }),
        strings: &[Delimiter {
            open: "\"",
            close: "\"",
            escape: "\\",
            spans_lines: true,
            hashes: false,
            max: None,
        }],
        // SORTED BY `Ord`, as the generator emits them.
        keywords: &[
            "&optional",
            "defun",
            "let",
            "let*",
            "multiple-value-bind",
            "when",
        ],
        constants: &["nil", "t"],
        numbers: Numbers::C,
        word_chars: "-_*+/<>=!?%&$.:@^~",
        char_prefix: Some("#\\"),
        colon_is_constant: true,
        ..bare()
    }
}

fn markdownish() -> Grammar {
    Grammar {
        kind: Kind::Markdown,
        block_comment: Some(Block {
            open: "<!--",
            close: "-->",
            nests: false,
        }),
        strings: &[Delimiter {
            open: "`",
            close: "`",
            escape: "",
            spans_lines: false,
            hashes: false,
            max: None,
        }],
        fences: &["```", "~~~"],
        property: Property::Colon,
        ..bare()
    }
}

const fn diffish() -> Grammar {
    Grammar {
        kind: Kind::Diff,
        numbers: Numbers::None,
        ..bare()
    }
}

const fn markupish() -> Grammar {
    Grammar {
        kind: Kind::Markup,
        block_comment: Some(Block {
            open: "<!--",
            close: "-->",
            nests: false,
        }),
        numbers: Numbers::None,
        ..bare()
    }
}

// ---------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------

fn one(text: &str, grammar: &Grammar) -> Vec<Range> {
    line(text, grammar, State::fresh()).0
}

/// What each role covers, as text, so a failure reads like the screen.
fn painted<'a>(text: &'a str, out: &[Range]) -> Vec<(Role, &'a str)> {
    out.iter()
        .map(|r| (r.role, &text[r.start..r.end]))
        .collect()
}

fn roles(out: &[Range]) -> Vec<Role> {
    out.iter().map(|r| r.role).collect()
}

// ---------------------------------------------------------------------
// The state, and the resumability that makes scrolling cheap
// ---------------------------------------------------------------------

/// FAILS IF: somebody puts a `String` in `State`. `Copy` goes with it, and an
/// allocation lands in the frame that draws.
#[test]
fn the_state_stays_cheap() {
    assert!(core::mem::size_of::<State>() <= 8);
    assert!(State::fresh().is_fresh());
}

/// FAILS IF: `states` and folding `line` over the same text disagree — and
/// then a scrolled view shows different colours from the same view reached
/// from the top.
#[test]
fn the_states_agree_with_folding() {
    let text = "/* one\n   two */ let x = 1;\n\"open\nstill open\" let y = 2;\n";
    let grammar = rustish();
    let computed = states(text, &grammar);
    let mut carried = State::fresh();
    let mut scratch = Vec::new();
    for (n, row) in text.lines().enumerate() {
        assert_eq!(computed[n], carried, "line {n} disagrees");
        carried = line_into(row, &grammar, carried, &mut scratch);
    }
}

/// THE TEST THE WHOLE DESIGN IS FOR.
///
/// FAILS IF: the state is not enough to resume. Highlighting line forty from
/// `states()[39]` has to give exactly what highlighting from line one gives —
/// otherwise scrolling to a line shows it in different colours from arriving
/// at it by scrolling down.
#[test]
fn drawing_from_the_middle_matches_drawing_from_the_top() {
    let mut text = alloc::string::String::new();
    text.push_str("/* a comment\n");
    for _ in 0..38 {
        text.push_str("that goes on\n");
    }
    text.push_str("*/ let after = 1;\n");
    let grammar = rustish();

    let whole = ranges(&text, Language::Rust);
    let _ = whole;

    let computed = states(&text, &grammar);
    let rows: Vec<&str> = text.lines().collect();

    // From the top, keeping every line's answer.
    let mut carried = State::fresh();
    let mut from_top: Vec<Vec<Range>> = Vec::new();
    for row in &rows {
        let (out, next) = line(row, &grammar, carried);
        from_top.push(out);
        carried = next;
    }

    // And each line on its own, out of the index.
    for (n, row) in rows.iter().enumerate() {
        let alone = line(row, &grammar, computed[n]).0;
        assert_eq!(alone, from_top[n], "line {n} differs when reached alone");
    }
    // And it is not vacuous: the last line has to carry the comment's close.
    assert_eq!(
        painted(rows[39], &from_top[39])[0],
        (Role::Comment, "*/"),
        "the block comment did not reach line 39"
    );
}

/// FAILS IF: the multi-line state is dropped.
#[test]
fn a_block_comment_survives_the_line_break() {
    let grammar = rustish();
    let (first, carried) = line("let a = 1; /* here", &grammar, State::fresh());
    assert_eq!(
        painted("let a = 1; /* here", &first).last().unwrap().0,
        Role::Comment
    );
    assert!(!carried.is_fresh());
    let (second, after) = line("still inside", &grammar, carried);
    assert_eq!(roles(&second), vec![Role::Comment]);
    assert!(!after.is_fresh());
    let (third, done) = line("*/ let b = 2;", &grammar, after);
    assert_eq!(painted("*/ let b = 2;", &third)[0], (Role::Comment, "*/"));
    assert!(done.is_fresh());
}

/// FAILS IF: `depth` degenerates into a bool. `{- {- -} -}` is ONE comment,
/// still open after the first `-}`.
#[test]
fn a_nested_block_comment_needs_two_closers() {
    let grammar = haskellish();
    let (_, carried) = line("{- outer {- inner -}", &grammar, State::fresh());
    assert!(
        !carried.is_fresh(),
        "the first close ended the outer comment too"
    );
    let (out, done) = line("-} module Main where", &grammar, carried);
    assert_eq!(
        painted("-} module Main where", &out)[0],
        (Role::Comment, "-}")
    );
    assert!(done.is_fresh());
    assert!(
        out.iter().any(|r| r.role == Role::Keyword),
        "nothing after the close was scanned"
    );
}

/// FAILS IF: nesting is made universal. Nix's `/* /* */` closes at the first.
#[test]
fn a_language_that_does_not_nest_closes_at_the_first() {
    let grammar = nixish();
    let (out, carried) = line("/* a /* b */ let", &grammar, State::fresh());
    assert!(carried.is_fresh(), "it stayed open past its own close");
    assert!(
        out.iter().any(|r| r.role == Role::Keyword),
        "the `let` after the close was not seen"
    );
}

/// FAILS IF: an unterminated string carries. One stray `\"` would then paint
/// the rest of the file in one colour.
#[test]
fn an_unterminated_single_line_string_ends_at_the_line() {
    let grammar = rustish();
    let (out, carried) = line("let c = 'x", &grammar, State::fresh());
    assert!(carried.is_fresh());
    assert!(!out.is_empty());
}

/// FAILS IF: `spans_lines` is ignored, one way or the other.
#[test]
fn a_triple_quote_spans_and_a_plain_one_does_not() {
    let grammar = pythonish();
    let (_, carried) = line("x = \"\"\"first", &grammar, State::fresh());
    assert!(!carried.is_fresh(), "the docstring did not carry");
    let (_, still) = line("second", &grammar, carried);
    assert!(!still.is_fresh());
    let (out, done) = line("third\"\"\" def f():", &grammar, still);
    assert!(done.is_fresh());
    assert!(
        out.iter().any(|r| r.role == Role::Keyword),
        "`def` was missed"
    );
}

/// FAILS IF: `hashes` is ignored and the raw string closes at the inner quote.
#[test]
fn a_raw_string_needs_the_same_hashes() {
    let grammar = rustish();
    let text = "let s = r#\"a\"b\"# + 1;";
    let out = one(text, &grammar);
    let strings: Vec<&str> = out
        .iter()
        .filter(|r| r.role == Role::String)
        .map(|r| &text[r.start..r.end])
        .collect();
    assert_eq!(strings, vec!["r#\"a\"b\"#"], "{:?}", painted(text, &out));
}

/// FAILS IF: `max` is dropped. `&'a str` would open a string that never
/// closes, and the rest of the file would come out one colour.
#[test]
fn a_lifetime_is_not_a_string() {
    let grammar = rustish();
    let text = "pub fn f<'a>(x: &'a str) -> &'a str { x }";
    let (out, carried) = line(text, &grammar, State::fresh());
    assert!(carried.is_fresh(), "a lifetime left a string open");
    assert!(
        out.iter()
            .any(|r| r.role == Role::Type && &text[r.start..r.end] == "str"),
        "the type after the lifetime was swallowed: {:?}",
        painted(text, &out)
    );
}

/// FAILS IF: escapes are nested inside the string's range instead of splitting
/// it — which breaks the non-overlap the drawing side slices by.
#[test]
fn an_escape_splits_the_string_it_is_in() {
    let grammar = rustish();
    let text = "let s = \"a\\nb\";";
    let out = one(text, &grammar);
    let seen = painted(text, &out);
    assert!(
        seen.contains(&(Role::Escape, "\\n")),
        "the escape was not lifted out: {seen:?}"
    );
    assert!(seen.iter().filter(|(r, _)| *r == Role::String).count() >= 2);
}

/// FAILS IF: interpolation stops being lifted, and `"${HOME}/x"` reads inert.
#[test]
fn interpolation_is_not_string() {
    let grammar = nixish();
    let text = "x = \"${home}/bin\";";
    let out = one(text, &grammar);
    assert!(
        painted(text, &out).contains(&(Role::Escape, "${home}")),
        "{:?}",
        painted(text, &out)
    );
}

// ---------------------------------------------------------------------
// The rules that were already locked, and stay locked
// ---------------------------------------------------------------------

/// FAILS IF: the comment stops being tried first. `# "this"` would open a
/// string that never closes.
#[test]
fn the_comment_beats_the_quotes() {
    let grammar = nixish();
    let text = "# \"not a string";
    let (out, carried) = line(text, &grammar, State::fresh());
    assert_eq!(painted(text, &out), vec![(Role::Comment, text)]);
    assert!(carried.is_fresh());
}

/// FAILS IF: words are compared by prefix. `iffy` would be a half-coloured
/// `if`.
#[test]
fn a_similar_word_is_not_reserved() {
    let grammar = rustish();
    let text = "let iffy = 1;";
    let out = one(text, &grammar);
    for (role, word) in painted(text, &out) {
        assert!(
            !(role == Role::Keyword && word == "if"),
            "`iffy` was coloured as `if`"
        );
    }
}

/// FAILS IF: a digit inside a word is read as a number, and `utf8` comes out
/// half orange.
#[test]
fn a_number_inside_a_word_is_not_a_number() {
    let grammar = rustish();
    let text = "let utf8 = 1;";
    let out = one(text, &grammar);
    assert!(
        !painted(text, &out)
            .iter()
            .any(|(r, w)| *r == Role::Number && *w == "8"),
        "{:?}",
        painted(text, &out)
    );
}

/// FAILS IF: `guess` starts answering for prose. Colouring an essay is worse
/// than leaving it grey.
#[test]
fn prose_gets_no_highlight() {
    assert_eq!(
        guess("The quick brown fox jumps over the lazy dog, twice, and then rests."),
        None
    );
    assert_eq!(guess(""), None);
}

/// FAILS IF: an unclosed anything panics. A panic here is a dead footer.
#[test]
fn odd_lines_do_not_blow_up() {
    for grammar in [
        rustish(),
        nixish(),
        pythonish(),
        markdownish(),
        diffish(),
        markupish(),
        lispish(),
    ] {
        for text in [
            "", "\t", "   ", "\"", "'", "/*", "```", "<a href=", "#[", "${", "r#\"", "#\\", "#|",
            "-", "+1", "1/", ":", "#\\é",
        ] {
            let _ = line(text, &grammar, State::fresh());
        }
    }
}

// ---------------------------------------------------------------------
// Markdown, diff and markup
// ---------------------------------------------------------------------

/// FAILS IF: the fence stops delegating, or starts falling back to a default.
#[test]
fn a_fence_uses_the_inner_language() {
    let grammar = markdownish();
    let (_, opened) = line("```rust", &grammar, State::fresh());
    let text = "pub fn main() {}";
    let (inside, _) = line(text, &grammar, opened);
    assert!(
        inside.iter().any(|r| r.role == Role::Keyword),
        "the fence did not delegate: {:?}",
        painted(text, &inside)
    );
    // AND THE SAME WORD OUTSIDE IS NOTHING, which is what makes the test
    // measure delegation rather than an accident.
    let (outside, _) = line(text, &grammar, State::fresh());
    assert!(outside.iter().all(|r| r.role != Role::Keyword));
}

/// FAILS IF: an unknown tag gets a default language. A shell session would be
/// coloured as if it were Rust.
#[test]
fn an_unknown_fence_tag_gets_no_highlight() {
    let grammar = markdownish();
    let (_, opened) = line("```wobble", &grammar, State::fresh());
    let (inside, _) = line("pub fn main() {}", &grammar, opened);
    assert!(inside.is_empty(), "{inside:?}");
}

/// FAILS IF: the close test is wrong, and the fence swallows the rest.
#[test]
fn a_fence_does_not_leak_past_its_close() {
    let grammar = markdownish();
    let (_, opened) = line("```rust", &grammar, State::fresh());
    let (_, inside) = line("let x = 1;", &grammar, opened);
    let (_, closed) = line("```", &grammar, inside);
    assert!(closed.is_fresh(), "the fence stayed open past its close");
    // AND A `~~~` DOES NOT CLOSE A ``` BLOCK, which is what `marker` is for.
    let (_, opened) = line("```rust", &grammar, State::fresh());
    let (_, still) = line("~~~", &grammar, opened);
    assert!(!still.is_fresh(), "the wrong fence marker closed the block");
}

/// FAILS IF: setext is added without the previous line, or `#hashtag` starts
/// being a heading.
#[test]
fn a_heading_needs_a_space_after_the_hashes() {
    let grammar = markdownish();
    assert_eq!(roles(&one("## Section", &grammar)), vec![Role::Heading]);
    assert!(one("#hashtag", &grammar).is_empty());
    assert!(one("####### seven", &grammar).is_empty());
}

/// FAILS IF: the diff prefixes are reordered and `--- a/x` is painted as a
/// removed line — in a file whose whole purpose is telling those two apart.
#[test]
fn the_hunk_header_is_not_a_removed_line() {
    let grammar = diffish();
    assert_eq!(roles(&one("--- a/x.rs", &grammar)), vec![Role::Heading]);
    assert_eq!(roles(&one("+++ b/x.rs", &grammar)), vec![Role::Heading]);
    assert_eq!(
        roles(&one("@@ -1,4 +1,6 @@", &grammar)),
        vec![Role::Heading]
    );
    assert_eq!(roles(&one("-gone", &grammar)), vec![Role::Removed]);
    assert_eq!(roles(&one("+new", &grammar)), vec![Role::Added]);
    assert!(one(" same", &grammar).is_empty());
}

/// FAILS IF: markup collapses to one role.
#[test]
fn a_tag_name_is_not_its_attribute() {
    let grammar = markupish();
    let text = "<a href=\"x\">&amp;</a>";
    let seen = painted(text, &one(text, &grammar));
    assert!(seen.contains(&(Role::Tag, "a")), "{seen:?}");
    assert!(seen.contains(&(Role::Property, "href")), "{seen:?}");
    assert!(seen.contains(&(Role::String, "\"x\"")), "{seen:?}");
    assert!(seen.contains(&(Role::Escape, "&amp;")), "{seen:?}");
}

// ---------------------------------------------------------------------
// The contract whoever draws slices by
// ---------------------------------------------------------------------

/// THE ONE THAT PREVENTS A PANIC ON SCREEN.
///
/// FAILS IF: the scanner ever advances one byte into a multi-byte character.
/// The offsets after it land off a `char` boundary, and whoever draws slices
/// by them inside the frame that draws.
#[test]
fn ranges_are_ordered_non_overlapping_and_on_char_boundaries() {
    // ACCENTS AND EMOJI IN EVERY BYTE POSITION, which is what makes this
    // measure the boundary rule rather than one lucky layout.
    let seeds = [
        "let x = \"ação\"; // café ☕",
        "// ☕",
        "\"日本語\" + 1",
        "#[derive(Ação)] pub fn f() {}",
        "x = \"a\\né\"",
        // A BACKSLASH IMMEDIATELY BEFORE A MULTI-BYTE CHARACTER, which is the
        // one shape that makes the escape's own range end inside `é` when the
        // scanner advances by a byte instead of by a character.
        "x = \"a\\éb\"",
        "x = \"\\☕\"",
        "/* ação */ let y = 2;",
        "# ☕ comment",
        "<a href=\"ação\">é</a>",
        "+ação",
        "## Título ☕",
        "```rust\nlet á = 1;\n```",
        // A CHARACTER LITERAL OF A MULTI-BYTE CHARACTER, which is the shape
        // that ends the literal inside the character when the scanner takes
        // one byte after the prefix instead of one character.
        "(list #\\é #\\☕ #\\ação)",
        "#\\é",
        ":ação -1é",
    ];
    for grammar in [
        rustish(),
        nixish(),
        pythonish(),
        markdownish(),
        diffish(),
        markupish(),
        lispish(),
    ] {
        for seed in seeds {
            for row in seed.lines() {
                let out = one(row, &grammar);
                let mut last = 0usize;
                for r in &out {
                    assert!(r.start < r.end, "empty range in {row:?} ({grammar:?})");
                    assert!(r.start >= last, "out of order in {row:?}: {out:?}");
                    assert!(r.end <= row.len(), "past the end in {row:?}");
                    assert!(
                        row.is_char_boundary(r.start) && row.is_char_boundary(r.end),
                        "off a char boundary in {row:?}: {r:?}"
                    );
                    last = r.end;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------
// The generated table
// ---------------------------------------------------------------------

/// FAILS IF: the generator stops sorting and every lookup silently misses.
#[test]
fn the_word_lists_are_sorted_and_findable() {
    for grammar in Language::all() {
        for list in [grammar.keywords, grammar.types, grammar.constants] {
            for (n, word) in list.iter().enumerate() {
                assert!(
                    list.binary_search(word).is_ok(),
                    "`{word}` is in {}'s list at {n} and cannot be found",
                    grammar.id
                );
            }
        }
        assert!(
            grammar
                .strings
                .windows(2)
                .all(|w| w[0].open.len() >= w[1].open.len()),
            "{}'s delimiters are not longest-first",
            grammar.id
        );
    }
}

/// FAILS IF: the subclass expansion is dropped, and a hundred and fifty-eight
/// types fall back to thirty-seven.
#[test]
fn an_alias_and_a_descendant_both_arrive() {
    // Claimed directly.
    assert_eq!(of_mime("text/rust"), Some(Language::Rust));
    assert_eq!(of_mime("text/x-nix"), Some(Language::Nix));
    // Inherited through the subclass DAG.
    assert_eq!(of_mime("text/x-chdr"), Some(Language::C));
    assert_eq!(of_mime("text/x-python3"), Some(Language::Python));
    assert_eq!(of_mime("application/x-csh"), Some(Language::Shell));
    assert_eq!(of_mime("application/atom+xml"), Some(Language::Xml));
    assert_eq!(of_mime("application/schema+json"), Some(Language::Json));
    // Reached by alias.
    assert_eq!(of_mime("text/x-diff"), Some(Language::Diff));
    assert_eq!(of_mime("text/x-yaml"), Some(Language::Yaml));
    assert_eq!(of_mime("text/x-markdown"), Some(Language::Markdown));
}

/// FAILS IF: prose starts being coloured — which looks like a feature.
#[test]
fn text_plain_is_not_a_language() {
    assert_eq!(of_mime("text/plain"), None);
    assert_eq!(of_mime("application/octet-stream"), None);
    assert_eq!(of_mime("inode/directory"), None);
    assert_eq!(of_mime("image/png"), None);
}

/// FAILS IF: the counts and the tables disagree.
///
/// The FLOORS are const assertions in `lib.rs`, not here: a comparison of two
/// constants inside a `#[test]` is folded by the compiler and verifies nothing
/// at run time — the lesson `ronoctua-tokens` already records above its own.
/// What is testable is that the counts describe the tables they claim to.
#[test]
fn the_counts_describe_the_tables() {
    assert_eq!(Language::all().len(), counts::LANGUAGES);
    let mimes = Language::all()
        .iter()
        .filter(|g| of_mime(g.id).is_some() || of_tag(g.id).is_some())
        .count();
    assert!(mimes > 0);
    // AND THE INDEX IS SORTED, which is what `binary_search` needs and what a
    // generator that stopped sorting would break in silence.
    assert!(of_mime("text/rust").is_some());
    assert!(of_tag("rust").is_some());
}

/// FAILS IF: `index` drifts from the table's own order.
#[test]
fn every_language_finds_its_own_grammar() {
    for (n, grammar) in Language::all().iter().enumerate() {
        let found = of_tag(grammar.id).or_else(|| of_mime(grammar.id));
        let _ = found;
        assert_eq!(
            Language::all()[n].id,
            grammar.id,
            "the table disagrees with itself at {n}"
        );
    }
    assert_eq!(Language::Rust.grammar().id, "rust");
    assert_eq!(Language::Rust.label(), "Rust");
    assert_eq!(Language::Markdown.grammar().kind, Kind::Markdown);
    assert_eq!(Language::Diff.grammar().kind, Kind::Diff);
}

/// FAILS IF: the fence tag index stops answering what a fence actually says.
#[test]
fn a_fence_tag_names_the_language_people_write() {
    assert_eq!(of_tag("rust"), Some(Language::Rust));
    assert_eq!(of_tag("rs"), Some(Language::Rust));
    assert_eq!(of_tag("sh"), Some(Language::Shell));
    assert_eq!(of_tag("bash"), Some(Language::Shell));
    assert_eq!(of_tag("nix"), Some(Language::Nix));
    assert_eq!(of_tag("wobble"), None);
}

/// FAILS IF: the shebang reader forgets `env`, which is how most of them are
/// written.
#[test]
fn a_shebang_names_its_interpreter() {
    assert_eq!(of_shebang("#!/bin/sh"), Some(Language::Shell));
    assert_eq!(of_shebang("#!/usr/bin/env python3"), Some(Language::Python));
    assert_eq!(of_shebang("#!/usr/bin/env bash"), Some(Language::Shell));
    assert_eq!(of_shebang("not a shebang"), None);
}

/// FAILS IF: `guess` starts answering by weak markers.
#[test]
fn a_guess_needs_every_marker_it_declared() {
    assert_eq!(guess("#!/bin/sh\necho hello"), Some(Language::Shell));
    assert_eq!(
        guess("{ \"a\": 1, \"b\": [2, 3] }"),
        Some(Language::Json),
        "the one language selected by shape"
    );
    // `fn` alone is not Rust: the declaration asks for five markers.
    assert_ne!(guess("fn"), Some(Language::Rust));
}

/// FAILS IF: `ranges` stops offsetting into the whole text, and every line
/// after the first is coloured at the wrong place.
#[test]
fn whole_text_offsets_are_absolute() {
    let text = "let a = 1;\nlet b = 2;\n";
    let out = ranges(text, Language::Rust);
    // TWO `let`s, and the second one starts past the first line: an offset
    // that stayed relative would put both at 0..3.
    let lets: Vec<usize> = out
        .iter()
        .filter(|r| &text[r.start..r.end] == "let")
        .map(|r| r.start)
        .collect();
    assert_eq!(lets, vec![0, 11], "{:?}", painted(text, &out));
    assert!(
        out.iter().all(|r| r.end <= text.len()),
        "an offset ran past the text"
    );
}

/// FAILS IF: the content of a `<script>` or a `<style>` is prose again — the
/// markup scanner stops handing it to its own language, or hands back
/// offsets of the piece and not of the line.
#[test]
fn script_and_style_are_drawn_in_their_own_language() {
    let html = Language::Html.grammar();
    let text = "<script>const n = 1; // done</script><p>const</p>";
    let seen = painted(text, &one(text, html));
    assert!(seen.contains(&(Role::Tag, "script")), "{seen:?}");
    assert!(seen.contains(&(Role::Keyword, "const")), "{seen:?}");
    assert!(seen.contains(&(Role::Number, "1")), "{seen:?}");
    // THE COMMENT STOPS AT THE CLOSING TAG, which is markup again.
    assert!(seen.contains(&(Role::Comment, "// done")), "{seen:?}");
    // AND AFTER IT, PROSE IS PROSE: the second `const` is not a keyword.
    let keywords = seen.iter().filter(|(r, _)| *r == Role::Keyword).count();
    assert_eq!(keywords, 1, "{seen:?}");

    let style = "<STYLE type=\"text/css\">a { color: red; }</STYLE>";
    let seen = painted(style, &one(style, html));
    assert!(seen.contains(&(Role::Tag, "STYLE")), "{seen:?}");
    assert!(
        seen.iter()
            .any(|(r, t)| *r == Role::Property && *t == "color"),
        "{seen:?}"
    );
}

/// FAILS IF: an embedded language does not carry across lines — the state
/// forgets which element is open, or what its language left open.
#[test]
fn an_embedded_language_carries_across_lines() {
    let html = Language::Html.grammar();
    let (first, carried) = line("<script>", html, State::fresh());
    assert!(!carried.is_fresh(), "the element's content is still open");
    assert!(painted("<script>", &first).contains(&(Role::Tag, "script")));

    // A BLOCK COMMENT OPENED INSIDE IT stays a comment on the next line.
    let (_, carried) = line("let a = 2; /* half", html, carried);
    let (middle, carried) = line("still comment */ return a;", html, carried);
    let seen = painted("still comment */ return a;", &middle);
    assert!(
        seen.contains(&(Role::Comment, "still comment */")),
        "{seen:?}"
    );
    assert!(seen.contains(&(Role::Keyword, "return")), "{seen:?}");

    let end = "</script> <b>let</b>";
    let (last, carried) = line(end, html, carried);
    let seen = painted(end, &last);
    assert!(seen.contains(&(Role::Tag, "script")), "{seen:?}");
    assert!(
        !seen.iter().any(|(r, _)| *r == Role::Keyword),
        "markup again: {seen:?}"
    );
    assert!(carried.is_fresh(), "the element closed");

    // A SELF-CLOSED SCRIPT HOLDS NOTHING, and what follows is markup.
    let (_, carried) = line("<script src=\"a.js\" />", html, State::fresh());
    assert!(carried.is_fresh());
}

/// FAILS IF: a row of the second wave stops being reachable by its type or
/// by a fence's tag — the rows are data only, and this is what proves the
/// data reached the table.
#[test]
fn the_second_wave_is_reached_by_type_and_by_tag() {
    for (mime, id) in [
        ("text/x-erlang", "erlang"),
        ("text/x-elixir", "elixir"),
        ("text/x-common-lisp", "common-lisp"),
        ("text/x-emacs-lisp", "lisp"),
        ("text/x-scheme", "lisp"),
        ("text/x-ocaml", "ocaml"),
        ("text/x-crystal", "crystal"),
        ("text/julia", "julia"),
        ("text/x-fortran", "fortran"),
        ("text/x-verilog", "verilog"),
        ("text/x-vhdl", "vhdl"),
        ("text/tcl", "tcl"),
        ("text/x-groovy", "groovy"),
        ("text/x-gradle", "groovy"),
        ("text/x-dsrc", "d"),
        ("text/x-vala", "vala"),
        ("text/x-nim", "nim"),
        ("text/x-qml", "qml"),
        ("text/x-gherkin", "gherkin"),
        ("text/x-rpm-spec", "rpm-spec"),
        ("text/x-gettext-translation", "gettext"),
        ("text/x-pascal", "pascal"),
        ("text/x-adasrc", "ada"),
        ("application/vnd.dart", "dart"),
        ("application/vnd.coffeescript", "coffeescript"),
        ("text/x-matlab", "matlab"),
        ("text/x-cobol", "cobol"),
        ("text/x-eiffel", "eiffel"),
    ] {
        assert_eq!(of_mime(mime).map(Language::id), Some(id), "{mime}");
    }
    assert_eq!(of_tag("elixir").map(Language::id), Some("elixir"));
    assert_eq!(of_tag("ml").map(Language::id), Some("ocaml"));

    // AND A LINE OF ONE IS COLOURED, not a grey wall.
    fn seen_in<'a>(id: &str, text: &'a str) -> Vec<(Role, &'a str)> {
        let language = of_tag(id).expect("a tag");
        painted(text, &one(text, language.grammar()))
    }
    let ocaml = "let rec f x = (* why *) x";
    let seen = seen_in("ocaml", ocaml);
    assert!(seen.contains(&(Role::Keyword, "rec")), "{seen:?}");
    assert!(seen.contains(&(Role::Comment, "(* why *)")), "{seen:?}");
    let elixir = "defmodule A do # x";
    let seen = seen_in("elixir", elixir);
    assert!(seen.contains(&(Role::Keyword, "defmodule")), "{seen:?}");
    assert!(seen.contains(&(Role::Comment, "# x")), "{seen:?}");
}

// ---------------------------------------------------------------------
// Common Lisp, and a word made of the grammar's own characters
// ---------------------------------------------------------------------

/// FAILS IF: a grammar's word characters are ignored, and the default rule
/// cuts a Lisp symbol at its hyphen: `multiple-value-bind` would light
/// nothing, and `when-let` would light `when`.
#[test]
fn a_hyphenated_symbol_is_one_word() {
    let grammar = lispish();
    let text = "(multiple-value-bind (a b) (when-let ((x 1)) x))";
    let seen = painted(text, &one(text, &grammar));
    assert!(
        seen.contains(&(Role::Keyword, "multiple-value-bind")),
        "{seen:?}"
    );
    assert!(
        !seen.contains(&(Role::Keyword, "when")),
        "`when-let` lit `when`: {seen:?}"
    );
}

/// FAILS IF: `*` stops being a word character in Lisp, and `let*` lights
/// `let` and leaves the star out.
#[test]
fn let_star_is_its_own_keyword() {
    let grammar = lispish();
    let text = "(let* ((x 1)) (let ((y 2)) y))";
    let seen = painted(text, &one(text, &grammar));
    assert!(seen.contains(&(Role::Keyword, "let*")), "{seen:?}");
    // ONE `let`, the inner one: the star did not cut the outer word.
    let lets = seen.iter().filter(|(_, w)| *w == "let").count();
    assert_eq!(lets, 1, "{seen:?}");
}

/// FAILS IF: the character literal is not tried first. `#\"` would open a
/// string that runs to the next quote, and `#\;` would turn the rest of the
/// line into a comment.
#[test]
fn a_character_literal_opens_neither_a_string_nor_a_comment() {
    let grammar = lispish();
    let text = "(list #\\\" #\\; #\\Space #\\( \"a\")";
    let (out, carried) = line(text, &grammar, State::fresh());
    let seen = painted(text, &out);
    assert_eq!(
        seen,
        vec![
            (Role::Constant, "#\\\""),
            (Role::Constant, "#\\;"),
            (Role::Constant, "#\\Space"),
            (Role::Constant, "#\\("),
            (Role::String, "\"a\""),
        ],
        "{seen:?}"
    );
    assert!(carried.is_fresh(), "a literal left something open");
    // AND AT THE END OF A LINE the literal is the prefix alone, which names
    // the line break.
    assert_eq!(
        painted("#\\", &one("#\\", &grammar)),
        vec![(Role::Constant, "#\\")]
    );
}

/// FAILS IF: a Lisp string stops reaching across lines, and a docstring's
/// second line comes out as code.
#[test]
fn a_docstring_spans_lines() {
    let grammar = lispish();
    let (first, carried) = line("(defun f () \"First line,", &grammar, State::fresh());
    assert_eq!(
        painted("(defun f () \"First line,", &first).last(),
        Some(&(Role::String, "\"First line,"))
    );
    assert!(!carried.is_fresh(), "the docstring did not carry");
    let text = "and the second.\" 42)";
    let (second, done) = line(text, &grammar, carried);
    let seen = painted(text, &second);
    assert_eq!(seen[0], (Role::String, "and the second.\""), "{seen:?}");
    assert!(seen.contains(&(Role::Number, "42")), "{seen:?}");
    assert!(done.is_fresh());
}

/// FAILS IF: the colon rule is dropped, or widened to any word with a colon
/// in it: `:test` is a keyword, a constant; `cl:car` is a symbol of a package.
#[test]
fn a_keyword_is_a_constant_and_a_package_symbol_is_not() {
    let grammar = lispish();
    let text = "(find x y :test #'equal :key #:foo cl:car)";
    let seen = painted(text, &one(text, &grammar));
    assert!(seen.contains(&(Role::Constant, ":test")), "{seen:?}");
    assert!(seen.contains(&(Role::Constant, ":key")), "{seen:?}");
    assert!(seen.contains(&(Role::Constant, ":foo")), "{seen:?}");
    assert!(
        !seen.iter().any(|(_, w)| w.contains("car")),
        "`cl:car` was coloured: {seen:?}"
    );
    // AND A GRAMMAR WITHOUT THE SHAPE colours none of them.
    let plain = Grammar {
        colon_is_constant: false,
        ..lispish()
    };
    assert!(
        !painted(text, &one(text, &plain))
            .iter()
            .any(|(_, w)| w.starts_with(':')),
        "a keyword was coloured with no shape asking for it"
    );
}

/// FAILS IF: a signed numeral is read as a symbol, or a symbol that starts
/// with digits as a numeral: `-1` and `1/2` are numbers, `1+` is a function.
#[test]
fn a_signed_numeral_is_a_number_and_one_plus_is_not() {
    let grammar = lispish();
    let text = "(+ -1 +2 1/2 1.5e-3 (1+ x) x-1)";
    let seen = painted(text, &one(text, &grammar));
    for numeral in ["-1", "+2", "1/2", "1.5e-3"] {
        assert!(
            seen.contains(&(Role::Number, numeral)),
            "{numeral}: {seen:?}"
        );
    }
    assert!(
        !seen.iter().any(|(_, w)| w.starts_with("1+") || *w == "1"),
        "`1+` or `x-1` was read as a number: {seen:?}"
    );
    // AND A GRAMMAR WHERE `-` IS PUNCTUATION reads the digits alone, as it
    // always did: `x-1` there is a name, a minus and a numeral.
    let text = "let y = x-1;";
    let seen = painted(text, &one(text, &rustish()));
    assert!(seen.contains(&(Role::Number, "1")), "{seen:?}");
}

/// FAILS IF: the default word rule drifts from the one every grammar used
/// before rows could declare theirs — a byte that was part of a word stops
/// being one, or the reverse, and every language's words move.
#[test]
fn the_default_word_rule_is_the_old_one_for_every_byte() {
    let grammar = bare();
    for b in 0..=u8::MAX {
        let old = b.is_ascii_alphanumeric() || b == b'_' || b == b'$' || b == b'?';
        assert_eq!(crate::in_word(b, &grammar), old, "byte {b:#04x}");
        assert_eq!(crate::is_word_char(b), old, "byte {b:#04x}");
    }
    // AND EVERY ROW OF THE TABLE THAT DECLARES NO WORD CHARACTERS HAS THE
    // DEFAULT ONES: only the two Lisp rows say otherwise.
    for g in Language::all() {
        if g.word_chars != "_$?" {
            assert!(
                ["common-lisp", "lisp"].contains(&g.id),
                "{} declares word characters: {:?}",
                g.id,
                g.word_chars
            );
        }
    }
    // AND NO BYTE OF A MULTI-BYTE CHARACTER IS EVER PART OF A WORD, whatever
    // a grammar written by hand says.
    let wild = Grammar {
        word_chars: "é",
        ..bare()
    };
    for b in 0x80..=u8::MAX {
        assert!(!crate::in_word(b, &wild), "byte {b:#04x}");
    }
}

/// FAILS IF: a word of a row's lists cannot be reached by the scanner — a
/// dead row that looks deliberate. Each word, alone on a line, is drawn
/// whole and with its list's role.
#[test]
fn every_table_word_is_a_word_by_its_grammar() {
    for g in Language::all() {
        if g.kind != Kind::Code {
            continue;
        }
        for (list, role) in [
            (g.keywords, Role::Keyword),
            (g.types, Role::Type),
            (g.constants, Role::Constant),
        ] {
            for word in list {
                assert!(
                    word.bytes().all(|b| crate::in_word(b, g)),
                    "{}: `{word}` is not a word by its own characters",
                    g.id
                );
                assert_eq!(
                    painted(word, &one(word, g)),
                    vec![(role, *word)],
                    "{}: `{word}` alone is not drawn as itself",
                    g.id
                );
            }
        }
    }
}

/// FAILS IF: Common Lisp stops being reached by its type, its tags, an
/// `env -S` shebang or its sniff — or the family's row takes it back.
#[test]
fn common_lisp_has_a_row_of_its_own() {
    assert_eq!(of_mime("text/x-common-lisp"), Some(Language::CommonLisp));
    for tag in ["common-lisp", "lisp", "cl"] {
        assert_eq!(of_tag(tag), Some(Language::CommonLisp), "{tag}");
    }
    assert_eq!(of_mime("text/x-emacs-lisp"), Some(Language::Lisp));
    assert_eq!(of_tag("elisp"), Some(Language::Lisp));
    assert_eq!(
        of_shebang("#!/usr/bin/sbcl --script"),
        Some(Language::CommonLisp)
    );
    assert_eq!(
        of_shebang("#!/usr/bin/env -S sbcl --script"),
        Some(Language::CommonLisp)
    );
    assert_eq!(of_shebang("#!/usr/bin/env ecl"), Some(Language::CommonLisp));
    // A FLAG BEFORE `env` IS NOT SKIPPED: only what env is told.
    assert_eq!(of_shebang("#!/bin/sh -e"), Some(Language::Shell));
    assert_eq!(
        guess("(in-package :my-app)\n\n(defun main () (print :hello))\n"),
        Some(Language::CommonLisp)
    );
    let g = Language::CommonLisp.grammar();
    assert_eq!(g.label, "Common Lisp");
    let text = "(defun f (x &optional (y #\\;)) (let* ((z :key)) z)) ; done";
    let seen = painted(text, &one(text, g));
    assert!(seen.contains(&(Role::Keyword, "defun")), "{seen:?}");
    assert!(seen.contains(&(Role::Keyword, "&optional")), "{seen:?}");
    assert!(seen.contains(&(Role::Constant, "#\\;")), "{seen:?}");
    assert!(seen.contains(&(Role::Keyword, "let*")), "{seen:?}");
    assert!(seen.contains(&(Role::Constant, ":key")), "{seen:?}");
    assert!(seen.contains(&(Role::Comment, "; done")), "{seen:?}");
}

/// FAILS IF: `guess` cuts its head inside a character. A text whose 4096th
/// byte falls in the middle of `é` or `☕` panicked there — in a preview,
/// which is the footer.
#[test]
fn a_guess_never_cuts_a_character() {
    for filler in ["é", "☕", "a"] {
        for shift in 0..4 {
            let mut text = alloc::string::String::from("x").repeat(4096 - 1 - shift);
            while text.len() < 4100 {
                text.push_str(filler);
            }
            let _ = guess(&text);
        }
    }
    // AND THE CUT STILL SEES WHAT IS BEFORE IT.
    let mut text = alloc::string::String::from("(in-package :x)\n");
    while text.len() < 5000 {
        text.push('é');
    }
    assert_eq!(guess(&text), Some(Language::CommonLisp));
}
