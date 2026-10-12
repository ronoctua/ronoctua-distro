//! The writer side: what the `ronoctua-index` unit runs (feature `write`).
//!
//! One [`Writer`] per index, taken by the unit (tantivy's lock refuses a
//! second: [`crate::Why::Busy`]). Documents are replaced by id —
//! [`Writer::delete`] then [`Writer::add`] — and made visible by
//! [`Writer::commit`], which the unit batches: a timer armed only while
//! events are pending (about a second of quiet, or ten thousand documents).
//!
//! # The ceiling
//!
//! `searchIndexMiB` (`assets/cache.nix`) bounds the directory. Past it
//! [`Writer::shed`] takes the CONTENTS of the oldest files out first — the
//! document stays, with its name, folders and dates — and merges the segments
//! that held them, which is when tantivy gives the space back. Names are what
//! the index is for; a body is what it can do without.
//!
//! A shed REWRITES the segments it merges, and in an index that has lived for
//! a while that is most of the folder. So it is measured against what the last
//! commit names ([`Writer::committed_bytes`] — a merge in flight is not the
//! index growing), it merges only the segments that hold deletions, and the
//! crawler asks it for room below the ceiling (`crate::crawl::ROOM_PERCENT`),
//! so the edits after it do not each cross the ceiling again.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use tantivy::collector::Count;
use tantivy::index::SegmentId;
use tantivy::indexer::LogMergePolicy;
use tantivy::query::TermQuery;
use tantivy::schema::{IndexRecordOption, Value};
use tantivy::{DocSet, IndexWriter, TERMINATED, TantivyDocument, Term};

pub use crate::engine::State;
use crate::engine::{self, Fields, Reader, failed};
use crate::{Kind, Refusal, Why, words};

/// tantivy refuses less than this per indexing thread.
const THREAD_ARENA_MIN: usize = 15_000_000;

/// The most rounds [`Writer::shed`] takes: past them the folder is left where
/// it got to, and the next commit tries again.
const SHED_ROUNDS: usize = 8;

/// A segment is merged once this share of it is deleted: the index is updated
/// in place for months, and deleted documents only give their space back in a
/// merge.
const DELETED_SHARE: f32 = 0.3;

/// One document of the index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    /// Its id (the replace key): a path, `pkg:<attr>`, `man:<name>.<section>`,
    /// `yt:<id>`.
    pub id: String,
    /// What it is.
    pub kind: Kind,
    /// What its name field holds (before folding), and what a hit shows.
    pub name: String,
    /// More words that find it by name without being shown (a package's
    /// `pname` and program beside its attribute).
    pub also: Vec<String>,
    /// The folder names above it, for a file.
    pub path_terms: Vec<String>,
    /// Its text: contents (≤ `searchContentKiB`), a manual's page, a
    /// package's description, a video's description. Indexed, not stored.
    pub body: Option<String>,
    /// The dim detail a hit shows beside the name (stored, not indexed).
    pub note: Option<String>,
    /// Its MIME type.
    pub mime: Option<String>,
    /// Its modification time, in seconds since the epoch — what the
    /// reconciling walk compares, with the size.
    pub mtime_s: i64,
    /// Its size, in bytes.
    pub size_bytes: u64,
}

impl Document {
    /// A file or folder found at `path`, with no body yet.
    #[must_use]
    pub fn of_path(
        path: &Path,
        kind: Kind,
        mime: Option<&str>,
        mtime_s: i64,
        size_bytes: u64,
    ) -> Self {
        Self {
            id: path.to_string_lossy().into_owned(),
            kind,
            name: path.file_name().map_or_else(
                || path.to_string_lossy().into_owned(),
                |n| n.to_string_lossy().into_owned(),
            ),
            also: Vec::new(),
            path_terms: words::folders_of(path),
            body: None,
            note: None,
            mime: mime.map(str::to_owned),
            mtime_s,
            size_bytes,
        }
    }
}

/// What the index holds of one id, from its last commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stamp {
    /// What it is.
    pub kind: Kind,
    /// Its modification time when indexed, in seconds since the epoch.
    pub mtime_s: i64,
    /// Its size when indexed, in bytes.
    pub size_bytes: u64,
    /// How many bytes of text its body holds (0: none, or taken out).
    pub body_bytes: u64,
}

/// What [`Writer::shed`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Shed {
    /// Documents whose contents were taken out.
    pub emptied: u64,
    /// What the last commit names after it, in bytes.
    pub size_bytes: u64,
    /// The bytes of the segments it rewrote (its merges, and the merge
    /// policy's while it ran).
    pub rewritten_bytes: u64,
    /// Rounds taken (a round empties, commits and merges once).
    pub rounds: u32,
}

/// The one writer of an index.
pub struct Writer {
    dir: PathBuf,
    index: tantivy::Index,
    /// Always there but for the instant [`Writer::settle`] waits for the
    /// merges in flight (tantivy's wait consumes the writer).
    writer: Option<IndexWriter<TantivyDocument>>,
    threads: usize,
    memory_bytes: usize,
    fields: Fields,
    reader: Reader,
    pending: usize,
    state: State,
    /// Bytes of index given back per byte of text taken, as the last shed
    /// measured it: the next shed starts from it and lands in one round.
    shed_yield: Option<f64>,
}

impl std::fmt::Debug for Writer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Writer")
            .field("dir", &self.dir)
            .field("pending", &self.pending)
            .finish_non_exhaustive()
    }
}

impl Writer {
    /// Opens (or creates) the index in `dir` for writing, with a memory budget
    /// for the indexing arena and one indexing thread.
    ///
    /// # Errors
    ///
    /// [`Why::Busy`] when another writer holds it, [`Why::Unreadable`], and
    /// [`Why::OtherFormat`] for an index this build did not write (the unit
    /// then [`Writer::wipe`]s it and builds again).
    pub fn open(dir: &Path, memory_bytes: usize) -> Result<Self, Refusal> {
        Self::open_with_threads(dir, 1, memory_bytes)
    }

    /// [`Writer::open`] with `threads` indexing threads (the bench's million
    /// documents; the unit uses one).
    ///
    /// # Errors
    ///
    /// As [`Writer::open`].
    pub fn open_with_threads(
        dir: &Path,
        threads: usize,
        memory_bytes: usize,
    ) -> Result<Self, Refusal> {
        std::fs::create_dir_all(dir).map_err(|_| Refusal {
            why: Why::Unreadable,
        })?;
        let (index, fields, state) = if dir.join(crate::META).is_file() {
            engine::open_index(dir)?
        } else {
            let (schema, fields) = Fields::schema();
            let index =
                tantivy::Index::create_in_dir(dir, schema).map_err(failed(Why::Unreadable))?;
            engine::register(&index);
            (index, fields, State::default())
        };
        let threads = threads.max(1);
        let memory_bytes = memory_bytes.max(threads * THREAD_ARENA_MIN);
        let writer = index_writer(&index, threads, memory_bytes)?;
        let reader = Reader::of(&index, fields)?;
        Ok(Self {
            dir: dir.to_path_buf(),
            index,
            writer: Some(writer),
            threads,
            memory_bytes,
            fields,
            reader,
            pending: 0,
            state,
            shed_yield: None,
        })
    }

    /// tantivy's writer.
    fn iw(&self) -> &IndexWriter<TantivyDocument> {
        self.writer
            .as_ref()
            .expect("the writer is only away inside `settle`")
    }

    /// tantivy's writer, to commit or merge.
    fn iw_mut(&mut self) -> &mut IndexWriter<TantivyDocument> {
        self.writer
            .as_mut()
            .expect("the writer is only away inside `settle`")
    }

    /// Waits for the merges in flight (by ending tantivy's writer, which is
    /// how it waits) and takes a new one: nothing is merging afterwards, so a
    /// merge asked for next cannot find its segments taken.
    fn settle(&mut self) -> Result<(), Refusal> {
        if let Some(writer) = self.writer.take() {
            writer.wait_merging_threads().map_err(failed(Why::Failed))?;
        }
        self.writer = Some(index_writer(&self.index, self.threads, self.memory_bytes)?);
        Ok(())
    }

    /// What the last commit said about the sources.
    #[must_use]
    pub const fn state(&self) -> &State {
        &self.state
    }

    /// Changes what the next commit says about the sources.
    pub fn state_mut(&mut self) -> &mut State {
        self.pending += 1;
        &mut self.state
    }

    /// Removes an index of another format from `dir`: every file tantivy
    /// wrote there (the folder is ours alone), so [`Writer::open`] starts
    /// afresh.
    ///
    /// # Errors
    ///
    /// The folder cannot be read or a file cannot be removed.
    pub fn wipe(dir: &Path) -> std::io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                std::fs::remove_file(entry.path())?;
            }
        }
        Ok(())
    }

    /// Where it is.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// How many adds and deletes wait for the next commit.
    #[must_use]
    pub const fn pending(&self) -> usize {
        self.pending
    }

    /// Adds a document (after [`Writer::delete`] of its id, to replace one).
    ///
    /// # Errors
    ///
    /// [`Why::Failed`].
    pub fn add(&mut self, document: &Document) -> Result<(), Refusal> {
        let f = self.fields;
        let mut doc = TantivyDocument::new();
        doc.add_text(f.id, &document.id);
        doc.add_u64(f.kind, document.kind.code());
        doc.add_text(f.name, &document.name);
        for also in &document.also {
            doc.add_text(f.name, also);
        }
        for folder in &document.path_terms {
            doc.add_text(f.path_terms, folder);
        }
        let body_bytes = document.body.as_ref().map_or(0, |b| b.len() as u64);
        if let Some(body) = &document.body {
            doc.add_text(f.body, body);
        }
        if let Some(note) = &document.note {
            doc.add_text(f.note, note);
        }
        if let Some(mime) = &document.mime {
            doc.add_text(f.mime, mime);
        }
        doc.add_i64(f.mtime, document.mtime_s);
        doc.add_u64(f.size, document.size_bytes);
        doc.add_u64(f.body_bytes, body_bytes);
        self.iw().add_document(doc).map_err(failed(Why::Failed))?;
        self.pending += 1;
        Ok(())
    }

    /// Replaces the document with this id (deletes it, then adds).
    ///
    /// # Errors
    ///
    /// As [`Writer::add`].
    pub fn replace(&mut self, document: &Document) -> Result<(), Refusal> {
        self.delete(&document.id)?;
        self.add(document)
    }

    /// Deletes every document with this id.
    ///
    /// # Errors
    ///
    /// None today; a `Result` so a writer that can refuse stays the same shape.
    pub fn delete(&mut self, id: &str) -> Result<(), Refusal> {
        self.iw()
            .delete_term(Term::from_field_text(self.fields.id, id));
        self.pending += 1;
        Ok(())
    }

    /// Deletes every document of a kind (a relist of the manuals or the
    /// packages starts from none).
    pub fn delete_kind(&mut self, kind: Kind) {
        self.iw()
            .delete_term(Term::from_field_u64(self.fields.kind, kind.code()));
        self.pending += 1;
    }

    /// Makes everything added and deleted visible to the readers (and writes
    /// `meta.json`, which is what their watch sees), then reads it back
    /// itself.
    ///
    /// # Errors
    ///
    /// [`Why::Failed`], [`Why::Unreadable`].
    pub fn commit(&mut self) -> Result<(), Refusal> {
        let payload =
            serde_json::to_string(&self.state).map_err(|_| Refusal { why: Why::Failed })?;
        let mut prepared = self
            .iw_mut()
            .prepare_commit()
            .map_err(failed(Why::Failed))?;
        prepared.set_payload(&payload);
        prepared.commit().map_err(failed(Why::Failed))?;
        self.pending = 0;
        self.reader.reload()?;
        self.collect();
        Ok(())
    }

    /// Removes the files no commit names any more.
    ///
    /// AFTER the reader moved on, and that order is the whole point: tantivy
    /// keeps every segment a searcher of THIS process still holds, so a
    /// collection before the reload would keep the segments the commit just
    /// replaced, and the folder would be measured with them in it. (A door in
    /// another process keeps its mapping of a removed file; the disk gives the
    /// space back when it lets go.)
    fn collect(&self) {
        if let Err(e) = self.iw().garbage_collect_files().wait() {
            tracing::warn!(error = %e, "the index could not remove its old files");
        }
    }

    /// What the last commit holds of `id`, when it holds it.
    #[must_use]
    pub fn stamp(&self, id: &str) -> Option<Stamp> {
        let searcher = self.reader.searcher();
        let term = Term::from_field_text(self.fields.id, id);
        for segment in searcher.segment_readers() {
            let Ok(inverted) = segment.inverted_index(self.fields.id) else {
                continue;
            };
            let Ok(Some(mut postings)) = inverted.read_postings(&term, IndexRecordOption::Basic)
            else {
                continue;
            };
            let fast = segment.fast_fields();
            let (Ok(kinds), Ok(mtimes), Ok(sizes), Ok(bodies)) = (
                fast.u64("kind"),
                fast.i64("mtime"),
                fast.u64("size"),
                fast.u64("body_bytes"),
            ) else {
                continue;
            };
            let mut doc = postings.doc();
            while doc != TERMINATED {
                if segment.alive_bitset().is_none_or(|a| a.is_alive(doc)) {
                    let kind = kinds.first(doc).and_then(Kind::of_code)?;
                    return Some(Stamp {
                        kind,
                        mtime_s: mtimes.first(doc).unwrap_or(0),
                        size_bytes: sizes.first(doc).unwrap_or(0),
                        body_bytes: bodies.first(doc).unwrap_or(0),
                    });
                }
                doc = postings.advance();
            }
        }
        None
    }

    /// Every id the last commit holds that starts with `prefix` (`""` is
    /// every one), in no promised order.
    ///
    /// An id is alive in one segment at most, because every add of the unit
    /// REPLACES (delete, then add): no set of the ids seen is kept, which for
    /// a million paths would be a hundred megabytes inside the unit's ceiling.
    pub fn each_id(&self, prefix: &str, mut each: impl FnMut(&str)) {
        let searcher = self.reader.searcher();
        let upper = engine::upper_bound(prefix.as_bytes());
        for segment in searcher.segment_readers() {
            let Ok(inverted) = segment.inverted_index(self.fields.id) else {
                continue;
            };
            let mut range = inverted.terms().range().ge(prefix.as_bytes());
            if let Some(upper) = &upper {
                range = range.lt(upper);
            }
            let Ok(mut stream) = range.into_stream() else {
                continue;
            };
            while stream.advance() {
                let key = stream.key();
                let Ok(id) = std::str::from_utf8(key) else {
                    continue;
                };
                // A deleted document leaves its term behind until a merge.
                let alive = match segment.alive_bitset() {
                    None => true,
                    Some(bits) => {
                        let term = Term::from_field_text(self.fields.id, id);
                        inverted
                            .read_postings(&term, IndexRecordOption::Basic)
                            .ok()
                            .flatten()
                            .is_some_and(|mut p| {
                                let mut doc = p.doc();
                                while doc != TERMINATED {
                                    if bits.is_alive(doc) {
                                        return true;
                                    }
                                    doc = p.advance();
                                }
                                false
                            })
                    }
                };
                if alive {
                    each(id);
                }
            }
        }
    }

    /// How many documents the last commit holds.
    #[must_use]
    pub fn documents(&self) -> u64 {
        self.reader.searcher().num_docs()
    }

    /// How many documents of `kind` the last commit holds.
    #[must_use]
    pub fn count_kind(&self, kind: Kind) -> u64 {
        let query = TermQuery::new(
            Term::from_field_u64(self.fields.kind, kind.code()),
            IndexRecordOption::Basic,
        );
        self.reader
            .searcher()
            .search(&query, &Count)
            .map_or(0, |n| n as u64)
    }

    /// The folder's bytes once the merges in flight have ended and their
    /// inputs are collected: what the disk holds at rest. A test's reading —
    /// mid-merge the folder is briefly both a merge's inputs and its output,
    /// and how long depends on the machine's load, not on the index.
    #[cfg(test)]
    pub(crate) fn settled_size_bytes(&mut self) -> Result<u64, Refusal> {
        self.settle()?;
        self.reader.reload()?;
        self.collect();
        Ok(self.size_bytes())
    }

    /// The bytes of the index's folder, as the disk counts them (the files
    /// tantivy wrote; a reader in another process may still map a removed
    /// one, which the disk gives back when it lets go).
    #[must_use]
    pub fn size_bytes(&self) -> u64 {
        size_of_dir(&self.dir)
    }

    /// The bytes of the files the last commit names: its segments and
    /// `meta.json`. What the ceiling is held against — a merge in flight
    /// writes its segment beside the ones it replaces, and the folder is
    /// briefly both; that is not the index growing, and a shed it started
    /// would rewrite the index for nothing. (The folder as the disk counts it
    /// is [`Writer::size_bytes`].)
    #[must_use]
    pub fn committed_bytes(&self) -> u64 {
        let Ok(metas) = self.index.searchable_segment_metas() else {
            return self.size_bytes();
        };
        let mut files: HashSet<PathBuf> = metas.iter().flat_map(|m| m.list_files()).collect();
        files.insert(PathBuf::from(crate::META));
        self.bytes_of(&files)
    }

    /// The bytes of these files of the index's folder (one gone is none).
    fn bytes_of(&self, files: &HashSet<PathBuf>) -> u64 {
        files
            .iter()
            .filter_map(|f| std::fs::metadata(self.dir.join(f)).ok())
            .map(|m| m.len())
            .sum()
    }

    /// Every segment the last commit names, with the bytes of its files.
    fn segment_bytes(&self) -> HashMap<SegmentId, u64> {
        self.index
            .searchable_segment_metas()
            .unwrap_or_default()
            .iter()
            .map(|m| (m.id(), self.bytes_of(&m.list_files())))
            .collect()
    }

    /// Takes the contents of the oldest files out until what the last commit
    /// names is at most `target_bytes`, or no file keeps a body. Each round
    /// empties the oldest bodies, commits, and merges the segments that hold
    /// deletions, so the deleted bodies leave the disk.
    ///
    /// How much text a round takes is MEASURED, not guessed: what an index
    /// gives back per byte of text depends on the text (a page of one word
    /// repeated costs almost nothing, a page of rare words costs a posting
    /// each). The first shed's first round takes twice what is over; every
    /// round after it takes what is over divided by what the last round gave
    /// back per byte, with a tenth more — and the writer keeps that number
    /// for its next shed, which then lands in one round: a round rewrites the
    /// segments it merges, most of the index.
    ///
    /// # Errors
    ///
    /// [`Why::Failed`].
    pub fn shed(&mut self, target_bytes: u64) -> Result<Shed, Refusal> {
        let mut shed = Shed {
            size_bytes: self.committed_bytes(),
            ..Shed::default()
        };
        for _ in 0..SHED_ROUNDS {
            if shed.size_bytes <= target_bytes {
                break;
            }
            let over = shed.size_bytes - target_bytes;
            #[allow(
                clippy::cast_precision_loss,
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss
            )]
            let wanted = self.shed_yield.map_or(over.saturating_mul(2), |per| {
                (over as f64 / per * 1.1).ceil() as u64
            });
            let (oldest, taken) = self.oldest_bodies(wanted.max(1))?;
            if oldest.is_empty() {
                break;
            }
            let before = self.segment_bytes();
            for document in &oldest {
                self.replace(document)?;
            }
            shed.emptied += oldest.len() as u64;
            shed.rounds += 1;
            self.commit()?;
            self.merge_deleted()?;
            // What the round rewrote: every segment it began with that is
            // gone — merged here, or by the merge policy, which merges a
            // segment on its own once a third of it is deleted.
            let after = self.segment_bytes();
            shed.rewritten_bytes += before
                .iter()
                .filter(|(id, _)| !after.contains_key(id))
                .map(|(_, bytes)| bytes)
                .sum::<u64>();
            let now = self.committed_bytes();
            let freed = shed.size_bytes.saturating_sub(now);
            #[allow(clippy::cast_precision_loss)]
            if freed > 0 && taken > 0 {
                self.shed_yield = Some(freed as f64 / taken as f64);
            }
            shed.size_bytes = now;
        }
        Ok(shed)
    }

    /// The oldest files with a body, as documents without it, until their
    /// bodies add up to `bytes` — and what they add up to.
    fn oldest_bodies(&self, bytes: u64) -> Result<(Vec<Document>, u64), Refusal> {
        let searcher = self.reader.searcher();
        let mut found: Vec<(i64, u64, tantivy::DocAddress)> = Vec::new();
        for (ord, segment) in searcher.segment_readers().iter().enumerate() {
            let fast = segment.fast_fields();
            let kinds = fast.u64("kind").map_err(failed(Why::Failed))?;
            let mtimes = fast.i64("mtime").map_err(failed(Why::Failed))?;
            let bodies = fast.u64("body_bytes").map_err(failed(Why::Failed))?;
            for doc in 0..segment.max_doc() {
                if segment.alive_bitset().is_some_and(|a| a.is_deleted(doc)) {
                    continue;
                }
                let body = bodies.first(doc).unwrap_or(0);
                if body == 0 || kinds.first(doc) != Some(Kind::File.code()) {
                    continue;
                }
                let ord = u32::try_from(ord).unwrap_or(u32::MAX);
                found.push((
                    mtimes.first(doc).unwrap_or(0),
                    body,
                    tantivy::DocAddress::new(ord, doc),
                ));
            }
        }
        found.sort_by_key(|&(mtime, _, _)| mtime);
        let f = self.fields;
        let mut out = Vec::new();
        let mut sum = 0u64;
        for (_, body, at) in found {
            if sum >= bytes {
                break;
            }
            sum += body;
            let doc: TantivyDocument = searcher.doc(at).map_err(failed(Why::Failed))?;
            let text = |field| {
                doc.get_first(field)
                    .and_then(|v| v.as_str())
                    .map(str::to_owned)
            };
            let Some(id) = text(f.id) else {
                continue;
            };
            let mut document = Document::of_path(
                Path::new(&id),
                Kind::File,
                text(f.mime).as_deref(),
                doc.get_first(f.mtime).and_then(|v| v.as_i64()).unwrap_or(0),
                doc.get_first(f.size).and_then(|v| v.as_u64()).unwrap_or(0),
            );
            if let Some(name) = text(f.name) {
                document.name = name;
            }
            out.push(document);
        }
        Ok((out, sum))
    }

    /// Merges the segments that hold deleted documents — and only those: a
    /// segment with none has nothing to give back, and merging it would be
    /// rewriting it for nothing — then waits for it (the deleted documents
    /// leave the disk).
    fn merge_deleted(&mut self) -> Result<(), Refusal> {
        // Nothing merging (a segment already in a merge cannot be asked for
        // again), and the reader on what that left.
        self.settle()?;
        self.reader.reload()?;
        let holding: Vec<SegmentId> = self
            .reader
            .searcher()
            .segment_readers()
            .iter()
            .filter(|s| s.num_deleted_docs() > 0)
            .map(tantivy::SegmentReader::segment_id)
            .collect();
        if !holding.is_empty() {
            self.iw_mut()
                .merge(&holding)
                .wait()
                .map_err(failed(Why::Failed))?;
        }
        self.reader.reload()?;
        self.collect();
        Ok(())
    }

    /// Ends the writer: waits for the merges in flight and removes the files
    /// no commit names any more (a one-shot build ends with its folder at its
    /// real size).
    ///
    /// # Errors
    ///
    /// [`Why::Failed`].
    pub fn finish(mut self) -> Result<(), Refusal> {
        // Merges end by leaving their inputs behind: a collection after them.
        self.settle()?;
        self.reader.reload()?;
        self.collect();
        Ok(())
    }
}

/// tantivy's writer over `index`, with this crate's merge policy.
fn index_writer(
    index: &tantivy::Index,
    threads: usize,
    memory_bytes: usize,
) -> Result<IndexWriter<TantivyDocument>, Refusal> {
    let writer = index
        .writer_with_num_threads(threads, memory_bytes)
        .map_err(|e| match e {
            tantivy::TantivyError::LockFailure(..) => Refusal { why: Why::Busy },
            other => failed(Why::Unreadable)(other),
        })?;
    let mut policy = LogMergePolicy::default();
    policy.set_del_docs_ratio_before_merge(DELETED_SHARE);
    writer.set_merge_policy(Box::new(policy));
    Ok(writer)
}

/// The bytes of the regular files directly in `dir` (tantivy's layout is
/// flat).
#[must_use]
pub fn size_of_dir(dir: &Path) -> u64 {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter_map(|e| e.metadata().ok())
                .filter(std::fs::Metadata::is_file)
                .map(|m| m.len())
                .sum()
        })
        .unwrap_or(0)
}
