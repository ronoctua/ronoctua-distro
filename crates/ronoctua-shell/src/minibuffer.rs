//! `M-x` — command entry through the minibuffer, §8 and §13.
//!
//! # Why this is not a new surface
//!
//! Invariant 14 forbids any chrome beyond the two-line footer.
//! The locator uses **the** surface: the prompt sits on the minibuffer's line and
//! the candidates appear in the expanded region, exactly like which-key, the
//! notification history and the manual. Four features, one
//! mechanism.
//!
//! # Why it exists even with the leader
//!
//! §13 asks for every feature to be **addressable by name**, and not only
//! by key — it is what keeps the door open for the IPC surface without
//! redesigning anything later. A command that only exists as a chord is not
//! addressable; one that exists as an id is, and `M-x` is the proof of that in the
//! user's hand.

/// One line of the locator.
///
/// # Why the label travels with the item
///
/// The locator listed `&'static str` — command ids — and resolved the
/// description through the registry. That works for exactly one kind of list,
/// and it is why `LEADER w w` never existed: a buffer has no entry in the
/// command registry, so there was nowhere for its name to come from.
///
/// With the label carried here, the same list serves commands and buffers. An
/// empty label means "ask the registry", which keeps the `M-x` path byte for
/// byte what it was.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Item {
    /// What gets executed or selected.
    pub id: String,
    /// What is shown. Empty = look it up in the command registry.
    pub label: String,
}

impl Item {
    /// An item that is a command: the registry knows its description.
    #[must_use]
    pub fn command(id: &str) -> Self {
        Self {
            id: id.to_owned(),
            label: String::new(),
        }
    }
}

/// The locator's state while it is open.
#[derive(Debug, Default)]
pub struct Minibuffer {
    /// Open?
    active: bool,
    /// What was typed.
    query: String,
    /// Items that match, in relevance order.
    candidates: Vec<Item>,
    /// Everything that COULD match — the universe this locator is filtering.
    ///
    /// For `M-x` it is rebuilt from the registry on open; for a list of buffers
    /// it is what the caller handed in. Keeping it here is what lets the filter
    /// run without knowing which of the two it is.
    universe: Vec<Item>,
    /// Which candidate is focused.
    cursor: usize,
    /// The question being asked on the line — a chosen command **waiting for
    /// an argument**, or a view asking for a name, a path, a password.
    ///
    /// # Why it exists
    ///
    /// Of the compositor's 141 actions, some require an argument —
    /// `focus-workspace <REFERENCE>`. A key sequence has no way of
    /// carrying a number: it is a path to an id, not a call with a
    /// parameter. Chosen in the locator, the action **asks**, as
    /// Emacs does with an interactive command that reads an argument.
    ///
    /// Without this, either those actions would disappear from the list — and "all the OS's
    /// actions" would stop being true — or they would be called with no argument
    /// and the compositor would answer with an error the user did not ask for.
    ///
    /// THE QUESTION IS A [`crate::line::Line`], the same one a window asks
    /// on its header (`door::Ui::ask`): one set of keys, one mask for a
    /// secret, in both doors.
    asking: Option<crate::line::Line>,
    /// Whether the last answer was typed on a SECRET line — asked by whoever
    /// logs the answer, so a password does not reach the journal through
    /// the line that says a command ran.
    answered_a_secret: bool,
    /// How many times the minibuffer has opened — see [`Minibuffer::asked`].
    asked: u64,
}

impl Minibuffer {
    /// Opens, with every command as a candidate.
    pub fn open(&mut self, registry: &ronoctua_cmd::Registry) {
        self.asked = self.asked.wrapping_add(1);
        self.active = true;
        self.query.clear();
        self.cursor = 0;
        self.asking = None;
        self.filter_by(registry);
    }

    /// Closes and forgets what was typed.
    pub fn close_it(&mut self) {
        self.active = false;
        self.query.clear();
        self.candidates.clear();
        self.cursor = 0;
        self.asking = None;
    }

    /// The query as the filter uses it — lower case.
    ///
    /// It exists so the drawing can recompute WHERE it matched, and light those
    /// letters. Keeping the positions along with each candidate would cost one
    /// allocation per item on every keystroke, and they only matter to the rows
    /// on screen.
    #[must_use]
    pub fn normalized_query(&self) -> String {
        self.query.to_lowercase()
    }

    /// Is it waiting for an argument?
    #[must_use]
    pub fn asking_for(&self) -> Option<&str> {
        self.asking.as_ref().map(|l| l.id)
    }

    /// The question on the line, whole — what the footer draws it from, with
    /// the builder a window's header uses (`door::list::question_pieces`).
    #[must_use]
    pub const fn line(&self) -> Option<&crate::line::Line> {
        self.asking.as_ref()
    }

    /// Was the last answer typed on a secret line? Then it is not logged.
    #[must_use]
    pub const fn answered_a_secret(&self) -> bool {
        self.answered_a_secret
    }

    /// Is a SECRET being typed now — a password, a PIN, a key? Then no key
    /// that reaches the line is logged either: the answer's log line was
    /// masked and the keystrokes that spelled it went to the log one by one.
    #[must_use]
    pub fn typing_a_secret(&self) -> bool {
        self.asking.as_ref().is_some_and(|l| l.secret)
    }

    /// The number of the question on the line: it changes every time one is
    /// asked, so a paste read for an older question — off the loop, answered
    /// later — is not typed into the one that replaced it. The window's line
    /// keeps the same count (`door::Ui::ask`).
    #[must_use]
    pub const fn asked(&self) -> u64 {
        self.asked
    }

    /// Lets the line go when it is asking for `id` — its question is over:
    /// the pairing it asked a PIN for ended, or another replaced it. Says
    /// whether it did.
    pub fn let_go_of(&mut self, id: &str) -> bool {
        if self.asking_for() == Some(id) {
            self.close_it();
            return true;
        }
        false
    }

    /// Is it open?
    #[must_use]
    pub const fn active(&self) -> bool {
        self.active
    }

    /// Opens the minibuffer ALREADY asking for a command's argument.
    ///
    /// # Why it exists
    ///
    /// The normal path to a command with an argument is `M-x`, choose, and
    /// only then type. The network view arrives in the middle of that path: the network
    /// has already been chosen from the list, and what is missing is the password. Without this, `Enter`
    /// over a secured network would have to send the person to `M-x` to type the
    /// name of a command they have already triggered.
    pub fn request(&mut self, id: &'static str, secret: bool) {
        self.request_with(id, secret, None);
    }

    /// The same, with the line already holding `initial`.
    ///
    /// # Why it exists
    ///
    /// Because changing a name or an address starts from what it IS: a web
    /// app called "Proton Mail" renamed to "Mail" is four `DEL`s, and asking
    /// with an empty line would make the person type the address back from
    /// memory to fix one letter of it. What was there is typed in the way a
    /// paste is — one line, the ends trimmed — and `Enter` answers with
    /// whatever the line holds then.
    pub fn request_with(&mut self, id: &'static str, secret: bool, initial: Option<&str>) {
        self.asked = self.asked.wrapping_add(1);
        self.active = true;
        self.asking = Some(crate::line::Line::new(id, secret, initial));
        self.query.clear();
        self.candidates.clear();
        self.cursor = 0;
    }

    /// The same, the question in the words of its own key — a brain module's
    /// action, whose key its module declared (`crate::line::Line::asking`).
    pub fn request_asking(&mut self, id: &'static str, asks: &'static str) {
        self.request_with(id, false, None);
        if let Some(line) = self.asking.as_mut() {
            line.asks = Some(asks);
        }
    }

    /// The same, with the whole line a view built — a SECRET in the words of
    /// its own key included, which neither of the two above can ask: the
    /// three steps of a new passphrase are one command, hidden, each with its
    /// own question (`belongings::Ask::line`). What the line holds already is
    /// taken as one line, as [`Minibuffer::request_with`] takes it.
    pub fn request_line(&mut self, line: crate::line::Line) {
        self.request_with(
            line.id,
            line.secret,
            Some(line.text.as_str()).filter(|t| !t.is_empty()),
        );
        if let Some(asking) = self.asking.as_mut() {
            asking.asks = line.asks;
            asking.asks_with = line.asks_with;
        }
    }

    /// What appears on the minibuffer's line, in the person's own language.
    ///
    /// THE COMMAND'S ID IS NOT A SENTENCE. It used to be drawn raw —
    /// `network-connect: •••••` — which is a UI string that no `.ftl` carries
    /// and that says nothing to somebody who did not write it. What is drawn
    /// now is the question the command's own text keys give: `ask-<id>` when
    /// there is one (a real question, "Password for this network"), and the
    /// command's own label otherwise, which every command has in both
    /// languages by construction.
    #[must_use]
    pub fn asking(&self, texts: &ronoctua_i18n::Texts) -> String {
        match &self.asking {
            Some(line) => line.drawn(texts),
            None => self.prompt(),
        }
    }

    /// The same, with no localiser — the shape the tests use.
    #[must_use]
    pub fn prompt(&self) -> String {
        // While asking for an argument, the prompt is the COMMAND'S NAME — `focus-workspace:`
        // — and not `M-x`: whoever got here has already chosen, and what is missing is
        // saying which workspace. Repeating `M-x` would make it look as if the choice
        // had not taken. A secret shows one dot per cell (`Line::shown`).
        match &self.asking {
            Some(line) => line.prompt(),
            // `M-x ` and the typed text. The prefix does NOT go through i18n: it is
            // the key's own notation, the same in every language — the same reason
            // `ronoctua-keymap` does not translate a key's name.
            None => format!("M-x {}", self.query),
        }
    }

    /// The visible candidates, in order.
    #[must_use]
    pub fn candidates(&self) -> &[Item] {
        &self.candidates
    }

    /// Which one is focused.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// The focused id, if there is one.
    #[must_use]
    pub fn chosen(&self) -> Option<&Item> {
        self.candidates.get(self.cursor)
    }

    /// Recomputes the candidates.
    ///
    /// # Fuzzy, and deterministic
    ///
    /// The search is by **subsequence**: `jme` finds `window-move-left`, and
    /// `fl` finds `wm-focus-left`. With a hundred and fifty commands, requiring the
    /// prefix forces one to remember how the name starts — and the locator exists
    /// precisely for whoever does not remember.
    ///
    /// The order is deterministic and the scoring is coarse on purpose:
    /// a prefix beats contiguous, contiguous beats a loose subsequence, and a tie
    /// goes to the shorter id and then alphabetically. Clever scoring
    /// would make the list dance between similar queries, and then muscle
    /// memory — which is what makes this fast — would be no use at all.
    fn filter_by(&mut self, registry: &ronoctua_cmd::Registry) {
        // `M-x` rebuilds its universe from the registry: a command may have been
        // registered since the last open, and a stale list would hide it.
        if self.universe.is_empty() {
            self.universe = registry.all_ones().map(|c| Item::command(c.id)).collect();
        }
        self.refilter();
    }

    /// Filters the universe by what has been typed.
    ///
    /// Matches on the id AND on the label, in that order: for a command the
    /// label is empty and this is the old behaviour exactly; for a buffer the
    /// name is what one types, and matching only the id would make the list
    /// unusable.
    fn refilter(&mut self) {
        let q = self.query.to_lowercase();
        let keys: Vec<Vec<&str>> = self
            .universe
            .iter()
            .map(|i| {
                if i.label.is_empty() {
                    vec![i.id.as_str()]
                } else {
                    vec![i.id.as_str(), i.label.as_str()]
                }
            })
            .collect();
        let refs: Vec<&[&str]> = keys.iter().map(Vec::as_slice).collect();
        self.candidates = rank(&q, &refs)
            .into_iter()
            .map(|i| self.universe[i].clone())
            .collect();
        self.cursor = self.cursor.min(self.candidates.len().saturating_sub(1));
    }

    /// Opens over an ARBITRARY list — buffers, files, anything with a name.
    ///
    /// It is the same locator: same filter, same keys, same rendering. What
    /// changes is only where the universe came from.
    pub fn open_over(&mut self, items: Vec<Item>) {
        self.asked = self.asked.wrapping_add(1);
        self.active = true;
        self.query.clear();
        self.cursor = 0;
        self.asking = None;
        self.universe = items;
        self.refilter();
    }

    /// Picks the candidate at index `n` and confirms, as a click does.
    ///
    /// # Why it goes through the same `Enter`
    ///
    /// A command that requires an argument **asks** instead of executing, and that
    /// rule lives in the handling of `Enter`. Duplicating it here would give a mouse that
    /// fires `focus-workspace` with no number while the keyboard asks — the
    /// same action with two behaviours, depending on how one got to it.
    pub fn choose(&mut self, n: usize, registry: &ronoctua_cmd::Registry) -> Action {
        if n >= self.candidates.len() {
            return Action::Nothing;
        }
        self.cursor = n;
        self.key(
            ronoctua_keymap::Key::simple(ronoctua_keymap::Sym::Enter),
            registry,
        )
    }

    /// Applies a key. Returns the id to run when the person confirms.
    ///
    /// # Navigation by `C-n`/`C-p`, and not by arrows
    ///
    /// The keymap's `Sym` has no arrow, and that is its decision: the project's key
    /// notation is Emacs's, where `C-n` and `C-p` are the canonical movement
    /// in a list. Adding an arrow to `Sym` just for `M-x` would make
    /// the keymap grow because of one consumer.
    pub fn key(&mut self, t: ronoctua_keymap::Key, registry: &ronoctua_cmd::Registry) -> Action {
        use ronoctua_keymap::Sym;

        // A QUESTION IS ASKED BY ITS LINE, and the line is the one a window
        // asks on too (`crate::line`): its keys are its own, and the finder's
        // list plays no part in it.
        if let Some(line) = self.asking.as_mut() {
            return match line.key(t) {
                crate::line::Step::Nothing => Action::Nothing,
                crate::line::Step::Changed => Action::Changed,
                crate::line::Step::WantsPaste => Action::WantsPaste,
                crate::line::Step::Answer(id, said) => {
                    let secret = line.secret;
                    self.close_it();
                    self.answered_a_secret = secret;
                    Action::RunWith(id.to_owned(), said)
                }
                crate::line::Step::Cancel => {
                    self.close_it();
                    Action::Closed
                }
            };
        }

        // PASTING, by the one answer every text input asks (`line::pastes`):
        // `C-v`, and `C-y`, Emacs's yank, which is the notation this line is
        // written in. It is not a gesture of `gesture_of`, which the file
        // manager's listing shares, and where `C-v` with nothing typed is the
        // preview. This side does not hold the clipboard; the footer does, and
        // answers with [`Minibuffer::paste`].
        if crate::line::pastes(t) {
            return Action::WantsPaste;
        }

        match gesture_of(t) {
            Gesture::Cancel => {
                self.close_it();
                return Action::Closed;
            }
            Gesture::Down => return self.move_by(1),
            Gesture::Up => return self.move_by(-1),
            Gesture::Nothing => return Action::Nothing,
            // The three that depend on what the view keeps are below.
            Gesture::Confirm | Gesture::Delete | Gesture::Insert(_) => {}
        }

        match t.sym {
            Sym::Enter => match self.chosen().cloned() {
                Some(item) => match argument_of(&item.id) {
                    // Chosen, but the argument is missing. The list disappears and the
                    // line becomes a prompt: going on showing 170
                    // candidates while one types a workspace number
                    // would be offering a choice that has already been made.
                    Some(id) => {
                        self.asked = self.asked.wrapping_add(1);
                        self.asking = Some(crate::line::Line::new(id, false, None));
                        self.query.clear();
                        self.candidates.clear();
                        self.cursor = 0;
                        Action::Changed
                    }
                    None => {
                        // AN ITEM WITH A LABEL IS NOT A COMMAND. It came from a
                        // list the caller built — a buffer, a file — and running
                        // it as a command id would find nothing in the registry
                        // and do nothing, in silence.
                        let selected = !item.label.is_empty();
                        self.close_it();
                        if selected {
                            Action::Selected(item.id)
                        } else {
                            Action::Run(item.id)
                        }
                    }
                },
                // Confirming with no candidate does NOT close: closing would erase
                // what was typed, and whoever got a letter wrong would have to
                // start over.
                None => Action::Nothing,
            },
            Sym::Backspace => {
                // ONE CLUSTER, not one `char` — `line::pop_cluster` says why.
                if !crate::line::pop_cluster(&mut self.query) {
                    return Action::Nothing;
                }
                self.filter_by(registry);
                Action::Changed
            }
            // A CONTROL CHARACTER IS NOT TYPED: `Delete` arrives as U+007F,
            // a character nobody can see in the query.
            Sym::Char(c) if c.is_control() => Action::Nothing,
            Sym::Char(c) => {
                self.query.push(c);
                self.filter_by(registry);
                Action::Changed
            }
            Sym::Space => {
                self.query.push(' ');
                self.filter_by(registry);
                Action::Changed
            }
            // `gesture_of` has already resolved everything else.
            _ => Action::Nothing,
        }
    }

    /// What was copied, typed in at the end of the line.
    ///
    /// ONE LINE: the minibuffer is a line, and a newline in it would be a
    /// character nobody can see — a path copied with its trailing newline
    /// would name a file that does not exist. Line breaks become spaces, and
    /// the ends are trimmed.
    pub fn paste(&mut self, text: &str, registry: &ronoctua_cmd::Registry) -> Action {
        if let Some(line) = self.asking.as_mut() {
            return match line.paste(text) {
                crate::line::Step::Changed => Action::Changed,
                _ => Action::Nothing,
            };
        }
        let line = crate::line::one_line(text);
        if line.is_empty() {
            return Action::Nothing;
        }
        self.query.push_str(&line);
        self.filter_by(registry);
        Action::Changed
    }

    /// Moves the cursor, without leaving the list.
    fn move_by(&mut self, d: i32) -> Action {
        let fresh = self.cursor as i32 + d;
        if fresh < 0 || fresh >= self.candidates.len() as i32 {
            return Action::Nothing;
        }
        self.cursor = fresh as usize;
        Action::Changed
    }
}

/// What a key MEANS in a list with a search field.
///
/// # Why this is a function, and not a copy in each view
///
/// Because it is the only part muscle memory notices. `Esc`, `C-g`,
/// `C-n`/`C-p`, the arrows and `Backspace` have to do the same thing in the
/// command locator and in the application one; if they diverge, the defect
/// shows up as "sometimes `C-n` moves down and sometimes it does not", which nobody connects to
/// two key tables.
///
/// What is left for each view is what it DOES with the gesture — not the
/// key's interpretation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gesture {
    /// `Esc` or `C-g`: it closes.
    Cancel,
    /// `Enter`.
    Confirm,
    /// `Backspace`.
    Delete,
    /// A letter, or the space.
    Insert(char),
    /// `↓` or `C-n`.
    Down,
    /// `↑` or `C-p`.
    Up,
    /// A key with no role here.
    Nothing,
}

/// The key, translated into the gesture.
#[must_use]
pub fn gesture_of(t: ronoctua_keymap::Key) -> Gesture {
    use ronoctua_keymap::Sym;
    if t.mods.ctrl {
        return match t.sym {
            Sym::Char('n') => Gesture::Down,
            Sym::Char('p') => Gesture::Up,
            // `C-g` cancels, as in Emacs.
            Sym::Char('g') => Gesture::Cancel,
            _ => Gesture::Nothing,
        };
    }
    match t.sym {
        Sym::Esc => Gesture::Cancel,
        Sym::Enter => Gesture::Confirm,
        Sym::Backspace => Gesture::Delete,
        Sym::Char(c) => Gesture::Insert(c),
        Sym::Space => Gesture::Insert(' '),
        Sym::Down => Gesture::Down,
        Sym::Up => Gesture::Up,
        // Left and right would move the cursor WITHIN the text, and there is no
        // text cursor here: what is typed always goes at the end. Doing nothing
        // is better than doing something else.
        Sym::Left | Sym::Right | Sym::Tab => Gesture::Nothing,
        // The page keys have no page to turn in a line.
        Sym::PageUp | Sym::PageDown | Sym::Home | Sym::End => Gesture::Nothing,
    }
}

/// What the locator asks of the host after a key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Nothing changed; do not redraw.
    Nothing,
    /// The list or the text changed.
    Changed,
    /// It left without executing.
    Closed,
    /// Run this id.
    Run(String),
    /// Run this id with the typed argument.
    RunWith(String, String),
    /// An item was chosen from a list the caller supplied — a buffer, a file.
    ///
    /// It is NOT a command: the registry knows nothing about it, and the caller
    /// is the one that knows what the id means.
    Selected(String),
    /// `C-v` or `C-y`: the caller holds the clipboard, and hands it to
    /// [`Minibuffer::paste`].
    WantsPaste,
}

/// The command's own id, when it requires a positional argument.
///
/// Only the compositor's deeds do, and what knows that is the generated
/// table — not a list of ours that would age silently. The id comes back
/// `'static` from that table, which is what a [`crate::line::Line`] asks for.
fn argument_of(id: &str) -> Option<&'static str> {
    ronoctua_scroll::deeds::spoken_of(id)
        .filter(|s| s.deed.argument.is_some())
        .map(|s| s.deed.id)
}

/// Scores a candidate against the query. `None` = no match.
///
/// Lower is better: 0 prefix, 1 contiguous in the middle, 2 loose subsequence.
fn score_it(query: &str, id: &str) -> Option<u8> {
    match_up(query, id).map(|(p, _)| p)
}

/// How much a SECONDARY key worsens the score.
///
/// Three, which is one more than the worst possible score (`2`, the loose
/// subsequence): that way **any** match in the name beats **any** match
/// in the id. It is what makes `arquiv` show "Arquivos" at the top even when
/// `org.gnome.Nautilus` would also match by subsequence — whoever types is
/// reading the names, and an item that matches through text that is not on screen
/// looks as if it got in by mistake.
const STEP_PER_KEY: u8 = 3;

/// Sorts candidates by relevance; returns the indices that match.
///
/// # Why one function, and not a copy in each locator
///
/// Because the ORDERING rule is what muscle memory uses: a prefix beats
/// contiguous, contiguous beats a loose subsequence, and a tie goes to the
/// shorter one and then alphabetically. Two copies would diverge the day one of them
/// gained a criterion — and the symptom would be "in `M-x` the first one is right and
/// in the application list it is not", which nobody connects to two sorting functions.
///
/// # One or more keys per item
///
/// Each item brings its keys in order of importance. The first is the one that
/// appears on screen; the following ones exist to find by what is **not** on the
/// screen — the application launcher passes the name and then the desktop id,
/// because the id is usually the binary and typing `nvim` to find "Neovim" is the
/// gesture of whoever knows the system. See [`STEP_PER_KEY`].
///
/// The tie-break by length and by text always uses the FIRST key: it is the
/// one that gets read, and sorting by the length of an invisible text would make the list
/// look shuffled.
#[must_use]
pub fn rank(query: &str, items: &[&[&str]]) -> Vec<usize> {
    let mut found: Vec<(u8, usize, &str, usize)> = items
        .iter()
        .enumerate()
        .filter_map(|(i, keys)| {
            let best = keys
                .iter()
                .enumerate()
                .filter_map(|(n, k)| {
                    let step = u8::try_from(n).unwrap_or(u8::MAX);
                    score_it(query, k).map(|p| p.saturating_add(step * STEP_PER_KEY))
                })
                .min()?;
            let principal = keys.first().copied().unwrap_or_default();
            // WITH NO QUERY, the length does NOT break ties: the open list is
            // alphabetical, which is the registry's order and the one muscle
            // memory uses. Sorting by size would put the shortest id at the top and
            // would make the list look different on every opening.
            let size = if query.is_empty() { 0 } else { principal.len() };
            Some((best, size, principal, i))
        })
        .collect();
    found.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(b.2)));
    found.into_iter().map(|(_, _, _, i)| i).collect()
}

/// Where the query matches in the candidate, and how well.
///
/// It returns `(score, positions in CHARACTERS)`. The positions exist so the
/// drawing can paint the matched letters in another colour — without them, a filtered
/// list does not say **why** each item is in it, and with a subsequence
/// search that stops being obvious: `jme` matching
/// `window-move-left` only makes sense when the three letters light up.
///
/// In characters and not in bytes: the consumer counts text cells, and a
/// byte index would come out wrong at the first accent.
#[must_use]
pub fn match_up(query: &str, id: &str) -> Option<(u8, Vec<usize>)> {
    if query.is_empty() {
        return Some((0, Vec::new()));
    }
    // BY CLUSTER, because the positions this returns are CELL columns:
    // `surface::runs_of` uses them to place the highlight, and it walks the
    // same clusters. Every id here is kebab-case ASCII (invariant 18), so the
    // two counts coincide today — the point is that they now coincide by
    // construction and not by luck.
    let folded = id.to_lowercase();
    let lowercase: Vec<&str> = ronoctua_render::layout::cells_of(&folded).collect();
    let target: Vec<&str> = ronoctua_render::layout::cells_of(query).collect();

    // CONTIGUOUS first: `buffer` inside `buffer-close` lights all six
    // together, and not six scattered letters that happen to be in order.
    if let Some(start) = adjacent_window(&lowercase, &target) {
        let pos = (start..start + target.len()).collect();
        return Some((u8::from(start > 0), pos));
    }

    // SUBSEQUENCE: the letters in order, with gaps. `jme` matches
    // `window-move-left` — and that is how one types when one remembers what
    // the command DOES, and not how it is spelt.
    let mut pos = Vec::with_capacity(target.len());
    let mut searching = target.iter();
    let mut current = searching.next();
    for (i, c) in lowercase.iter().enumerate() {
        if Some(c) == current {
            pos.push(i);
            current = searching.next();
            if current.is_none() {
                return Some((2, pos));
            }
        }
    }
    None
}

/// Where `target` appears whole inside `text`, if it appears.
///
/// Over CLUSTERS, which is what the caller counts in — the index it gives back
/// is a cell column, and `surface::runs_of` lights the cell.
fn adjacent_window(text: &[&str], target: &[&str]) -> Option<usize> {
    if target.len() > text.len() {
        return None;
    }
    (0..=text.len() - target.len()).find(|&i| &text[i..i + target.len()] == target)
}

#[cfg(test)]
mod tests {
    /// FAILS IF: the footer's key log is not told a secret is being typed —
    /// every letter of a password reached the debug log, one per line — or
    /// is still told so once the line is answered.
    #[test]
    fn a_secret_line_says_it_is_being_typed() {
        use ronoctua_keymap::{Key, Sym};
        let registry = ronoctua_cmd::Registry::fresh();
        let mut m = super::Minibuffer::default();
        assert!(!m.typing_a_secret());
        m.request("bluetooth-accept", true);
        assert!(m.typing_a_secret());
        m.key(Key::simple(Sym::Char('0')), &registry);
        assert!(m.typing_a_secret());
        m.key(Key::simple(Sym::Enter), &registry);
        assert!(!m.typing_a_secret(), "the answer left and the mask stayed");
        m.request("workspace-go", false);
        assert!(!m.typing_a_secret());
    }

    /// FAILS IF: a paste read off the loop for one question can be typed into
    /// the one that replaced it — every question the line asks has a number of
    /// its own — or a pairing's PIN line outlives the question it asked for.
    #[test]
    fn every_question_has_its_number_and_a_line_is_let_go_by_its_id() {
        let registry = ronoctua_cmd::Registry::fresh();
        let mut m = super::Minibuffer::default();
        m.request("bluetooth-accept", true);
        let first = m.asked();
        m.request("bluetooth-accept", true);
        assert_ne!(m.asked(), first, "a new question kept the old number");
        let second = m.asked();
        m.open(&registry);
        assert_ne!(m.asked(), second);
        // LET GO ONLY BY ITS OWN ID.
        m.request("bluetooth-accept", true);
        assert!(!m.let_go_of("network-connect"));
        assert_eq!(m.asking_for(), Some("bluetooth-accept"));
        assert!(m.let_go_of("bluetooth-accept"));
        assert_eq!(m.asking_for(), None);
        assert!(!m.active());
    }

    /// FAILS IF: the footer draws its question without the two clicks a
    /// window's line has — `ESC` lets it go and `RET` answers, pressed where
    /// they are drawn (invariant 19) — or cuts what is typed at its end.
    #[test]
    fn the_footers_question_is_drawn_with_its_two_clicks() {
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let mut m = super::Minibuffer::default();
        m.request_with("software-web-name", false, Some(&"a long name ".repeat(20)));
        let line = m.line().expect("a question on the line");
        let pieces = crate::door::list::question_pieces(line, &texts, 60);
        let marks = crate::chrome::hint_marks(&pieces, 1, 0, 200);
        let pressed: Vec<String> = marks
            .iter()
            .filter_map(|m| match m.press {
                Some(crate::chrome::Act::Key(k)) => Some(k.notation()),
                _ => None,
            })
            .collect();
        assert!(pressed.iter().any(|k| k == "ESC"), "{pressed:?}");
        assert!(pressed.iter().any(|k| k == "RET"), "{pressed:?}");
        // THE END OF WHAT IS TYPED STAYS IN SIGHT.
        let shown: String = pieces.iter().map(|p| p.text.as_str()).collect();
        assert!(shown.contains("a long name"), "{shown}");
        assert!(shown.contains('…'), "a long answer was not cut: {shown}");
    }

    /// FAILS IF: a question as wide as a narrow door pushes what is typed
    /// past the line's cut — a header is cut at its end, and the small
    /// column showed "The folder to save the set in…" and nothing of the
    /// path being typed: the question is cut instead, and what is typed
    /// keeps its end in sight on the first line, in either language.
    #[test]
    fn a_long_question_gives_way_to_what_is_typed() {
        use ronoctua_render::layout::width;
        for language in ["en-GB", "pt-BR"] {
            let texts = ronoctua_i18n::Texts::fresh(language).expect("a language");
            let mut m = super::Minibuffer::default();
            m.request_with("player-enqueue", false, Some("/home/someone/Music/sets"));
            let line = m.line().expect("a question on the line");
            for columns in [24_u16, 30, 36] {
                let pieces = crate::door::list::question_pieces(line, &texts, columns);
                let usable = columns - crate::fold::WIDTH;
                let (asked, typed) = (&pieces[0].text, &pieces[1].text);
                assert!(
                    width(asked) + width(typed) <= usable,
                    "{language} at {columns}: {asked:?} + {typed:?} past {usable}"
                );
                assert!(
                    typed.ends_with("sets"),
                    "{language} at {columns}: {typed:?}"
                );
                assert!(
                    width(asked) > 2,
                    "{language} at {columns}: no question left"
                );
            }
            // A WIDE DOOR keeps the question whole.
            let pieces = crate::door::list::question_pieces(line, &texts, 120);
            assert!(!pieces[0].text.contains('…'), "{:?}", pieces[0].text);
        }
    }

    /// The Wi-Fi password must NOT appear on the minibuffer's line.
    ///
    /// # The defect this test locks out
    ///
    /// The first version of `request` reused `workspace-go`'s path, which
    /// shows what was typed — and showing makes sense for a workspace
    /// number. For the Wi-Fi password it does not: the footer's screen usually has more
    /// people looking at it than an installer's, and nothing warned.
    #[test]
    fn the_requested_password_comes_out_masked_and_the_mask_does_not_linger() {
        use ronoctua_keymap::{Key, Sym};
        let registry = ronoctua_cmd::Registry::fresh();
        let mut m = super::Minibuffer::default();
        m.request("network-connect", true);
        for c in "s3nh4-secreta".chars() {
            m.key(Key::simple(Sym::Char(c)), &registry);
        }
        let line = m.prompt();
        assert!(
            !line.contains("s3nh4-secreta"),
            "the password leaked: {line}"
        );
        assert!(line.contains("•"), "it masked nothing: {line}");
        // The length appears: without that one cannot see that the key took.
        assert_eq!(line.matches('•').count(), "s3nh4-secreta".chars().count());

        // And the mask does NOT carry over to the next request, which is not a secret.
        m.close_it();
        m.request("workspace-go", false);
        m.key(Key::simple(Sym::Char('3')), &registry);
        assert!(m.prompt().contains('3'), "it masked what is not a secret");
    }

    /// THE ANSWER OF A SECRET LINE SAYS SO, for the one line that logs every
    /// answer (`Footer::minibuffer_key`): a Wi-Fi password used to reach the
    /// journal there, in the clear, as the "argument" of `network-connect`.
    #[test]
    fn a_secret_answer_says_it_is_one_and_the_next_does_not() {
        use ronoctua_keymap::{Key, Sym};
        let registry = ronoctua_cmd::Registry::fresh();
        let mut m = super::Minibuffer::default();
        m.request("network-connect", true);
        for c in "s3nh4".chars() {
            m.key(Key::simple(Sym::Char(c)), &registry);
        }
        assert_eq!(
            m.key(Key::simple(Sym::Enter), &registry),
            super::Action::RunWith("network-connect".into(), "s3nh4".into())
        );
        assert!(m.answered_a_secret());
        m.request("network-import", false);
        m.key(Key::simple(Sym::Char('/')), &registry);
        m.key(Key::simple(Sym::Enter), &registry);
        assert!(!m.answered_a_secret(), "a path was taken for a secret");
    }

    use super::{Action, Minibuffer};
    use ronoctua_keymap::{Key, Mods, Sym};

    fn registry() -> ronoctua_cmd::Registry {
        crate::leader::default_of()
            .expect("the default keymap closes")
            .1
    }

    fn t(sym: Sym) -> Key {
        Key {
            mods: Mods::default(),
            sym,
        }
    }

    fn ctrl(c: char) -> Key {
        Key {
            mods: Mods {
                ctrl: true,
                ..Mods::default()
            },
            sym: Sym::Char(c),
        }
    }

    fn type_in(m: &mut Minibuffer, r: &ronoctua_cmd::Registry, s: &str) {
        for c in s.chars() {
            m.key(t(Sym::Char(c)), r);
        }
    }

    #[test]
    fn a_question_can_start_from_what_is_there_and_is_edited_from_it() {
        let r = registry();
        let mut m = Minibuffer::default();
        m.request_with("software-web-name", false, Some("  Proton Mail\n"));
        assert!(m.prompt().ends_with(": Proton Mail"), "{}", m.prompt());
        for _ in 0.."Mail".len() {
            m.key(t(Sym::Backspace), &r);
        }
        type_in(&mut m, &r, "Box");
        assert_eq!(
            m.key(t(Sym::Enter), &r),
            Action::RunWith("software-web-name".into(), "Proton Box".into())
        );
        // AND A PLAIN REQUEST STILL STARTS EMPTY, after one that did not.
        m.request_with("software-web-address", false, None);
        assert_eq!(
            m.key(t(Sym::Enter), &r),
            Action::RunWith("software-web-address".into(), String::new())
        );
    }

    #[test]
    fn opens_with_every_command() {
        let r = registry();
        let mut m = Minibuffer::default();
        assert!(!m.active());
        m.open(&r);
        assert!(m.active());
        assert_eq!(m.candidates().len(), r.amount());
    }

    #[test]
    fn filters_and_a_prefix_beats_a_containment() {
        // Determinism matters more than cleverness: the list must not dance
        // between similar queries, otherwise muscle memory is no use.
        let r = registry();
        let mut m = Minibuffer::default();
        m.open(&r);
        type_in(&mut m, &r, "buffer");
        assert!(!m.candidates().is_empty(), "no buffer command at all?");
        assert!(
            m.candidates().iter().all(|i| i.id.contains("buffer")),
            "something that does not match got in: {:?}",
            m.candidates()
        );
    }

    #[test]
    fn confirm_gives_back_the_id_and_closes() {
        let r = registry();
        let mut m = Minibuffer::default();
        m.open(&r);
        let expected = m.chosen().expect("there is a candidate").id.clone();
        assert_eq!(m.key(t(Sym::Enter), &r), Action::Run(expected));
        assert!(!m.active(), "confirming has to close");
    }

    #[test]
    fn confirm_with_no_candidate_does_not_close() {
        // Closing would erase what was typed, and whoever got a letter wrong would have
        // to start over from scratch.
        let r = registry();
        let mut m = Minibuffer::default();
        m.open(&r);
        type_in(&mut m, &r, "zzzznaoexiste");
        assert!(m.candidates().is_empty());
        assert_eq!(m.key(t(Sym::Enter), &r), Action::Nothing);
        assert!(m.active(), "it could not have closed");
    }

    #[test]
    fn esc_closes_without_running() {
        let r = registry();
        let mut m = Minibuffer::default();
        m.open(&r);
        assert_eq!(m.key(t(Sym::Esc), &r), Action::Closed);
        assert!(!m.active());
    }

    #[test]
    fn the_cursor_does_not_go_past_the_ends() {
        let r = registry();
        let mut m = Minibuffer::default();
        m.open(&r);
        assert_eq!(
            m.key(ctrl('p'), &r),
            Action::Nothing,
            "it came up from the top"
        );
        // Moving down MORE than the list's size, and not a hard-coded number: it was
        // 100 steps in a list of 28 commands, and the compositor's 141 actions
        // pushed it past 100 — the test failed for landing in the middle, and not
        // because the cursor had escaped.
        for _ in 0..m.candidates().len() + 5 {
            m.key(ctrl('n'), &r);
        }
        assert_eq!(m.cursor(), m.candidates().len() - 1);
        assert_eq!(
            m.key(ctrl('n'), &r),
            Action::Nothing,
            "it went past the end"
        );
    }

    #[test]
    fn the_cursor_does_not_end_outside_the_list_on_filtering() {
        // Moving down and then typing shortens the list. Without the `min`, the cursor
        // would end up outside and `chosen()` would return `None` with candidates on
        // screen — Enter would do nothing, without saying why.
        let r = registry();
        let mut m = Minibuffer::default();
        m.open(&r);
        for _ in 0..5 {
            m.key(ctrl('n'), &r);
        }
        type_in(&mut m, &r, "quit");
        assert!(m.cursor() < m.candidates().len().max(1));
        assert!(m.chosen().is_some() || m.candidates().is_empty());
    }

    #[test]
    fn the_search_is_by_subsequence() {
        // WITH 172 COMMANDS, requiring the prefix forces one to remember how the name
        // starts — and the locator exists for whoever remembers what the command
        // DOES, not how it is spelt.
        let r = registry();
        let mut m = Minibuffer::default();
        m.open(&r);
        type_in(&mut m, &r, "wml");
        assert!(
            m.candidates().iter().any(|i| i.id == "window-move-left"),
            "`wml` did not find `window-move-left`: {:?}",
            m.candidates()
        );
    }

    #[test]
    fn the_prefix_beats_the_subsequence() {
        // Determinism matters more than cleverness: with the list dancing
        // between similar queries, muscle memory is no use.
        let r = registry();
        let mut m = Minibuffer::default();
        m.open(&r);
        type_in(&mut m, &r, "buffer");
        assert!(
            m.chosen().is_some_and(|i| i.id.starts_with("buffer")),
            "the first one does not start with the query: {:?}",
            m.chosen()
        );
    }

    #[test]
    fn an_action_with_an_argument_asks_instead_of_running() {
        // THE DEFECT THIS PREVENTS: going to a workspace requires a number, and
        // running the command without one makes the compositor answer
        // `Invalid workspace command (expected at least 1 argument, got 0)` —
        // an error the user did not cause and does not understand. It comes
        // back in the REPLY, which is the shape of every refusal there.
        let r = registry();
        let mut m = Minibuffer::default();
        m.open(&r);
        type_in(&mut m, &r, "wm-workspace-go");
        assert_eq!(m.chosen().map(|i| i.id.as_str()), Some("wm-workspace-go"));

        assert_eq!(
            m.key(t(Sym::Enter), &r),
            Action::Changed,
            "it ran straight away"
        );
        assert_eq!(m.asking_for(), Some("wm-workspace-go"));
        assert!(m.active(), "it closed instead of asking");
        // The prompt becomes the command's NAME: whoever got here has already chosen.
        assert!(m.prompt().starts_with("wm-workspace-go:"), "{}", m.prompt());

        type_in(&mut m, &r, "3");
        assert_eq!(
            m.key(t(Sym::Enter), &r),
            Action::RunWith("wm-workspace-go".to_owned(), "3".to_owned())
        );
        assert!(!m.active());
    }

    #[test]
    fn an_action_with_no_argument_runs_straight_away() {
        // And the converse: asking where there is nothing to ask would be one
        // step more in every command in the system.
        let r = registry();
        let mut m = Minibuffer::default();
        m.open(&r);
        type_in(&mut m, &r, "wm-focus-left");
        assert_eq!(
            m.key(t(Sym::Enter), &r),
            Action::Run("wm-focus-left".to_owned())
        );
    }

    #[test]
    fn clicking_a_candidate_is_the_same_as_choosing_and_confirming() {
        // The click must NOT be a second path. If it executed directly,
        // an action that requires an argument would be fired without it by the mouse and
        // would ask through the keyboard — the same action with two behaviours.
        let r = registry();
        let mut m = Minibuffer::default();
        m.open(&r);
        type_in(&mut m, &r, "wm-focus-left");
        assert_eq!(
            m.choose(0, &r),
            Action::Run("wm-focus-left".to_owned()),
            "clicking the first one did not run the first one"
        );

        // And the one that requires an argument ASKS, through the mouse too.
        let mut m = Minibuffer::default();
        m.open(&r);
        type_in(&mut m, &r, "wm-workspace-go");
        assert_eq!(m.choose(0, &r), Action::Changed);
        assert_eq!(m.asking_for(), Some("wm-workspace-go"));
    }

    #[test]
    fn clicking_outside_the_list_does_nothing() {
        // The index comes from the pointer's position; between the event and the handling,
        // the list may have shrunk. Running `candidates[n]` without checking
        // would be picking another command — or panicking.
        let r = registry();
        let mut m = Minibuffer::default();
        m.open(&r);
        let n = m.candidates().len();
        assert_eq!(m.choose(n, &r), Action::Nothing);
        assert_eq!(m.choose(9999, &r), Action::Nothing);
        assert!(m.active(), "it closed on a click outside");
    }

    #[test]
    fn the_match_says_where_it_matched() {
        // The positions exist so the drawing can light the letters. In
        // CHARACTERS and not in bytes: a byte index would come out wrong at the
        // first accent, and the highlight would appear in the wrong place.
        //
        // GREEDY TO THE LEFT: the `l` that matches after the `m` is the one in "(l)eft", and
        // not a later one. Preferring a word start would read better in
        // kebab-case, but it can CHOOSE a position that stops the rest from
        // matching — and then a valid candidate would disappear from the list. A highlight on a
        // neighbouring letter is cheap; an item that disappears from the search is not.
        let (_, pos) = super::match_up("wml", "window-move-left").unwrap();
        assert_eq!(pos, vec![0, 7, 12], "the three letters of the subsequence");

        // CONTIGUOUS lights the whole block, together — and not scattered letters
        // that happen to be in order.
        let (_, pos) = super::match_up("buffer", "buffer-close").unwrap();
        assert_eq!(pos, vec![0, 1, 2, 3, 4, 5]);
        let (_, pos) = super::match_up("close", "buffer-close").unwrap();
        assert_eq!(pos, vec![7, 8, 9, 10, 11]);

        // With no query, nothing lights up.
        assert_eq!(
            super::match_up("", "anything").unwrap().1,
            Vec::<usize>::new()
        );
        assert!(super::match_up("zzz", "buffer-close").is_none());
    }

    #[test]
    fn the_normalized_query_is_the_one_the_filter_used() {
        // The drawing recomputes the match with it. If it diverged from the one that
        // filtered, the highlight would light letters from another search.
        let r = registry();
        let mut m = Minibuffer::default();
        m.open(&r);
        type_in(&mut m, &r, "BuFf");
        assert_eq!(m.normalized_query(), "buff");
        for id in m.candidates() {
            assert!(
                super::match_up(&m.normalized_query(), &id.id).is_some(),
                "`{}` is in the list and the highlight cannot find where it matched",
                id.id
            );
        }
    }

    #[test]
    fn the_prompt_shows_what_was_typed() {
        let r = registry();
        let mut m = Minibuffer::default();
        m.open(&r);
        assert_eq!(m.prompt(), "M-x ");
        type_in(&mut m, &r, "sai");
        assert_eq!(m.prompt(), "M-x sai");
    }

    /// FAILS IF: `C-v` goes back to being a key with no role — the owner
    /// copied a path in the file manager, asked the player for a file, and
    /// nothing was pasted — or a pasted newline gets into the line.
    #[test]
    fn a_copy_is_pasted_as_one_line() {
        let r = registry();
        let mut m = Minibuffer::default();
        m.open(&r);
        for c in ['v', 'y'] {
            let key = Key {
                mods: Mods {
                    ctrl: true,
                    ..Mods::default()
                },
                sym: Sym::Char(c),
            };
            assert_eq!(m.key(key, &r), Action::WantsPaste, "C-{c}");
        }
        assert_eq!(m.paste("/music/a b.flac\n", &r), Action::Changed);
        assert_eq!(m.prompt(), "M-x /music/a b.flac");
        assert_eq!(m.paste("  \n", &r), Action::Nothing);
    }

    /// THE DEFECT THIS CATCHES: backspace taking one CODE POINT.
    ///
    /// `String::pop` removes a code point and a cluster is up to six of them,
    /// so one press left the wreckage of an emoji in the query — and the
    /// password mask beside it counts clusters, so the row of dots and the
    /// text behind it stopped agreeing about how much had been typed.
    #[test]
    fn backspace_removes_a_whole_cluster() {
        let r = registry();
        let mut m = Minibuffer::default();
        m.open(&r);
        // Six code points, one cluster: the couple with a heart.
        for c in "\u{1F469}\u{200D}\u{2764}\u{FE0F}\u{200D}\u{1F468}".chars() {
            m.key(t(Sym::Char(c)), &r);
        }
        m.key(t(Sym::Char('a')), &r);
        assert_eq!(
            m.prompt(),
            "M-x \u{1F469}\u{200D}\u{2764}\u{FE0F}\u{200D}\u{1F468}a"
        );

        m.key(t(Sym::Backspace), &r);
        assert_eq!(
            m.prompt(),
            "M-x \u{1F469}\u{200D}\u{2764}\u{FE0F}\u{200D}\u{1F468}"
        );
        // ONE more press, and the whole couple goes — not a fifth of it.
        m.key(t(Sym::Backspace), &r);
        assert_eq!(m.prompt(), "M-x ");
        // And an empty query still reports that there was nothing to remove.
        assert_eq!(m.key(t(Sym::Backspace), &r), Action::Nothing);
    }

    /// An ideograph is one cluster too, and the query is where a person types
    /// a file name.
    #[test]
    fn backspace_removes_an_ideograph_in_one_press() {
        let r = registry();
        let mut m = Minibuffer::default();
        m.open(&r);
        for c in "\u{65E5}\u{672C}".chars() {
            m.key(t(Sym::Char(c)), &r);
        }
        m.key(t(Sym::Backspace), &r);
        assert_eq!(m.prompt(), "M-x \u{65E5}");
    }
}
