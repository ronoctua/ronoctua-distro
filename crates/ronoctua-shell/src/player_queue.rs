//! THE QUEUE AS A TREE — the home's section, drawn from the outline the door
//! keeps (`View::outline`), and which of its groups are open.
//!
//! # Groups fold
//!
//! A named PLAYLIST holds entries, and a SET holds lanes; each is a group, its
//! row headed by a triangle — folded or open — which a click turns at once
//! (`player_rows::Cell::control`). `TAB` turns the group under the cursor,
//! `→` opens it and `←` folds it; on an entry that holds nothing the arrows
//! still move it among its siblings (`player::moved`). What a group holds is
//! set in by `view.nestCells`, a level at a time.
//!
//! Which group is open is the VIEW's, by id: as a person left it, and — for
//! one nobody touched — open while what plays is in it. What plays ENTERING a
//! group opens it; leaving one never folds it, so the row under the cursor is
//! never taken away by the queue moving on ([`View::remember_the_folds`]).
//!
//! # A set is never one file's name
//!
//! The queue words an entry's name here (`player::name_of`): a set nobody
//! named is EVERY source it plays, joined by `player-set-joiner` and cut by
//! grapheme to the row — never the name of one of its files, which would
//! hide that a commentary or a dubbing plays with it (the owner's rule).
//!
//! # Every row is its entry
//!
//! By id — `entry-{id}` — which a move keeps: the cursor, a click and "play
//! this one" never land on another entry after the queue changed shape
//! ([`View::outline_heard`]).

use crate::player::{Errand, Target, View, name_of, node_of};
use crate::player_rows::{Builder, Entry};
use ronoctua_i18n::key;
use ronoctua_player::own::{Lane, LaneKind};
use ronoctua_player::queue::{EntryId, Name, Node, NodeKind, Outline, QueueKept};
use ronoctua_player::wire::State;
use ronoctua_player::wire::{Ask, Trouble};
use ronoctua_tokens as tokens;

/// The spot of entry `id`'s row.
#[must_use]
pub fn spot_of(id: EntryId) -> String {
    format!("entry-{}", id.0)
}

/// Whether `node` is a group: a playlist, or a set.
#[must_use]
pub const fn is_group(node: &Node) -> bool {
    matches!(node.kind, NodeKind::Playlist | NodeKind::Set)
}

/// Whether `node` is `id`, or holds it.
fn holds(node: &Node, id: EntryId) -> bool {
    node.id == id || node.children.iter().any(|n| holds(n, id))
}

/// The groups `id` is in, outermost first — and `id` itself, when it is one:
/// a set that plays is open, its lanes under it.
fn groups_holding(nodes: &[Node], id: EntryId) -> Vec<EntryId> {
    for node in nodes {
        if holds(node, id) {
            let mut out = Vec::new();
            if is_group(node) {
                out.push(node.id);
            }
            out.extend(groups_holding(&node.children, id));
            return out;
        }
    }
    Vec::new()
}

/// A lane's glyph: what kind it is, first in its row.
#[must_use]
pub const fn lane_glyph(kind: LaneKind) -> char {
    match kind {
        LaneKind::Video => tokens::icon::PLAYER_LANE_PICTURE,
        LaneKind::Audio => tokens::icon::PLAYER_LANE_SOUND,
        LaneKind::Subtitle => tokens::icon::PLAYER_SUBTITLES,
    }
}

/// A lane's name: the title it was given, else its file's name — and, when
/// another of `among` (the lanes it plays with) reads the same, what KIND it
/// is after it. A film playing alone is its picture, its sound and its
/// subtitles, three lanes with one file's name: side by side on a page, three
/// rows that differ by a glyph alone were rows nobody could tell apart.
#[must_use]
pub fn lane_name(
    lane: &Lane,
    among: &[Lane],
    texts: &ronoctua_i18n::Texts,
    width_cells: u16,
) -> String {
    let own = |l: &Lane, width: u16| {
        l.title
            .as_deref()
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map_or_else(
                || name_of(&Name::Location(l.source.clone()), texts, width),
                ToOwned::to_owned,
            )
    };
    let whole = own(lane, crate::player::NAME_IN_A_SENTENCE);
    let twin = among
        .iter()
        .any(|o| o.id != lane.id && own(o, crate::player::NAME_IN_A_SENTENCE) == whole);
    if !twin {
        return own(lane, width_cells);
    }
    let kind = match lane.kind {
        LaneKind::Video => "video",
        LaneKind::Audio => "audio",
        LaneKind::Subtitle => "subtitle",
    };
    // THE KIND IS KEPT WHOLE, and the name cut for it.
    let with = |name: &str| texts.t_with(key::PLAYER_LANE_TWIN, &[("name", name), ("kind", kind)]);
    let kind_cells = ronoctua_render::layout::width(&with(""));
    with(&own(lane, width_cells.saturating_sub(kind_cells)))
}

impl View {
    /// Whether group `node` is open: as a person left it, else while `current`
    /// is in it.
    #[must_use]
    pub(crate) fn is_open(&self, node: &Node, current: Option<EntryId>) -> bool {
        self.folds
            .get(&node.id)
            .copied()
            .unwrap_or_else(|| current.is_some_and(|c| holds(node, c)))
    }

    /// What plays moved to another entry: every group it ENTERED opens — one
    /// it was already in stays as the person left it, so a playlist folded
    /// while it plays does not open again at every track — and is remembered
    /// open, so the queue moving on never folds a group away from under the
    /// cursor.
    ///
    /// A group opened ABOVE the cursor puts rows over it: the cursor follows
    /// the row it was on by its spot, or the next `RET` or `d` would act on
    /// another entry.
    pub(crate) fn remember_the_folds(
        &mut self,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) {
        let current = state.and_then(|s| s.current);
        let Some(outline) = self.outline.as_deref() else {
            return;
        };
        if current == self.last_current {
            return;
        }
        let holding =
            |id: Option<EntryId>| id.map_or_else(Vec::new, |c| groups_holding(&outline.entries, c));
        let before = holding(self.last_current);
        let entered: Vec<EntryId> = holding(current)
            .into_iter()
            .filter(|g| !before.contains(g))
            .collect();
        // THE ROW UNDER THE CURSOR as it was drawn, with what played before:
        // a group opening above it puts rows over it, and the home's windows
        // move with what plays.
        let drawn = state.map(|s| {
            let mut drawn = s.clone();
            drawn.current = self.last_current;
            drawn
        });
        let here = self.spot_here(drawn.as_ref(), columns, texts);
        self.last_current = current;
        self.home_held = None;
        for id in entered {
            self.folds.insert(id, true);
        }
        if let Some(spot) = here {
            self.follow_entry(&spot, state, columns, texts);
        }
    }

    /// The cursor on the row `spot` was on — and, when that was an entry the
    /// home's window no longer shows, on what plays: never on another entry
    /// by its place, which the next `RET`, `d` or move would act on.
    pub(crate) fn follow_entry(
        &mut self,
        spot: &str,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) {
        let gone = entry_of_spot(spot).is_some()
            && !self
                .rows(state, columns, texts)
                .iter()
                .any(|r| r.spot == spot);
        let current = state.and_then(|s| s.current).map(spot_of);
        match current.filter(|_| gone) {
            Some(playing) => self.follow(&playing, state, columns, texts),
            None => self.follow(spot, state, columns, texts),
        }
    }

    /// Group `id` folded or opened — to `to`, or the other way round — and the
    /// cursor on the row it was on; inside what folded, on the group's row.
    pub(crate) fn fold(
        &mut self,
        id: EntryId,
        to: Option<bool>,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) {
        let current = state.and_then(|s| s.current);
        let Some(open) = self
            .outline
            .as_deref()
            .and_then(|o| node_of(o, id))
            .map(|node| self.is_open(node, current))
        else {
            return;
        };
        let want = to.unwrap_or(!open);
        if want == open {
            return;
        }
        let here = self.spot_here(state, columns, texts);
        self.folds.insert(id, want);
        let rows = self.rows(state, columns, texts);
        let spot = here
            .filter(|spot| rows.iter().any(|r| &r.spot == spot))
            .unwrap_or_else(|| spot_of(id));
        self.follow(&spot, state, columns, texts);
        self.top_mut().moved = true;
    }

    /// The queue's outline, as the door's watch heard it — and the cursor on
    /// the row it was on, BY ITS SPOT: a move sends the whole outline again,
    /// and the entry under the cursor is found where it went. The ONE path
    /// both doors take ([`View::outline_arrived`] alone keeps no cursor).
    pub fn outline_heard(
        &mut self,
        outline: Option<Box<Outline>>,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) {
        let spot = self.spot_here(state, columns, texts);
        self.outline_arrived(outline);
        if let Some(spot) = spot {
            self.follow_entry(&spot, state, columns, texts);
        }
        self.settle_in(state, columns, texts);
    }
}

/// What the Queue's title says after its name — the file it is kept in —
/// and in which tone: the file's name while the queue is what it says, and
/// NOT SAVED in the danger tone while it is not, or was never saved at all.
/// Nothing while it holds nothing.
#[must_use]
pub fn kept_mark(
    kept: &QueueKept,
    texts: &ronoctua_i18n::Texts,
) -> Option<(String, ronoctua_tokens::Color)> {
    match kept {
        QueueKept::Empty => None,
        QueueKept::Never => Some((
            texts.t(key::PLAYER_QUEUE_NEW_UNSAVED),
            tokens::color::STATE_ERROR,
        )),
        QueueKept::Same { name, .. } => Some((name.clone(), tokens::color::TEXT_HEADING)),
        QueueKept::Changed { name, .. } => Some((
            texts.t_with(key::PLAYER_QUEUE_UNSAVED, &[("name", name)]),
            tokens::color::STATE_ERROR,
        )),
    }
}

/// The entries of one level of the queue the home shows — `None` for all of
/// them, when they fit — as the range of their places: the ones that hold an
/// entry of `kept` (what plays, and the one a person is moving, so a move
/// never takes the row from under the cursor) with `around` on each side
/// (`space.player.queueAround`), moved in from an end so the window is
/// always as long; with none of them on this level, the first ones. The
/// home is about what plays, and what follows it there (Listen and watch,
/// More features) is never two hundred lines below: the whole queue is its
/// own page ([`crate::player::PageId::Queue`]).
#[must_use]
pub fn home_window(
    entries: &[Node],
    kept: &[EntryId],
    around: usize,
) -> Option<std::ops::Range<usize>> {
    let most = around.saturating_mul(2).saturating_add(1);
    if entries.len() <= most {
        return None;
    }
    let held: Vec<usize> = entries
        .iter()
        .enumerate()
        .filter(|(_, n)| kept.iter().any(|k| holds(n, *k)))
        .map(|(i, _)| i)
        .collect();
    let (lo, hi) = (
        held.iter().copied().min().unwrap_or(0),
        held.iter().copied().max().unwrap_or(0),
    );
    // AS LONG AS IT HAS TO BE to hold both, never shorter than `most`.
    let len = most.max(hi - lo + 1);
    let first = lo
        .saturating_sub((len - (hi - lo + 1)) / 2)
        .min(entries.len() - len);
    Some(first..first + len)
}

/// How many entries of `nodes` play — a playlist is what it holds, every
/// other entry (a file, a stream, a set) one: what "the whole queue" says it
/// holds, where the home showed a window of a playlist that plays.
#[must_use]
pub fn plays(nodes: &[Node]) -> usize {
    nodes
        .iter()
        .map(|n| match n.kind {
            NodeKind::Playlist => plays(&n.children),
            _ => 1,
        })
        .sum()
}

/// The entry a row's spot stands for — `None` for a row that is no entry's.
#[must_use]
pub fn entry_of_spot(spot: &str) -> Option<EntryId> {
    spot.strip_prefix("entry-")?.parse().ok().map(EntryId)
}

/// THE QUEUE: its title — the file it is kept in, "not saved" in the danger
/// tone, the glyph that saves it again while the file says something else,
/// and the one that empties it — and the tree. On the HOME (`whole` false)
/// a long queue is a window of it ([`home_window`]) and a row that opens its
/// page, saying how many it holds; the page (`whole`) is every entry, titled
/// by the page's own key.
pub(crate) fn queue(
    v: &View,
    s: &State,
    outline: Option<&Outline>,
    whole: bool,
    b: &mut Builder<'_>,
) {
    let title = if whole {
        crate::player::PageId::Queue.key()
    } else {
        key::PLAYER_HEADING_QUEUE
    };
    b.title_mark(kept_mark(&s.queue_file, b.texts));
    // THE GLYPH THAT SAVES AGAIN only while the file says something else.
    if matches!(s.queue_file, QueueKept::Changed { .. }) {
        b.heading_controls(
            title,
            &[
                (
                    tokens::icon::PLAYER_SAVE_AGAIN,
                    Target::SaveQueueAgain,
                    "player-save-queue-again",
                ),
                (
                    tokens::icon::PLAYER_CLEAR_QUEUE,
                    Target::ClearQueue,
                    "player-clear",
                ),
            ],
        );
    } else {
        b.heading_with(
            title,
            tokens::icon::PLAYER_CLEAR_QUEUE,
            Target::ClearQueue,
            "player-clear",
        );
    }
    let entries: &[Node] = outline.map_or(&[], |o| o.entries.as_slice());
    if entries.is_empty() {
        b.note(key::PLAYER_QUEUE_EMPTY);
    }
    // ON THE HOME, every level a window: the top, and a playlist open under
    // it — one that plays opens by itself, and two hundred tracks of it were
    // two hundred lines.
    let kept: Vec<EntryId> = s.current.into_iter().chain(v.home_held).collect();
    let windowed = level(
        v,
        entries,
        s,
        (0, ""),
        (!whole).then_some(kept.as_slice()),
        b,
    );
    // THE ROW THAT OPENS THE WHOLE QUEUE, under its window, with how many it
    // holds: "Queue · N".
    if windowed {
        let count = b.texts.t_args(
            key::PLAYER_QUEUE_ENTRIES,
            &[(
                "count",
                ronoctua_i18n::Arg::Number(i64::try_from(plays(entries)).unwrap_or(i64::MAX)),
            )],
        );
        let page = crate::player::PageId::Queue;
        b.pair(b.t(page.key()), count, Target::Go(page), page.spot());
    }
}

/// Entry `id`'s number in the queue as its row says it — `3.2.` is the
/// second of the third — without the last dot, as a sentence says it.
#[must_use]
pub fn number_of(outline: &Outline, id: EntryId) -> Option<String> {
    fn walk(nodes: &[Node], id: EntryId, prefix: &str) -> Option<String> {
        for (node, n) in nodes.iter().zip(1..) {
            let number = format!("{prefix}{n}.");
            if node.id == id {
                return Some(number);
            }
            if let Some(found) = walk(&node.children, id, &number) {
                return Some(found);
            }
        }
        None
    }
    walk(&outline.entries, id, "").map(|n| n.trim_end_matches('.').to_owned())
}

impl View {
    /// The question a refusal the person may overrule is asked with, and
    /// what its "yes" sends — `again`, said as the order it is: the
    /// queue's "already there, add it again?" (decision A5) and "the file
    /// changed on the disk, write over it?". `None` for a refusal that is
    /// none of those.
    pub(crate) fn overrule(
        &self,
        again: &Ask,
        trouble: &Trouble,
        texts: &ronoctua_i18n::Texts,
    ) -> Option<(String, Errand)> {
        match trouble {
            Trouble::AlreadyQueued { entry, name } => {
                let name = name_of(name, texts, crate::player::NAME_IN_A_SENTENCE);
                let number = self.outline.as_deref().and_then(|o| number_of(o, *entry));
                let sentence = match &number {
                    Some(number) => texts.t_with(
                        key::ASK_PLAYER_ALREADY_QUEUED,
                        &[("name", name.as_str()), ("number", number)],
                    ),
                    None => texts.t_with(
                        key::ASK_PLAYER_ALREADY_QUEUED_SOMEWHERE,
                        &[("name", name.as_str())],
                    ),
                };
                let play = matches!(again, Ask::Put { play: true, .. });
                let said = texts.t_with(
                    if play {
                        key::PLAYER_PLAYING_NOW
                    } else {
                        key::PLAYER_ADDED
                    },
                    &[("name", &name)],
                );
                Some((sentence, Errand::TellSaying(again.clone(), said)))
            }
            Trouble::ChangedOnDisk { path } => {
                let file = std::path::Path::new(path)
                    .file_name()
                    .map_or_else(|| path.clone(), |f| f.to_string_lossy().into_owned());
                Some((
                    texts.t_with(key::ASK_PLAYER_QUEUE_CHANGED_ON_DISK, &[("path", path)]),
                    Errand::TellSaying(
                        again.clone(),
                        texts.t_with(key::PLAYER_SAVED_AGAIN, &[("name", &file)]),
                    ),
                ))
            }
            _ => None,
        }
    }

    /// `C-s` and the glyph on the Queue's title: the queue SAVED AGAIN into
    /// the file it was opened from or saved as — refused with a question
    /// when that file changed on the disk since (`Told::Asks`). A queue
    /// never saved asks its name and folder, as "Save the queue…" does; an
    /// empty one says so.
    pub(crate) fn save_again(
        &mut self,
        state: Option<&State>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        let Some(s) = state else {
            return Errand::Said(texts.t(key::PLAYER_NOT_RUNNING));
        };
        match &s.queue_file {
            QueueKept::Empty => Errand::Said(texts.t(key::PLAYER_QUEUE_EMPTY)),
            QueueKept::Never => self.save(crate::player::Saved::Queue, state, columns, texts),
            QueueKept::Same { path, .. } | QueueKept::Changed { path, .. } => {
                let file = std::path::Path::new(path)
                    .file_name()
                    .map_or_else(|| path.clone(), |f| f.to_string_lossy().into_owned());
                Errand::TellSaying(
                    Ask::SaveQueueAgain { force: false },
                    texts.t_with(key::PLAYER_SAVED_AGAIN, &[("name", &file)]),
                )
            }
        }
    }
}

/// The rows of one level of the tree, `depth` levels in — each numbered
/// after `prefix`, its group's own number, so `3.2.` reads as the second of
/// the third and never as another `2.` of the top — and of what each open
/// group under it holds. On the home (`kept`, the entries a window keeps in
/// sight) each level is a window of itself ([`home_window`]); whether any
/// was is the answer.
fn level(
    v: &View,
    nodes: &[Node],
    s: &State,
    (depth, prefix): (u16, &str),
    kept: Option<&[EntryId]>,
    b: &mut Builder<'_>,
) -> bool {
    let window =
        kept.and_then(|k| home_window(nodes, k, usize::from(tokens::space::PLAYER_QUEUEAROUND)));
    let mut windowed = window.is_some();
    let shown = window.unwrap_or(0..nodes.len());
    // THE WIDEST NUMBER OF WHAT IS SHOWN: a window's names line up with each
    // other, and every entry keeps its number in the whole queue.
    let widest = format!("{prefix}{}.", shown.end.min(nodes.len()));
    let number_cells =
        ronoctua_protocol::cells(&widest).saturating_add(tokens::space::PLAYER_QUEUEAIR);
    // THE TRIANGLE'S ROOM on every level but a flat queue's top: under a
    // group, the names begin a step further in than the group's own, even on
    // a level of no groups.
    let glyph_room = depth > 0 || nodes.iter().any(is_group);
    let indent = depth.saturating_mul(tokens::space::VIEW_NESTCELLS);
    let head = number_cells.saturating_add(if glyph_room {
        tokens::space::PLAYER_QUEUEGLYPH
    } else {
        0
    });
    let room = b.room.saturating_sub(indent.saturating_add(head));
    let last = nodes.len().saturating_sub(1);
    for (i, (node, n)) in nodes.iter().zip(1..).enumerate() {
        if !shown.contains(&i) {
            continue;
        }
        let group = is_group(node);
        let open = group && v.is_open(node, s.current);
        let number = format!("{prefix}{n}.");
        let name = name_of(&node.name, b.texts, room);
        let name = if s.resolving.contains(&node.id) {
            b.texts
                .t_with(key::PLAYER_RESOLVING, &[("name", name.as_str())])
        } else {
            name
        };
        let move_to =
            |later: bool, glyph: char| Some((glyph, Target::EntryMove { id: node.id, later }));
        b.entry(Entry {
            depth,
            number: (number.clone(), number_cells),
            fold: group.then_some((open, Target::Fold(node.id))),
            mark: None,
            glyph_room,
            name,
            // WHAT PLAYS, and a group FOLDED over it: the row that stands for
            // it while it is out of sight.
            current: s
                .current
                .is_some_and(|c| c == node.id || (group && !open && holds(node, c))),
            target: if group {
                Target::Group(node.id)
            } else {
                Target::Entry(node.id)
            },
            spot: spot_of(node.id),
            moves: [
                if i > 0 {
                    move_to(false, tokens::icon::PLAYER_MOVE_EARLIER)
                } else {
                    None
                },
                if i < last {
                    move_to(true, tokens::icon::PLAYER_MOVE_LATER)
                } else {
                    None
                },
            ],
        });
        if !open {
            continue;
        }
        match node.kind {
            NodeKind::Playlist => {
                windowed |= level(v, &node.children, s, (depth + 1, &number), kept, b);
            }
            NodeKind::Set => lanes(node, s, depth + 1, b),
            NodeKind::Single | NodeKind::Stream => {}
        }
    }
    windowed
}

/// What an open SET holds: its lanes, each by its kind's glyph and its name,
/// when it is what plays — the state carries the lanes of what plays and of
/// nothing else — and otherwise how many it has.
fn lanes(node: &Node, s: &State, depth: u16, b: &mut Builder<'_>) {
    let indent = depth.saturating_mul(tokens::space::VIEW_NESTCELLS);
    if s.current != Some(node.id) {
        let said = b.texts.t_args(
            key::PLAYER_TOGETHER_LANES,
            &[("count", ronoctua_i18n::Arg::Number(i64::from(node.lanes)))],
        );
        let at = b.left.saturating_add(indent);
        b.inert(vec![Builder::piece(
            at,
            said,
            tokens::color::TEXT_SECONDARY,
        )]);
        return;
    }
    // A LANE IS NO ENTRY: no number, and its kind's glyph where an entry's
    // triangle would be — its name in the column its siblings' would be.
    let number_cells = ronoctua_protocol::cells(&format!("{}.", s.lanes.len()))
        .saturating_add(tokens::space::PLAYER_QUEUEAIR);
    let room = b.room.saturating_sub(
        indent
            .saturating_add(number_cells)
            .saturating_add(tokens::space::PLAYER_QUEUEGLYPH),
    );
    for lane in &s.lanes {
        b.entry(Entry {
            depth,
            number: (String::new(), number_cells),
            fold: None,
            mark: Some(lane_glyph(lane.kind)),
            glyph_room: true,
            name: lane_name(lane, &s.lanes, b.texts, room),
            current: false,
            target: Target::Lane(lane.id),
            spot: format!("queue-lane-{}", lane.id.0),
            moves: [None, None],
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::fixture::{alone, track};
    use crate::player::{Errand, Press};
    use crate::player_rows::Row;
    use ronoctua_player::own::LaneId;
    use ronoctua_player::wire::{Ask, TrackKind as Kind};

    fn texts(language: &str) -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh(language).expect("a language")
    }

    fn node(id: u64, name: Name, kind: NodeKind, children: Vec<Node>, lanes: u16) -> Node {
        Node {
            id: EntryId(id),
            name,
            kind,
            children,
            lanes,
        }
    }

    fn single(id: u64, title: &str) -> Node {
        node(
            id,
            Name::Titled(title.into()),
            NodeKind::Single,
            Vec::new(),
            0,
        )
    }

    /// A radio, a film, a playlist of three, and a set nobody named of a
    /// film, a commentary and a subtitle.
    fn the_queue() -> Box<Outline> {
        Box::new(Outline {
            revision: 3,
            entries: vec![
                single(1, "Morning radio"),
                single(2, "The Lighthouse Keeper"),
                node(
                    3,
                    Name::Titled("Road trip".into()),
                    NodeKind::Playlist,
                    vec![
                        node(
                            4,
                            Name::Location("/music/road/01 - Open Road.flac".into()),
                            NodeKind::Single,
                            Vec::new(),
                            0,
                        ),
                        single(5, "Tides of March"),
                        single(6, "Night Drive"),
                    ],
                    0,
                ),
                node(
                    7,
                    Name::Together(vec![
                        "Harbour film".into(),
                        "Director's commentary".into(),
                        "harbour.pt-BR.srt".into(),
                    ]),
                    NodeKind::Set,
                    Vec::new(),
                    3,
                ),
            ],
        })
    }

    /// What plays: the film alone, as the queue's entry `current`.
    fn playing(current: u64) -> State {
        let mut s = alone(
            "/films/harbour.mkv",
            vec![
                track(1, Kind::Video, None, true),
                track(1, Kind::Audio, Some("Stereo"), true),
            ],
        );
        s.current = Some(EntryId(current));
        s
    }

    fn queued(current: Option<u64>) -> (View, State) {
        let mut v = View::blank(None);
        let mut s = playing(current.unwrap_or(2));
        s.current = current.map(EntryId);
        v.outline_arrived(Some(the_queue()));
        (v, s)
    }

    fn row_of<'a>(rows: &'a [Row], target: &Target) -> Option<(usize, &'a Row)> {
        rows.iter().enumerate().find(|(_, r)| &r.target == target)
    }

    fn name_on(row: &Row) -> String {
        row.fields
            .last()
            .map(|f| f.text.clone())
            .unwrap_or_default()
    }

    /// The cursor's entry, by the row it is on.
    fn under_cursor(v: &View, s: &State, t: &ronoctua_i18n::Texts) -> Target {
        v.rows(Some(s), 80, t)[v.cursor()].target.clone()
    }

    /// FAILS IF: an entry moved out of the home's window by `C-↓` leaves the
    /// rows — the cursor falling back to its row's PLACE, on another entry,
    /// which the next `C-↓` would move instead — or the window stops showing
    /// what plays while it keeps the moved one in sight.
    #[test]
    fn a_move_at_the_edge_of_the_home_s_window_keeps_its_entry() {
        let t = texts("en-GB");
        let order = |ids: &[u64]| {
            Box::new(Outline {
                revision: 1,
                entries: ids
                    .iter()
                    .map(|i| single(*i, &format!("Track {i}")))
                    .collect(),
            })
        };
        let mut ids: Vec<u64> = (1..=200).collect();
        let mut v = View::blank(None);
        v.outline_arrived(Some(order(&ids)));
        let s = playing(100);
        v.settle_in(Some(&s), 80, &t);
        v.follow(&spot_of(EntryId(103)), Some(&s), 80, &t);
        assert_eq!(under_cursor(&v, &s, &t), Target::Entry(EntryId(103)));
        for step in 0..2_usize {
            let Errand::Tell(Ask::Move { id, after, .. }) = v.key(Press::Later, Some(&s), 80, &t)
            else {
                panic!("not moved");
            };
            assert_eq!(id, EntryId(103), "step {step}: the same entry moves");
            // THE PLAYER MOVES IT, and the whole outline comes again.
            let at = ids.iter().position(|i| *i == 103).expect("there");
            ids.remove(at);
            let after = after.expect("after one").0;
            let to = ids.iter().position(|i| *i == after).expect("there") + 1;
            ids.insert(to, 103);
            v.outline_heard(Some(order(&ids)), Some(&s), 80, &t);
            assert_eq!(
                under_cursor(&v, &s, &t),
                Target::Entry(EntryId(103)),
                "step {step}: the cursor stays on what it moved"
            );
            let rows = v.rows(Some(&s), 80, &t);
            assert!(row_of(&rows, &Target::Entry(EntryId(100))).is_some());
        }
        // WHAT PLAYS MOVES ON, far away (shuffle): the window goes with it,
        // and the cursor whose entry it left lands on what plays — never on
        // another entry by its place.
        let far = playing(150);
        v.settle_in(Some(&far), 80, &t);
        assert_eq!(under_cursor(&v, &far, &t), Target::Entry(EntryId(150)));
        let rows = v.rows(Some(&far), 80, &t);
        assert!(row_of(&rows, &Target::Entry(EntryId(103))).is_none());
    }

    /// FAILS IF: a playlist that plays — open by itself on the home — lists
    /// every track there, pushing what follows the queue two hundred lines
    /// down, instead of a window of it around what plays and the row that
    /// opens the whole queue.
    #[test]
    fn a_long_playlist_that_plays_is_a_window_on_the_home() {
        use crate::player::PageId;
        let t = texts("en-GB");
        let tracks: Vec<Node> = (1..=200)
            .map(|i| single(i, &format!("Track {i}")))
            .collect();
        let list = node(
            1000,
            Name::Titled("Long".into()),
            NodeKind::Playlist,
            tracks,
            0,
        );
        let mut v = View::blank(None);
        v.outline_arrived(Some(Box::new(Outline {
            revision: 1,
            entries: vec![single(2000, "Before"), list, single(3000, "After")],
        })));
        let s = playing(150);
        let rows = v.rows(Some(&s), 80, &t);
        let shown: Vec<u64> = rows
            .iter()
            .filter_map(|r| match r.target {
                Target::Entry(id) if id.0 <= 200 => Some(id.0),
                _ => None,
            })
            .collect();
        assert_eq!(shown, (147..=153).collect::<Vec<_>>());
        let (_, at) = row_of(&rows, &Target::Entry(EntryId(147))).expect("its row");
        assert!(
            at.fields.iter().any(|f| f.text == "2.147."),
            "its number in the whole queue: {:?}",
            at.fields
        );
        let (_, whole) = row_of(&rows, &Target::Go(PageId::Queue)).expect("Queue · N");
        assert_eq!(
            whole.fields[1].text, "202 entries",
            "what plays, not the top's three"
        );
        assert!(rows.len() < 60, "{} rows", rows.len());
        // THE PAGE is every track.
        v.key(Press::Go(PageId::Queue), Some(&s), 80, &t);
        let rows = v.rows(Some(&s), 80, &t);
        assert_eq!(
            rows.iter()
                .filter(|r| matches!(r.target, Target::Entry(id) if id.0 <= 200))
                .count(),
            200
        );
    }

    /// FAILS IF: the home lists every entry of a long queue — two hundred
    /// loose files pushing "Listen and watch" two hundred lines down — instead
    /// of a window around what plays and a row that opens the whole queue
    /// saying how many it holds; or the window renumbers its entries, or
    /// the whole queue's page leaves one out.
    #[test]
    fn the_home_shows_a_window_of_a_long_queue() {
        use crate::player::{PageId, Press};
        let t = texts("en-GB");
        let long = |n: u64| {
            Box::new(Outline {
                revision: 1,
                entries: (1..=n).map(|i| single(i, &format!("Track {i}"))).collect(),
            })
        };
        let entries = |rows: &[Row]| -> Vec<u64> {
            rows.iter()
                .filter_map(|r| match r.target {
                    Target::Entry(id) => Some(id.0),
                    _ => None,
                })
                .collect()
        };
        // A SHORT QUEUE is whole on the home, with no row to open it.
        let mut v = View::blank(None);
        v.outline_arrived(Some(long(7)));
        let rows = v.rows(Some(&playing(3)), 80, &t);
        assert_eq!(entries(&rows), (1..=7).collect::<Vec<_>>());
        assert!(row_of(&rows, &Target::Go(PageId::Queue)).is_none());
        // A LONG ONE: what plays with three on each side, moved in at an end.
        v.outline_arrived(Some(long(200)));
        for (current, first) in [(100, 97), (1, 1), (2, 1), (200, 194), (199, 194)] {
            let rows = v.rows(Some(&playing(current)), 80, &t);
            let shown = entries(&rows);
            assert_eq!(
                shown,
                (first..first + 7).collect::<Vec<_>>(),
                "playing {current}"
            );
            // EACH KEEPS ITS NUMBER in the whole queue.
            let (_, row) = row_of(&rows, &Target::Entry(EntryId(first))).expect("its row");
            assert!(
                row.fields.iter().any(|f| f.text == format!("{first}.")),
                "{:?}",
                row.fields
            );
            let (_, whole) = row_of(&rows, &Target::Go(PageId::Queue)).expect("Queue · N");
            assert_eq!(whole.fields[0].text, t.t(key::PLAYER_PAGE_QUEUE));
            assert_eq!(whole.fields[1].text, "200 entries");
            // AND WHAT FOLLOWS IS NEAR: the window and its row, then the
            // next section.
            assert!(rows.len() < 60, "{} rows", rows.len());
        }
        // NOTHING PLAYING: the first ones.
        let mut s = playing(1);
        s.current = None;
        assert_eq!(
            entries(&v.rows(Some(&s), 80, &t)),
            (1..=7).collect::<Vec<_>>()
        );
        // THE ROW OPENS THE WHOLE QUEUE, every entry, titled by its key.
        let s = playing(100);
        v.key(Press::Go(PageId::Queue), Some(&s), 80, &t);
        assert_eq!(v.page(), PageId::Queue);
        let rows = v.rows(Some(&s), 80, &t);
        assert_eq!(entries(&rows), (1..=200).collect::<Vec<_>>());
        assert_eq!(
            rows[v.cursor()].target,
            Target::Entry(EntryId(100)),
            "it opens on what plays"
        );
        assert!(
            rows.iter().any(|r| r.heading
                && r.fields
                    .iter()
                    .any(|f| f.text == t.t(key::PLAYER_PAGE_QUEUE))),
            "titled by its own key"
        );
        assert!(row_of(&rows, &Target::Go(PageId::Queue)).is_none());
        // IN PORTUGUESE too.
        let pt = texts("pt-BR");
        let mut v = View::blank(None);
        v.outline_arrived(Some(long(200)));
        let rows = v.rows(Some(&playing(100)), 80, &pt);
        let (_, whole) = row_of(&rows, &Target::Go(PageId::Queue)).expect("Fila · N");
        let count = pt.t_args(
            key::PLAYER_QUEUE_ENTRIES,
            &[("count", ronoctua_i18n::Arg::Number(200))],
        );
        assert_eq!(whole.fields[1].text, count);
        assert_ne!(
            count,
            t.t_args(
                key::PLAYER_QUEUE_ENTRIES,
                &[("count", ronoctua_i18n::Arg::Number(200))]
            )
        );
    }

    /// FAILS IF: a set nobody named is drawn under the name of ONE of its
    /// files — its film, say — in either language, whole or cut to a narrow
    /// door: it is every source it plays, joined, and a cut ends in `…`, by
    /// grapheme, never on a lone source's name; or what plays says one
    /// file's artist in front of it.
    #[test]
    fn a_set_is_never_named_after_one_file() {
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            let joiner = t.t(key::PLAYER_SET_JOINER);
            let (v, s) = queued(Some(2));
            for columns in [120, 40, 26] {
                let rows = v.rows(Some(&s), columns, &t);
                let (_, set) = row_of(&rows, &Target::Group(EntryId(7))).expect("the set is a row");
                let name = name_on(set);
                for one in ["Harbour film", "Director's commentary", "harbour.pt-BR.srt"] {
                    assert_ne!(name, one, "{language} at {columns}: one file's name");
                }
                assert!(
                    name.contains(joiner.trim()),
                    "{language} at {columns}: no joiner in {name:?}"
                );
                if columns == 120 {
                    assert_eq!(
                        name,
                        ["Harbour film", "Director's commentary", "harbour.pt-BR.srt"]
                            .join(&joiner)
                    );
                } else {
                    assert!(name.ends_with('…'), "{language} at {columns}: {name:?}");
                }
            }
            // AND WHAT PLAYS says the same, whole — with no one file's artist
            // in front of it: the tag the player read is its film's.
            let mut s = playing(7);
            s.title = Some("Harbour film".into());
            s.artist = Some("Marta Quintela".into());
            let said = crate::player::now_playing(&s, v.outline.as_deref(), &t).expect("a name");
            assert_eq!(
                said,
                ["Harbour film", "Director's commentary", "harbour.pt-BR.srt"].join(&joiner)
            );
        }
    }

    /// FAILS IF: a group of the queue stops folding and opening by `TAB`, by
    /// its triangle — a click that acts at once — or by `→` and `←`; or what
    /// it holds stops being set in under it; or the cursor is left on a row
    /// that folded away instead of on the group's own.
    #[test]
    fn a_group_folds_by_tab_and_by_its_glyph() {
        let t = texts("en-GB");
        let (mut v, s) = queued(Some(2));
        v.settle_in(Some(&s), 80, &t);
        let children = |v: &View| {
            v.rows(Some(&s), 80, &t)
                .iter()
                .filter(|r| matches!(r.target, Target::Entry(EntryId(id)) if (4..=6).contains(&id)))
                .count()
        };
        assert_eq!(
            children(&v),
            0,
            "a playlist nothing plays in is born folded"
        );
        // `TAB` on its row opens it, and its entries are set in under it.
        let rows = v.rows(Some(&s), 80, &t);
        let (at, _) = row_of(&rows, &Target::Group(EntryId(3))).expect("the playlist");
        v.clicked(at, &rows);
        assert_eq!(crate::player::pressed("TAB"), Some(Press::Fold));
        assert_eq!(v.key(Press::Fold, Some(&s), 80, &t), Errand::Nothing);
        assert_eq!(children(&v), 3);
        let rows = v.rows(Some(&s), 80, &t);
        let (_, group) = row_of(&rows, &Target::Group(EntryId(3))).expect("the playlist");
        let (_, child) = row_of(&rows, &Target::Entry(EntryId(5))).expect("a child");
        // THE NAME, and not a glyph at the row's end: the cursor's row
        // carries the arrows that move it.
        let name_at = |r: &Row| {
            r.fields
                .iter()
                .find(|f| f.text.chars().count() > 2 && !f.text.starts_with(char::is_numeric))
                .map_or(0, |f| f.at)
        };
        let number_at = |r: &Row| r.fields.first().map_or(0, |f| f.at);
        assert_eq!(
            number_at(child),
            number_at(group) + tokens::space::VIEW_NESTCELLS,
            "a child is not set in by one level"
        );
        assert!(name_at(child) > name_at(group), "a child's name not set in");
        assert_eq!(
            group.fields[1].text,
            tokens::icon::PLAYER_UNFOLDED.to_string(),
            "the open triangle"
        );
        // `TAB` again folds it; `→` opens it and `←` folds it — and
        // `C-↑`/`C-↓` move an entry, a group too.
        v.key(Press::Fold, Some(&s), 80, &t);
        assert_eq!(children(&v), 0);
        v.key(Press::More, Some(&s), 80, &t);
        assert_eq!(children(&v), 3);
        v.key(Press::More, Some(&s), 80, &t);
        assert_eq!(children(&v), 3, "`→` on an open group leaves it open");
        v.key(Press::Less, Some(&s), 80, &t);
        assert_eq!(children(&v), 0);
        // ITS TRIANGLE, clicked: at once, not on a second click.
        let rows = v.rows(Some(&s), 80, &t);
        let (at, group) = row_of(&rows, &Target::Group(EntryId(3))).expect("the playlist");
        let cell = group.cells.first().expect("the triangle is a control");
        assert_eq!(cell.target, Target::Fold(EntryId(3)));
        assert!(
            v.clicked_at(at, cell.from, &rows),
            "the triangle acts at once"
        );
        assert_eq!(v.key(Press::Enter, Some(&s), 80, &t), Errand::Nothing);
        assert_eq!(children(&v), 3);
        // THE CURSOR INSIDE, and the group folded by its triangle: the cursor
        // comes up to the group's row.
        let rows = v.rows(Some(&s), 80, &t);
        let (inside, _) = row_of(&rows, &Target::Entry(EntryId(6))).expect("a child");
        v.clicked(inside, &rows);
        assert_eq!(
            v.key(Press::Earlier, Some(&s), 80, &t),
            Errand::Tell(Ask::Move {
                id: EntryId(6),
                after: Some(EntryId(4)),
                into: None
            }),
            "`C-↑` moves an entry that holds nothing"
        );
        assert_eq!(
            v.key(Press::Less, Some(&s), 80, &t),
            Errand::Nothing,
            "`←` on an entry is no move any more: `C-↑`/`C-↓` are"
        );
        let rows = v.rows(Some(&s), 80, &t);
        let (at, group) = row_of(&rows, &Target::Group(EntryId(3))).expect("the playlist");
        assert!(v.clicked_at(at, group.cells[0].from, &rows));
        v.key(Press::Enter, Some(&s), 80, &t);
        assert_eq!(children(&v), 0);
        let rows = v.rows(Some(&s), 80, &t);
        assert_eq!(rows[v.cursor()].target, Target::Group(EntryId(3)));
        // AND A SECOND CLICK ON THE GROUP'S NAME plays it.
        let name = group.fields.last().map_or(0, |f| f.at);
        v.clicked_at(at, name, &rows);
        assert!(
            v.clicked_at(at, name, &rows),
            "the second click on the name"
        );
        assert_eq!(
            v.key(Press::Enter, Some(&s), 80, &t),
            Errand::Tell(Ask::Jump { id: EntryId(3) })
        );
    }

    /// FAILS IF: what plays entering a group stops opening it, or leaving it
    /// folds it again — the row under the cursor taken away by the queue
    /// moving on.
    #[test]
    fn what_plays_opens_its_group_and_leaving_never_folds_it() {
        let t = texts("en-GB");
        let (mut v, _) = queued(Some(2));
        let mut s = playing(5);
        v.settle_in(Some(&s), 80, &t);
        let shows = |v: &View, s: &State, id: u64| {
            row_of(&v.rows(Some(s), 80, &t), &Target::Entry(EntryId(id))).is_some()
        };
        assert!(shows(&v, &s, 5), "the group what plays is in is open");
        s.current = Some(EntryId(1));
        v.settle_in(Some(&s), 80, &t);
        assert!(shows(&v, &s, 5), "the queue moving on folded the group");
    }

    /// FAILS IF: a group a person folded, opened because what plays entered
    /// it ABOVE the cursor, leaves the cursor's index where it was — on
    /// another entry, which the next `RET` or `d` would play or remove — or a
    /// playlist folded WHILE it plays opens again at its next track: only
    /// entering a group opens it.
    #[test]
    fn a_group_opened_above_the_cursor_keeps_it_on_its_entry() {
        let t = texts("en-GB");
        let (mut v, mut s) = queued(Some(1));
        v.settle_in(Some(&s), 80, &t);
        // FOLDED BY A PERSON — opened, and folded again: a group nobody
        // touched opens by itself while what plays is in it.
        v.fold(EntryId(3), Some(true), Some(&s), 80, &t);
        v.fold(EntryId(3), Some(false), Some(&s), 80, &t);
        assert_eq!(v.folds.get(&EntryId(3)), Some(&false));
        let rows = v.rows(Some(&s), 80, &t);
        let (at, _) = row_of(&rows, &Target::Group(EntryId(7))).expect("the set");
        v.clicked(at, &rows);
        // A STATE ARRIVES, as a door hands it over: the spot followed, then
        // the rows settled.
        let arrives = |v: &mut View, s: &mut State, current: u64| {
            let spot = v.spot_here(Some(s), 80, &t).expect("a spot");
            s.current = Some(EntryId(current));
            v.follow(&spot, Some(s), 80, &t);
            v.settle_in(Some(s), 80, &t);
        };
        arrives(&mut v, &mut s, 5);
        let rows = v.rows(Some(&s), 80, &t);
        assert!(
            row_of(&rows, &Target::Entry(EntryId(5))).is_some(),
            "what plays entered the folded playlist, which opens"
        );
        assert_eq!(
            rows[v.cursor()].target,
            Target::Group(EntryId(7)),
            "the rows opened above the cursor took it to another entry"
        );
        // FOLDED WHILE IT PLAYS: the next track of it keeps it folded.
        v.fold(EntryId(3), Some(false), Some(&s), 80, &t);
        arrives(&mut v, &mut s, 6);
        let rows = v.rows(Some(&s), 80, &t);
        assert!(
            row_of(&rows, &Target::Entry(EntryId(6))).is_none(),
            "a playlist folded while it plays opened again at its next track"
        );
        assert_eq!(rows[v.cursor()].target, Target::Group(EntryId(7)));
    }

    /// FAILS IF: a GROUP of the queue — a playlist, a set — cannot be moved
    /// by the keyboard: `C-↑`/`C-↓` on its row move it among its siblings, as
    /// they move a file, read by their whole notation in every door (a plain
    /// `↓` still walks down); or `C-↑` is not taught on an entry's row.
    #[test]
    fn a_group_moves_by_the_held_arrows() {
        use crate::player::{press_of_key, pressed};
        let key = |n: &str| ronoctua_keymap::Key::parse(n).expect(n);
        assert_eq!(press_of_key(key("C-<up>")), Some(Press::Earlier));
        assert_eq!(press_of_key(key("C-<down>")), Some(Press::Later));
        assert_eq!(press_of_key(key("<down>")), Some(Press::Down));
        assert_eq!(
            press_of_key(key("M-<down>")),
            Some(Press::Down),
            "an arrow held with a modifier the table does not name is the arrow"
        );
        assert_eq!(pressed("C-<up>"), Some(Press::Earlier));
        let t = texts("en-GB");
        let (mut v, s) = queued(Some(1));
        v.settle_in(Some(&s), 80, &t);
        let rows = v.rows(Some(&s), 80, &t);
        let (at, _) = row_of(&rows, &Target::Group(EntryId(3))).expect("the playlist");
        v.clicked(at, &rows);
        assert_eq!(
            v.key(Press::Later, Some(&s), 80, &t),
            Errand::Tell(Ask::Move {
                id: EntryId(3),
                after: Some(EntryId(7)),
                into: None
            }),
            "`C-↓` on a playlist moves it after the set"
        );
        assert_eq!(
            v.key(Press::Earlier, Some(&s), 80, &t),
            Errand::Tell(Ask::Move {
                id: EntryId(3),
                after: Some(EntryId(1)),
                into: None
            }),
            "`C-↑` on a playlist moves it before the film"
        );
        let taught: Vec<String> = v
            .hint(Some(&s), 80, &t)
            .iter()
            .map(|p| p.text.clone())
            .collect();
        assert!(
            taught.iter().any(|k| k.contains("C-")),
            "the held arrows are not taught on a group's row: {taught:?}"
        );
    }

    /// FAILS IF: the entry under the cursor is lost when the queue changes
    /// shape — a move sends the whole outline again, and the cursor stays on
    /// the entry, wherever it went, never on the row that took its place.
    #[test]
    fn the_cursor_keeps_the_id_across_a_reorder() {
        let t = texts("en-GB");
        let (mut v, s) = queued(Some(1));
        v.settle_in(Some(&s), 80, &t);
        let rows = v.rows(Some(&s), 80, &t);
        let (at, _) = row_of(&rows, &Target::Entry(EntryId(2))).expect("the film");
        v.clicked(at, &rows);
        let Errand::Tell(Ask::Move { .. }) = v.key(Press::Earlier, Some(&s), 80, &t) else {
            panic!("`C-↑` moves an entry");
        };
        // THE PLAYER MOVED IT FIRST: its outline, whole, again.
        let mut moved = the_queue();
        moved.revision = 4;
        moved.entries.swap(0, 1);
        v.outline_heard(Some(moved), Some(&s), 80, &t);
        let rows = v.rows(Some(&s), 80, &t);
        assert_eq!(rows[v.cursor()].target, Target::Entry(EntryId(2)));
        assert_eq!(
            v.cursor(),
            at - 1,
            "the entry went up one, and the cursor with it"
        );
    }

    /// FAILS IF: the set that plays stops showing its lanes under it — each
    /// by its kind's glyph and its name, `d` taking it off — or a set that
    /// does not play, whose lanes the state does not carry, stops saying how
    /// many it has.
    #[test]
    fn a_set_holds_its_lanes() {
        let t = texts("en-GB");
        let (mut v, mut s) = queued(Some(7));
        let mut commentary = Lane::new(LaneId(4), LaneKind::Audio, "/films/commentary.flac");
        commentary.title = Some("Director's commentary".into());
        s.lanes.push(commentary);
        v.settle_in(Some(&s), 80, &t);
        let rows = v.rows(Some(&s), 80, &t);
        let lanes: Vec<&Row> = rows
            .iter()
            .filter(|r| matches!(r.target, Target::Lane(_)))
            .collect();
        assert_eq!(lanes.len(), 3, "{rows:#?}");
        assert_eq!(
            lanes[2].fields[1].text,
            tokens::icon::PLAYER_LANE_SOUND.to_string()
        );
        assert_eq!(name_on(lanes[2]), "Director's commentary");
        let (at, _) = row_of(&rows, &Target::Lane(LaneId(4))).expect("the commentary");
        v.clicked(at, &rows);
        assert_eq!(
            v.key(Press::Remove, Some(&s), 80, &t),
            Errand::Tell(Ask::Lane {
                lane: LaneId(4),
                change: ronoctua_player::wire::LaneChange::Remove,
            })
        );
        // ANOTHER SET, opened while something else plays: how many lanes.
        let (mut v, s) = queued(Some(1));
        v.settle_in(Some(&s), 80, &t);
        v.fold(EntryId(7), Some(true), Some(&s), 80, &t);
        let words: Vec<String> = v
            .rows(Some(&s), 80, &t)
            .iter()
            .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
            .collect();
        assert!(words.contains(&"3 lanes".to_owned()), "{words:?}");
    }

    /// A queue of three, current the first, kept in `kept`.
    fn kept_as(kept: ronoctua_player::queue::QueueKept) -> (State, Box<Outline>) {
        let outline = crate::player::fixture::outline(vec![
            Name::Titled("One".into()),
            Name::Titled("Two".into()),
            Name::Titled("Three".into()),
        ]);
        let s = State {
            queue_file: kept,
            ..crate::player::fixture::idle()
        };
        (s, outline)
    }

    /// FAILS IF: the Queue's title stops saying the file it is kept in, or
    /// "not saved" — and "new, not saved" — stops being said in the danger
    /// tone, in both languages; or the glyph that saves it again is drawn
    /// while the file says what the queue is, or missing while it does not;
    /// or `C-s` is taught where there is nothing to save again.
    #[test]
    fn the_queue_title_says_whether_it_is_saved() {
        use ronoctua_player::queue::QueueKept;
        let changed = QueueKept::Changed {
            path: "/q/road trip.rqueue".into(),
            name: "road trip".into(),
        };
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            for (kept, words, danger, glyph) in [
                (QueueKept::Empty, None, false, false),
                (
                    QueueKept::Never,
                    Some(t.t(key::PLAYER_QUEUE_NEW_UNSAVED)),
                    true,
                    false,
                ),
                (
                    QueueKept::Same {
                        path: "/q/road trip.rqueue".into(),
                        name: "road trip".into(),
                    },
                    Some("road trip".to_owned()),
                    false,
                    false,
                ),
                (
                    changed.clone(),
                    Some(t.t_with(key::PLAYER_QUEUE_UNSAVED, &[("name", "road trip")])),
                    true,
                    true,
                ),
            ] {
                let (s, outline) = kept_as(kept.clone());
                let mut v = View::blank(None);
                v.outline_arrived(Some(outline));
                let rows = v.rows(Some(&s), 100, &t);
                let title = rows
                    .iter()
                    .position(|r| r.cells.iter().any(|c| c.about == Some("player-clear")))
                    .expect("the queue's title");
                let fields = &rows[title].fields;
                let heading = t.t(key::PLAYER_HEADING_QUEUE);
                assert!(
                    fields[0].text.starts_with(&heading),
                    "{language}: {fields:?}"
                );
                match &words {
                    None => assert!(
                        !fields.iter().any(|f| f.color == tokens::color::STATE_ERROR),
                        "{language}: {kept:?}"
                    ),
                    Some(words) => {
                        let mark = fields
                            .iter()
                            .find(|f| &f.text == words)
                            .unwrap_or_else(|| panic!("{language}: {words} in {fields:?}"));
                        assert_eq!(
                            mark.color == tokens::color::STATE_ERROR,
                            danger,
                            "{language}: {kept:?}"
                        );
                    }
                }
                assert_eq!(
                    rows[title]
                        .cells
                        .iter()
                        .any(|c| c.about == Some("player-save-queue-again")),
                    glyph,
                    "{language}: {kept:?}"
                );
                let hint: String = v
                    .hint(Some(&s), 100, &t)
                    .iter()
                    .map(|p| p.text.clone())
                    .collect();
                assert_eq!(
                    hint.contains(&t.t("player-save-queue-again")),
                    glyph,
                    "{language}: {kept:?} teaches {hint}"
                );
            }
        }
    }

    /// FAILS IF: saving again stops asking the player to write over the
    /// file it came from — saying so — or a queue never saved stops opening
    /// the save's picker (where its folder is chosen and its name typed), or
    /// an empty one stops saying so.
    #[test]
    fn save_again_writes_the_file_it_came_from() {
        use ronoctua_player::queue::QueueKept;
        let t = texts("en-GB");
        let mut v = View::blank(None);
        let (s, outline) = kept_as(QueueKept::Changed {
            path: "/q/road trip.rqueue".into(),
            name: "road trip".into(),
        });
        v.outline_arrived(Some(outline));
        match v.key(Press::SaveQueueAgain, Some(&s), 100, &t) {
            Errand::TellSaying(Ask::SaveQueueAgain { force: false }, said) => {
                assert!(said.contains("road trip.rqueue"), "{said}");
            }
            other => panic!("{other:?}"),
        }
        let (never, _) = kept_as(QueueKept::Never);
        assert_eq!(
            v.key(Press::SaveQueueAgain, Some(&never), 100, &t),
            Errand::Nothing
        );
        assert_eq!(
            v.page(),
            crate::player::PageId::Pick(crate::player::Purpose::SaveQueue),
            "a queue never saved opens the save's picker"
        );
        v.let_go();
        let (empty, _) = kept_as(QueueKept::Empty);
        assert_eq!(
            v.key(Press::SaveQueueAgain, Some(&empty), 100, &t),
            Errand::Said(t.t(key::PLAYER_QUEUE_EMPTY))
        );
    }

    /// FAILS IF: "already in the queue" stops being a question on the page —
    /// "no" first and the cursor born on it, the entry's number said — or its
    /// "yes" stops putting it again without `once`; or the file changed on
    /// the disk stops asking before it is written over, `force` on its yes;
    /// or another refusal is taken for one of these.
    #[test]
    fn a_refusal_the_person_may_overrule_is_asked() {
        use crate::player::Told;
        use ronoctua_player::queue::QueueKept;
        let t = texts("en-GB");
        let (s, outline) = kept_as(QueueKept::Never);
        let mut v = View::blank(None);
        v.outline_arrived(Some(outline));
        let put = crate::player::put("/m/two.flac".into(), false);
        let trouble = Trouble::AlreadyQueued {
            entry: EntryId(2),
            name: Name::Titled("Two".into()),
        };
        let again = crate::player::overruled(&put, &trouble).expect("overruled");
        assert!(matches!(
            again,
            Ask::Put {
                once: false,
                play: false,
                ..
            }
        ));
        assert!(v.told(
            &Told::Asks {
                again: Box::new(again.clone()),
                trouble,
            },
            &t
        ));
        v.settle_in(Some(&s), 100, &t);
        let rows = v.rows(Some(&s), 100, &t);
        let words: String = rows
            .iter()
            .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
            .collect::<Vec<_>>()
            .join(" ");
        assert!(
            words.contains("Two is already in the queue as 2"),
            "{words}"
        );
        assert_eq!(rows[v.cursor()].target, Target::Answer(false), "no first");
        assert_eq!(v.key(Press::Escape, Some(&s), 100, &t), Errand::Nothing);
        // AND AGAIN, answered yes.
        let trouble = Trouble::AlreadyQueued {
            entry: EntryId(2),
            name: Name::Titled("Two".into()),
        };
        v.told(
            &Told::Asks {
                again: Box::new(again.clone()),
                trouble,
            },
            &t,
        );
        v.settle_in(Some(&s), 100, &t);
        v.key(Press::Down, Some(&s), 100, &t);
        assert!(matches!(
            v.key(Press::Enter, Some(&s), 100, &t),
            Errand::TellSaying(a, _) if a == again
        ));
        // THE FILE CHANGED ON THE DISK: written over only after a yes.
        let save = Ask::SaveQueueAgain { force: false };
        let changed = Trouble::ChangedOnDisk {
            path: "/q/road trip.rqueue".into(),
        };
        let force = crate::player::overruled(&save, &changed).expect("overruled");
        assert_eq!(force, Ask::SaveQueueAgain { force: true });
        v.told(
            &Told::Asks {
                again: Box::new(force),
                trouble: changed,
            },
            &t,
        );
        v.settle_in(Some(&s), 100, &t);
        let rows = v.rows(Some(&s), 100, &t);
        assert_eq!(rows[v.cursor()].target, Target::Answer(false), "no first");
        v.key(Press::Down, Some(&s), 100, &t);
        assert!(matches!(
            v.key(Press::Enter, Some(&s), 100, &t),
            Errand::TellSaying(Ask::SaveQueueAgain { force: true }, _)
        ));
        // ANOTHER REFUSAL is none of these.
        assert_eq!(crate::player::overruled(&save, &Trouble::NoOrigin), None);
        assert_eq!(
            crate::player::overruled(
                &crate::player::put("/m/a.flac".into(), true),
                &Trouble::AlreadyQueued {
                    entry: EntryId(1),
                    name: Name::Titled("One".into()),
                }
            ),
            None,
            "played now was never asked once"
        );
    }

    /// FAILS IF: an entry's number stops being the one its row says —
    /// `3.2` the second of the third.
    #[test]
    fn an_entry_is_numbered_as_its_row() {
        let outline = Outline {
            revision: 1,
            entries: vec![
                single(1, "a"),
                single(2, "b"),
                node(
                    3,
                    Name::Titled("list".into()),
                    NodeKind::Playlist,
                    vec![single(4, "c"), single(5, "d")],
                    0,
                ),
            ],
        };
        assert_eq!(number_of(&outline, EntryId(2)).as_deref(), Some("2"));
        assert_eq!(number_of(&outline, EntryId(5)).as_deref(), Some("3.2"));
        assert_eq!(number_of(&outline, EntryId(9)), None);
    }
}
