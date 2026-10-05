//! The engine: tantivy's schema, the tokenizer, the reader and the query
//! (feature `engine`).
//!
//! # Why the reader has no clock
//!
//! tantivy's default reload policy is a thread that polls `meta.json`; this
//! opens with [`ReloadPolicy::Manual`] and reloads when the door's inotify
//! watch on `meta.json` fires ([`crate::Index::reload`]). The writer's commit
//! IS the event: tantivy writes `meta.json` last, by rename, after every
//! segment it names is on disk.
//!
//! # Two passes, one per budget
//!
//! A query runs over the NAMES (name ×4, the folders above ×1) and, when it
//! asks for contents, over the BODIES (×1) — the two numbers of the gate: p99 ≤
//! 8 ms for names and ≤ 25 ms for both. A file whose name matched is a
//! [`Kind::File`] hit; one found only by what is written in it is a
//! [`Kind::Content`] hit. A manual page, a package or a video found in its
//! body stays what it is, with the two scores added.
//!
//! # The query
//!
//! Every typed word must match (AND). The words before the last are terms,
//! plus a Levenshtein-1 fuzzy match from five letters on; the last is a prefix
//! over the FST term dictionary, at most [`EXPANSIONS`] terms in dictionary
//! order (so the word itself, when it is one, comes first), plus the fuzzy
//! match. The word itself scores by BM25; a longer word it only begins
//! scores a constant below it, whatever that word's rarity. Each kind keeps its own best `most` — fast-access shows sections by
//! kind, and one crowded kind must not empty the others.
//!
//! # Recency and frecency
//!
//! The collector multiplies each score by a recency factor from the `mtime`
//! fast field (a half-life of [`HALF_LIFE_DAYS`]); the app's history
//! ([`crate::History`]) multiplies the hits it knows, and its ids are asked
//! for in a second, narrow query so a frecent hit outside the first window is
//! still answered.

use std::cmp::Reverse;
use std::collections::{BTreeSet, BinaryHeap, HashMap};
use std::path::{Path, PathBuf};

use tantivy::collector::{Collector, SegmentCollector};
use tantivy::columnar::ColumnValues;
use tantivy::directory::MmapDirectory;
use tantivy::query::{
    BooleanQuery, BoostQuery, ConstScoreQuery, DisjunctionMaxQuery, EmptyQuery, FuzzyTermQuery,
    Occur, Query, TermQuery, TermSetQuery,
};
use tantivy::schema::{
    Field, INDEXED, IndexRecordOption, STORED, STRING, Schema, TextFieldIndexing, TextOptions,
    Value,
};
use tantivy::tokenizer::{Token, TokenStream, Tokenizer};
use tantivy::{
    DocAddress, DocId, IndexReader, ReloadPolicy, Score, Searcher, SegmentOrdinal, SegmentReader,
    TantivyDocument, Term,
};

use crate::{History, Hit, Kind, Query as Asked, Refusal, Why, words};

/// The tokenizer's name in tantivy's registry.
pub(crate) const TOKENIZER: &str = "ronoctua";

/// The format every commit names in its payload, and a reader checks: an
/// index written by another format is refused ([`Why::OtherFormat`]) and the
/// unit rebuilds it, rather than answering with fields read in the wrong
/// order.
pub(crate) const FORMAT: &str = "ronoctua-index/1";

/// What a commit says about itself (tantivy's commit payload, in
/// `meta.json`): the format, and what the writer last took from each source —
/// committed WITH the documents, so the two can never disagree after a crash.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct State {
    /// [`FORMAT`], for this build.
    pub format: String,
    /// The signature of the package list indexed (its file, size and time).
    #[serde(default)]
    pub packages: Option<String>,
    /// The signature of the manual folders indexed (where they resolve).
    #[serde(default)]
    pub manuals: Option<String>,
    /// The video catalogue's generation indexed.
    #[serde(default)]
    pub videos: Option<u64>,
    /// Whether the last full walk read contents.
    #[serde(default)]
    pub contents: Option<bool>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            format: FORMAT.to_owned(),
            packages: None,
            manuals: None,
            videos: None,
            contents: None,
        }
    }
}

/// The state a payload holds, or `None` when it is not this format's.
pub(crate) fn state_of(payload: &str) -> Option<State> {
    serde_json::from_str::<State>(payload)
        .ok()
        .filter(|s| s.format == FORMAT)
}

/// The most terms the last (prefix) word expands to, per field.
pub const EXPANSIONS: usize = 64;

/// Words this long and longer also match at Levenshtein distance 1.
pub const FUZZY_FROM: usize = 5;

/// The field weights: a name counts four times a folder or a body.
const NAME_WEIGHT: Score = 4.0;
const PATH_WEIGHT: Score = 1.0;
const BODY_WEIGHT: Score = 1.0;

/// A fuzzy match counts a constant (tantivy's automaton weight scores 1), cut
/// to this share of its field's weight: a typo is found, below the exact word.
const FUZZY_SHARE: Score = 0.5;

/// What a word only BEGUN scores (the typed prefix of a longer word, before
/// its field's weight): a constant, and not BM25, because a completion's
/// rarity says nothing about what the person meant — scored by its own IDF,
/// `rgl` (one package) outranked `ripgrep`, whose program IS `rg`. One is
/// below what an exact word scores in all but the commonest words.
const BEGUN: Score = 1.0;

/// The most a short name adds to its score: a one-word name scores
/// `1 + SHORT / 2` times, a long one barely more than once.
const SHORT: Score = 0.25;

/// Recency: a file changed now scores `1 + RECENCY` times; the bonus halves
/// every [`HALF_LIFE_DAYS`].
const RECENCY: Score = 0.5;

/// The half-life of the recency bonus, in days.
pub const HALF_LIFE_DAYS: f32 = 30.0;

/// Frecency: a hit the app's history weighs `w` (0 to 1) scores `1 +
/// FRECENCY × w` times.
const FRECENCY: Score = 1.0;

/// A hit whose whole name is what was typed (`git commit` for `git-commit`,
/// `ls` for `ls`) scores this many times: BM25 counts words, and a longer
/// name that repeats them in a short body would otherwise pass the one that
/// IS the answer.
const EXACT: Score = 1.5;

/// The index's fields.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Fields {
    pub(crate) id: Field,
    pub(crate) kind: Field,
    pub(crate) name: Field,
    pub(crate) path_terms: Field,
    pub(crate) body: Field,
    pub(crate) mime: Field,
    pub(crate) mtime: Field,
    pub(crate) size: Field,
    pub(crate) note: Field,
    /// Written by the writer (what [`crate::write::Writer::shed`] reads);
    /// a reader never asks for it.
    #[cfg_attr(not(feature = "write"), allow(dead_code))]
    pub(crate) body_bytes: Field,
}

impl Fields {
    /// The schema this build writes, and its fields.
    pub(crate) fn schema() -> (Schema, Self) {
        let words = |stored: bool| {
            let indexing = TextFieldIndexing::default()
                .set_tokenizer(TOKENIZER)
                .set_index_option(IndexRecordOption::WithFreqs);
            let options = TextOptions::default().set_indexing_options(indexing);
            if stored {
                options.set_stored()
            } else {
                options
            }
        };
        let mut b = Schema::builder();
        let fields = Self {
            id: b.add_text_field("id", STRING | STORED),
            kind: b.add_u64_field("kind", INDEXED | STORED | tantivy::schema::FAST),
            name: b.add_text_field("name", words(true)),
            path_terms: b.add_text_field("path_terms", words(false)),
            body: b.add_text_field("body", words(false)),
            mime: b.add_text_field("mime", STRING | STORED),
            mtime: b.add_i64_field("mtime", STORED | tantivy::schema::FAST),
            size: b.add_u64_field("size", STORED | tantivy::schema::FAST),
            note: b.add_text_field("note", STORED),
            body_bytes: b.add_u64_field("body_bytes", STORED | tantivy::schema::FAST),
        };
        (b.build(), fields)
    }
}

/// The tokenizer: [`words::indexed`], as tantivy asks for it.
#[derive(Debug, Clone, Default)]
pub(crate) struct Words;

/// Its stream: the pieces of one text, cut up front.
pub(crate) struct WordStream {
    tokens: Vec<Token>,
    at: usize,
}

impl Tokenizer for Words {
    type TokenStream<'a> = WordStream;

    fn token_stream<'a>(&'a mut self, text: &'a str) -> WordStream {
        let tokens = words::indexed(text)
            .into_iter()
            .map(|p| Token {
                offset_from: p.offset.start,
                offset_to: p.offset.end,
                position: p.position,
                text: p.text,
                position_length: 1,
            })
            .collect();
        WordStream { tokens, at: 0 }
    }
}

impl TokenStream for WordStream {
    fn advance(&mut self) -> bool {
        if self.at < self.tokens.len() {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn token(&self) -> &Token {
        &self.tokens[self.at - 1]
    }

    fn token_mut(&mut self) -> &mut Token {
        &mut self.tokens[self.at - 1]
    }
}

/// Registers the tokenizer on an opened index (tokenizers are not saved in it).
pub(crate) fn register(index: &tantivy::Index) {
    index.tokenizers().register(TOKENIZER, Words);
}

/// A tantivy error, as the index's refusal.
pub(crate) fn failed(why: Why) -> impl Fn(tantivy::TantivyError) -> Refusal {
    move |e| {
        tracing::warn!(error = %e, "the index refused");
        Refusal { why }
    }
}

/// Opens the index in `dir` as it is on disk, checked against this build's
/// schema and format.
pub(crate) fn open_index(dir: &Path) -> Result<(tantivy::Index, Fields, State), Refusal> {
    if !dir.join("meta.json").is_file() {
        return Err(Refusal { why: Why::NotBuilt });
    }
    let directory = MmapDirectory::open(dir).map_err(|_| Refusal {
        why: Why::Unreadable,
    })?;
    let index = tantivy::Index::open(directory).map_err(failed(Why::Unreadable))?;
    let (schema, fields) = Fields::schema();
    if index.schema() != schema {
        return Err(Refusal {
            why: Why::OtherFormat,
        });
    }
    let payload = index.load_metas().map_err(failed(Why::Unreadable))?.payload;
    // An index with no commit of ours yet (created, never committed) has no
    // payload and no documents: it is simply empty, not of another format.
    let state = match payload.as_deref() {
        None => State::default(),
        Some(p) => state_of(p).ok_or(Refusal {
            why: Why::OtherFormat,
        })?,
    };
    register(&index);
    Ok((index, fields, state))
}

/// The read side: an index, its reader (no reload clock) and its fields.
pub(crate) struct Reader {
    pub(crate) reader: IndexReader,
    pub(crate) fields: Fields,
}

impl std::fmt::Debug for Reader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Reader")
            .field("documents", &self.reader.searcher().num_docs())
            .finish_non_exhaustive()
    }
}

impl Reader {
    /// Opens the index in `dir` read-only.
    pub(crate) fn open(dir: &Path) -> Result<Self, Refusal> {
        let (index, fields, _) = open_index(dir)?;
        Self::of(&index, fields)
    }

    /// A reader over an index already opened (the writer's own).
    pub(crate) fn of(index: &tantivy::Index, fields: Fields) -> Result<Self, Refusal> {
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()
            .map_err(failed(Why::Unreadable))?;
        Ok(Self { reader, fields })
    }

    /// Sees the last commit.
    pub(crate) fn reload(&self) -> Result<(), Refusal> {
        self.reader.reload().map_err(failed(Why::Unreadable))
    }

    /// The searcher of the commit this reader sees.
    pub(crate) fn searcher(&self) -> Searcher {
        self.reader.searcher()
    }

    /// The best hits for `asked`, `most` of each kind, best first.
    pub(crate) fn query(
        &self,
        asked: &Asked,
        history: &History,
        now_s: i64,
    ) -> Result<Vec<Hit>, Refusal> {
        let searcher = self.searcher();
        let f = self.fields;
        let Some(pieces) = Pieces::of(&asked.words) else {
            return Ok(Vec::new());
        };
        let most = asked.most.max(1);
        let asked_kinds = asked_kinds(asked);
        let mimes = mime_filter(&searcher, f, &asked.mime_prefixes)?;
        let top = Top {
            most,
            now_s,
            name: f.name,
        };

        let mut found: HashMap<DocAddress, (Kind, Score)> = HashMap::new();
        let names_kinds: Vec<Kind> = asked_kinds
            .iter()
            .copied()
            .filter(|k| {
                matches!(
                    k,
                    Kind::File | Kind::Folder | Kind::Man | Kind::Package | Kind::Video
                )
            })
            .collect();
        let mut body_kinds: Vec<Kind> = Vec::new();
        if asked.contents {
            if asked_kinds.contains(&Kind::Content) {
                body_kinds.push(Kind::File);
            }
            body_kinds.extend(
                asked_kinds
                    .iter()
                    .copied()
                    .filter(|k| matches!(k, Kind::Man | Kind::Package | Kind::Video)),
            );
        }

        let names_fields = [(f.name, NAME_WEIGHT), (f.path_terms, PATH_WEIGHT)];
        let body_fields = [(f.body, BODY_WEIGHT)];
        let passes: [Pass<'_>; 2] = [(&names_kinds, &names_fields), (&body_kinds, &body_fields)];
        for (pass, (kinds, fields)) in passes.into_iter().enumerate() {
            if kinds.is_empty() {
                continue;
            }
            let Some(words) = pieces.query(&searcher, fields)? else {
                continue;
            };
            let filters = filters(f, kinds, mimes.as_ref());
            let mut runs = vec![and(words.box_clone(), &filters, None)];
            if !history.is_empty() {
                runs.push(and(words, &filters, Some(ids_query(f, history))));
            }
            // The history's narrow run finds some of the same documents: one
            // pass answers each document once.
            let mut this_pass: HashMap<DocAddress, (Kind, Score)> = HashMap::new();
            for run in runs {
                let collected = searcher
                    .search(run.as_ref(), &top)
                    .map_err(failed(Why::Failed))?;
                for (kind, score, at) in collected {
                    let kind = if pass == 1 && kind == Kind::File {
                        Kind::Content
                    } else {
                        kind
                    };
                    let slot = this_pass.entry(at).or_insert((kind, score));
                    slot.1 = slot.1.max(score);
                }
            }
            for (at, (kind, score)) in this_pass {
                merge(&mut found, at, kind, score);
            }
        }

        let mut hits = Vec::with_capacity(found.len());
        for (at, (kind, score)) in found {
            let doc: TantivyDocument = searcher.doc(at).map_err(failed(Why::Failed))?;
            let mut hit = hit_of(&doc, f, kind, score);
            if let Some(w) = history.weight(&hit.id) {
                hit.score *= 1.0 + FRECENCY * w;
            }
            if pieces.names_exactly(&hit) {
                hit.score *= EXACT;
            }
            hits.push(hit);
        }
        Ok(best_of_each_kind(hits, most))
    }
}

/// One pass of a query: the kinds it collects and the weighted fields it
/// asks.
type Pass<'a> = (&'a [Kind], &'a [(Field, Score)]);

/// The kinds a question covers: the ones it names, or every indexed kind.
fn asked_kinds(asked: &Asked) -> Vec<Kind> {
    if asked.kinds.is_empty() {
        vec![
            Kind::File,
            Kind::Folder,
            Kind::Content,
            Kind::Man,
            Kind::Package,
            Kind::Video,
        ]
    } else {
        asked.kinds.clone()
    }
}

/// One document of the body pass into the answer of the names pass: a file
/// found by its name stays a File hit (not also a Content one); a manual, a
/// package or a video found in both adds the two scores.
fn merge(found: &mut HashMap<DocAddress, (Kind, Score)>, at: DocAddress, kind: Kind, score: Score) {
    match found.get_mut(&at) {
        None => {
            found.insert(at, (kind, score));
        }
        Some((was, old)) => {
            if *was == kind {
                *old += score;
            }
        }
    }
}

/// Keeps the best `most` of each kind, best first; kinds in fast-access's
/// order.
fn best_of_each_kind(mut hits: Vec<Hit>, most: usize) -> Vec<Hit> {
    hits.sort_by(|a, b| {
        let order = |k: Kind| Kind::ALL.iter().position(|x| *x == k).unwrap_or(usize::MAX);
        order(a.kind)
            .cmp(&order(b.kind))
            .then(b.score.total_cmp(&a.score))
            .then_with(|| a.id.cmp(&b.id))
    });
    let mut out = Vec::with_capacity(hits.len());
    let mut kind = None;
    let mut taken = 0;
    for hit in hits {
        if kind != Some(hit.kind) {
            kind = Some(hit.kind);
            taken = 0;
        }
        if taken < most {
            taken += 1;
            out.push(hit);
        }
    }
    out
}

/// A stored document as a hit.
fn hit_of(doc: &TantivyDocument, f: Fields, kind: Kind, score: Score) -> Hit {
    let text = |field: Field| {
        doc.get_first(field)
            .and_then(|v| v.as_str())
            .map(str::to_owned)
    };
    let id = text(f.id).unwrap_or_default();
    let path =
        matches!(kind, Kind::File | Kind::Folder | Kind::Content).then(|| PathBuf::from(&id));
    Hit {
        name: text(f.name).unwrap_or_default(),
        note: text(f.note),
        mime: text(f.mime),
        mtime_s: doc.get_first(f.mtime).and_then(|v| v.as_i64()),
        size_bytes: doc.get_first(f.size).and_then(|v| v.as_u64()),
        id,
        kind,
        score,
        path,
    }
}

/// The typed words, cut the index's way: every piece but the last is a term,
/// the last a prefix.
struct Pieces {
    terms: Vec<String>,
    prefix: String,
}

impl Pieces {
    fn of(typed: &[String]) -> Option<Self> {
        let mut all: Vec<String> = typed.iter().flat_map(|w| words::typed(w)).collect();
        let prefix = all.pop()?;
        Some(Self { terms: all, prefix })
    }

    /// Whether the hit's whole name is what was typed: one typed piece per
    /// word of the name, in order, every one whole. A file is named without
    /// its extension (`report` is the whole of `report.odt`).
    fn names_exactly(&self, hit: &Hit) -> bool {
        let name = match hit.kind {
            Kind::File | Kind::Folder | Kind::Content => Path::new(&hit.name)
                .file_stem()
                .map_or_else(|| hit.name.clone(), |s| s.to_string_lossy().into_owned()),
            _ => hit.name.clone(),
        };
        // The words at each position (a chunk and its parts share one).
        let mut at: Vec<Vec<String>> = Vec::new();
        for piece in words::indexed(&name) {
            if at.len() <= piece.position {
                at.resize(piece.position + 1, Vec::new());
            }
            at[piece.position].push(piece.text);
        }
        if at.len() != self.terms.len() + 1 {
            return false;
        }
        let terms_match = self
            .terms
            .iter()
            .zip(&at)
            .all(|(typed, here)| here.iter().any(|w| w == typed));
        terms_match && at.last().is_some_and(|here| here.contains(&self.prefix))
    }

    /// The query every piece must match over `fields`, or `None` when one of
    /// them cannot match at all (a prefix nothing starts with and too short
    /// for a fuzzy match).
    ///
    /// Within one field a piece scores its BEST way of matching (a
    /// disjunction-max): `reportFinal` holds both `report` and `reportfinal`,
    /// and typing `report` must not count it twice against `report.odt`.
    /// Across fields the scores add, by weight.
    fn query(
        &self,
        searcher: &Searcher,
        fields: &[(Field, Score)],
    ) -> Result<Option<Box<dyn Query>>, Refusal> {
        let mut musts: Vec<(Occur, Box<dyn Query>)> = Vec::new();
        for term in &self.terms {
            let mut per_field: Vec<(Occur, Box<dyn Query>)> = Vec::new();
            for &(field, weight) in fields {
                let mut ways = vec![term_query(field, term)];
                ways.extend(fuzzy(field, term));
                per_field.push((Occur::Should, boosted(best_of(ways), weight)));
            }
            musts.push((Occur::Must, Box::new(BooleanQuery::new(per_field))));
        }
        let mut per_field: Vec<(Occur, Box<dyn Query>)> = Vec::new();
        for &(field, weight) in fields {
            let expanded = expansions(searcher, field, &self.prefix)?;
            let mut ways: Vec<Box<dyn Query>> = Vec::new();
            if expanded.contains(&self.prefix) {
                ways.push(term_query(field, &self.prefix));
            }
            let begun: Vec<Term> = expanded
                .iter()
                .filter(|w| **w != self.prefix)
                .map(|w| Term::from_field_text(field, w))
                .collect();
            if !begun.is_empty() {
                ways.push(Box::new(ConstScoreQuery::new(
                    Box::new(TermSetQuery::new(begun)),
                    BEGUN,
                )));
            }
            ways.extend(fuzzy(field, &self.prefix));
            if !ways.is_empty() {
                per_field.push((Occur::Should, boosted(best_of(ways), weight)));
            }
        }
        if per_field.is_empty() {
            return Ok(None);
        }
        musts.push((Occur::Must, Box::new(BooleanQuery::new(per_field))));
        Ok(Some(Box::new(BooleanQuery::new(musts))))
    }
}

/// The best-scoring of several ways to match (one way needs no wrapper).
fn best_of(mut ways: Vec<Box<dyn Query>>) -> Box<dyn Query> {
    if ways.len() == 1 {
        ways.remove(0)
    } else {
        Box::new(DisjunctionMaxQuery::new(ways))
    }
}

fn term_query(field: Field, word: &str) -> Box<dyn Query> {
    Box::new(TermQuery::new(
        Term::from_field_text(field, word),
        IndexRecordOption::WithFreqs,
    ))
}

/// A Levenshtein-1 match of `word` (from [`FUZZY_FROM`] letters), at its
/// share.
fn fuzzy(field: Field, word: &str) -> Option<Box<dyn Query>> {
    (word.chars().count() >= FUZZY_FROM).then(|| {
        boosted(
            Box::new(FuzzyTermQuery::new(
                Term::from_field_text(field, word),
                1,
                true,
            )),
            FUZZY_SHARE,
        )
    })
}

fn boosted(query: Box<dyn Query>, weight: Score) -> Box<dyn Query> {
    if (weight - 1.0).abs() < f32::EPSILON {
        query
    } else {
        Box::new(BoostQuery::new(query, weight))
    }
}

/// The first [`EXPANSIONS`] terms of `field` that begin with `prefix`, across
/// every segment, in dictionary order.
fn expansions(searcher: &Searcher, field: Field, prefix: &str) -> Result<Vec<String>, Refusal> {
    let mut out: BTreeSet<Vec<u8>> = BTreeSet::new();
    let upper = upper_bound(prefix.as_bytes());
    for segment in searcher.segment_readers() {
        let inverted = segment.inverted_index(field).map_err(failed(Why::Failed))?;
        let mut range = inverted.terms().range().ge(prefix.as_bytes());
        if let Some(upper) = &upper {
            range = range.lt(upper);
        }
        let mut stream = range
            .into_stream()
            .map_err(|_| Refusal { why: Why::Failed })?;
        let mut taken = 0;
        while taken < EXPANSIONS && stream.advance() {
            out.insert(stream.key().to_vec());
            taken += 1;
        }
    }
    Ok(out
        .into_iter()
        .take(EXPANSIONS)
        .filter_map(|k| String::from_utf8(k).ok())
        .collect())
}

/// The smallest byte string greater than every string starting with
/// `prefix`, or `None` when there is none (all `0xFF`).
pub(crate) fn upper_bound(prefix: &[u8]) -> Option<Vec<u8>> {
    let mut upper = prefix.to_vec();
    while let Some(last) = upper.pop() {
        if last < u8::MAX {
            upper.push(last + 1);
            return Some(upper);
        }
    }
    None
}

/// The MIME types of the index that begin with one of `prefixes`, as a
/// filter; `None` when no prefix was asked.
fn mime_filter(
    searcher: &Searcher,
    f: Fields,
    prefixes: &[String],
) -> Result<Option<Vec<Term>>, Refusal> {
    if prefixes.is_empty() {
        return Ok(None);
    }
    let mut terms: BTreeSet<Vec<u8>> = BTreeSet::new();
    for segment in searcher.segment_readers() {
        let inverted = segment
            .inverted_index(f.mime)
            .map_err(failed(Why::Failed))?;
        for prefix in prefixes {
            let mut range = inverted.terms().range().ge(prefix.as_bytes());
            if let Some(upper) = upper_bound(prefix.as_bytes()) {
                range = range.lt(upper);
            }
            let mut stream = range
                .into_stream()
                .map_err(|_| Refusal { why: Why::Failed })?;
            while stream.advance() {
                terms.insert(stream.key().to_vec());
            }
        }
    }
    Ok(Some(
        terms
            .into_iter()
            .filter_map(|k| String::from_utf8(k).ok())
            .map(|m| Term::from_field_text(f.mime, &m))
            .collect(),
    ))
}

/// The filters of a pass: its kinds and, when asked, the MIME types.
fn filters(f: Fields, kinds: &[Kind], mimes: Option<&Vec<Term>>) -> Vec<Box<dyn Query>> {
    let mut out: Vec<Box<dyn Query>> = vec![Box::new(TermSetQuery::new(
        kinds.iter().map(|k| Term::from_field_u64(f.kind, k.code())),
    ))];
    if let Some(mimes) = mimes {
        out.push(if mimes.is_empty() {
            Box::new(EmptyQuery)
        } else {
            Box::new(TermSetQuery::new(mimes.iter().cloned()))
        });
    }
    out
}

/// The ids of the app's history, as a filter.
fn ids_query(f: Fields, history: &History) -> Box<dyn Query> {
    Box::new(TermSetQuery::new(
        history.ids().map(|id| Term::from_field_text(f.id, id)),
    ))
}

/// `words` AND every filter (scoring nothing: a filter's constant would add
/// the same to every hit and skew the recency product).
fn and(
    words: Box<dyn Query>,
    filters: &[Box<dyn Query>],
    extra: Option<Box<dyn Query>>,
) -> Box<dyn Query> {
    let mut clauses: Vec<(Occur, Box<dyn Query>)> = vec![(Occur::Must, words)];
    for filter in filters.iter().map(|q| q.box_clone()).chain(extra) {
        clauses.push((Occur::Must, Box::new(BoostQuery::new(filter, 0.0))));
    }
    Box::new(BooleanQuery::new(clauses))
}

/// The recency factor of a document changed at `mtime_s`, seen at `now_s`.
#[must_use]
pub fn recency(mtime_s: i64, now_s: i64) -> Score {
    if mtime_s <= 0 {
        return 1.0;
    }
    #[allow(clippy::cast_precision_loss)]
    let age_days = (now_s - mtime_s).max(0) as f32 / 86_400.0;
    1.0 + RECENCY * (-age_days / HALF_LIFE_DAYS).exp2()
}

/// The factor a name of `words` words scores by: a little more for a short
/// one, so among matches that score alike — every name a typed word only
/// begins scores the same constant — `firefox` comes before
/// `faust2firefox`.
#[must_use]
pub fn shortness(words: u32) -> Score {
    #[allow(clippy::cast_precision_loss)]
    let words = words as Score;
    1.0 + SHORT / (1.0 + words)
}

/// The collector: the best `most` of each kind, score × recency × shortness.
struct Top {
    most: usize,
    now_s: i64,
    name: Field,
}

/// A score that orders (`total_cmp`), for the heaps.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Ordered(Score);

impl Eq for Ordered {}

impl PartialOrd for Ordered {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Ordered {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}

/// One segment's side of [`Top`].
struct TopSegment {
    ord: SegmentOrdinal,
    most: usize,
    now_s: i64,
    kinds: std::sync::Arc<dyn ColumnValues<u64>>,
    mtimes: std::sync::Arc<dyn ColumnValues<i64>>,
    /// How many words each name holds.
    names: tantivy::fieldnorm::FieldNormReader,
    /// One min-heap per kind code (index = code).
    heaps: Vec<BinaryHeap<Reverse<(Ordered, DocId)>>>,
}

impl Collector for Top {
    type Fruit = Vec<(Kind, Score, DocAddress)>;
    type Child = TopSegment;

    fn for_segment(
        &self,
        ord: SegmentOrdinal,
        segment: &SegmentReader,
    ) -> tantivy::Result<TopSegment> {
        let fast = segment.fast_fields();
        Ok(TopSegment {
            ord,
            most: self.most,
            now_s: self.now_s,
            kinds: fast.u64("kind")?.first_or_default_col(0),
            mtimes: fast.i64("mtime")?.first_or_default_col(0),
            names: segment.get_fieldnorms_reader(self.name)?,
            heaps: (0..=Kind::ALL.len()).map(|_| BinaryHeap::new()).collect(),
        })
    }

    fn requires_scoring(&self) -> bool {
        true
    }

    fn merge_fruits(
        &self,
        fruits: Vec<Vec<(Kind, Score, DocAddress)>>,
    ) -> tantivy::Result<Self::Fruit> {
        let mut all: Vec<(Kind, Score, DocAddress)> = fruits.into_iter().flatten().collect();
        all.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.total_cmp(&a.1)));
        let mut out = Vec::with_capacity(all.len());
        let mut kind = None;
        let mut taken = 0;
        for hit in all {
            if kind != Some(hit.0) {
                kind = Some(hit.0);
                taken = 0;
            }
            if taken < self.most {
                taken += 1;
                out.push(hit);
            }
        }
        Ok(out)
    }
}

impl SegmentCollector for TopSegment {
    type Fruit = Vec<(Kind, Score, DocAddress)>;

    fn collect(&mut self, doc: DocId, score: Score) {
        let code = self.kinds.get_val(doc);
        let Some(heap) = usize::try_from(code)
            .ok()
            .and_then(|c| self.heaps.get_mut(c))
        else {
            return;
        };
        let score = score
            * recency(self.mtimes.get_val(doc), self.now_s)
            * shortness(self.names.fieldnorm(doc));
        if heap.len() < self.most {
            heap.push(Reverse((Ordered(score), doc)));
        } else if heap.peek().is_some_and(|Reverse((low, _))| low.0 < score) {
            heap.pop();
            heap.push(Reverse((Ordered(score), doc)));
        }
    }

    fn harvest(self) -> Self::Fruit {
        let ord = self.ord;
        self.heaps
            .into_iter()
            .enumerate()
            .flat_map(|(code, heap)| {
                let kind = Kind::of_code(code as u64);
                heap.into_iter().filter_map(move |Reverse((score, doc))| {
                    kind.map(|k| (k, score.0, DocAddress::new(ord, doc)))
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_payload_of_another_format_is_not_a_state() {
        let ours = serde_json::to_string(&State::default()).unwrap();
        assert_eq!(state_of(&ours), Some(State::default()));
        assert_eq!(state_of(r#"{"format":"ronoctua-index/0"}"#), None);
        assert_eq!(state_of("ronoctua-index/1"), None);
        // A field this build does not know is a newer writer's addition, and
        // the format says whether the documents can be read.
        let later = r#"{"format":"ronoctua-index/1","videos":7,"extra":1}"#;
        assert_eq!(state_of(later).and_then(|s| s.videos), Some(7));
    }

    #[test]
    fn the_upper_bound_closes_a_prefix() {
        assert_eq!(upper_bound(b"rep"), Some(b"req".to_vec()));
        assert_eq!(upper_bound(b"a\xff"), Some(b"b".to_vec()));
        assert_eq!(upper_bound(b"\xff\xff"), None);
    }

    #[test]
    fn a_shorter_name_scores_a_little_more() {
        assert!(shortness(1) > shortness(4));
        assert!((shortness(1) - 1.125).abs() < 1e-6);
        assert!(shortness(100) > 1.0 && shortness(100) < 1.01);
    }

    #[test]
    fn recency_halves_and_never_goes_below_one() {
        let now = 1_000_000_000;
        assert!((recency(now, now) - 1.5).abs() < 1e-6);
        let month = (HALF_LIFE_DAYS * 86_400.0) as i64;
        assert!((recency(now - month, now) - 1.25).abs() < 1e-4);
        assert!((recency(0, now) - 1.0).abs() < 1e-6);
        assert!(recency(now - 100 * month, now) >= 1.0);
        // A file from the future is as recent as now, not more.
        assert!((recency(now + 999, now) - 1.5).abs() < 1e-6);
    }

    fn hit(kind: Kind, name: &str) -> Hit {
        Hit {
            id: name.to_owned(),
            kind,
            name: name.to_owned(),
            note: None,
            score: 1.0,
            path: None,
            mime: None,
            mtime_s: None,
            size_bytes: None,
        }
    }

    #[test]
    fn a_whole_name_is_an_exact_match_and_a_longer_one_is_not() {
        let typed =
            |t: &str| Pieces::of(&t.split(' ').map(str::to_owned).collect::<Vec<_>>()).unwrap();
        assert!(typed("git commit").names_exactly(&hit(Kind::Man, "git-commit")));
        assert!(!typed("git comm").names_exactly(&hit(Kind::Man, "git-commit")));
        assert!(!typed("git commit").names_exactly(&hit(Kind::Man, "git-commit-graph")));
        assert!(!typed("commit").names_exactly(&hit(Kind::Man, "git-commit")));
        assert!(typed("report").names_exactly(&hit(Kind::File, "report.odt")));
        assert!(typed("reportfinal").names_exactly(&hit(Kind::File, "reportFinal.odt")));
        assert!(!typed("report").names_exactly(&hit(Kind::File, "report-2024.odt")));
    }

    #[test]
    fn the_last_piece_is_the_prefix() {
        let p = Pieces::of(&["my-file".to_owned(), "Rep".to_owned()]).unwrap();
        assert_eq!(p.terms, ["my", "file"]);
        assert_eq!(p.prefix, "rep");
        assert!(Pieces::of(&["--".to_owned()]).is_none());
        assert!(Pieces::of(&[]).is_none());
    }
}
