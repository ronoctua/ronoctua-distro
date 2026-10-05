//! THE PREVIEW AND THE FACTS OF ONE FILE — the two panels the file manager
//! puts beside its listing, as a part of their own.
//!
//! # Why they left `files::View`
//!
//! Because they were never the listing's. What a panel shows is about ONE
//! file — what it looks like and what it is — and the listing is only the
//! first place a file is chosen from: a search, a picker and a player's own
//! browser choose files too, and each of them would otherwise have to hold a
//! directory listing for the sake of two panels. So the state and the
//! arithmetic of both panels live in [`Panel`], which is told what it is about
//! ([`Panel::focus`], or [`Panel::follow`] for a host with a list) and asked
//! what to draw. `files::View` holds one, and keeps the listing, the sidebar
//! and the menus.
//!
//! # What the host still decides
//!
//! WHERE the panels go. The manager puts them beside its listing, under it in
//! a strip, or across the whole region, and that is `files::Panes` — the
//! view's layout, not theirs. A panel answers for one box at a time
//! ([`Panel::section`]); [`Panel::sections`] is the plain arrangement for a
//! host with nothing beside them. What the host also says, before every
//! question, is whether something covers the panels and which section has the
//! keyboard ([`Around`]) — told, and never kept as a second copy that could go
//! stale.
//!
//! # And the threads that fill them
//!
//! A preview is a child process (`ronoctua-inspect`), a film is a player
//! (`crate::motion`) and a shape is a stage (`crate::stage`). [`Threads`]
//! raises the three and hands what they say back to the host's loop as one
//! [`Heard`], which the host passes on to the panel. Nothing here waits on any
//! of them: `systemd-run --scope` already taught this project what a child
//! waited on inside the loop that draws costs.
//!
//! What is inside a file, as rows of words, is the sheet's module:
//! [`crate::inspection_sheet`].

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use ronoctua_files::{
    Entry, Kind, mime,
    mode::{self, Explanation},
    when::Zone,
    who::Roster,
};
use ronoctua_inspect::motion::Show;
use ronoctua_tokens::Color;
use smithay_client_toolkit::reexports::calloop;

use crate::files::{
    BAR, FRAME_MARK, Field, Hit, Ink, PANEL_FLOOR, PREVIEW_ALL_SHOWN, Pane, Panel as Which, Panes,
    Row, Section, Step, TRANSPORT_LEAD, TRANSPORT_SOUND, TRANSPORT_THEATRE, centred, clock_of,
    clock_text, does, ink, letterbox, width_of,
};
use crate::inspection_sheet::{Doable, Fact, SheetFor};

/// What the inspector was asked about one file.
///
/// The path travels back with the answer because the cursor moves while a
/// child process runs: without it the panel would show the previous file's
/// picture under the current file's name, which is the one failure a preview
/// must not have.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ask {
    /// The file, absolute.
    pub path: PathBuf,
    /// The type to act on — already through `ronoctua_inspect::settled`.
    pub mime: &'static str,
    /// How many pixels across the longer side, before quantisation.
    pub target: u32,
    /// The cache key this answer will carry back.
    pub key: String,
    /// How wide a tabulation is, in cells.
    ///
    /// It travels the way `target` does, and for the same reason: it is a
    /// TOKEN (`space.files.tabWidth`), and `ronoctua-inspect` does not receive
    /// token values. Whoever asks holds the tokens.
    pub tab: usize,
    /// The waveform's ink, for a sound with no artwork.
    ///
    /// It travels the way `tab` does, and for the same reason: it is a TOKEN
    /// (`surface.wave`), and `ronoctua-inspect` does not receive token values.
    pub wave: String,
    /// A film that might belong to this picture, from the SAME folder.
    ///
    /// # Why the folder is asked about HERE
    ///
    /// An iPhone's Live Photo is two files — `IMG_1.HEIC` and `IMG_1.MOV` —
    /// and the inspector's whole promise is what is inside ONE file. Knowing
    /// what else is in the directory is the view's business, and the listing is
    /// already in memory, so the candidate costs a scan of a `Vec` and no
    /// syscall.
    ///
    /// It is a CANDIDATE and not a pairing: whether the two belong together is
    /// decided by the `ContentIdentifier` both carry, which is a question about
    /// what is inside them.
    pub beside: Option<PathBuf>,
}

/// What the inspector came back with.
///
/// `Eq` is NOT derived, and it is the frame rate's doing: `Moving` carries a
/// `f64` cadence, and a float has no total equality. `PartialEq` is what the
/// comparisons here actually want — "is this still the same answer" — and it is
/// what they have always used.
#[derive(Debug, Clone, PartialEq)]
pub struct Looked {
    /// `Stamp::key()` — what identifies this answer in the texture map.
    pub key: String,
    /// The file it is about.
    pub path: PathBuf,
    /// The pixels, when there are any.
    pub rgba: Option<ronoctua_render::image::Rgba>,
    /// How this file's frames would be produced, when it moves.
    ///
    /// The player is started by the DOOR and not here: this crate answers what
    /// is in a file, and running a child process for as long as a cursor rests
    /// on it is the shell's business.
    pub moving: Option<ronoctua_inspect::motion::Moving>,
    /// The things this file names, when it names things rather than being one.
    ///
    /// A playlist has no picture and is not read as text: it is a LIST, and it
    /// is drawn as one. See [`ronoctua_inspect::playlist`].
    pub list: Option<ronoctua_inspect::playlist::List>,
    /// The letters, when there are any and there is no picture.
    ///
    /// # Everything expensive is already done
    ///
    /// Read, split, de-tabbed and HIGHLIGHTED, once, on the inspector's own
    /// thread. It has to be: a highlight state carries between lines, so
    /// colouring "only the visible ones" would still mean running from line
    /// zero — which is the whole file, per line, per frame. What is left for
    /// `preview_fields` is indexing one line and walking its handful of runs.
    pub text: Option<ronoctua_inspect::text::Read>,
    /// Why there are none, when there are none.
    pub why: Option<ronoctua_inspect::Refusal>,
    /// What the tools said the file is.
    pub facts: Vec<ronoctua_inspect::facts::Fact>,
    /// The type this was decided to be, already through `settled`.
    ///
    /// Carried rather than worked out again: `wants_look` has just resolved it
    /// and the stage needs the same answer, and two `examined` calls on one
    /// file is two chances for them to differ.
    pub mime: &'static str,
    /// Whether this is a shape the manager turns.
    ///
    /// Set only when the mesh actually READ: a `.stl` that is damaged carries
    /// `false` and a `why` instead, so the panel says what is wrong rather
    /// than waiting for ever for a frame that is not coming. It is the same
    /// distinction `moving` makes — the door starts the thread, this only
    /// says whether there is one to start.
    pub turns: bool,
    /// How many of a boundary representation's faces came out as an OUTLINE
    /// rather than filled.
    ///
    /// It is on `Looked` and not only among the facts because the preview says
    /// it too: a piece drawn part solid and part wire is a piece somebody
    /// should be told about, and the panel is where they are looking. Zero for
    /// everything that is not a STEP or an IGES.
    pub outlined: usize,
    /// WHERE THE FILE SAYS IT WAS MADE, in signed decimal degrees.
    ///
    /// It is on `Looked` and not only among the facts because a coordinate is
    /// not only a row: it is what a globe is pointed at and what a map is
    /// opened on. Reading it back out of the fact's formatted value would be a
    /// second parse of a number this side already had — and `f32` because the
    /// projection is in `f32` and five decimals is about a metre, which is more
    /// than any of it needs.
    pub at: Option<(f32, f32)>,
    /// What the panel can CHANGE in it beyond what `lstat` answers: a song's
    /// tags, a picture's details (`inspection_edit`).
    pub amendable: crate::inspection_edit::Amendable,
}

/// What the inspector answers about one ask — on the thread that asks it.
///
/// The body of [`inspector`]'s loop, and nothing else: taken out so that
/// a caller with no loop to wait on — a photograph of the door — asks the
/// same function the thread does and draws the same answer.
///
/// Synchronous, and it spawns children: never call it on a loop that draws.
///
/// The WHOLE answer, what can be changed included — for a caller that has
/// one moment to draw (a photograph, `inspect_now`). The thread answers in
/// two messages instead ([`inspector`]).
#[must_use]
pub fn look(ask: Ask) -> Looked {
    let mut looked = look_inside(ask);
    looked.amendable = crate::inspection_edit::read(&looked.path, looked.mime);
    looked
}

/// [`look`] without what can be changed: the facts, the picture, the text —
/// everything the panel shows before a second round of children (the tag
/// program, exiftool's read of the details) has answered.
#[must_use]
pub fn look_inside(ask: Ask) -> Looked {
    // THE FACTS FIRST, because their probe is what says whether the
    // file MOVES — and a file that moves needs no still picture.
    let told = ronoctua_inspect::facts::facts(&ask.path, ask.mime);
    // AND WHETHER IT IS A SHAPE, read ONCE.
    //
    // The same read answers the facts and the question of whether
    // there is anything to turn, so it happens here rather than
    // inside `facts::facts` — that function is a front for the five
    // external programs and none of them is involved. Reading a
    // hundred-megabyte file twice to keep the shapes tidy would be
    // a hundred megabytes read twice.
    let shape = ronoctua_inspect::pixels::turns(ask.mime)
        .then(|| ronoctua_inspect::mesh::read(&ask.path, ask.mime));
    // AND WHETHER IT MOVES, out of the probe the facts already ran:
    // one document, two readers, and no second spawn.
    let moving =
        ronoctua_inspect::motion::moving(&ask.path, ask.mime, &told.probe, ask.beside.as_deref());
    // A FILE THAT MOVES GETS NO STILL PICTURE.
    //
    // It used to get one, and the owner reported what that looks
    // like: *"está sendo carregada primeiro a imagem de um só
    // frame, e na sequência o vídeo, o que acaba fazendo essa
    // imagem apenas piscar rapidamente na tela"*. The poster is a
    // whole ffmpeg run — a spawn, a seek and a decode — for a
    // picture replaced 84 milliseconds later by the player's first
    // frame. Work paid to produce a flicker.
    //
    // What fills the gap is NOTHING: `preview_key` answers `None`
    // until a frame arrives, so the panel is empty rather than
    // briefly wrong.
    //
    // A MOTION PHOTO IS THE EXCEPTION, and it is the whole point of
    // one: the photograph is what it IS, and the film is a second
    // and a half of what was happening around it. Both are wanted,
    // and the loop alternates between them. The condition is
    // therefore "is this a film" and not "does this move".
    //
    // AND SOUND IS NOT A FILM EITHER, which is what makes this
    // line say what the paragraph above already claimed. A sound's
    // picture is DRAWN, and it is drawn only while it plays — and
    // a sound never starts by itself. Asking `does this move` here
    // would leave every audio file with an empty panel for ever,
    // with no sentence, because the frame that was meant to fill
    // it is waiting for a key nobody has a reason to press.
    let a_film = moving
        .as_ref()
        .is_some_and(|m| m.inside.is_none() && m.show == Show::Film);
    let made = if a_film {
        ronoctua_inspect::Made::unasked()
    } else {
        ronoctua_inspect::pixels::preview(&ask.path, ask.mime, ask.target, &ask.wave)
    };
    // THE TEXT, ONLY WHEN THERE IS NO PICTURE. A picture wins, and
    // reading a mebibyte to throw it away is a mebibyte read for
    // every video walked past. It is also why SVG and EPS need no
    // exception: the database says both descend from `text/plain`,
    // and both get a picture first.
    //
    // AND A FILM IS NOT TEXT EITHER: without this a video, now
    // having no still, would be read for a mebibyte to be told it
    // is not letters.
    //
    // NOR IS A SHAPE, and three of them ARE text by the database's
    // reckoning: `model/obj`, `model/x-off` and `model/step` all
    // descend from `text/plain`, because they are. Without this
    // arm the owner's 4.3 MB `.obj` was read for a mebibyte,
    // highlighted, and then thrown away — `preview_fields` returns
    // blanks for anything that turns, so not one line of it could
    // ever reach the screen.
    // A LIST OF THINGS TO PLAY, which is neither a picture nor
    // text: `audio/x-mpegurl` descends from `text/plain`, so
    // without this the panel would show an `.m3u` as source code
    // with the paths highlighted — true, and not what the file is
    // FOR.
    let mut listed = ronoctua_inspect::playlist::of(&ask.path, ask.mime);
    // AND WHAT THE LIST DID NOT SAY, ASKED — here, on the
    // inspector's own thread, where every other child process this
    // manager starts is started.
    //
    // The reader itself stays pure and spawns nothing: it answers
    // WHICH entries to ask about, with its own ceiling, and this
    // asks. See `playlist::TIMED_CEILING` for what it costs.
    if let Some(Ok(list)) = listed.as_mut() {
        for n in list.untimed() {
            let ronoctua_inspect::playlist::Where::Local(path) = &list.entries[n].at else {
                continue;
            };
            let found = ronoctua_inspect::facts::duration_of(path);
            list.entries[n].seconds = found;
        }
    }
    // A DOCUMENT IS ASKED BEFORE TEXT, and it has to be.
    //
    // Every office format that is a zip — `.docx`, `.odt`, `.ods`,
    // `.xlsx`, `.pptx`, `.epub` — is an `application/zip` by the
    // database's reckoning, so nothing above claims it and it would
    // fall through to "no preview". And `.rtf` is worse than that:
    // `text/rtf` descends from `text/plain`, so the text reader DID
    // claim it, and the panel drew `{\rtf1\ansi\deff0{\fonttbl…`
    // — the file rather than the document.
    //
    // `document::of` answers `None` for everything it is not, so
    // this is one question in front of another and not a branch.
    let said = (!made.drawable() && moving.is_none() && shape.is_none() && listed.is_none())
        .then(|| {
            ronoctua_inspect::document::of(&ask.path, ask.mime, ask.tab)
                .or_else(|| ronoctua_inspect::text::of(&ask.path, ask.mime, ask.tab))
        })
        .flatten();
    let rgba = made.rgba.map(|r| ronoctua_render::image::Rgba {
        width: r.width,
        height: r.height,
        pixels: r.pixels,
    });
    if let Some(r) = rgba.as_ref() {
        tracing::debug!(
            width = r.width,
            height = r.height,
            kind = ask.mime,
            "preview decoded"
        );
    }
    // WHEN THE FILE IS TEXT, THE TEXT PATH'S ANSWER IS THE ONE
    // THAT COUNTS. Said once, here, rather than at three sites:
    // it is what stops a `.md` carrying `Refusal::Nothing`, and
    // what makes a `chmod 000` `.rs` say it could not be opened
    // instead of "what came back was not a picture".
    let (rgba, why, text) = match said {
        None => (rgba, made.why, None),
        Some(Ok(read)) => (None, None, Some(read)),
        Some(Err(why)) => (None, Some(why), None),
    };
    // AND THE LIST'S OWN ANSWER, folded the same way: a spelling
    // this reader does not open is a SENTENCE, and a panel that is
    // blank and a panel that was refused look the same on screen.
    let (list, why) = match listed {
        None => (None, why),
        Some(Ok(read)) => (Some(read), why),
        Some(Err(refused)) => (None, Some(refused)),
    };
    // AND THE SHEET SAYS THE PICTURE MOVES. Nothing else on it
    // does: the dimensions, the camera and the date are all about
    // the photograph, and a person looking at the facts has no
    // other way to learn there is a film in the file.
    let mut facts = told.facts;
    if let Some(row) = moving.as_ref().and_then(ronoctua_inspect::motion::fact_of) {
        facts.push(row);
    }
    // AND THE SHAPE'S OWN FACTS, out of the read above. A file that
    // is a shape says so; one that CLAIMS to be and does not parse
    // carries the refusal instead, so the panel gets a sentence
    // rather than waiting for a frame nobody is going to draw.
    let (turns, outlined, why) = match shape {
        None => (false, 0, why),
        Some(Ok(read)) => {
            let outlined = read.census.outlined;
            facts.extend(ronoctua_inspect::mesh::facts(&read));
            (true, outlined, why)
        }
        Some(Err(refused)) => (false, 0, Some(refused)),
    };
    // WHERE IT SAYS IT WAS MADE, out of the SAME facts the panel
    // draws — never a second run of exiftool. The row is a phrase
    // with two named arguments, so the numbers are still numbers
    // here and nothing is parsed back out of a sentence.
    let at = ronoctua_inspect::facts::coordinate(&facts);
    // WHAT CAN BE CHANGED IN IT is NOT read here: see `look` and `inspector`.
    Looked {
        key: ask.key,
        path: ask.path,
        rgba,
        moving,
        list,
        text,
        why,
        facts,
        mime: ask.mime,
        turns,
        outlined,
        at,
        amendable: crate::inspection_edit::Amendable::default(),
    }
}

/// A NAMED thread, always: an unnamed one in `top` says `ronoctua-shell` and
/// nothing about what it is doing, on the one screen that exists to answer
/// that.
/// Starts the thread that looks INSIDE files.
///
/// # Why one long-lived NAMED thread, and not one per ask
///
/// Because holding an arrow key down over a folder of videos is sixty asks a
/// second, and each one is `ffmpeg` seeking a frame. One thread per ask would
/// spawn sixty processes to throw away fifty-nine of them; this one takes the
/// NEWEST and drops the rest on the floor, which is the answer somebody
/// scrolling actually wants. It is the arrangement `apps::painter` already has,
/// for the same reason and with the same two channels: `mpsc` in, because only
/// it has a blocking `recv` and the thread should sleep while nobody asks, and
/// the loop's own `calloop` channel out.
///
/// **Nothing here touches the frame.** `ronoctua-inspect` is synchronous and
/// spawns children; waiting for one inside the loop that draws is the trap this
/// project paid for once with `systemd-run --scope`.
#[must_use]
pub fn inspector(out: calloop::channel::Sender<Heard>) -> Option<std::sync::mpsc::Sender<Ask>> {
    let (queue, asks) = std::sync::mpsc::channel::<Ask>();
    let born = std::thread::Builder::new()
        .name("ronoctua-inspect".to_owned())
        .spawn(move || {
            let mut waiting: Option<Ask> = None;
            loop {
                let first = match waiting.take() {
                    Some(ask) => ask,
                    None => match asks.recv() {
                        Ok(ask) => ask,
                        Err(_) => return,
                    },
                };
                // ONLY THE NEWEST. Everything queued behind it is a file the
                // cursor has already left, and decoding it would only make the
                // one being looked at arrive later.
                let mut ask = first;
                while let Ok(newer) = asks.try_recv() {
                    ask = newer;
                }
                // TWO ANSWERS, AND THE FACTS FIRST. What can be CHANGED is a
                // second round of children — the tag program runs ffprobe and
                // exiftool again, and lofty; a picture's details are a second
                // exiftool — and a slow share or a large film would hold the
                // size, the dimensions and the preview back behind it. So the
                // panel is drawn from the first answer, and the editable rows
                // arrive in the second, keyed to the same ask.
                let looked = look_inside(ask);
                let (key, path, mime) = (looked.key.clone(), looked.path.clone(), looked.mime);
                if out.send(Heard::Looked(Box::new(looked))).is_err() {
                    // A closed channel = the door left.
                    return;
                }
                // A NEWER ASK is a cursor that has already moved on: its facts
                // come before this file's rows, which nobody is looking at.
                if let Ok(newer) = asks.try_recv() {
                    waiting = Some(newer);
                    continue;
                }
                let amendable = crate::inspection_edit::read(&path, mime);
                if out
                    .send(Heard::Changeable {
                        key,
                        amendable: Box::new(amendable),
                    })
                    .is_err()
                {
                    return;
                }
            }
        });
    match born {
        Ok(_) => Some(queue),
        Err(e) => {
            tracing::error!(error = %e, "could not create the inspection thread");
            None
        }
    }
}

/// What the panels' threads came back with — ONE message, whichever of the
/// three it is from, so a host has one place to receive and hand on.
#[derive(Debug)]
pub enum Heard {
    /// What the inspector found inside a file. Handed to [`Panel::looked`],
    /// which drops it when the cursor has moved on.
    Looked(Box<Looked>),
    /// A frame of the film, its end, or its trouble. Handed to
    /// [`Panel::played`].
    Beat(crate::motion::Beat),
    /// A view of the shape being turned. Handed to [`Panel::drew`].
    Turned(crate::stage::Beat),
    /// What can be changed in the file an earlier [`Heard::Looked`] was
    /// about — the inspector's second answer. Handed to
    /// [`Panel::changeable`], which drops it when the cursor has moved on.
    Changeable {
        /// The ask it answers (`Looked::key`).
        key: String,
        /// What the panel can change.
        amendable: Box<crate::inspection_edit::Amendable>,
    },
}

/// The three threads that draw into the panels, raised together.
///
/// # One per door, on the door's own loop
///
/// A door has one `calloop` and the answers are the door's to paint; the
/// panel it paints lives in the view, which a door hands a key or a pointer
/// and never a thread. So the host raises these and, once a frame, tells them
/// what the panel wants ([`Threads::look`], [`Threads::play`],
/// [`Threads::turn`]) — each a comparison of values, so an unchanged wish
/// costs no message.
pub struct Threads {
    /// Where an ask for a preview and its facts is posted.
    ///
    /// `None` when the thread could not be raised: the panels then draw the
    /// sentence for a file with no preview, which is honest, rather than the
    /// door refusing to open.
    looking: Option<std::sync::mpsc::Sender<Ask>>,
    /// The thread that follows a moving file, when there is one to follow.
    playing: Option<crate::motion::Player>,
    /// The thread that draws a shape at the angle it is being looked at.
    turning: Option<crate::stage::Stage>,
}

impl Threads {
    /// Raises the three, each with a CHANNEL OF ITS OWN on the host's loop,
    /// and hands whatever they say to `heard`, on that loop.
    ///
    /// Their own channels, and not the door's reports': a frame arrives ten
    /// times a second while a cursor rests on a film, and a view of a shape
    /// every few tens of milliseconds during a drag — and none of them is news
    /// about a directory. AND THE FRAME ARRIVING IS THE CLOCK: neither door has
    /// a timer for a film, because the player sleeps for the frame's own delay
    /// and sends it, so when it stops, the wake-ups stop with it.
    ///
    /// # Errors
    ///
    /// When the loop refuses one of the channels.
    pub fn raise<D: 'static>(
        handle: &calloop::LoopHandle<'static, D>,
        heard: fn(&mut D, Heard),
    ) -> anyhow::Result<Self> {
        use calloop::channel::{Event, channel};
        let (looks_sender, looks) = channel::<Heard>();
        handle
            .insert_source(looks, move |event, (), data: &mut D| {
                if let Event::Msg(h) = event {
                    heard(data, h);
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the inspector's channel: {e}"))?;
        let (frames_sender, frames) = channel::<crate::motion::Beat>();
        handle
            .insert_source(frames, move |event, (), data: &mut D| {
                if let Event::Msg(b) = event {
                    heard(data, Heard::Beat(b));
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the motion channel: {e}"))?;
        let (turns_sender, turns) = channel::<crate::stage::Beat>();
        handle
            .insert_source(turns, move |event, (), data: &mut D| {
                if let Event::Msg(b) = event {
                    heard(data, Heard::Turned(b));
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the stage channel: {e}"))?;
        Ok(Self {
            looking: inspector(looks_sender),
            playing: crate::motion::player(frames_sender),
            turning: crate::stage::stage(turns_sender),
        })
    }

    /// NO THREAD AT ALL — a door with nothing behind it (a photograph), whose
    /// panels are answered on the spot with [`look`].
    #[must_use]
    pub const fn none() -> Self {
        Self {
            looking: None,
            playing: None,
            turning: None,
        }
    }

    /// Posts what the panels should be asked for, when there is an inspector
    /// to ask and the wish has something to ask.
    ///
    /// The wish is asked ONLY when there is a thread, and that is not tidiness:
    /// asking it writes down what was asked (`Panel::asked`), and a panel with
    /// nobody behind it would then wait for ever for an answer — drawn as "not
    /// yet" — instead of saying there is no preview.
    pub fn look(&self, wish: impl FnOnce() -> Option<Ask>) {
        if let Some(queue) = self.looking.as_ref()
            && let Some(ask) = wish()
        {
            let _ = queue.send(ask);
        }
    }

    /// Starts, or stops, the player that follows a moving file — told only
    /// when the panel's wish differs from what it was last told.
    pub fn play(&mut self, wish: impl FnOnce() -> Option<crate::motion::Ask>) {
        if let Some(player) = self.playing.as_mut()
            && let Some(ask) = wish()
        {
            player.tell(ask);
        }
    }

    /// Starts, or stops, the thread that draws a shape — the same shape as
    /// [`Threads::play`], for the same reason.
    pub fn turn(&mut self, wish: impl FnOnce() -> Option<crate::stage::Ask>) {
        if let Some(stage) = self.turning.as_mut()
            && let Some(ask) = wish()
        {
            stage.tell(ask);
        }
    }
}

/// WHAT THE PANELS ARE ABOUT: one file, as a listing or a path knows it.
///
/// Everything the two panels say that is not inside the file comes from here
/// — its path, what `lstat` said, who owns it — so a door that shows a file
/// without holding a listing (a search) builds one with [`Subject::of_path`]
/// and gets the same panels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subject {
    /// The folder it was found in.
    pub folder: PathBuf,
    /// What `lstat` said about it, under the name it has in `folder`.
    pub entry: Entry,
    /// Where it is, whole — THROUGH THE PARENT for the way out, and never a
    /// path with `..` inside it: the sheet and the inspector's request carry
    /// this spelling to a screen and to another process.
    pub path: PathBuf,
    /// Who owns it: the name the roster knows, or the number.
    pub owner: String,
    /// Its group, the same way.
    pub group: String,
    /// What a directory measured to, once somebody asked for the walk.
    pub measured: Option<u64>,
    /// The row of the host's list it is on — what the sheet's targets name
    /// (`Hit::Bit`, `Hit::Owner`, `Hit::Measure`), because pressing one acts on
    /// that row exactly as pressing it in the listing does. Nought for a
    /// subject no list holds.
    pub row: usize,
    /// A film that might be its other half, from the same folder — see
    /// [`Ask::beside`]. Asked of the disk by name for a subject made of a
    /// path (never by reading the folder); a host that holds the folder's
    /// listing answers it out of the listing instead.
    pub beside: Option<PathBuf>,
}

impl Subject {
    /// A file known by its PATH alone: `lstat`, the roster, and the film that
    /// might be its other half, asked of the disk by name
    /// ([`beside_on_disk`]) — a handful of `lstat`s whatever the folder holds,
    /// so a host may build one on its loop.
    ///
    /// `None` when there is nothing there — or nothing a panel could be about,
    /// which is a path with no parent or no name (`/`). The search's detail
    /// (`fast_access_detail.rs`) points its panels at one of these: it shows
    /// a file it found without listing the file's folder.
    #[must_use]
    pub fn of_path(path: &Path, roster: &Roster) -> Option<Self> {
        let folder = path.parent()?;
        let name = path.file_name()?.to_string_lossy().into_owned();
        let entry = Entry::of_path(name, path)?;
        let mut subject = Self::of_entry(folder, &entry, roster, None, 0);
        subject.beside = beside_on_disk(path);
        Some(subject)
    }

    /// An entry of a listing: the folder it was read in and what it said, who
    /// owns it, what it measured to and which row it is on.
    #[must_use]
    pub fn of_entry(
        folder: &Path,
        entry: &Entry,
        roster: &Roster,
        measured: Option<u64>,
        row: usize,
    ) -> Self {
        // THROUGH THE PARENT AND NOT THROUGH A `..`: a path with `..` in it is
        // a path shown on screen and handed to other processes.
        let path = if entry.is_parent() {
            folder
                .parent()
                .map_or_else(|| folder.to_path_buf(), Path::to_path_buf)
        } else {
            folder.join(&entry.name)
        };
        Self {
            folder: folder.to_path_buf(),
            entry: entry.clone(),
            path,
            owner: roster
                .person(entry.uid)
                .map_or_else(|| entry.uid.to_string(), ToOwned::to_owned),
            group: roster
                .group(entry.gid)
                .map_or_else(|| entry.gid.to_string(), ToOwned::to_owned),
            measured,
            row,
            beside: None,
        }
    }

    /// The path the way the host's list SPELLS it — the folder and the name,
    /// the way out included. What the wide preview names a folder by.
    #[must_use]
    pub fn joined(&self) -> PathBuf {
        self.folder.join(&self.entry.name)
    }

    /// Whether this is still what a host with a list is looking at.
    fn is(&self, seen: &Seen<'_>) -> bool {
        self.row == seen.row
            && self.measured == seen.measured
            && self.entry == *seen.entry
            && self.folder == seen.folder
    }
}

/// The spellings of a film's extension a camera writes, in the order they
/// are asked for — see [`beside_on_disk`].
const FILM_SPELLINGS: [&str; 4] = ["MOV", "mov", "MP4", "mp4"];

/// A film beside a picture, found by ASKING THE DISK FOR IT BY NAME — the
/// answer a listing gives out of memory (`files::View::film_beside`), for a
/// subject that came with no listing.
///
/// The same rule: the same stem, and one of the two extensions Apple writes.
/// It is a CANDIDATE and never the answer; whether the two belong together is
/// `facts::content_identifier`'s question, one crate along.
///
/// # By name, and never by reading the folder
///
/// A host builds a [`Subject::of_path`] on its own loop, each time its choice
/// moves, and reading the folder would be every entry of it read to find one
/// — a folder of forty thousand photographs, forty thousand, on the thread
/// that draws. So the candidates are NAMED, the stem with each spelling in
/// [`FILM_SPELLINGS`], and each costs one `lstat`: at most four, the price
/// `of_path` already pays once for the subject itself, whatever the folder
/// holds. The listing reads an extension in any case; this asks for the
/// spellings that are written, so a `.Mov` is the one it does not find.
///
/// NEVER ITS OWN EXTENSION in another case: on a filesystem that folds case
/// (a camera's card), `IMG_1.mov` IS `IMG_1.MOV`, and a film is not its own
/// other half. The first spelling found wins, in the order above, so the
/// answer is the same every time.
fn beside_on_disk(path: &Path) -> Option<PathBuf> {
    let stem = path.file_stem()?;
    let own = path
        .extension()
        .map(|x| x.to_string_lossy().to_ascii_lowercase());
    FILM_SPELLINGS.iter().find_map(|spelling| {
        if own.as_deref() == Some(spelling.to_ascii_lowercase().as_str()) {
            return None;
        }
        let mut name = stem.to_os_string();
        name.push(".");
        name.push(spelling);
        let other = path.with_file_name(name);
        std::fs::symlink_metadata(&other).is_ok().then_some(other)
    })
}

/// What a host with a LIST is looking at, borrowed — so asking whether it
/// changed is a comparison and no allocation, on every question a frame asks.
#[derive(Debug, Clone, Copy)]
pub struct Seen<'a> {
    /// The folder the listing is of.
    pub folder: &'a Path,
    /// The entry under the cursor.
    pub entry: &'a Entry,
    /// Who owns what, by name.
    pub roster: &'a Roster,
    /// What that entry measured to, when it is a directory somebody measured.
    pub measured: Option<u64>,
    /// Its row in the list.
    pub row: usize,
}

/// What the host's layout says about the panels, at the moment of asking.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Around {
    /// Something MODAL covers them — a menu, a question or the detail sheet
    /// (`files::View::plain` is false). Neither panel is on screen then, and a
    /// globe nobody can see answers nothing.
    pub covered: bool,
    /// Which of the host's sections has the keyboard. A shape and a globe
    /// share their zoom keys, and this is what tells them apart.
    pub keyboard: Pane,
}

/// What a film in the preview is DOING.
///
/// # It is not the frame, and it is not the file
///
/// The frame is what arrived; this is what was asked for. Keeping them apart is
/// what lets the picture keep showing the last frame while the film is paused,
/// and what lets `wants_motion` compare a wish against an order without
/// consulting pixels.
///
/// Every field resets when the cursor lands on another file: coming back to a
/// film starts it, and never resumes somebody else's pause.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transport {
    /// Whether there should be a child at all.
    ///
    /// `false` is the film having ENDED, or having been stopped. Pausing does
    /// not clear it — a paused film keeps its child, which is what makes
    /// resuming instant instead of a 108-to-242-millisecond seek.
    pub on: bool,
    /// Whether it is held on one frame.
    pub paused: bool,
    /// Whether the sound was asked for.
    ///
    /// Off by default, which is the owner's decision and the only sensible one
    /// for a panel that follows a cursor: a preview that speaks when a cursor
    /// passes over a file is a preview one turns off.
    pub sound: bool,
    /// Where the child was told to start, in seconds.
    ///
    /// This and not [`Transport::at`] is what the player is given: `at` moves
    /// with every frame, and a wish that moved with every frame would respawn
    /// the film thirty times a second.
    pub from: f64,
    /// Where the picture is now, in seconds.
    pub at: f64,
}

impl Default for Transport {
    fn default() -> Self {
        Self {
            // IT PLAYS ON ARRIVAL. A film in a preview that waits to be
            // started is a preview that shows one frame, which is the still
            // picture it already had.
            on: true,
            paused: false,
            sound: false,
            from: 0.0,
            at: 0.0,
        }
    }
}

impl Transport {
    /// How the transport stands the instant the cursor LANDS on a file.
    ///
    /// # A film runs and a sound waits, and they are not the same question
    ///
    /// A film that waited would show one frame — the still it already had —
    /// so there is nothing to see until somebody presses a key, and the key
    /// would be a tax on every video walked past.
    ///
    /// A SOUND is the other way round, and the owner asked for it in as many
    /// words: it never starts by itself. A panel that begins speaking because
    /// a cursor passed over a file is a panel one turns off — which is the
    /// argument [`Transport::sound`] already makes about a film's audio, one
    /// register louder, because here the sound is the whole content.
    ///
    /// And `on: false` rather than `paused: true`: a pause still spawns a
    /// child, holds the file open and hands it to the five-minute killer, for
    /// a file nobody has asked to hear. With `on` clear there is no child at
    /// all, the panel shows the cover, and `play_pause`'s third state already
    /// means "start from the top".
    fn arriving(show: Option<Show>, a_list: bool) -> Self {
        if a_list {
            // A LIST IS SOUND BY ANOTHER NAME, and the owner's rule reaches it
            // for the same reason: nine tracks starting because a cursor
            // passed over an `.m3u` is the panel one turns off.
            return Self {
                on: false,
                sound: true,
                ..Self::default()
            };
        }
        match show {
            // AND THE SOUND IS ON, because a sound player that is silent is
            // not a player. It costs nothing while `on` is false.
            Some(Show::Sound { .. }) => Self {
                on: false,
                sound: true,
                ..Self::default()
            },
            _ => Self::default(),
        }
    }
}

/// A bound on the work of wrapping a refusal, in lines.
///
/// Not a limit anybody reaches: the longest sentence this system has is 94
/// cells, which is four rows in the narrowest panel. It is here so that a
/// refusal carrying something pathological cannot make the wrapper walk a
/// megabyte — the same bound `fold::wrap_pieces` and `layout::wrap` carry.
pub(crate) const REFUSAL_CEILING: usize = 64;

/// A BLANK ROW AT THE END OF THE FACTS — CONTENT, and never a reserve.
///
/// The expand button is drawn in the panel's bottom-right corner, so scrolled
/// to the last fact it would sit on top of one. The sheet therefore has one
/// more ROW than it has facts and that row is empty: it comes into view only
/// once everything else has been scrolled past, which is the owner's own
/// requirement — *"uma linha em branco ao final do conteúdo, ou seja, uma
/// linha em branco que apareça apenas após scroll down todo o conteúdo"*.
///
/// # The first shape of this took the line off the PANEL, and he read the cost
///
/// *"Você fez a coluna perder uma linha de informação em um espaço que já é
/// pequeno e já mostra poucas linhas."* A reserve costs a fact on EVERY screen;
/// a row at the end of the content costs one only where the button would
/// otherwise have covered something. The two are one line of code apart and
/// they are not the same promise.
pub(crate) const BLANK: usize = 1;

/// THE WIDTH ONE COLUMN OF THE EXPANDED FACTS IS MEASURED AGAINST.
///
/// `space.view.labelCells` is where every list in this system starts its value,
/// and [`PANEL_FLOOR`] is the least room in which a fact still reads — the
/// widest ordinary one is `Dimensions  3024 × 4032`, twenty-three cells. A
/// column that holds both is a column where neither half is cut, and that is
/// what expanding is FOR.
///
/// # The other answer was read off the screen
///
/// Measured against [`PANEL_CEILING`] alone, the owner's region came out as
/// FOUR columns of forty-three: *"em vez de 4 colunas pequenas … seria melhor
/// que tivesse sido dividido em 3 colunas maiores, pois assim o conteúdo das
/// labels e valores maiores aparecerão com mais caracteres em tela, assim
/// melhorando a leitura"*. At this width the same region is three columns of
/// fifty-eight — five more cells of label and ten more of value on every row.
pub(crate) const FACT_COLUMN: u16 = ronoctua_tokens::space::VIEW_LABELCELLS + PANEL_FLOOR;

/// How many lines a PREVIEW wants before it is a picture rather than a smudge.
///
/// The same number the bottom strip takes, and deliberately the same: it is one
/// answer to one question — "how short may a preview get and still be worth
/// drawing" — and two numbers for it would be two answers.
pub(crate) fn preview_lines() -> u16 {
    ronoctua_tokens::space::FILES_STRIPLINES
}

/// What the preview panel says when there is no picture in it.
///
/// Not "the file could not be read" — that is what `ronoctua_inspect`'s own
/// refusals say, and they win when there is one. This is the answer for a
/// directory, a broken link and a file the tools have not been asked about
/// yet: there is no picture, and the panel says so instead of drawing a large
/// icon that says what the listing's own icon column already said.
pub(crate) const NO_PREVIEW: &str = ronoctua_i18n::key::FILES_NO_PREVIEW;

/// What a transport key says when there is no film under the cursor.
pub(crate) const NOTHING_PLAYING: &str = ronoctua_i18n::key::FILES_THEATRE_NOTHING;

/// What the sound key says on a film that carries none.
pub(crate) const NO_SOUND_IN_IT: &str = ronoctua_i18n::key::FILES_SOUND_NONE;

/// What pressing an entry that is not on this machine says.
pub(crate) const ENTRY_NOT_HERE: &str = ronoctua_i18n::key::FILES_ENTRY_NOT_HERE;

/// What the queue keys say on a list where nothing can be played.
pub(crate) const NOTHING_TO_PLAY: &str = ronoctua_i18n::key::FILES_LIST_NOTHING;

/// And at either end of one.
pub(crate) const LIST_AT_START: &str = ronoctua_i18n::key::FILES_LIST_AT_START;

/// And at either end of one.
pub(crate) const LIST_AT_END: &str = ronoctua_i18n::key::FILES_LIST_AT_END;

/// What either whole-region key says with nothing under the cursor.
pub(crate) const THEATRE_NOTHING: &str = ronoctua_i18n::key::FILES_THEATRE_NOTHING;

/// And what it says about a panel that is not on the screen to be expanded.
pub(crate) const WIDE_HIDDEN: &str = ronoctua_i18n::key::FILES_WIDE_HIDDEN;

/// The whole region, given to the preview.
pub(crate) const THEATRE_ON: &str = ronoctua_i18n::key::FILES_THEATRE_ON;

/// And given back.
pub(crate) const THEATRE_OFF: &str = ronoctua_i18n::key::FILES_THEATRE_OFF;

/// The whole region, given to the facts.
pub(crate) const FACTS_WIDE_ON: &str = ronoctua_i18n::key::FILES_FACTS_WIDE_ON;

/// And given back.
pub(crate) const FACTS_WIDE_OFF: &str = ronoctua_i18n::key::FILES_FACTS_WIDE_OFF;

/// The most lines a panel may ASK the region for.
///
/// Asking is not getting — the footer's ceiling is `expansion.maxLines` and a
/// window has the height somebody dragged it to — but a number that leaves
/// `Panel::panel_lines` is a number somebody multiplies, and a two-thousand-line
/// file must not be able to ask for two thousand lines of anything.
///
/// Sixty-four: more than any door's height, less than anything worth
/// multiplying.
pub(crate) const PANEL_WANT: usize = 64;

/// What the facts key says when there is nothing to scroll.
///
/// A key that does nothing and says nothing is the one outcome this project
/// treats as a defect: from the outside, a refusal and a dead key look the
/// same. It is `files-sidebar-only`'s shape, once more.
pub(crate) const FACTS_ALL_SHOWN: &str = ronoctua_i18n::key::FILES_FACTS_ALL_SHOWN;

/// One fact, in full, on the message line.
///
/// A panel is [`PANEL_FLOOR`] cells and a path is two hundred characters, so
/// the row that had to be cut is exactly the row somebody wants to read whole.
/// It goes where which-key already puts a description; nothing floats
/// (invariant 14).
pub(crate) const FACT_SAYS: &str = ronoctua_i18n::key::FILES_FACT_SAYS;

/// How far one key moves along a film, in seconds.
///
/// Five, which is about a sentence of speech and a shot of picture: enough to
/// be worth a key press and short enough to be held down.
pub(crate) const SEEK_STEP: f64 = 5.0;

/// How far one key turns a shape, in radians.
///
/// About nine degrees: forty presses go all the way round, which is few enough
/// to be worth holding the key down for and many enough to stop on the face
/// one wanted. The pointer is the coarse gesture and this is the fine one.
pub(crate) const SPIN_STEP: f32 = 0.15;

// How far a key moves the globe — the globe's own step, which every view
// that embeds one shares.
use crate::globe::GLOBE_STEP;

/// How much one key moves a shape nearer or further — the globe's own step,
/// the two figures of one panel answering one key the same way.
pub(crate) const ZOOM_STEP: f32 = crate::globe::KEY_ZOOM;

// How much one notch of the wheel does — the globe's, which the shape
// borrows so the two figures of one panel answer a wheel alike.
use crate::globe::WHEEL_ZOOM;

/// `Changed` when a gesture moved something, `Nothing` when it did not.
///
/// The four turn keys and the three zoom keys all answer the same way, and
/// writing it out seven times is seven chances to answer `Changed` for a
/// gesture that did nothing — which is a repaint per keypress on every file
/// that is not a shape.
pub(crate) const fn stepped(moved: bool) -> Step {
    if moved { Step::Changed } else { Step::Nothing }
}

/// One line of the text preview, cut into the coloured runs of its highlight.
///
/// # Bytes to CELLS, and the ruler is the clusters'
///
/// `Field::at` is in cells; a `Range` is in bytes. The conversion is
/// `layout::width`, which counts grapheme CLUSTERS — the same ruler
/// `Field::covers`, `elide_end` and both painters use. Counting `char`s would
/// put a field one cell off the column it is drawn in the first time an
/// accent arrives as a combining mark, and this file has already paid once for
/// having three rulers.
///
/// # And the cut comes AFTER the mapping, never before
///
/// Eliding the line first and applying the ranges to what is left would move
/// every run past the cut. So the line is walked whole, each piece is placed,
/// and the piece that crosses the edge is the one that gets the ellipsis — in
/// the colour of whatever it truncated, which is right: it belongs to that run.
///
/// # Every field carries the SAME target
///
/// Which is what makes `Field::covers`' one-cell overlap harmless: neighbouring
/// runs answer the same `Hit`, so wherever the pointer lands on a line of code
/// it means the same thing. A trailing field of blanks goes last so the panel
/// is clickable across its whole width, including the ragged right end — the
/// promise the picture path already makes.
fn text_fields(
    line: &ronoctua_inspect::text::Line,
    at: u16,
    room: u16,
    column: usize,
    target: Hit,
) -> Vec<Field> {
    let mut out = Vec::new();
    let mut cut = 0usize;
    let mut cell = 0usize;

    for r in &line.runs {
        // A RANGE OUT OF STEP WITH THE TEXT IS A PANIC ON SCREEN, so the
        // boundary is checked here too. The crate promises it and the tests
        // lock it; this is the belt beside the braces, because the cost of
        // being wrong is a dead footer rather than a wrong colour.
        if r.start < cut
            || r.end > line.text.len()
            || !line.text.is_char_boundary(r.start)
            || !line.text.is_char_boundary(r.end)
        {
            continue;
        }
        let plain = &line.text[cut..r.start];
        place(
            &mut out,
            plain,
            at,
            room,
            column,
            cell,
            ronoctua_tokens::color::TEXT_PRIMARY,
            &target,
        );
        cell += usize::from(ronoctua_render::layout::width(plain));
        let run = &line.text[r.start..r.end];
        let color = crate::chrome::syntax_color(r.role);
        place(&mut out, run, at, room, column, cell, color, &target);
        cell += usize::from(ronoctua_render::layout::width(run));
        cut = r.end;
    }
    let rest = &line.text[cut.min(line.text.len())..];
    place(
        &mut out,
        rest,
        at,
        room,
        column,
        cell,
        ronoctua_tokens::color::TEXT_PRIMARY,
        &target,
    );

    // AND THE WHOLE WIDTH IS CLICKABLE. Blanks draw no ink, and `field_at`
    // walks from the right, so this is what the pointer finds where a short
    // line leaves the panel ragged.
    out.push(Field {
        at,
        text: " ".repeat(usize::from(room)),
        color: ronoctua_tokens::color::TEXT_PRIMARY,
        hit: target,
        explain: None,
        ink: Ink::Grid,
    });
    out
}

/// One piece of a preview line, placed at the cell it starts in — less
/// whatever the sideways offset has taken off the left.
///
/// A free function and not a closure inside [`text_fields`], because the
/// closure would have to own the `Hit` it clones into every field and there is
/// one per run.
#[allow(clippy::too_many_arguments)]
fn place(
    out: &mut Vec<Field>,
    text: &str,
    at: u16,
    room: u16,
    column: usize,
    cell: usize,
    color: Color,
    target: &Hit,
) {
    let wide = usize::from(ronoctua_render::layout::width(text));
    // ENTIRELY OFF THE LEFT EDGE: nothing of it is on screen.
    if wide == 0 || cell + wide <= column {
        return;
    }
    // OR IT STARTS OFF THE EDGE, and only its tail is. `window` cuts by
    // cluster for the reason `elide_end` does: cutting by byte invents a
    // drawing.
    let skip = column.saturating_sub(cell);
    let placed = cell + skip - column;
    let left = usize::from(room).saturating_sub(placed);
    if left == 0 {
        return;
    }
    let piece = ronoctua_render::layout::window(
        text,
        u16::try_from(skip).unwrap_or(u16::MAX),
        u16::try_from(left).unwrap_or(u16::MAX),
    );
    if piece.is_empty() {
        return;
    }
    out.push(Field {
        at: at + u16::try_from(placed).unwrap_or(0),
        text: piece,
        color,
        hit: target.clone(),
        explain: None,
        ink: Ink::Grid,
    });
}

/// THE TWO PANELS' STATE: what they show, where each is scrolled to, what the
/// threads last said and were last told.
///
/// # Why the fields are the crate's
///
/// Because the manager's tests SET a state rather than walk to it — a panel
/// scrolled to line fifty, an answer already asked for — and they live in
/// `files.rs`, beside the view that holds this. Nothing else writes them: the
/// view asks the methods, which is what keeps every answer one answer.
#[derive(Debug)]
pub struct Panel {
    /// What the panels are about, told by the host before every question.
    ///
    /// A `RefCell` because a host asks from `&self` — a frame is drawn, and a
    /// pointer hit-tested, with the view borrowed immutably — and what it is
    /// looking at may have moved since the last question: a key walked the
    /// cursor, a directory was read again. See [`Panel::follow`].
    subject: RefCell<Option<Subject>>,
    /// What the host's layout said at the same moment.
    around: Cell<Around>,
    /// The language bundle, SHARED with the view and the door that opened it.
    texts: Rc<ronoctua_i18n::Texts>,
    /// The time zone every date of the sheet is said in.
    zone: Zone,
    /// Whether the PREVIEW panel is asked for.
    ///
    /// **ON at the start**, and the key is how one turns it OFF. It used to be
    /// the other way round, and the argument was the cost — a preview is a
    /// child process per file walked past. The owner overruled it from the
    /// screen: the panels are what the manager is FOR, and a feature that has
    /// to be switched on every time is a feature most of the time nobody has.
    /// The cost is still real and it is still bounded, by the one thread
    /// `inspector` runs and by its "only the newest" rule.
    pub(crate) preview_on: bool,
    /// Whether the INFO panel is asked for. On at the start, same argument.
    pub(crate) info_on: bool,
    /// The first fact the info panel shows.
    ///
    /// The panel is as tall as the region and a file can have twenty facts, so
    /// the list does not always fit — and a list that is cut with nothing
    /// saying so is the failure this whole file keeps naming. What says so is
    /// `files::View::facts_bar`, and this is what it points at.
    ///
    /// It goes back to zero whenever the CURSOR moves: the facts are about the
    /// focused entry, and keeping a scroll position across two different files
    /// would open the next one half way down a list it does not have.
    pub(crate) info_offset: usize,
    /// Which fact the keyboard is on, once the `TAB` ring has reached the
    /// panel.
    ///
    /// # Why a panel needed a cursor at all
    ///
    /// Because the owner asked for one — *"ao apertar `Tab` para ir para a
    /// seção de infos, deveria ser possível ir para cima e para baixo nos
    /// itens"* — and because a panel that only scrolled had nothing for a band
    /// to light, which is the other half of the same report: with the keyboard
    /// in a panel, nothing on screen changed at all.
    ///
    /// It is `side_cursor`'s twin, and it obeys the same rule: the view
    /// scrolls to keep it in sight rather than the cursor following the view.
    pub(crate) info_cursor: usize,
    /// What the tools said about the file the cursor was on.
    ///
    /// ONE, and not a map. The panels are about the focused entry and there is
    /// one of those; a cache of every file walked past would be the footer's
    /// memory, which is the argument `ronoctua_render::image` already makes
    /// about the textures it keeps between frames.
    pub(crate) looked: Option<Looked>,
    /// The frame a moving file is showing right now.
    ///
    /// It REPLACES the still one on screen and does not disturb it in
    /// `looked`: a file that stops moving — because the cursor left, or the
    /// preview was switched off — falls back to the picture it always had.
    pub(crate) frame: Option<crate::motion::Frame>,
    /// What the film is DOING — the transport, and the whole of it.
    ///
    /// Kept beside the frame and never inside it: a frame is what arrived, and
    /// this is what was asked for. It is reset whenever the cursor lands on
    /// another file, which is what makes leaving and coming back start the
    /// film again rather than resume somebody else's pause.
    pub(crate) transport: Transport,
    /// The last order the player was given.
    ///
    /// [`Panel::wants_motion`] is the difference between this and what SHOULD be
    /// running. One comparison, in one place: enumerating the ways a film can
    /// need restarting is how a case gets missed, which is the lesson
    /// `wants_look` already carries.
    pub(crate) told: Option<crate::motion::Reel>,
    /// Whether the player has been told to hold.
    ///
    /// Beside [`Transport::paused`] and not inside it, because they answer two
    /// different questions: that one is what somebody asked for, this one is
    /// what the player was told. The difference between them is the order.
    pub(crate) holding: bool,
    /// What went wrong with the film, when something did.
    ///
    /// **Refusal is a value, not an absence.** A picture that is not there and
    /// a picture that was refused look the same on screen, and only one of them
    /// has a sentence — the rule the panels already keep.
    pub(crate) trouble: Option<crate::motion::Trouble>,
    /// WHICH panel was asked for the whole region, if either was.
    ///
    /// A wish and not a state: [`Panel::wide`] is what the doors read, and it
    /// answers `None` the moment there is nothing to give the region TO.
    ///
    /// ONE FIELD AND NOT TWO FLAGS, because the region is one. A pair of
    /// booleans can both say yes, and then the layout has to choose between
    /// them — a third opinion about a question this shape answers by
    /// construction.
    pub(crate) theatre: Option<Which>,
    /// Where a shape is being looked at from.
    ///
    /// Beside `transport` and for the same reason: it is what was ASKED for,
    /// not what arrived. Reset when the cursor lands on another file, so
    /// coming back to a piece shows it the way it is first shown rather than
    /// at the angle somebody left a different piece at.
    pub(crate) pose: ronoctua_inspect::solid::Pose,
    /// The last view of a shape that arrived.
    pub(crate) turned: Option<crate::stage::Frame>,
    /// The last order the stage was given.
    ///
    /// [`Panel::wants_turn`] is the difference between this and what SHOULD be
    /// drawn — the same one comparison in one place that `told` is for the
    /// film, and for the same reason: enumerating the ways a picture can need
    /// redrawing is how a case gets missed.
    pub(crate) told_turn: Option<crate::stage::Turn>,
    /// What went wrong with the shape, when something did.
    pub(crate) shape_trouble: Option<ronoctua_inspect::Refusal>,
    /// Which section the pointer is resting in.
    ///
    /// What decides the transport's buttons are drawn — they appear under the
    /// pointer and the timeline does not, which is the owner's choice. It is a
    /// `Cell` for the reason `drawn_lines` is: `files::View::hit` runs on every
    /// pointer motion and takes `&self`.
    pub(crate) pointing: Cell<Option<Pane>>,
    /// The preview's own box, in pixels, as it was last laid out.
    ///
    /// What the frame's size is asked FOR. It cannot come from the door at the
    /// moment the player is told, because only the layout knows how many cells
    /// the panel got — so the layout writes it down, once, where it decides.
    pub(crate) drawn_preview: Cell<(f32, f32)>,
    /// The scale of the screen the preview is drawn on — what turns the box
    /// above, in LOGICAL pixels, into the frame a film or a turning piece is
    /// decoded at, in PHYSICAL ones: drawn into the logical box, the picture
    /// is then as sharp as the letters beside it (`crate::hidpi`).
    pub(crate) preview_scale: Cell<i32>,
    /// Which entry of the playlist under the cursor is the one being played.
    ///
    /// # Why it is here and not on `Transport` or on `Looked`
    ///
    /// Not on [`Transport`]: every field there is `Copy`, it is compared by
    /// value each frame and reset wholesale by two keys.
    ///
    /// Not on [`Looked`]: that is what the FILE says, boxed and compared field
    /// by field. Which entry somebody is on is what the PERSON did, and it
    /// survives none of the re-reads that answer is subject to.
    ///
    /// It is an index into `List::entries` — the drawn rows — and not into the
    /// playable ones, so the mark and the row are one arithmetic.
    pub(crate) playing: usize,
    /// The first line of the text preview that is on screen.
    ///
    /// Its own, like `info_offset` and `side_offset`: a gesture over the
    /// preview must move the preview and nothing else, which is the rule
    /// [`Panel::wheel`] writes down.
    pub(crate) preview_offset: usize,
    /// Which entry of a PLAYLIST the keyboard is on.
    ///
    /// Only a playlist has one: a photograph, a film and a shape have no rows,
    /// and a page of text has lines that are read rather than chosen. There the
    /// arrows go on doing what they did — scrolling, or walking the files — and
    /// what says the panel has the keyboard is its title.
    pub(crate) preview_cursor: usize,
    /// The first COLUMN of it, for a line wider than the panel.
    ///
    /// The owner asked for sideways scrolling rather than a silent cut: a
    /// twenty-four-cell panel would otherwise hide most of every line of code
    /// with an ellipsis and no way to see past it.
    pub(crate) preview_column: usize,
    /// The stamp of what was last ASKED for.
    ///
    /// It is what stops the host asking again, and it has to be separate from
    /// [`Panel::looked`] for the reason `surface`'s `icons` map already
    /// records: a refusal is an ANSWER, and without somewhere to write "I have
    /// asked and the answer was no" the same file is asked about on every
    /// frame.
    pub(crate) asked: Option<String>,
    /// How many lines the panels were last DRAWN in.
    ///
    /// The same arrangement as `files::View::drawn_at` and for the same reason: a
    /// key arrives without a height, and how far the facts can scroll depends
    /// on one. The view cannot work it out — the footer's region has a ceiling
    /// and a window has the height somebody dragged it to — so the door that
    /// draws the bar is the one that knows, and `files::View::facts_bar` is where
    /// every frame of both of them goes through.
    pub(crate) drawn_lines: Cell<u16>,
    /// How many CELLS wide the panels' content was last drawn in.
    ///
    /// The same arrangement as [`Panel::drawn_lines`] and for the same reason:
    /// [`Panel::preview_total`] has to say how tall a wrapped refusal is, and
    /// that depends on a width nine of its callers do not have. The layout is
    /// the one place that knows, so the layout writes it down.
    pub(crate) drawn_panel: Cell<u16>,
    /// How many CELLS wide the FACTS panel was last drawn in.
    ///
    /// Its own and not [`Panel::drawn_panel`], which is the PREVIEW's: the two
    /// panels are different widths whenever only one of them is on, and with
    /// the region given away one of them is not drawn at all. What depends on
    /// this is [`Panel::info_grid`] — how many columns the facts are laid out in
    /// — and a grid computed against the other panel's width is a grid the
    /// painter and the scrollbar disagree about.
    ///
    /// The layout is the one place that knows, so the layout writes it down;
    /// the same arrangement as the two cells above and for the same reason.
    pub(crate) drawn_info: Cell<u16>,
    /// WHETHER THE POINTER IS ON THE GLOBE, remembered where it was resolved.
    ///
    /// The wheel arrives with a cell and the answer needs a LINE as well, and
    /// the door already resolved both to draw the band. Asking again from
    /// inside `wheel` would be a second opinion about where the picture is —
    /// which is the whole of what `Panes::section_at` was written to end.
    pub(crate) pointing_at_the_globe: Cell<bool>,
    /// THE LAST SHEET BUILT, and what it was built for.
    ///
    /// # Why this is not an optimisation to be weighed
    ///
    /// Because `sheet_of` runs on EVERY painted frame — `sections_of` builds
    /// all four sections, and the panel is one of them — and it formats one
    /// Fluent message per row. With a dozen rows that is invisible. Once the
    /// panel started showing every tag the file carries, a photograph became
    /// 167 rows and a Pixel DNG 149, and it would have been that many
    /// formats per frame, forever, while nothing on screen changed.
    ///
    /// THE KEY IS WHAT CAN CHANGE THE ANSWER: the name in focus, its mtime
    /// (the same file rewritten in place is a different sheet), whether a
    /// directory has been measured, and which inspection has come back. Not
    /// the language — a language change rebuilds the `Rc<Texts>` this panel
    /// holds, and `texts` is compared by pointer for exactly that reason.
    pub(crate) remembered: RefCell<Option<(SheetFor, Vec<Fact>)>>,
    /// THE GLOBE THIS PANEL IS SHOWING, when the focused file has a coordinate.
    ///
    /// On the VIEW and not on the door, because a view lives in two doors and a
    /// pose kept in the door would reset the globe when the manager moved from
    /// the footer to a window. The thread is the door's; this is the value in
    /// between. See `crate::globe::Globe`, which belongs to no app.
    pub(crate) globe: RefCell<crate::globe::Globe>,
}

impl Panel {
    /// The language changed: the panels speak it from the next question on,
    /// and the facts remembered in the old one are let go — they were words,
    /// not readings, and a sheet kept by its subject would go on saying them.
    pub fn speak(&mut self, texts: Rc<ronoctua_i18n::Texts>) {
        self.texts = texts;
        self.remembered.replace(None);
    }

    /// Both panels on, about nothing yet.
    ///
    /// THE PANELS COME UP ON. The owner's decision, reported from the screen:
    /// "prévia e info devem estar sempre habilitadas".
    #[must_use]
    pub fn new(texts: Rc<ronoctua_i18n::Texts>, zone: Zone) -> Self {
        Self {
            subject: RefCell::new(None),
            around: Cell::new(Around::default()),
            texts,
            zone,
            preview_on: true,
            info_on: true,
            info_offset: 0,
            info_cursor: 0,
            looked: None,
            frame: None,
            transport: Transport::default(),
            told: None,
            holding: false,
            trouble: None,
            theatre: None,
            pose: ronoctua_inspect::solid::Pose::default(),
            turned: None,
            told_turn: None,
            shape_trouble: None,
            pointing: Cell::new(None),
            drawn_preview: Cell::new((0.0, 0.0)),
            preview_scale: Cell::new(1),
            playing: 0,
            preview_offset: 0,
            preview_cursor: 0,
            preview_column: 0,
            asked: None,
            drawn_lines: Cell::new(0),
            drawn_panel: Cell::new(0),
            drawn_info: Cell::new(0),
            pointing_at_the_globe: Cell::new(false),
            remembered: RefCell::new(None),
            globe: RefCell::new(crate::globe::Globe::new(crate::globe::Owner::Files)),
        }
    }

    /// Points the panels at a file — or at nothing.
    ///
    /// What a host WITHOUT a list calls, once, when its choice moves. The
    /// panels reset nothing here: a new subject starts at the top when its
    /// answer ARRIVES ([`Panel::looked`]), which is causal and covers every way
    /// there is of arriving.
    ///
    /// The manager's view is a host with a list, and tells
    /// ([`Panel::follow`]); the search's detail has none and calls this.
    pub fn focus(&self, subject: Option<Subject>) {
        *self.subject.borrow_mut() = subject;
    }

    /// What the layout of a host WITHOUT a list says about the panels:
    /// whether something modal covers them, and which of its sections has the
    /// keyboard.
    ///
    /// # A second call, and not a second argument of `focus`
    ///
    /// The two move apart: a question opens over the panels and the file is
    /// the same file; the keyboard walks into the facts and nothing was
    /// chosen. A host tells this one whenever its layout changes, and
    /// [`Panel::focus`] whenever its choice does. Without it the panels would
    /// be drawn under a modal that covers them and a globe in the facts would
    /// never get the zoom keys, which is what [`Around`] is read for. A host
    /// WITH a list says both with every question ([`Panel::follow`], which
    /// writes it here too: one writer of the field).
    pub fn around(&self, around: Around) {
        self.around.set(around);
    }

    /// What a host WITH a list calls before every question: what it is looking
    /// at, and what its layout says.
    ///
    /// # Told every time, and kept as nothing
    ///
    /// The listing's cursor is written at sixteen places, a directory is read
    /// again by a watch, a filter narrows the rows under it — and a subject
    /// kept here and updated at each of those would be a second copy of the
    /// cursor, stale the day one of them is forgotten. So the host tells, and
    /// what it tells is BORROWED: a [`Subject`] is built only when the entry,
    /// its row or its measurement differs from the last one.
    pub fn follow(&self, seen: Option<Seen<'_>>, around: Around) {
        self.around(around);
        let same = match (self.subject.borrow().as_ref(), seen.as_ref()) {
            (None, None) => true,
            (Some(was), Some(now)) => was.is(now),
            _ => false,
        };
        if !same {
            *self.subject.borrow_mut() =
                seen.map(|s| Subject::of_entry(s.folder, s.entry, s.roster, s.measured, s.row));
        }
    }

    /// Whether there is anything under the panels at all.
    #[must_use]
    pub fn has_a_subject(&self) -> bool {
        self.subject.borrow().is_some()
    }

    /// Whether the INFO panel is on screen: nothing covers it, and it has the
    /// region or it is on beside the preview.
    ///
    /// The answer `files::Panes::info` gives, from what the panel knows — which
    /// is all of it: the width decides WHERE a panel goes and never WHETHER.
    #[must_use]
    pub fn info_shown(&self) -> bool {
        !self.around.get().covered
            && match self.wide() {
                Some(which) => which == Which::Info,
                None => self.info_on,
            }
    }

    /// Whether the PREVIEW is on screen, the same way.
    #[must_use]
    pub fn preview_shown(&self) -> bool {
        !self.around.get().covered
            && match self.wide() {
                Some(which) => which == Which::Preview,
                None => self.preview_on,
            }
    }

    /// HOW MANY ROWS OF THE FACTS' CONTENT THE GLOBE TAKES, and the ONE place
    /// that decides.
    ///
    /// Zero unless the panel is on, the focused file carries a coordinate and
    /// this machine has world data — and then `space.atlas.lines`, clamped so
    /// at least two facts are left. Everything asks it; nothing else decides,
    /// which is the sentence `Section` opens with.
    pub fn globe_lines(&self) -> usize {
        if !self.info_shown() || !self.wants_a_globe() {
            return 0;
        }
        // AND THE SUBJECT IS ASKED, NEVER THE GLOBE.
        //
        // This asked the globe whether it was POINTED for one round, and that
        // is a deadlock with no symptom: the component is pointed by
        // `measured_globe`, the door only calls that when `globe_px` hands back
        // a box, and `globe_px` refuses a box of fewer than two lines. Zero lines reserved → no box →
        // nobody points it → zero lines for ever. The screen showed a photo
        // with a coordinate in it and a panel of plain facts, which reads as
        // the coordinate not being there.
        //
        // ITS FULL ASK OR NOTHING, and two facts left over.
        //
        // A disc squeezed into the two lines a bottom strip could spare is not
        // a globe — it is one line of picture under one line of address, which
        // says less than the coordinate it replaced. `space.atlas.lines` is
        // what the drawing was measured for; a panel that cannot give that
        // many shows the facts, the coordinate among them.
        //
        // AND THE STRIP GROWS SO THAT IT CAN, which is the other half of this:
        // see `files::View::panes`. Without it the window door — where the panels are
        // a four-line strip until the window is ninety-six cells wide — never
        // had the room and never drew a globe at all, which is what the owner
        // photographed.
        let room = usize::from(self.drawn_lines.get());
        let want = usize::from(ronoctua_tokens::space::ATLAS_LINES);
        if room < want + 2 { 0 } else { want }
    }

    /// THE GLOBE'S ROWS ARE CONTENT, AND THEY SCROLL WITH IT.
    ///
    /// # Why they stopped being chrome
    ///
    /// They were `Section::above` — the panel's first lines, pinned, the
    /// arrangement the listing's path and header have. Two things were wrong
    /// with that and the owner found both.
    ///
    /// The first is arithmetic: a pinned band makes the bar's track shorter
    /// than the panel (`Section::visible`) while the DRAG measured the panel
    /// (`Room::panel`), so grabbing the thumb over a file with a globe threw it
    /// far below the hand — *"a barra da scrollbar se desloca imediatamente
    /// muito abaixo da posição de onde está o cursor"*. With the rows inside
    /// the scroll there is no `above` to disagree about.
    ///
    /// The second is what he asked for next, and it is the better screen: the
    /// globe costs six of the footer's lines, and pinned it costs them
    /// for as long as the file is under the cursor. Scrolling gives them back —
    /// *"ao arrastar a scrollbar ele se move junto liberando espaço para linhas
    /// de conteúdo"* — which is also what retired `C-g`.
    ///
    /// # And the PICTURE shrinks rather than sliding out
    ///
    /// Because nothing in this renderer clips: an image drawn half outside its
    /// panel is drawn over the listing above it, not cut. So what the door is
    /// given is the rows the globe still HAS — `globe_rows() - offset` — and
    /// `Area::centre` fits the picture into them. The map gives its lines to
    /// the facts from the bottom up, and at `offset >= globe_rows()` there is
    /// no picture to ask a thread for at all.
    #[must_use]
    pub fn globe_rows(&self) -> usize {
        if self.globe_columns() > 0 {
            return 0;
        }
        self.globe_lines()
    }

    /// Whether this file, on this machine, has a globe to draw at all.
    ///
    /// THE SUBJECT AND NOTHING ELSE: what the file carries and whether this
    /// build has the world table. Both are known before a single line is
    /// measured, which is what
    /// lets the LAYOUT ask it — `files::View::panes` grows the strip by it, and a
    /// predicate that looked at the drawing would be the deadlock this file
    /// records one function up.
    ///
    /// **It does not ask `Globe::absent`.** That is the answer for ONE
    /// coordinate and it arrives from a thread a frame or two later; a strip
    /// whose height followed it would change under the hand. When it comes back
    /// absent the lines stay and the address line says so — which is the same
    /// arrangement the preview has for a refusal: a refusal is a VALUE.
    #[must_use]
    pub fn wants_a_globe(&self) -> bool {
        self.looked.as_ref().is_some_and(|l| l.at.is_some()) && crate::globe::present()
    }

    /// How many ROWS of content the panel has room for, which is all its lines.
    ///
    /// NOTHING IS SUBTRACTED ANY MORE, and that is the repair: the globe's rows
    /// scroll with the facts (see [`Panel::globe_rows`]) and the blank row at the
    /// end is content too (see [`BLANK`]). A number here that the DRAG did not
    /// also subtract is the misalignment the owner reported.
    pub fn info_room(&self) -> usize {
        usize::from(self.drawn_lines.get())
    }

    /// The facts' offset, KEPT INSIDE WHAT THERE IS TO SCROLL.
    ///
    /// # Why it is clamped when it is read and not when it is written
    ///
    /// Because what makes it too big is not a write: it is the GRID changing
    /// underneath it. `C-W` turns one column of forty rows into three of
    /// fourteen, and an offset of thirty is then past the end — the owner read
    /// exactly that off the screen: *"ao arrastar a scrollbar totalmente para
    /// baixo no modo não expandido, e depois trocar para o modo expandido, todo
    /// o conteúdo … sobe a ponto de sumir da tela"*.
    ///
    /// Clamping on READ also keeps the place: give the region back and the
    /// stored offset is inside range again, so the sheet is where it was left.
    pub(crate) fn facts_offset(&self, total: usize) -> usize {
        self.info_offset
            .min(total.saturating_sub(self.info_room().max(1)))
    }

    /// HOW MANY COLUMNS OF `FACT_COLUMN` THE PANEL'S WIDTH HOLDS, at least one.
    ///
    /// Measured against the panel's own width and not the content's, for the
    /// reason `sections_of` writes out about the same cell: `content_room` is a
    /// cell narrower when there is a scrollbar, and whether there IS one is the
    /// question this answer decides.
    fn fits_columns(&self) -> usize {
        usize::from(self.drawn_info.get() / FACT_COLUMN).max(1)
    }

    /// HOW MANY COLUMNS THE GLOBE TAKES — one, or none at all.
    ///
    /// # Why it is a column there and a band here
    ///
    /// Beside the listing the facts are one narrow column, so the only place a
    /// picture can go is across their top — that is [`Panel::globe_rows`], and
    /// those rows SCROLL, which is what gives their lines back.
    ///
    /// With the region given away that is the wrong shape twice over: the band
    /// is as wide as the screen and a disc never grows, so most of it is empty,
    /// and the facts start a picture's height down the page. The owner
    /// photographed exactly that — *"há um enorme espaço em branco
    /// disperdiçado acima das colunas de conteúdo … não há uma coluna com o
    /// mapa"*. So the globe becomes the FIRST COLUMN, the height of the panel,
    /// and the facts fill the rest.
    ///
    /// **Only where the facts still have a column of their own.** A globe that
    /// took the one column there was room for would leave the sheet nowhere to
    /// be, which is a picture winning an argument against the thing it
    /// annotates.
    fn globe_columns(&self) -> usize {
        if self.wide() != Some(Which::Info) || !self.wants_a_globe() {
            return 0;
        }
        usize::from(self.fits_columns() >= 2)
    }

    /// THE FACTS' GRID: how many columns the GLOBE takes, how many the FACTS
    /// take, and how many ROWS that makes them. The ONE place that decides.
    ///
    /// `(0, 1, facts + 1)` in the ordinary panel, which is what it has always
    /// been plus the blank row at the end (see [`BLANK`]). With the region
    /// given away it becomes a newspaper: the facts fill the first column top
    /// to bottom and then the next, so a ROW of the panel holds one fact out of
    /// each column, and the globe — if there is one — has the leftmost column
    /// to itself.
    ///
    /// # Why the columns arrive only with the region
    ///
    /// Because a panel beside the listing is already a column of a page, and a
    /// second column inside it would be a second thing to read past in
    /// twenty-four cells. The owner asked for the columns as the EXPANDED
    /// view's own answer — *"ao expandir, todo o conteúdo seja dividido em
    /// algumas colunas, dentro do espaço disponível"* — and nothing about the
    /// narrow panel changes because of it.
    ///
    /// # Why the count comes from the width AND from the facts
    ///
    /// From the width, because that is the requirement in his own words — *"a
    /// quantidade de colunas depende do tamanho disponível na tela"* — and
    /// [`FACT_COLUMN`] is what a column is measured against.
    ///
    /// And from the facts, because a column nobody fills is white space with a
    /// ragged edge down it: eight facts in five columns is one fact each and
    /// four columns of nothing. Never more columns than the facts need in order
    /// to fit the lines there are.
    pub(crate) fn grid_of(&self, facts: usize) -> (usize, usize, usize) {
        // THE BLANK ROW IS ONE OF THE SHEET'S, counted here so that every
        // caller — the bar, the wheel, the paging key and the painter — gets
        // the same answer about how far there is to scroll.
        let all = facts.saturating_add(BLANK);
        if self.wide() != Some(Which::Info) {
            return (0, 1, all);
        }
        let globe = self.globe_columns();
        let lines = self.info_room().max(1);
        let room = self.fits_columns().saturating_sub(globe).max(1);
        let columns = room.min(all.div_ceil(lines).max(1));
        (globe, columns, all.div_ceil(columns))
    }

    /// The same grid, asked without the sheet in hand.
    ///
    /// `sheet()` is remembered but it hands back a CLONE, so the painter —
    /// which already holds the sheet — asks [`Panel::grid_of`] instead. This is
    /// for the keys and the wheel, which arrive with nothing.
    #[must_use]
    pub fn info_grid(&self) -> (usize, usize, usize) {
        self.grid_of(self.sheet().len())
    }

    /// HOW MANY ROWS OF CONTENT THE FACTS PANEL HAS: the globe's, then the
    /// grid's. What the bar, the wheel, the paging key and the DRAG all
    /// measure against, and the one place it is added up.
    #[must_use]
    pub fn info_total(&self) -> usize {
        self.globe_rows().saturating_add(self.info_grid().2)
    }

    /// WHERE THE GLOBE GOES inside the facts panel: how many LINES it is given
    /// and how many CELLS wide it is. `None` when there is none to draw.
    ///
    /// The one answer both doors ask, because the picture that is DRAWN has to
    /// be the picture that was MEASURED — and there are two shapes now, the
    /// band across the panel's top and the column of the expanded sheet.
    #[must_use]
    pub fn globe_place(&self) -> Option<(usize, u16, usize)> {
        if self.globe_columns() > 0 {
            let (globe, columns, _) = self.info_grid();
            let wide = u16::try_from(globe + columns).unwrap_or(1).max(1);
            // THE BAR'S CELL PAID FOR WHETHER OR NOT THERE IS ONE.
            //
            // The fields divide `content_room`, which is a cell narrower when
            // the sheet scrolls; this divides the panel's own width, because
            // the grid must not be measured against an answer that depends on
            // it. Taking the cell off here makes the picture's column the
            // narrower of the two in every case — a cell of air on a picture's
            // right is invisible, and a cell of overlap is the picture touching
            // the next column's first letter.
            return Some((
                usize::from(self.drawn_lines.get()),
                self.drawn_info.get().saturating_sub(BAR) / wide,
                // A COLUMN DOES NOT SCROLL, so none of it has gone.
                0,
            ));
        }
        // THE ROWS IT STILL HAS, and how many have gone.
        //
        // Its band SCROLLS, so `offset` rows of it are off the top — and the
        // picture keeps its SIZE while they go, because what is cut is the
        // DRAWING and not the box (`crate::globe::paint`). The owner read the
        // other answer off the screen: *"o globo vai encolhendo … quero que o
        // globo mantenha o tamanho"*.
        let air = self.globe_rows();
        let gone = self.facts_offset(air.saturating_add(self.info_grid().2));
        let left = air.saturating_sub(gone);
        (left > 0).then(|| (left, self.drawn_info.get(), gone.min(air)))
    }

    /// How many LINES the panels want, when they sit on the right.
    ///
    /// The taller of the two, and it is what makes `row_count` and `rows_from`
    /// agree: a region that reserved lines nothing draws into is blank space
    /// below the listing, which reads as the manager having lost the rest of
    /// it. The preview asks for [`preview_lines`] whatever is in it — a picture
    /// one line tall is not a picture — and the info panel asks for as many
    /// lines as it has facts.
    ///
    /// **Asking is not getting.** The footer's region has a ceiling in lines
    /// (invariant 14) and a photograph has more facts than that, so what the
    /// panel is GIVEN is the door's business and can be less than this.
    /// What says so on screen is `files::View::facts_bar`; before it there was
    /// nothing, and the facts past the ceiling were simply not there.
    pub(crate) fn panel_lines(&self) -> usize {
        // NOTHING UNDER THE CURSOR, NOTHING TO ASK FOR. A query that matches no
        // file leaves both panels with no subject, and a region that stayed at
        // its ceiling to hold two empty boxes is a region that stopped
        // answering the person typing. `leader-vm` is what caught it: typing
        // `zzz` has to narrow the list, and with the panels on by default it
        // did not — 288 px before and 288 px after.
        //
        // It is the panels' own rule, not an exception to it: they are always
        // ON, and what they ask for is what they have to say.
        if self.subject.borrow().is_none() {
            return 0;
        }
        // AND WITH THE REGION GIVEN AWAY THE PREVIEW ASKS FOR EVERYTHING.
        //
        // `C-w` means "the whole region", so asking for a picture's four lines
        // would be the key contradicting itself: the footer's region grows to
        // what its panels ask for, and it stayed four lines tall. The ceiling
        // still holds (invariant 14) — this is a request, and the door's own
        // limit in lines is what answers it.
        if self.whole() {
            return usize::from(ronoctua_tokens::space::EXPANSION_MAXLINES);
        }
        // AND THE PANELS ASK FOR WHAT THEY HAVE TO SAY, which is a picture's
        // box or a sheet's rows.
        //
        // This is what makes the region grow to its ceiling whenever the
        // manager is open, and that is deliberate rather than a side effect:
        // the panels are on from the start now, and a panel given four lines
        // out of a sheet of ten is a panel one has to scroll to read at all.
        // The ceiling still holds (invariant 14), and `files::View::facts_bar` is
        // what says the sheet was cut when even the ceiling is not enough —
        // which for a photograph it never is.
        //
        // The cost is that the region's HEIGHT stops being an answer to what
        // was typed; `leader-vm` used to read it that way and now reads the
        // text instead, which is what it was actually asking about.
        let mut lines = 0usize;
        if self.preview_on {
            // A PICTURE'S BOX, OR THE FILE'S OWN LINES.
            //
            // Without the second half a markdown file would be given four
            // lines in the footer — the picture's box — because the sheet
            // beside it is short. The owner asked to be able to scroll the
            // preview "to see a good part of the content", and four lines is
            // not a good part of anything.
            //
            // `PANEL_WANT` is a CEILING and it is not decoration: `take` in
            // this file was once handed `usize::MAX` as a sentinel and
            // twenty-seven tests grew a `Vec` of two-to-the-sixty-four rows
            // until the machine's fourteen gibibytes were gone. A number that
            // leaves this function is a number somebody multiplies.
            lines =
                lines.max(usize::from(preview_lines()).max(self.preview_total().min(PANEL_WANT)));
        }
        if self.info_on {
            // AND THE GLOBE'S LINES ON TOP OF THE FACTS', because they do not
            // share: the globe takes the panel's first lines and the facts get
            // what is left. Without this the region would grow to the sheet's
            // height and the globe would push exactly that many facts off the
            // bottom.
            //
            // `globe_lines` asks `drawn_lines`, which is the LAST frame's — so
            // on the very first frame the globe asks for nothing and the panel
            // settles a frame later. That is the same one-frame lag `fold`
            // already has, and for the same reason: what the region can spare
            // is remembered where it was measured, never asked from inside the
            // sum (`the-footer-does-not-ask-itself`).
            // AND THE SUBJECT IS ASKED HERE TOO, for the reason `globe_lines`
            // writes out: a globe that is only pointed once a box exists can
            // never ask for the lines that make the box.
            let mine = if self.wants_a_globe() {
                usize::from(ronoctua_tokens::space::ATLAS_LINES)
            } else {
                0
            };
            // AND THE BLANK ROW ON TOP OF BOTH, so a sheet that fits does not
            // grow a scrollbar for the empty line at the end of it.
            lines = lines.max(self.sheet().len().saturating_add(mine) + BLANK);
        }
        // AND THE TITLE IS A LINE THEY NEED. It does not scroll, so a panel
        // given exactly as many lines as it has to SAY would push its last row
        // out and grow a scrollbar for it — a bar about one line, which is the
        // kind of chrome about nothing the listing's own bar refuses to be.
        //
        // Only when there is something to title. Asking for a line here with
        // both panels off would grow the region for a panel that is not drawn.
        lines
    }

    /// One line of the two panels, wherever they sit.
    ///
    /// `n` is the line INSIDE the panels' block: with them on the right it is
    /// the listing's own line number, and down in the strip it is the strip's.
    /// The panels do not know which, and that is the point — one builder, two
    /// places, and no branch that could answer differently in each.
    pub(crate) fn panel_fields(
        &self,
        n: usize,
        rows: usize,
        boxes: &[(Which, u16, u16)],
        sheet: &[Fact],
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<Field> {
        boxes
            .iter()
            .flat_map(|(which, at, room)| match which {
                Which::Preview => self.preview_fields(n, rows, *at, *room, texts),
                Which::Info => self.info_fields(n, rows, *at, *room, sheet),
            })
            .collect()
    }

    /// The preview panel's line `n`.
    ///
    /// **THE PANEL ALWAYS DRAWS SOMETHING.** With a picture the fields are
    /// BLANK — the texture is not text, and what these carry is the click
    /// target, so that clicking a picture of a file opens it exactly as
    /// clicking its name does.
    ///
    /// # With no picture it is a SENTENCE, and no longer a glyph
    ///
    /// It used to draw the type's icon, big and centred, with the reason under
    /// it. The owner asked for the icon to go: *"when there is no preview,
    /// instead of showing an icon it should say, in a very discreet
    /// formatting, that there is no preview"* — and he is right about what the glyph was doing.
    /// It said what the file IS, which the listing's own icon column already
    /// says on the row the cursor is on, and it said it at the size of a
    /// picture, so an empty panel looked like a panel with something in it.
    ///
    /// So: one dim line, and it says the REASON when the tools gave one and
    /// the plain sentence when they did not. A blank panel and a refused panel
    /// must not look the same — that is `ronoctua-inspect`'s rule, "refusal is
    /// a VALUE, not an absence" — and now neither of them is blank.
    /// The transport's own line: the two buttons, the timeline and the clock.
    ///
    /// ```text
    /// ▶  🔇   ▁▁▁▁▁▁▁●▁▁▁▁▁▁▁▁▁   1:23 / 5:40
    /// ```
    ///
    /// # It is drawn ON the film, and all of it arrives with the pointer
    ///
    /// The owner asked for both: *"quero que todos os controles do player
    /// apareçam por cima do vídeo/gif, de forma a deixar o vídeo/gif ocupando
    /// toda a altura possível"*, and *"tudo deve sumir e aparecer apenas com o
    /// hover do mouse sobre QUALQUER parte do elemento de preview"*.
    ///
    /// So the picture keeps the whole panel and this row is painted over its
    /// last line, on a half-transparent band so the glyphs read against a bright
    /// frame as well as a dark one. And nothing here is drawn unless the
    /// pointer is somewhere in the preview — the timeline included, which used
    /// to stay.
    ///
    /// The CELLS are still fixed whatever the pointer does. That is not the
    /// same question: what comes and goes is the ink, and a track that changed
    /// LENGTH on hover would slide the play cursor out from under the pointer
    /// in the instant of the click.
    ///
    /// The track itself is drawn by the DOOR, as a rectangle, for the reason
    /// every other bar is: where it is drawn is where it is grabbed, and a row
    /// of glyphs cannot promise that. What is here is the room it occupies and
    /// the two things one clicks.
    pub(crate) fn transport_fields(
        &self,
        at: u16,
        room: u16,
        time: Option<&ronoctua_design::Scroll>,
    ) -> Vec<Field> {
        // A BACKING FIELD FIRST, AND ALWAYS — even with the controls away.
        //
        // Without it this row carries nothing, `Row::field_at` answers `None`,
        // and `files::View::hit` therefore answers `None` for every point on it. The
        // consequence is the one the owner ruled out in as many words —
        // *"inclusive deve se manter em tela com o hover nos próprios
        // controles"*: reaching for the play glyph would land on a line that
        // resolves to no section, the pointer would stop being "in the
        // preview", and the controls would vanish from under the hand going
        // for them.
        let mut out = vec![Field {
            at,
            text: " ".repeat(usize::from(room)),
            color: ronoctua_tokens::color::TEXT_PRIMARY,
            // AND THE TRANSPORT'S BACKING FIELD TOGGLES TOO, so the whole row
            // answers the way the picture above it does — including the air
            // between the buttons and the clock.
            hit: Hit::Play,
            explain: None,
            ink: Ink::Grid,
        }];
        // AND NOTHING MORE WITHOUT THE POINTER — for a film. The row still
        // holds the panel's last line either way, so the picture does not move
        // when they arrive.
        //
        // A SOUND KEEPS ITS CONTROLS, which is the owner's word and the reason
        // this asks `transport_inked` rather than the pointer directly.
        if !self.transport_inked() {
            return out;
        }
        let at = at.saturating_add(TRANSPORT_LEAD);
        let holding = self.transport.paused || !self.transport.on;
        let glyph = |g: char| g.to_string();
        out.push(Field {
            at,
            text: glyph(if holding {
                ronoctua_tokens::icon::PLAY
            } else {
                ronoctua_tokens::icon::PAUSE
            }),
            color: ronoctua_tokens::color::TEXT_PRIMARY,
            hit: Hit::Play,
            explain: Some(Explanation {
                key: if holding {
                    ronoctua_i18n::key::FILES_PLAY_RESUME
                } else {
                    ronoctua_i18n::key::FILES_PLAY_HOLD
                },
                args: Vec::new(),
            }),
            ink: Ink::Grid,
        });
        // AND THE SOUND, WHICH A SILENT FILM DOES NOT OFFER. A control that
        // does nothing where it is drawn is the shape `menu::Applies` exists to
        // refuse — and here it would also be a lie, because ffmpeg refuses the
        // whole command when there is no audio stream to fill the output with.
        if self.has_sound() {
            out.push(Field {
                at: at.saturating_add(TRANSPORT_SOUND),
                text: glyph(if self.transport.sound {
                    ronoctua_tokens::icon::VOLUME
                } else {
                    ronoctua_tokens::icon::VOLUME_OFF
                }),
                color: ronoctua_tokens::color::TEXT_PRIMARY,
                hit: Hit::Sound,
                explain: Some(Explanation {
                    key: if self.transport.sound {
                        ronoctua_i18n::key::FILES_SOUND_OFF
                    } else {
                        ronoctua_i18n::key::FILES_SOUND_ON
                    },
                    args: Vec::new(),
                }),
                ink: Ink::Grid,
            });
        }
        // AND THE CLOCK, at the right-hand end, in the file's own terms. A film
        // with no length has none, which is the GIF: it loops, and "0:03 / "
        // with nothing after it says less than nothing.
        if let Some(time) = time {
            let said = clock_text(time);
            let wide = width_of(&said);
            out.push(Field {
                at: at
                    .saturating_add(room)
                    .saturating_sub(TRANSPORT_LEAD)
                    .saturating_sub(TRANSPORT_LEAD)
                    // AND THE BUTTON PAST IT, which is where the owner asked
                    // for it. `track_px` takes the same cells off the other
                    // end of the track, so the air stays identical.
                    .saturating_sub(TRANSPORT_THEATRE)
                    .saturating_sub(wide),
                text: said,
                color: ronoctua_tokens::color::TEXT_PRIMARY,
                hit: Hit::Nothing,
                explain: None,
                ink: Ink::Grid,
            });
        }
        // AND THE EXPAND BUTTON, at the very end of the row.
        //
        // WITH THE PLAYER, because that is where the owner put it: *"esse botão
        // deve aparecer junto com o player à direita da marcação de tempo"*.
        // Without one it goes in the panel's corner instead — see
        // `theatre_field`.
        out.push(
            self.theatre_field(
                at.saturating_add(room)
                    .saturating_sub(TRANSPORT_LEAD)
                    .saturating_sub(TRANSPORT_LEAD),
                Which::Preview,
            ),
        );
        out
    }

    /// The button that gives a panel the whole region, and gives it back.
    ///
    /// One field, drawn in three places — on the transport's row beside the
    /// clock, in the preview's bottom-right corner when there is no transport,
    /// and in the facts' bottom-right corner. The GLYPH says which way it goes,
    /// which is the owner's own requirement: *"um botão de expandir (e de
    /// reduzir, quando estiver expandindo)"*.
    ///
    /// `which` is the panel whose corner this is, and it is carried rather than
    /// derived: both corners can be on screen at the same moment, and a button
    /// that only said "expand" would say nothing about what.
    fn theatre_field(&self, at: u16, which: Which) -> Field {
        let wide = self.wide() == Some(which);
        // THE SENTENCE NAMES THE PANEL, because with two of them "the whole
        // region" no longer says which thing is about to fill it.
        let (on, off) = match which {
            Which::Preview => (
                ronoctua_i18n::key::FILES_THEATRE_ON,
                ronoctua_i18n::key::FILES_THEATRE_OFF,
            ),
            Which::Info => (
                ronoctua_i18n::key::FILES_FACTS_WIDE_ON,
                ronoctua_i18n::key::FILES_FACTS_WIDE_OFF,
            ),
        };
        Field {
            at,
            text: if wide {
                ronoctua_tokens::icon::THEATRE_OFF
            } else {
                ronoctua_tokens::icon::THEATRE
            }
            .to_string(),
            color: ronoctua_tokens::color::TEXT_PRIMARY,
            hit: Hit::Theatre(which),
            // AND IT SAYS WHICH WAY IT GOES on the line above, as every drawn
            // control in this manager does.
            explain: Some(Explanation {
                key: if wide { off } else { on },
                args: Vec::new(),
            }),
            ink: Ink::Grid,
        }
    }

    /// Whether the transport's controls are on screen right now.
    ///
    /// What the doors ask before painting the band, the track and the cursor —
    /// all three arrive and leave together.
    #[must_use]
    pub fn transport_showing(&self) -> bool {
        self.plays_here() && self.transport_inked()
    }

    /// Whether the transport's INK is drawn — as opposed to its cells.
    ///
    /// # A film's controls come and go, and a sound's never leave
    ///
    /// A film's hide with the pointer, which is the owner's decision and what
    /// keeps the picture whole: the controls sit ON the film, and a band
    /// across it permanently is chrome across a picture.
    ///
    /// A SOUND has nothing under them to uncover — the panel IS the player —
    /// and the owner asked for them to stay. A player one has to find with the
    /// mouse before one can stop it is not a player.
    ///
    /// The CELLS are reserved either way, by `plays_here`, so the row never
    /// changes height under a pointer and the track never slides out from
    /// under a click.
    fn transport_inked(&self) -> bool {
        self.sound_here() || self.pointing_at_preview()
    }

    /// Whether what is under the cursor is a SOUND rather than a picture.
    fn sound_here(&self) -> bool {
        self.plays_here()
            && self.looked.as_ref().is_some_and(|l| {
                // A LIST IS SOUND BY ANOTHER NAME. Its controls stay for the
                // same reason a track's do: the panel IS the player, and
                // there is nothing under them to uncover.
                l.list.is_some()
                    || l.moving
                        .as_ref()
                        .is_some_and(|m| matches!(m.show, Show::Sound { .. }))
            })
    }

    /// Whether the pointer is anywhere in the facts.
    ///
    /// ANYWHERE, as the preview's twin below reads it: `files::View::hit` resolves the
    /// SECTION first, so resting on a value, on a label or on the air beside
    /// them all keep the button out.
    fn pointing_at_info(&self) -> bool {
        self.pointing.get() == Some(Pane::Info)
    }

    /// Whether the pointer is anywhere in the preview.
    ///
    /// ANYWHERE, which is the owner's word: the picture, the controls, the air
    /// beside them. `files::View::hit` resolves the SECTION first, and every line of
    /// the preview carries a field — so resting on the play glyph keeps the
    /// controls up exactly as resting on the film does.
    fn pointing_at_preview(&self) -> bool {
        self.pointing.get() == Some(Pane::Preview)
    }

    /// Whether what is under the cursor carries any sound.
    pub(crate) fn has_sound(&self) -> bool {
        self.looked.as_ref().is_some_and(|l| {
            // A LIST NAMES SOUND, and the key that turns it off must be
            // offered — a control that is not drawn is a key one cannot
            // discover, which is the argument the silent film's refusal makes
            // from the other side.
            l.list.is_some() || l.moving.as_ref().is_some_and(|m| m.shape.sound)
        })
    }

    pub(crate) fn preview_fields(
        &self,
        n: usize,
        rows: usize,
        at: u16,
        room: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<Field> {
        let out = self.preview_content(n, rows, at, room, texts);
        // AND THE EXPAND BUTTON, IN THE CORNER — the two cases without a
        // player.
        //
        // The owner gave both: *"quando não houver player, deve aparecer com o
        // hover do mouse no canto inferior direito nos casos de conteúdo grande
        // que ocupe o espaço de todo o preview, e para quando o conteúdo não
        // for grande, deve aparecer no canto inferior direito do preview mesmo
        // quando não houver hover do mouse, pois nesse caso não haverá conteúdo
        // abaixo dele, então ele não cobrirá nada"*.
        //
        // So the rule is exactly "does it cover anything?": over content it
        // waits for the pointer, and over a blank corner it simply stays. With
        // a player it is not here at all — it rides the transport's row, beside
        // the clock.
        // A PANEL WITH NO FILE UNDER THE CURSOR HAS NOTHING TO EXPAND, and
        // `preview_content` answers with no fields at all there. The guard is
        // the FILE and not the emptiness of the line: a short sentence leaves
        // the last line empty too, and that is exactly the case the button is
        // always drawn in.
        // THE BUTTON IS NOT PUSHED HERE. It goes on the panel's LAST line,
        // which this loop cannot see — the transport and the sideways bar are
        // added after it. See `files::View::sections_of`.
        let _ = (n, rows);
        out
    }

    /// Whether the preview's last line has something on it already.
    ///
    /// A picture, a film and a shape take the whole panel by construction; a
    /// list, a text and a wrapped refusal reach the last line only when they
    /// are long enough, and where the panel is scrolled to decides that.
    fn theatre_covers(&self, rows: usize) -> bool {
        if self.looking_still()
            || self
                .looked
                .as_ref()
                .is_some_and(|l| l.rgba.is_some() || l.moving.is_some() || l.turns)
        {
            return true;
        }
        self.preview_total().saturating_sub(self.preview_offset) >= rows
    }

    /// The preview's own content, a line at a time.
    fn preview_content(
        &self,
        n: usize,
        rows: usize,
        at: u16,
        room: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<Field> {
        let subject = self.subject.borrow();
        let Some(subject) = subject.as_ref() else {
            return Vec::new();
        };
        let path = subject.joined();
        let looked = self.looked.as_ref().filter(|l| l.path == path);
        // WHAT A CLICK ON THE PREVIEW DOES, and it is no longer "open it".
        //
        // The owner asked for both halves: *"ao clicar com o mouse em qualquer
        // preview, não quero mais que abra o arquivo; e ao clicar em vídeos ou
        // gif animado, quero que ocorra um toggle no play/pause"*.
        //
        // Opening was the old answer and it was a hazard on a panel one now
        // points at constantly — the controls only appear under the pointer, so
        // reaching for them means putting the pointer on the picture, and a
        // stray click launched the file in another program.
        let target = if looked.is_some_and(|l| l.moving.is_some()) {
            Hit::Play
        } else {
            Hit::Nothing
        };
        // A FIELD OF BLANKS, and not "no field at all". A picture has no text
        // to hit-test against, and a panel one cannot click would be the mouse
        // reaching less than the keyboard — the one direction
        // `files-everything-has-a-key` rules out.
        //
        // THREE CASES SHARE IT, and the last two are the owner's report.
        //
        // A picture; a file that MOVES, which now has no still at all and whose
        // frames go over these blanks; and a file still being LOOKED AT. That
        // third one is *"momentos antes do preview aparecer, é mostrada a
        // mensagem 'no preview', que é desnecessária, pois há um preview sendo
        // carregado"* — the answer had not arrived, so the panel fell through
        // to the sentence for "nothing here", and the sentence was replaced by
        // a picture a fraction of a second later. An empty panel is the honest
        // drawing of "not yet".
        // A SHAPE IS THE FOURTH CASE, and it is the same one as the film: it
        // has no still either, its views go over these blanks, and without it
        // the panel falls through to "no preview" — a sentence about a file
        // that is about to be drawn, which is the very report this arm was
        // added for.
        let coming = self.looking_still();
        if coming || looked.is_some_and(|l| l.rgba.is_some() || l.moving.is_some() || l.turns) {
            // A SENTENCE OVER THE PICTURE, and only when part of the piece is
            // not filled.
            //
            // A boundary representation is meshed face by face, and one this
            // reader cannot mesh is drawn as its own outline instead — see
            // `ronoctua_inspect::mesh::brep`. A piece that comes out part
            // solid and part wire is a piece somebody should be TOLD about,
            // and the panel is where they are already looking.
            //
            // It goes on the LAST line, and it reads because of the drawing
            // order the transport band already relies on: rectangles, then
            // images, then text.
            let outlined = looked.map_or(0, |l| l.outlined);
            if outlined > 0 && rows > 0 && n + 1 == rows {
                // `t_args` AND NOT `t_with`: the sentence has a plural branch,
                // and Fluent picks one on a NUMBER. A string carries no branch
                // and would print "1 faces".
                let said = texts.t_args(
                    ronoctua_i18n::key::FILES_PREVIEW_OUTLINED,
                    &[(
                        "n",
                        ronoctua_i18n::Arg::Number(i64::try_from(outlined).unwrap_or(i64::MAX)),
                    )],
                );
                let said = ronoctua_render::layout::elide_end(&said, room);
                return vec![Field {
                    at: centred(at, room, width_of(&said)),
                    text: said,
                    color: ronoctua_tokens::color::TEXT_DIM,
                    hit: target,
                    explain: None,
                    ink: Ink::Grid,
                }];
            }
            return vec![Field {
                at,
                text: " ".repeat(usize::from(room)),
                color: ronoctua_tokens::color::TEXT_PRIMARY,
                hit: target,
                explain: None,
                ink: Ink::Grid,
            }];
        }
        // A LIST OF THINGS TO PLAY, which is a list and not a picture and not
        // text — `audio/x-mpegurl` descends from `text/plain`, so without this
        // arm an `.m3u` would be shown as source code with its paths coloured.
        if let Some(list) = self.showing_list() {
            let row = n.saturating_add(self.preview_offset);
            if list.cut && row == list.entries.len() {
                let said = texts.t_with(
                    ronoctua_i18n::key::FILES_PREVIEW_CUT,
                    &[("lines", &list.entries.len().to_string())],
                );
                let said = ronoctua_render::layout::elide_end(&said, room);
                return vec![Field {
                    at: centred(at, room, width_of(&said)),
                    text: said,
                    color: ronoctua_tokens::color::TEXT_DIM,
                    hit: Hit::Nothing,
                    explain: None,
                    ink: Ink::Grid,
                }];
            }
            let Some(entry) = list.entries.get(row) else {
                return Vec::new();
            };
            return self.entry_fields(entry, row, at, room);
        }
        // THE FILE'S OWN CONTENT, when there is any. It comes BEFORE the
        // sentence for the same reason the picture comes before both: a panel
        // that has something to show shows it.
        if let Some(t) = self.showing_text() {
            let row = n.saturating_add(self.preview_offset);
            // THE LAST ROW SAYS THE FILE GOES ON, and it is only ever seen at
            // the end of the scroll — which is exactly when it is the true
            // thing to say. It is a row and not a refusal: a refusal means
            // there is nothing on screen, and there are two thousand lines.
            if t.cut && row == t.lines.len() {
                let said = texts.t_with(
                    ronoctua_i18n::key::FILES_PREVIEW_CUT,
                    &[("lines", &t.lines.len().to_string())],
                );
                let said = ronoctua_render::layout::elide_end(&said, room);
                return vec![Field {
                    at: centred(at, room, width_of(&said)),
                    text: said,
                    color: ronoctua_tokens::color::TEXT_DIM,
                    hit: target,
                    explain: None,
                    ink: Ink::Grid,
                }];
            }
            let Some(line) = t.lines.get(row) else {
                return Vec::new();
            };
            return text_fields(line, at, room, self.preview_column, target);
        }
        // THE SENTENCE, WRAPPED — and it was one line cut with an ellipsis.
        //
        // The owner selected `playlist.asx` and read half a refusal: that
        // sentence is 71 cells in en-GB and the panel is 24 to 40
        // (`PANEL_FLOOR`..`PANEL_CEILING`), with four empty rows under it. A
        // line that does not fit is not a reason to say less — `layout::wrap`
        // exists and hands back EVERYTHING, so the caller can hang a bar on the
        // rest, which is what `preview_total` now counts.
        //
        // WHOLE-REGION MODE IS UNCHANGED BY CONSTRUCTION: there `room` is the
        // whole width, every sentence this system has fits on one line, and
        // `lines.len()` is 1 — so the file's name goes on the row it always
        // went on.
        let refused = looked.and_then(|l| l.why.as_ref());
        let said = match refused {
            Some(why) => {
                let args: Vec<(&str, String)> = why.args();
                let borrowed: Vec<(&str, &str)> =
                    args.iter().map(|(k, v)| (*k, v.as_str())).collect();
                texts.t_with(why.key(), &borrowed)
            }
            None => texts.t(NO_PREVIEW),
        };
        let wrapped = ronoctua_render::layout::wrap(&said, room, REFUSAL_CEILING);
        // AND A REFUSAL IS READ, WHICH IS WHY IT IS NOT CENTRED.
        //
        // The owner's words, and he drew the line himself: *"as mensagens de
        // erro que aparecem no preview, diferente da mensagem 'no preview',
        // devem ficar alinhadas à esquerda"*. `no preview` is two words and
        // sits in the middle of an empty panel like a label; a refusal is a
        // SENTENCE, it wraps over three or four rows of a panel twenty-four
        // cells wide, and centred wrapped text gives every row a different
        // left edge — so the eye has to hunt for the start of each one.
        //
        // One rule for the whole block, including the file's name underneath
        // it in whole-region mode: a centred name under a left-aligned
        // sentence would be two alignments in one paragraph.
        let start = |what: u16| {
            if refused.is_some() {
                at
            } else {
                centred(at, room, what)
            }
        };
        // TWO LINES WITH THE WHOLE REGION, and one without.
        //
        // The sentence says a preview could not be made; with the region given
        // over to it, the owner asked for the file to say WHICH file it is —
        // *"também mostrar o nome completo do arquivo, ou path caso for um
        // diretório; mas apenas no modo preview largo"*. In the narrow panel
        // the name is a cell away in the listing; with the listing gone it is
        // nowhere, and a whole screen saying only "no preview" says nothing.
        let named = self.wide() == Some(Which::Preview);
        if named && n == wrapped.len() {
            // A DIRECTORY IS A PLACE and a file is a name, which is what the
            // owner distinguished: what identifies a folder is the path one
            // has walked to it, and what identifies a file is what it is
            // called.
            let said = if matches!(subject.entry.kind, ronoctua_files::Kind::Directory) {
                subject.joined().display().to_string()
            } else {
                // `shown` and not `name`: an entry of the gathered trash is
                // named by its whole path, and the caption is what it is
                // CALLED.
                subject.entry.shown().to_owned()
            };
            if said.is_empty() {
                return Vec::new();
            }
            let said = ronoctua_render::layout::elide_middle(&said, room);
            return vec![Field {
                at: start(width_of(&said)),
                text: said,
                color: ronoctua_tokens::color::TEXT_SECONDARY,
                hit: target,
                explain: None,
                ink: Ink::Grid,
            }];
        }
        // AND NOTHING PAST THE SENTENCE: a line that does nothing must not look
        // like a line that does something, which is the rule the blank rows of
        // a short listing already follow.
        let Some(text) = wrapped.get(n).cloned() else {
            return Vec::new();
        };
        vec![Field {
            at: start(width_of(&text)),
            text,
            // THE DIMMEST TEXT THERE IS, which is the whole of the "very
            // discreet formatting" that was asked for: no box, no rule, no
            // second size. Hierarchy is colour and air (invariant 15).
            color: ronoctua_tokens::color::TEXT_DIM,
            hit: target,
            explain: None,
            ink: Ink::Grid,
        }]
    }

    /// The info panel's line `n`: one fact, label and value.
    ///
    /// # Why a row here answers the pointer like a row of the listing
    ///
    /// Because it is the same fact about the same file. The panel used to be
    /// `Hit::Nothing` throughout, on the grounds that "a fact is not something
    /// one acts on" — and that was true of a date and false of everything the
    /// listing's own row already lets one change. The owner reported it as
    /// exactly that asymmetry: the permission letters are clickable in the
    /// table and dead in the panel that exists to show them.
    ///
    /// So the targets come from [`Fact::does`], which is decided once in
    /// [`crate::inspection_sheet::sheet_of`], and every one of them is a target the LISTING
    /// already produces — the same `Hit`, the same `confirm_mode`, the same
    /// picker. There is no second machinery, which is what stops the two
    /// disagreeing about what clicking a `w` does.
    ///
    /// **And what a row cannot offer is not drawn as if it could.** A size is
    /// not a number one types and a date is not something one edits: those
    /// stay `Hit::Nothing`, with the `explain` that carries the value
    /// UN-ELIDED — the row that had to be cut to fit is exactly the row
    /// somebody wants to read in full, and the message line is where it goes.
    /// Nothing floats (invariant 14).
    /// The globe's own rows: its targets, and the address under it.
    ///
    /// EVERY LINE CARRIES A FIELD, and one with no text at that. It is not an
    /// invention: the film's transport row already has a background field for
    /// exactly this reason — without one `hit` answers `None` over the picture,
    /// and a control the pointer cannot name is a control that runs away from
    /// the hand reaching for it.
    ///
    /// The LAST line is the address, because that is where the owner asked for
    /// it: *"abaixo do globo apareça o endereço detectado"*.
    fn globe_fields(&self, n: usize, air: usize, at: u16, room: u16) -> Vec<Field> {
        let last = air.saturating_sub(1);
        if n == last {
            let globe = self.globe.borrow();
            let said = globe.address().map_or_else(
                || {
                    if globe.absent() {
                        self.texts.t(ronoctua_i18n::key::ATLAS_ABSENT)
                    } else {
                        String::new()
                    }
                },
                |a| crate::globe::said(a, &self.texts),
            );
            return vec![Field {
                at,
                text: ronoctua_render::layout::elide_end(&said, room),
                color: ronoctua_tokens::color::TEXT_PRIMARY,
                // THE ADDRESS IS A DOOR, and the only one on this screen that
                // goes out of the machine.
                hit: if said.is_empty() {
                    Hit::Globe
                } else {
                    Hit::GlobeMap
                },
                // THE WHOLE OF IT, AND WHAT PRESSING IT DOES — the address is
                // three names and a comma and the panel is twenty-four cells,
                // and the same line has to say that this one leaves the
                // machine. The network view names its two hosts for the same
                // reason: what goes out is said before it goes.
                explain: (!said.is_empty()).then(|| mode::Explanation {
                    key: ronoctua_i18n::key::FILES_EXPLAIN_GLOBE_ADDRESS,
                    args: vec![("value", said.clone())],
                }),
                ink: Ink::Grid,
            }];
        }
        // THE PICTURE'S OWN LINES: no text, a target, and the sentence that
        // teaches the gesture.
        //
        // AND THE CELL THE DOT LANDED IN IS A TARGET OF ITS OWN, because WHERE
        // A THING IS DRAWN IS WHERE IT IS GRABBED — the rule the scrollbars
        // keep. The globe's thread reports the dot's pixels with the frame, so
        // this is one projection answering both "draw it here" and "this is
        // what was pressed"; a second opinion would be a dot the eye sees in
        // one place and the hand finds in another.
        let globe = self.globe.borrow();
        let cell = ronoctua_tokens::metrics::cell_width(ronoctua_tokens::text::size());
        let line = ronoctua_tokens::metrics::line_height(ronoctua_tokens::text::size());
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let dot = globe.dot().and_then(|(x, y)| {
            let row = (y / line) as usize;
            (row == n).then(|| at.saturating_add((x / cell) as u16))
        });
        // A FIELD IS AS WIDE AS ITS TEXT, and these two carry none.
        //
        // They were born with `String::new()`, and `Field::covers` is
        // `cell >= at && cell < at + width_of(text) + 1` — so each of them owned
        // exactly ONE cell of a picture hundreds of pixels wide. Every symptom
        // the owner reported is that one line: the drag never started (the
        // press looks for `Hit::Globe` and found nothing), the wheel scrolled
        // the facts underneath (the door asks the same hit before it lets the
        // figure swallow the gesture), and the dot could not be clicked.
        //
        // SPACES, and it is not a trick: it is what `transport_fields` already
        // does for the film's controls, one panel along, for the same reason —
        // the row has to be nameable everywhere it is DRAWN. Nothing of this
        // reaches the screen; the picture is painted over it.
        let mut fields = vec![Field {
            at,
            text: " ".repeat(usize::from(room)),
            color: ronoctua_tokens::color::TEXT_DIM,
            hit: Hit::Globe,
            explain: Some(does(ronoctua_i18n::key::FILES_EXPLAIN_GLOBE)),
            ink: Ink::Grid,
        }];
        if let Some(where_) = dot {
            // AND THE DOT IS GRABBED WHERE IT IS DRAWN, which is a disc and not
            // a point: `space.atlas.dotRadius` is in tenths of a pixel, so the
            // target is the cells the halo covers, and never fewer than one.
            // A one-cell target on a mark that is drawn wider is a mark the eye
            // finds and the hand misses.
            let radius = f32::from(ronoctua_tokens::space::ATLAS_DOTRADIUS) / 10.0;
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let wide = ((radius * 2.0 / cell).ceil() as u16).max(1);
            fields.push(Field {
                at: where_.saturating_sub(wide / 2),
                text: " ".repeat(usize::from(wide)),
                color: ronoctua_tokens::color::TEXT_DIM,
                hit: Hit::GlobeDot,
                explain: Some(does(ronoctua_i18n::key::FILES_EXPLAIN_GLOBE_DOT)),
                ink: Ink::Grid,
            });
        }
        fields
    }

    /// ONE LINE OF THE FACTS PANEL: the globe — as a band's row or as a
    /// column — one fact out of each of the other columns, and the expand
    /// button where the line is the last one.
    ///
    /// One column in the ordinary panel, several with the region given away —
    /// [`Panel::grid_of`] is what says which, and it is the only thing that
    /// does. The line is a `Row` like any other, so `hit`, the hover band, a
    /// click and a key all keep going through the machine that already exists.
    fn info_fields(&self, n: usize, rows: usize, at: u16, room: u16, sheet: &[Fact]) -> Vec<Field> {
        let (globe, columns, tall) = self.grid_of(sheet.len());
        let wide = u16::try_from(globe + columns).unwrap_or(1).max(1);
        let each = room / wide;
        let mut out = Vec::new();
        // THE GLOBE'S OWN COLUMN, and it is the same builder the band uses:
        // one picture, one target, one address line, wherever it is put.
        if globe > 0 {
            out.extend(self.globe_fields(n, rows, at, each));
        }
        // THE OFFSET IS THE PANEL'S OWN. The listing scrolls with the region
        // and the facts do not: they are about ONE entry, and there are more
        // of them than the region has lines. See `files::View::facts_bar`.
        let air = self.globe_rows();
        let row = n.saturating_add(self.facts_offset(air.saturating_add(tall)));
        if row < air {
            // A ROW OF THE GLOBE'S BAND, which SCROLLS: it is content, not
            // chrome. `air - offset` of them are left on screen and they are
            // the panel's first lines, so the address stays at the picture's
            // foot as the picture gives its lines back. See `Panel::globe_rows`.
            out.extend(self.globe_fields(row, air, at, room));
        } else {
            for c in 0..columns {
                // COLUMN-MAJOR, which is what "divided into columns" means: the
                // facts fill the first column top to bottom and then the next,
                // so reading DOWN a column is reading the sheet in its own
                // order. Row by row would put the second fact beside the first,
                // and a sheet one reads left to right is a table nobody wrote.
                //
                // PAST THE LAST FACT IS THE BLANK ROW, which `sheet.get`
                // answers `None` for — see [`BLANK`].
                let k = row - air;
                let Some(fact) = sheet.get(c.saturating_mul(tall).saturating_add(k)) else {
                    continue;
                };
                let mine = u16::try_from(globe + c).unwrap_or(0).saturating_mul(each);
                // AND THE LAST COLUMN TAKES WHAT THE DIVISION LEFT OVER, which
                // is the rule `Panes::boxes` already follows for the panels
                // themselves: cells left at the right-hand end are cells
                // nothing draws in, and the owner measured that once already.
                let span = if c + 1 == columns {
                    room.saturating_sub(mine)
                } else {
                    each
                };
                out.extend(self.fact_fields(fact, at.saturating_add(mine), span));
            }
        }
        // AND THE EXPAND BUTTON, IN THE CORNER, on the panel's last line.
        //
        // OVER whatever that line holds, which is the rule the preview's own
        // corner already follows and which the owner set there: a control that
        // covers content while the hand is on it is a control one can reach,
        // and scrolled to the end it covers the blank row instead of a fact.
        //
        // PUSHED LAST so that `lit_field` — an `rposition` — hands the cell to
        // it rather than to the value underneath: what the eye sees lit is by
        // construction what the click takes.
        // AND NOT HERE EITHER — see `files::View::sections_of`, which knows which
        // line is the last one.
        out
    }

    /// ONE FACT, in the cells one column of the grid gives it.
    ///
    /// # One arithmetic and one pair of tones
    ///
    /// Where the label ends and the value begins is `chrome::field_pair`'s
    /// answer — the one every `label: value` row of this system takes — and
    /// the TONES are the row's [`chrome::Kind`]: a row that changes the file
    /// ([`Doable::edits`]) is a SETTING, every other one a FACT. They were
    /// picked here by hand (a dim label, a bright value, for every row alike),
    /// which drew the name one can press and the inode count one cannot in
    /// the same colours.
    ///
    /// # And the whole row answers
    ///
    /// An editable row carries its question on the label AND on the value
    /// (`Hit::Amend`), so pressing either asks it — as `RET` does on the row.
    /// The permission, the owner and the walk keep their finer targets on the
    /// value: one per bit, one per name.
    fn fact_fields(&self, fact: &Fact, at: u16, room: u16) -> Vec<Field> {
        let kind = crate::chrome::Kind::of(fact.does.edits());
        // AND ONE CELL AT THE END IS THE BAR'S, always.
        //
        // Reserved whether or not there is one to draw, because a column that
        // gave a cell back when the last fact was scrolled into view would
        // re-elide every value on the row underneath it — layout that moves
        // while one reads is worse than a cell of air. See `files::View::facts_bar`
        // for why the bar cannot sit in the surface's own last cell.
        let room = room.saturating_sub(1);
        // THE SENTENCE IS THE ROW'S, and both halves give it: a label cut to
        // eight cells and a value cut to sixteen are two halves of ONE fact,
        // and the line under the pointer puts them back together — always, and
        // not only when something was cut, because a sentence that appeared
        // and vanished between adjacent rows would read as a flaky tooltip. A
        // row that has its own (a date: which second, in whose time; a tag
        // that cannot be written: why) says that one; an editable row says it
        // can be pressed — and the place's row says what pressing it DOES,
        // because it changes nothing: it only offers to take the place out.
        let says = fact.says.clone().unwrap_or_else(|| mode::Explanation {
            key: match fact.does {
                crate::inspection_sheet::Doable::RemoveLocation => {
                    ronoctua_i18n::key::FILES_FACT_TAKES_PLACE_OUT
                }
                does if does.edits() => ronoctua_i18n::key::FILES_FACT_EDITS,
                _ => FACT_SAYS,
            },
            args: vec![("label", fact.label.clone()), ("value", fact.value.clone())],
        });
        let n = self.subject.borrow().as_ref().map_or(0, |s| s.row);
        let whole = if fact.does.edits() {
            Hit::Amend(n, fact.does)
        } else {
            Hit::Nothing
        };
        let mut fields = crate::chrome::field_pair(at, room, &fact.label, &fact.value, kind, false);
        for f in &mut fields {
            f.hit = whole.clone();
            f.explain = Some(says.clone());
        }
        // THE FINER TARGETS, where the value is several things at once. They
        // start where the pair put the value — or, for a value too narrow to
        // have been placed, where the value column is.
        let value_at = fields
            .get(1)
            .map_or_else(|| crate::chrome::value_column(at, room), |v| v.at);
        let value_room = (at + room).saturating_sub(value_at);
        let acted = match fact.does {
            Doable::Mode => self.mode_fields(value_at, value_room),
            Doable::Ownership => self.ownership_fields(value_at, value_room, &fact.value),
            Doable::Measure => self.measure_field(value_at, value_room, &fact.value),
            _ => Vec::new(),
        };
        if !acted.is_empty() {
            fields.truncate(1);
            fields.extend(acted);
        }
        fields
    }

    /// The permission triplet inside the info panel, one target per character.
    ///
    /// The same ten cells the listing's `Perms` column draws, from the same
    /// `mode::cells`, with the same `Hit`s and the same sentences — and then
    /// the octal, which is not a target because there is nothing to flip in
    /// it. Anything past `room` is DROPPED rather than drawn: nothing leaks
    /// off the screen, which is the one rule that never gives way.
    pub(crate) fn mode_fields(&self, at: u16, room: u16) -> Vec<Field> {
        let subject = self.subject.borrow();
        let Some(subject) = subject.as_ref() else {
            return Vec::new();
        };
        let (entry, owner, group) = (&subject.entry, &subject.owner, &subject.group);
        let n = subject.row;
        let setting = crate::chrome::Kind::Setting.tones(false).1;
        let mut fields = Vec::new();
        for (k, cell) in mode::cells(entry.mode).into_iter().enumerate() {
            let Ok(k) = u16::try_from(k) else { break };
            if k >= room {
                break;
            }
            let hit = match (cell.special, cell.segment) {
                (Some(s), _) => Hit::Special(n, s),
                (None, mode::Segment::Bit(who, what)) => Hit::Bit(n, who, what),
                (None, mode::Segment::Kind) => Hit::Nothing,
            };
            fields.push(Field {
                at: at + k,
                text: cell.letter.to_string(),
                // A BIT THAT IS SET SAYS THE SETTING'S VALUE; one that is not
                // is the dash between them. The row's `Kind` decides the tone
                // and the bit only whether it is lit.
                color: if cell.set {
                    setting
                } else {
                    ronoctua_tokens::color::TEXT_DIM
                },
                hit,
                explain: Some(mode::explain(cell, entry.mode, owner, group)),
                ink: Ink::Grid,
            });
        }
        // THE OCTAL, when there is room for it whole. Half of `(0644)` is not
        // a mode, and a mode read wrong is worse than one not shown.
        let number = mode::octal(entry.mode);
        let octal = format!("({number})");
        let used = u16::try_from(fields.len()).unwrap_or(room);
        let left = room.saturating_sub(used + 1);
        if left >= width_of(&octal) {
            let start = at + used + 1;
            // EACH DIGIT IS ITS OWN FIELD, and the brackets are punctuation.
            //
            // The number was ONE field with ONE sentence saying what the form
            // is — "one digit for the owner, one for the group, one for
            // everybody else" — which is a definition of octal and not an
            // answer about THIS file. The owner pointed at the digits and got
            // it: *"o minibuffer está mostrando uma explicação genérica, em vez
            // de explicar para cada número, o que o número representa"*.
            //
            // Ten letters each explain themselves; there is no reason three
            // digits should not. What each one says is whose part of the number
            // it is and what that value permits — which is the SAME question
            // the letters answer, asked three characters at a time.
            //
            // The brackets keep the general sentence: pointing at the shape
            // still says what the shape is.
            let whole = Explanation {
                key: ronoctua_i18n::key::FILES_EXPLAIN_OCTAL,
                args: vec![("octal", number.clone())],
            };
            fields.push(Field {
                at: start,
                text: "(".to_owned(),
                color: ronoctua_tokens::color::TEXT_DIM,
                hit: Hit::Nothing,
                explain: Some(whole.clone()),
                ink: Ink::Grid,
            });
            let kind = mode::kind_word(entry.kind);
            for (i, digit) in number.chars().enumerate() {
                let Ok(k) = u16::try_from(i) else { break };
                let part = mode::part_of(number.chars().count(), i);
                // THE VERBS ARE RESOLVED HERE, because this is where the texts
                // are: `mode` is the FILE half and knows no language. The
                // fragment goes in as an argument, which is what the address
                // under the globe already does with a name.
                // THE VALUE GOES IN AS A NUMBER, and that is not a detail:
                // a Fluent selector matches `[6]` against a NUMBER, and a
                // string argument falls through to the default branch. Every
                // digit said `can read, change and run it` — the `*[7]` — and
                // the test that printed the three lines is what showed it.
                // `AGENTS.md` records the same trap on the months.
                let can = self.texts.t_args(
                    ronoctua_i18n::key::FILES_OCTAL_CAN,
                    &[
                        ("kind", ronoctua_i18n::Arg::Text(kind)),
                        (
                            "bits",
                            ronoctua_i18n::Arg::Number(i64::from(digit.to_digit(8).unwrap_or(0))),
                        ),
                    ],
                );
                fields.push(Field {
                    at: start + 1 + k,
                    text: digit.to_string(),
                    color: ronoctua_tokens::color::TEXT_DIM,
                    // NOTHING TO CLICK — there is no bit in a number to turn,
                    // and a target that lights and then refuses is worse than
                    // one that never promised (`menu::Applies`, one register
                    // down).
                    hit: Hit::Nothing,
                    explain: Some(Explanation {
                        key: ronoctua_i18n::key::FILES_EXPLAIN_OCTAL_DIGIT,
                        args: vec![
                            ("part", part.word().to_owned()),
                            ("digit", digit.to_string()),
                            ("octal", number.clone()),
                            (
                                "name",
                                match part {
                                    mode::Part::Group => group.clone(),
                                    _ => owner.clone(),
                                },
                            ),
                            ("can", can),
                        ],
                    }),
                    ink: Ink::Grid,
                });
            }
            #[allow(clippy::cast_possible_truncation)]
            let after = start + 1 + number.chars().count() as u16;
            fields.push(Field {
                at: after,
                text: ")".to_owned(),
                color: ronoctua_tokens::color::TEXT_DIM,
                hit: Hit::Nothing,
                explain: Some(whole),
                ink: Ink::Grid,
            });
        }
        fields
    }

    /// `owner:group` inside the info panel, as two targets and a colon.
    ///
    /// The colon is `Hit::Nothing` on purpose: it is punctuation between two
    /// answers, and a click on it could only guess which of the two was meant.
    fn ownership_fields(&self, at: u16, room: u16, value: &str) -> Vec<Field> {
        let Some((owner, group)) = value.split_once(':') else {
            return Vec::new();
        };
        let n = self.subject.borrow().as_ref().map_or(0, |s| s.row);
        let mut fields = Vec::new();
        let mut used = 0u16;
        let mut put = |text: String, hit: Hit, explain: Option<Explanation>, dim: bool| {
            let left = room.saturating_sub(used);
            if left == 0 {
                return;
            }
            let text = ronoctua_render::layout::elide_end(&text, left);
            let width = width_of(&text);
            fields.push(Field {
                at: at + used,
                text,
                // THE TWO NAMES ARE THE SETTING'S VALUE, the colon between them
                // is punctuation.
                color: if dim {
                    ronoctua_tokens::color::TEXT_DIM
                } else {
                    crate::chrome::Kind::Setting.tones(false).1
                },
                hit,
                explain,
                ink: Ink::Grid,
            });
            used += width;
        };
        put(
            owner.to_owned(),
            Hit::Owner(n),
            Some(Explanation {
                key: ronoctua_i18n::key::FILES_EXPLAIN_OWNER,
                args: vec![("who", owner.to_owned())],
            }),
            false,
        );
        put(":".to_owned(), Hit::Nothing, None, true);
        put(
            group.to_owned(),
            Hit::Group(n),
            Some(Explanation {
                key: ronoctua_i18n::key::FILES_EXPLAIN_GROUP,
                args: vec![("who", group.to_owned())],
            }),
            false,
        );
        fields
    }

    /// A DIRECTORY's size inside the info panel: the same walk `TAB` asks for.
    ///
    /// Only a directory gets one, and that is decided in [`crate::inspection_sheet::sheet_of`]. A
    /// file's size is a fact the kernel already knows and nothing can change
    /// from here — so it is drawn, and it is not a target.
    fn measure_field(&self, at: u16, room: u16, value: &str) -> Vec<Field> {
        let subject = self.subject.borrow();
        let Some(subject) = subject.as_ref() else {
            return Vec::new();
        };
        vec![Field {
            at,
            text: ronoctua_render::layout::elide_end(value, room),
            // A WALK MEASURES AND CHANGES NOTHING: the size is a fact, and
            // pressing it only asks for the number.
            color: crate::chrome::Kind::Fact.tones(false).1,
            hit: Hit::Measure(subject.row),
            explain: Some(Explanation {
                key: ronoctua_i18n::key::FILES_EXPLAIN_MEASURE,
                args: vec![("bytes", subject.entry.bytes.to_string())],
            }),
            ink: Ink::Grid,
        }]
    }

    /// Whether there is a film in the preview right now.
    ///
    /// Beside `shape_here` and asking the same way: the player's own answer,
    /// and only while the preview is the file the cursor is on.
    #[must_use]
    pub fn film_here(&self) -> bool {
        self.showing_now().is_some() && self.looked.as_ref().is_some_and(|l| l.moving.is_some())
    }

    /// Moves a panel's cursor, and scrolls the panel to keep it in sight.
    ///
    /// # Why the view follows the cursor and not the other way round
    ///
    /// Because that is what every other list in this system does — the
    /// sidebar, the listing, the locator — and a panel that scrolled while the
    /// cursor stayed put would be the one place where the arrows moved the
    /// paper instead of the hand. The paging keys (`C-,` and `C-.`) are still
    /// there for moving the paper, and they come back round; this stops at the
    /// ends, as a cursor should.
    pub(crate) fn walk_panel(&mut self, by: i32, total: usize, preview: bool) -> Step {
        if total == 0 {
            return Step::Say(does(if preview {
                PREVIEW_ALL_SHOWN
            } else {
                FACTS_ALL_SHOWN
            }));
        }
        let last = total - 1;
        let from = if preview {
            self.preview_cursor
        } else {
            self.info_cursor
        };
        let to = if by < 0 {
            from.saturating_sub(by.unsigned_abs() as usize)
        } else {
            from.saturating_add(by.unsigned_abs() as usize).min(last)
        }
        .min(last);
        // AND THE PANEL SCROLLS UNDER IT. `drawn_lines` is what the last frame
        // gave this panel — the title already taken off — so this is the same
        // number the bar and the paging keys use.
        //
        // THE FACTS' OWN ROOM, which is not the panel's lines: the globe takes
        // lines off the top of the info panel and they do not scroll, so a
        // cursor walked to the bottom would go looking for a fact that is not
        // drawn.
        let room = if preview {
            usize::from(self.drawn_lines.get()).max(1)
        } else {
            self.info_room().max(1)
        };
        let offset = if preview {
            &mut self.preview_offset
        } else {
            &mut self.info_offset
        };
        if to < *offset {
            *offset = to;
        } else if to >= offset.saturating_add(room) {
            *offset = to.saturating_sub(room - 1);
        }
        if preview {
            self.preview_cursor = to;
        } else {
            self.info_cursor = to;
        }
        if to == from {
            return Step::Nothing;
        }
        Step::Changed
    }

    /// Walks the text preview by a line, and stops at the ends.
    ///
    /// `step_facts`'s twin, one arm above in `step_by`, and deliberately the
    /// same shape: it STOPS rather than wraps, because an arrow held down
    /// should come to rest at the end of a file rather than start again at the
    /// top. The key that wraps is `C-,`, which is the one pressed without
    /// looking.
    ///
    /// It needs no "all shown" sentence: `step_by` only reaches it when there
    /// is more than fits, and falls through to `move_by` otherwise — which
    /// always does something.
    pub(crate) fn step_preview(&mut self, by: i32) -> Step {
        let room = usize::from(self.drawn_lines.get());
        let total = self.preview_total();
        if room == 0 || total <= room {
            return Step::Nothing;
        }
        let last = total - room;
        let to = if by < 0 {
            self.preview_offset
                .saturating_sub(by.unsigned_abs() as usize)
        } else {
            self.preview_offset
                .saturating_add(by.unsigned_abs() as usize)
                .min(last)
        };
        if to == self.preview_offset {
            return Step::Nothing;
        }
        self.preview_offset = to;
        Step::Changed
    }

    /// Everything known about the file the panels are about.
    pub(crate) fn sheet(&self) -> Vec<Fact> {
        let subject = self.subject.borrow();
        let Some(subject) = subject.as_ref() else {
            return Vec::new();
        };
        // REMEMBERED, and it is not an optimisation to be weighed: this runs on
        // every painted frame and formats one Fluent message per row. See
        // `Which::remembered` for what the key is made of and why.
        let wanted = SheetFor {
            name: subject.entry.name.clone(),
            modified: subject.entry.modified,
            measured: subject.measured,
            looked: self.looked.as_ref().map(|l| l.key.clone()),
            changeable: self
                .looked
                .as_ref()
                .is_some_and(|l| l.amendable != crate::inspection_edit::Amendable::default()),
            said: Rc::as_ptr(&self.texts) as usize,
        };
        if let Some((was, sheet)) = self.remembered.borrow().as_ref()
            && *was == wanted
        {
            return sheet.clone();
        }
        let built = crate::inspection_sheet::sheet_of(
            subject,
            self.looked.as_ref(),
            &self.zone,
            &self.texts,
        );
        *self.remembered.borrow_mut() = Some((wanted, built.clone()));
        built
    }

    /// Turns the info panel to its next page of facts, and round to the start.
    ///
    /// # Why it pages, and why it wraps
    ///
    /// A page because the panel is a block one reads, not a line one steps
    /// through: a fact list of a dozen rows in four lines is three presses,
    /// and three presses of one key is less to learn than two keys. It wraps
    /// for the reason `files-sort` cycles — a key one can press again without
    /// having to know where the end is — and `files::View::facts_bar` is what says
    /// where one has got to, so the wrap is never a surprise.
    ///
    /// **With everything already on screen it SAYS so.** From the outside a
    /// key that refuses and a key that is dead look the same, and that is the
    /// one outcome this project treats as a defect.
    pub(crate) fn scroll_facts(&mut self) -> Step {
        // THE HEIGHT IS THE LAST ONE DRAWN, which is the only one true of what
        // is on screen. A key arrives without one — see `Which::drawn_lines`.
        // THE PANEL'S ROWS AND NOT ITS LINES. The globe takes lines off the
        // top and they do not scroll (`Section::above`), so paging by the
        // region's height would step past facts nobody has seen.
        let room = self.info_room();
        // THE GRID'S ROWS, which is what one pages through: with the facts in
        // columns the sheet is shorter down the page than it is long.
        let total = self.info_total();
        if !self.info_shown() || room == 0 || total <= room {
            return Step::Say(does(FACTS_ALL_SHOWN));
        }
        self.info_offset = self.facts_offset(total).saturating_add(room);
        if self.info_offset >= total {
            self.info_offset = 0;
        }
        Step::Changed
    }

    /// Pages the text preview, and comes back round.
    ///
    /// `scroll_facts`'s twin, and the same shape for the same reason written
    /// there: a list one can hold a key on without having to know where the
    /// end is. The bar says where one got to.
    pub(crate) fn scroll_preview(&mut self) -> Step {
        let room = usize::from(self.drawn_lines.get());
        let total = self.preview_total();
        if !self.preview_shown() || room == 0 || total <= room {
            return Step::Say(does(PREVIEW_ALL_SHOWN));
        }
        self.preview_offset = self.preview_offset.saturating_add(room);
        if self.preview_offset >= total {
            self.preview_offset = 0;
        }
        Step::Changed
    }

    /// Starts one entry of the playlist under the cursor.
    ///
    /// One place, because three things reach it — the two keys and the click —
    /// which is the rule `seek_to` already keeps for the transport.
    pub(crate) fn play_entry(&mut self, to: usize) -> Step {
        let Some(list) = self.showing_list() else {
            return Step::Say(does(NOTHING_PLAYING));
        };
        // AN ENTRY THAT CANNOT BE PLAYED IS NOT SKIPPED SILENTLY. A remote one
        // is on screen with the address it names, and pressing it should say
        // why nothing happened rather than jumping to the next.
        let Some(entry) = list.entries.get(to) else {
            return Step::Nothing;
        };
        if !matches!(entry.at, ronoctua_inspect::playlist::Where::Local(_)) {
            // AND IT SAYS WHICH ONE. "This entry is somewhere else" about a
            // list of nine is a sentence one still has to go and check.
            return Step::Say(Explanation {
                key: ENTRY_NOT_HERE,
                args: vec![("path", entry.written.clone())],
            });
        }
        self.playing = to;
        // FROM THE TOP OF THE NEW ONE, and audible: an entry chosen by hand is
        // an entry somebody wants to hear.
        self.transport = Transport {
            on: true,
            paused: false,
            from: 0.0,
            at: 0.0,
            sound: true,
        };
        Step::Changed
    }

    /// Walks to the next entry that can be played, or the one before it.
    ///
    /// `C-{` and `C-}`. The braces and not the brackets: `C-[` and `C-]` slide
    /// a long line sideways and belong to the panel, and this pair belongs to
    /// the LIST — the family reads as "brackets move the line, braces move the
    /// entry".
    pub(crate) fn step_entry(&mut self, by: isize) -> Step {
        let Some(list) = self.showing_list() else {
            return Step::Say(does(NOTHING_PLAYING));
        };
        let playable: Vec<usize> = list.playable().collect();
        if playable.is_empty() {
            return Step::Say(does(NOTHING_TO_PLAY));
        }
        // WHERE THE QUEUE IS AMONG THE PLAYABLE ONES, and not among the drawn
        // rows: stepping past a remote entry must not stop on it.
        let now = playable
            .iter()
            .position(|n| *n >= self.playing)
            .unwrap_or(playable.len() - 1);
        let next = if by < 0 {
            now.checked_sub(1)
        } else {
            (now + 1 < playable.len()).then_some(now + 1)
        };
        let Some(next) = next else {
            return Step::Say(does(if by < 0 { LIST_AT_START } else { LIST_AT_END }));
        };
        self.play_entry(playable[next])
    }

    /// How much of the preview's foot the controls cover, in hundredths.
    ///
    /// # Derived, and not a token
    ///
    /// The controls are ONE line, and how much of the panel one line is depends
    /// on how many lines the panel got — four in the strip, a screenful with
    /// the region given over. A token would have to be one number for both.
    ///
    /// The two the view already records are enough: `drawn_lines` is how many
    /// lines the panel was laid out in, and one of them is the transport's. No
    /// font size crosses this line — which matters, because the two doors do
    /// not use the same one.
    fn controls_foot(&self) -> u16 {
        let lines = self.drawn_lines.get().max(1);
        (100 / lines).min(100)
    }

    /// How long what is under the cursor runs, in seconds.
    ///
    /// # One source, because four things ask it
    ///
    /// The timeline draws from it, the two seek keys clamp against it and the
    /// drag maps a fraction onto it. Written out at each, a playlist answered
    /// three of the four — the drawing knew its entry's length and the two
    /// keys and the drag went on asking `moving`, which a playlist has not
    /// got. The controls appeared and did nothing, which is exactly what the
    /// owner reported: *"nenhum botão de pause e de linha de tempo está
    /// funcionando nos arquivos de playlist"*.
    ///
    /// A PLAYLIST'S IS ITS OWN WORD. Nothing here has opened the entry, so the
    /// only length available is the one the list declares — and a list that
    /// declares none gets no timeline, which is the answer a GIF gets and for
    /// the same reason.
    pub(crate) fn playing_seconds(&self) -> Option<f64> {
        if let Some(list) = self.showing_list() {
            return list.entries.get(self.playing)?.seconds;
        }
        self.looked.as_ref()?.moving.as_ref()?.shape.seconds
    }

    /// The next entry that could be played, when there is one.
    ///
    /// `None` at the end of the list, and that is what stops it: a preview
    /// that never stops is a child process that never stops. Pressing play on
    /// a finished list starts it again from the first entry, which is
    /// `play_pause`'s third state doing what it already does.
    fn after_the_one_playing(&self) -> Option<usize> {
        let list = self.showing_list()?;
        list.playable().find(|n| *n > self.playing)
    }

    /// Holds the film, or lets it go on. `C-;`, and the play glyph.
    ///
    /// # Three states, one key
    ///
    /// Playing, held, and ENDED — and the third is why this is not a `not`. A
    /// film that ran out has no child to resume, so pressing play there starts
    /// it again from the beginning, which is what everybody expects a play
    /// button at the end of a film to do.
    pub(crate) fn play_pause(&mut self) -> Step {
        if !self.plays_here() {
            return Step::Say(does(NOTHING_PLAYING));
        }
        if self.transport.on {
            self.transport.paused = !self.transport.paused;
        } else {
            // FROM THE TOP. The child is gone, so this is a spawn either way,
            // and starting where the film ended would show its last frame and
            // stop.
            //
            // AND A LIST THAT RAN OUT STARTS AGAIN AT THE TOP, which is the
            // same sentence one register up: the child is gone, so this is a
            // spawn either way, and resuming on the last entry would play its
            // final second and stop.
            if let Some(first) = self.showing_list().and_then(|l| l.playable().next())
                && self.after_the_one_playing().is_none()
            {
                self.playing = first;
            }
            // AND THE SOUND CHOICE SURVIVES. `Transport::default` is silent,
            // and a sound arrives with its audio on — so rebuilding wholesale
            // would answer a key about PLAYING by muting the file, once, at
            // the end, with nothing saying why.
            self.transport = Transport {
                on: true,
                paused: false,
                from: 0.0,
                at: 0.0,
                sound: self.transport.sound,
            };
        }
        Step::Changed
    }

    /// Lets the film be heard, or not. `C-'`, and the sound glyph.
    ///
    /// A film with no audio stream REFUSES rather than pretending: measured
    /// against the real binary, asking ffmpeg for a sound output it cannot fill
    /// makes it refuse the whole command — so the picture would stop, in answer
    /// to a key about the sound.
    pub(crate) fn hear(&mut self) -> Step {
        let has = self
            .looked
            .as_ref()
            .and_then(|l| l.moving.as_ref())
            .is_some_and(|m| m.shape.sound);
        if !self.plays_here() {
            return Step::Say(does(NOTHING_PLAYING));
        }
        if !has {
            return Step::Say(does(NO_SOUND_IN_IT));
        }
        self.transport.sound = !self.transport.sound;
        // AND IT IS A RESPAWN, FROM WHERE THE PICTURE IS. The sound comes out
        // of the same child, so there is no way to add it to one already
        // running — and starting again from nought would be a key about the
        // sound that moved the picture.
        self.transport.from = self.transport.at;
        Step::Changed
    }

    /// Moves along the film, and says where it got to. `C--` and `C-=`.
    ///
    /// A GIF refuses, and it is the length that refuses it: its frames are
    /// inter-dependent, so reaching frame five hundred means composing five
    /// hundred — 707 ms on the owner's own forty-four-frame sample. No
    /// duration is what draws that line, with no branch of its own.
    pub(crate) fn seek_by(&mut self, by: f64) -> Step {
        let Some(total) = self.playing_seconds() else {
            return Step::Say(does(NOTHING_PLAYING));
        };
        if !self.plays_here() {
            return Step::Say(does(NOTHING_PLAYING));
        }
        self.seek_to((self.transport.at + by).clamp(0.0, (total - 0.1).max(0.0)));
        Step::Say(Explanation {
            key: ronoctua_i18n::key::FILES_FILM_SEEK,
            args: vec![
                ("at", clock_of(self.transport.from)),
                ("total", clock_of(total)),
            ],
        })
    }

    /// Puts the film at a point, in seconds.
    ///
    /// One place, because three things reach it — the two keys and the drag —
    /// and a seek written three times is three chances for the picture and the
    /// cursor to disagree about where the film is.
    pub fn seek_to(&mut self, to: f64) {
        self.transport.on = true;
        self.transport.paused = false;
        self.transport.from = to.max(0.0);
        self.transport.at = self.transport.from;
        // AND THE PICTURE ON SCREEN STAYS until the new one arrives.
        //
        // Clearing it was the first shape of this and it was worse: a seek is a
        // kill and a respawn, 108 to 242 milliseconds, and blanking the panel
        // for each of them turns a drag along the timeline into a strobe. The
        // texture is reused under the same key, so the new frame simply
        // overwrites the old one when it lands.
    }

    /// Gives a panel the whole region, or gives it back. `C-w` and `C-W`.
    ///
    /// It REFUSES when there is nothing under the cursor, and the refusal is
    /// the point: a manager whose listing vanished because a key was pressed
    /// over an empty directory is a manager that lost its listing.
    ///
    /// **And it refuses for a panel that is HIDDEN**, which is not the same
    /// refusal and does not deserve the same sentence. `C-v` and `C-i` take a
    /// panel off the screen; handing the region to one of those was a key that
    /// said "the panel has the whole region" and changed nothing at all — the
    /// one outcome this project treats as a defect, because from the outside a
    /// key that lies and a key that is dead look the same.
    pub(crate) fn theatre(&mut self, which: Which) -> Step {
        if self.subject.borrow().is_none() {
            return Step::Say(does(THEATRE_NOTHING));
        }
        if !self.panel_on(which) {
            return Step::Say(does(WIDE_HIDDEN));
        }
        // THE OTHER KEY TAKES IT OVER rather than adding a second wide panel:
        // the region is ONE, so the two keys are the two ends of one wish.
        self.theatre = if self.theatre == Some(which) {
            None
        } else {
            Some(which)
        };
        let wide = self.theatre == Some(which);
        Step::Say(does(match (which, wide) {
            (Which::Preview, true) => THEATRE_ON,
            (Which::Preview, false) => THEATRE_OFF,
            (Which::Info, true) => FACTS_WIDE_ON,
            (Which::Info, false) => FACTS_WIDE_OFF,
        }))
    }

    /// What the inspector should be asked for, or `None` when nothing is due.
    ///
    /// # Why the ASK is remembered and not only the answer
    ///
    /// Because `None` is an ANSWER — "this file has no picture" — and without
    /// somewhere to write "I have asked, and that was the reply" the host asks
    /// again on every frame, which is a child process per frame. It is the rule
    /// `surface`'s `icons` map already learned, and the reason
    /// `ronoctua_inspect::Made` carries the refusal alongside the pixels.
    ///
    /// `target` is `space.files.panelPixels`; the crate quantises it, so
    /// dragging a window's edge re-decodes at most twice.
    pub fn wants_look(&mut self, target: u32) -> Option<Ask> {
        if !self.preview_on && !self.info_on {
            return None;
        }
        // COPIED OUT BEFORE ANYTHING IS WRITTEN. The subject is borrowed and
        // the line that records the ask writes beside it; taking the facts
        // first is what lets the two coexist.
        let (kind, path, bytes, modified, broken, beside) = {
            let subject = self.subject.borrow();
            let subject = subject.as_ref()?;
            let entry = &subject.entry;
            (
                // WHAT IT OPENS AS, and never `kind`. A symlink to a
                // photograph is a photograph when you look at it — following
                // the link is the one operation that should — and `kind` would
                // have refused every one of them with nothing on screen saying
                // why. Found by pointing the panels at a directory of symlinks.
                entry.opens_as(),
                subject.path.clone(),
                entry.bytes,
                entry.modified,
                entry.broken(),
                subject.beside.clone(),
            )
        };
        // A DIRECTORY HAS NO CONTENT TO LOOK INTO. Neither has a broken link,
        // and asking about one would spawn a child to be told what `lstat`
        // already said.
        if !matches!(kind, Kind::File) || broken {
            return None;
        }
        // THE SECONDS AS THE ENTRY CARRIES THEM. `Entry::modified` is already an
        // `i64` of seconds since the epoch — the listing formats it with
        // `when::civil` — so there is no `SystemTime` in the middle to convert
        // and get wrong.
        let stamp = ronoctua_inspect::Stamp::new(path.clone(), modified, bytes, target);
        // THE THEME IS PART OF THE KEY. The inspector paints the waveform in a
        // token's VALUE (`wave` below), and the door uploads a still only when
        // its key is missing from the GPU cache; a key that ignored the theme
        // would keep the old ink on screen after a switch, and the re-ask a
        // theme change makes would be paid for a picture nobody sees.
        let key = format!("{}:{}", stamp.key(), ronoctua_tokens::theme::palette());
        if self.asked.as_deref() == Some(key.as_str()) {
            return None;
        }
        self.asked = Some(key.clone());
        // THE NAME MAY BE FINER THAN THE SNIFF, and `settled` is where that is
        // decided: a `.dng` sniffs as `image/tiff` because it IS one, and the
        // TIFF decoder reads the RAW planes and refuses.
        let (named, sniffed) = mime::examined(&path, kind);
        let mime = ronoctua_inspect::settled(named, sniffed);
        Some(Ask {
            // A FILM THAT MIGHT BELONG TO THIS PICTURE, as the subject found
            // it — see `Ask::beside`. It is a CANDIDATE: whether the two are
            // one Live Photo is settled by what is inside them, which is the
            // inspector's question and not this one. A host holding a listing
            // answers it out of the listing (`files::View::wants_look`).
            beside,
            path,
            mime,
            target,
            key,
            tab: usize::from(ronoctua_tokens::space::FILES_TABWIDTH),
            wave: ink(ronoctua_tokens::color::SURFACE_WAVE),
        })
    }

    /// What the inspector came back with.
    ///
    /// A stale answer is DROPPED, not drawn: the cursor moves while a child
    /// process runs, and one file's picture under another file's name is the
    /// one failure a preview must not have.
    pub fn looked(&mut self, l: Looked) {
        if self.asked.as_deref() != Some(l.key.as_str()) {
            return;
        }
        // A NEW SUBJECT STARTS AT THE TOP, and a re-decode of the SAME file
        // does not: dragging a window's edge asks again at a new `target`, and
        // jumping back to line one while somebody is reading is worse than the
        // drag.
        //
        // AND IT IS THE ONE PLACE. `self.cursor` is written at sixteen sites in
        // this file and only `move_by` used to reset `info_offset`, so CLICKING
        // another file kept the facts scrolled where the previous one left
        // them. Resetting here instead is causal — the subject changed — and it
        // covers all sixteen, every directory change, and the preview's two
        // offsets, in three lines.
        if self.looked.as_ref().is_none_or(|old| old.path != l.path) {
            self.preview_offset = 0;
            self.preview_column = 0;
            self.info_offset = 0;
            // AND SO DO THE TWO CURSORS, for the same causal reason: a cursor
            // on the seventh fact of one file has nothing to point at in
            // another file's sheet.
            self.preview_cursor = 0;
            self.info_cursor = 0;
            // AND THE QUEUE GOES BACK TO THE TOP, by the same causal rule one
            // line down: the subject changed. Without it, walking from one
            // playlist to another would start the second one at whichever
            // entry the first was on.
            self.playing = 0;
            // AND THE FILM STARTS AGAIN, which is the same causal argument one
            // line up: the subject changed. Without it, leaving a film paused,
            // walking two files away and coming back would show another film
            // held at somebody else's pause, muted or not according to a
            // decision made about a different file.
            //
            // A SOUND ARRIVES STOPPED, which is the one thing that differs and
            // the reason this asks the answer what it is. See
            // [`Transport::arriving`].
            self.transport =
                Transport::arriving(l.moving.as_ref().map(|m| m.show), l.list.is_some());
            // AND THE WIDE MODE STAYS. It used to go with the file, and that is
            // the opposite of what the owner asked for: *"sendo inclusive
            // possível navegar para os próximos arquivos a partir do próprio
            // preview largo"*. It is a way of LOOKING, not a property of what
            // is being looked at — and `whole` still gives the region back by
            // itself when there is nothing under the cursor.
            self.trouble = None;
            self.frame = None;
            // AND THE SHAPE IS SHOWN FROM THE FRONT AGAIN. Same argument: the
            // subject changed. Leaving one piece turned upside down and
            // walking to the next would otherwise hand over an angle chosen
            // for something else.
            self.pose = ronoctua_inspect::solid::Pose::default();
            self.turned = None;
            self.shape_trouble = None;
        }
        self.looked = Some(l);
    }

    /// What went wrong with the film, when something did.
    ///
    /// **Refusal is a value, not an absence** — a panel that is blank and a
    /// panel that was refused look the same on screen, and only one of them has
    /// a sentence. The doors put it on the message line.
    #[must_use]
    pub fn film_trouble(&self) -> Option<Explanation> {
        let what = self.trouble?;
        // THE CURSOR IS STILL ON A FILM, and that is the whole condition. It is
        // NOT "there is a frame": a starved film has none, and that is exactly
        // when the sentence matters most — the panel is empty and nothing else
        // says why.
        self.showing_now()?;
        self.looked.as_ref()?.moving.as_ref()?;
        Some(match what {
            crate::motion::Trouble::NoSound => Explanation {
                key: ronoctua_i18n::key::FILES_NO_SOUND,
                args: Vec::new(),
            },
            crate::motion::Trouble::Starved => Explanation {
                key: ronoctua_i18n::key::FILES_FILM_STUCK,
                args: vec![(
                    "seconds",
                    ronoctua_inspect::motion::STARVED.as_secs().to_string(),
                )],
            },
        })
    }

    /// The texture key of the picture the preview is showing, if there is one.
    ///
    /// The door asks `Images::missing` with it BEFORE uploading — a preview is
    /// up to a mebibyte of RGBA, and putting that on the bus every frame is the
    /// transfer the key exists to avoid.
    ///
    /// # A FRAME counts, and that was a defect
    ///
    /// It used to answer only when there was a still picture, and a film whose
    /// poster failed to decode then drew NOTHING for ever: the player ran, the
    /// frames arrived, the view stored them, and no door ever asked for a
    /// rectangle to put them in. A moving file has a picture whether or not it
    /// has a poster.
    #[must_use]
    pub fn preview_key(&self) -> Option<String> {
        self.showing_now()?;
        let l = self.looked.as_ref()?;
        // A LIST IS DRAWN AS A LIST, and its player's frames are never drawn.
        //
        // They are still MADE, and that is not waste with a name: the frames
        // are the clock — the position, the end of a track and the pause all
        // come out of them, and a door with no frames would need a timer, a
        // second way to pause and a message for the end. What is dropped is
        // only the picture, so the rows underneath stay legible; the frame is
        // asked for at the smallest size there is, so what is thrown away is
        // sixteen bytes a frame. See `Panel::wish`.
        if l.list.is_some() {
            return None;
        }
        // A FRAME AND A PHOTOGRAPH ARE TWO TEXTURES, and that is a defect
        // avoided rather than found.
        //
        // A door uploads the still only `if im.missing(&key)` — a preview is up
        // to a mebibyte — while every frame is written under the key it is
        // given. Sharing one key, the film's frames overwrite the photograph,
        // and on a MOTION PHOTO the loop's return to the photograph would find
        // the key present and upload nothing: the panel would hold the film's
        // last frame for ever, and the feature would work once.
        //
        // Two keys, two slots of the renderer's ceiling. Not a key per frame, which is
        // what would fill the cache with one file's film.
        if self.frame.as_ref().is_some_and(|f| f.key == l.key)
            || self.turned.as_ref().is_some_and(|f| f.key == l.key)
        {
            // A TURNED SHAPE IS A FRAME LIKE ANY OTHER. It shares the mark and
            // therefore the slot, which is right: the two can never both be in
            // hand, because a file is either a film or a shape.
            return Some(format!("{}{FRAME_MARK}", l.key));
        }
        l.rgba.as_ref().map(|_| l.key.clone())
    }

    /// The pixels behind that key, for the door that has to upload them.
    #[must_use]
    pub fn preview_pixels(&self) -> Option<&ronoctua_render::image::Rgba> {
        self.showing_now()?;
        self.looked.as_ref()?.rgba.as_ref()
    }

    /// What a shape is drawn with, out of the tokens.
    ///
    /// A function and not a constant because a `Color` cannot be taken apart at
    /// compile time, and a second spelling of the same colour is exactly what
    /// invariant 9 exists to prevent. The light's two angles are integers of
    /// DEGREES for the reason `space.nix` gives — this file is where they become a
    /// direction.
    pub(crate) fn paint_of() -> ronoctua_inspect::solid::Paint {
        let azimuth = f32::from(ronoctua_tokens::space::FILES_MESHLIGHTAZIMUTH).to_radians();
        let elevation = f32::from(ronoctua_tokens::space::FILES_MESHLIGHTELEVATION).to_radians();
        ronoctua_inspect::solid::Paint {
            // sRGB BYTES and not `to_linear`: what comes out of the rasteriser is
            // a texture, and the pipeline does the curve on write. Handing it
            // linear values here would lighten the piece by exactly the amount
            // nobody can trace back to a token.
            solid: ronoctua_tokens::color::SURFACE_MESH.to_rgba8(),
            // Camera space: x to the right, y up, z towards whoever is looking.
            light: [
                elevation.cos() * azimuth.cos(),
                elevation.cos() * azimuth.sin(),
                elevation.sin(),
            ],
            ambient: f32::from(ronoctua_tokens::space::FILES_MESHSHADOW) / 100.0,
            // THE TWO A CAGE USES. A STEP or an IGES holds no surface, so it
            // is drawn as lines; see `ronoctua_inspect::mesh::Wires`. Tenths
            // of a pixel, because a whole one is too coarse next to the
            // rasteriser's two samples per side.
            wire: f32::from(ronoctua_tokens::space::FILES_MESHWIREWIDTH) / 10.0,
            fade: f32::from(ronoctua_tokens::space::FILES_MESHWIREFADE) / 100.0,
        }
    }

    /// The picture the preview's box should be sized from.
    ///
    /// # The frame first, and it is the defect the owner photographed
    ///
    /// Both doors used to measure the box from [`Panel::preview_pixels`] — the
    /// STILL frame — and then draw whatever the player had sent into it. The
    /// two are different shapes, so every film was stretched to the poster's
    /// proportions. Under the old height guess that was a ninefold vertical
    /// stretch, which is what *"the frames look rotated"* was.
    ///
    /// One answer, asked by both doors, and the picture that is DRAWN is the
    /// picture that is measured.
    #[must_use]
    pub fn preview_now(&self) -> Option<&ronoctua_render::image::Rgba> {
        self.live_frame().or_else(|| self.preview_pixels())
    }

    /// The picture that is REDRAWN, whatever is redrawing it.
    ///
    /// # Why the doors ask this and not the two behind it
    ///
    /// Because they do not upload it the way they upload a still. A still goes
    /// up only `if im.missing(&key)` — it is up to a mebibyte — while a picture
    /// that changes must be written under the same key every time, or the
    /// second one never reaches the screen.
    ///
    /// A film was the only thing that did that, so the doors asked
    /// `moving_frame` by name. A turned shape does it too, and the shape of the
    /// defect was exactly what one would expect: the stage drew every frame,
    /// the log said so, and the panel stayed empty because nothing uploaded
    /// them. One question, asked in both doors, and the next thing that redraws
    /// itself needs no third site.
    #[must_use]
    pub fn live_frame(&self) -> Option<&ronoctua_render::image::Rgba> {
        self.moving_frame().or_else(|| self.turned_frame())
    }

    /// The drawn shape, when there is one and it is about this file.
    #[must_use]
    pub fn turned_frame(&self) -> Option<&ronoctua_render::image::Rgba> {
        self.showing_now()?;
        let l = self.looked.as_ref()?;
        let f = self.turned.as_ref()?;
        (f.key == l.key).then_some(&f.rgba)
    }

    /// Whether the cursor is on a shape the manager turns.
    #[must_use]
    pub fn shape_here(&self) -> bool {
        self.showing_now().is_some() && self.looked.as_ref().is_some_and(|l| l.turns)
    }

    /// The player had something to say.
    ///
    /// Dropped when it is not about the file the cursor is on — the same rule
    /// [`Panel::looked`] keeps, and for the same reason: one file's picture
    /// under another file's name is the one failure a preview must not have.
    pub fn played(&mut self, beat: crate::motion::Beat) {
        let key = match &beat {
            crate::motion::Beat::Frame(f) => &f.key,
            crate::motion::Beat::Ended { key }
            | crate::motion::Beat::Still { key }
            | crate::motion::Beat::Trouble { key, .. } => key,
        };
        if self.asked.as_deref() != Some(key.as_str()) {
            return;
        }
        match beat {
            crate::motion::Beat::Frame(f) => {
                self.transport.at = f.at;
                self.frame = Some(f);
            }
            // THE FILM RAN OUT, and it has to SAY so. Before this the frames
            // simply stopped arriving, the last one stayed up for ever and the
            // play glyph never came back — the absence of a wake-up is not an
            // event and cannot be drawn.
            crate::motion::Beat::Ended { .. } => {
                // A LIST GOES ON TO THE NEXT ONE, and it costs no new message
                // and no new state machine: `wish` builds the reel from the
                // entry, `Reel` is compared by value, and a changed path is
                // therefore already a respawn.
                if let Some(next) = self.after_the_one_playing() {
                    self.playing = next;
                    self.transport.from = 0.0;
                    self.transport.at = 0.0;
                    self.transport.paused = false;
                    return;
                }
                self.transport.on = false;
                self.transport.paused = false;
            }
            // A MOTION PHOTO'S OTHER HALF. Letting the frame go is what puts
            // the photograph back on screen: the panel draws the still whenever
            // there is no frame in hand.
            crate::motion::Beat::Still { .. } => {
                self.frame = None;
                self.transport.at = 0.0;
            }
            crate::motion::Beat::Trouble { what, .. } => match what {
                // THE MACHINE HAS NO SOUND. The picture goes on without it and
                // the wish is corrected, so nothing asks again every frame.
                crate::motion::Trouble::NoSound => {
                    self.transport.sound = false;
                    self.trouble = Some(crate::motion::Trouble::NoSound);
                }
                crate::motion::Trouble::Starved => {
                    self.transport.on = false;
                    self.frame = None;
                    self.trouble = Some(crate::motion::Trouble::Starved);
                }
            },
        }
    }

    /// What SHOULD be running, as a value.
    ///
    /// `None` is "nothing at all": no film under the cursor, the preview hidden,
    /// or a film that has ended or been stopped.
    pub(crate) fn wish(&self) -> Option<crate::motion::Reel> {
        let l = self.looked.as_ref()?;
        self.showing_now()?;
        if !self.transport.on {
            return None;
        }
        let fit = self.preview_fit()?;
        // A PLAYLIST PLAYS A FILE IT HAS NOT LOOKED INSIDE, and that is a
        // limit worth writing down rather than working around.
        //
        // Everything else the player is asked for comes out of a probe the
        // inspector already ran for the facts. An entry has had no such probe:
        // it is another file, named by this one, and probing it here would be
        // an ffprobe on the door's event loop with the five-second patience
        // `tools::run` gives every child — which is the footer frozen.
        //
        // So an entry plays with what is safe to assume: the waveform, which
        // needs no picture in the file, and `cover: false`, which is the SAFE
        // half of that flag — claiming a cover a file has not got makes ffmpeg
        // refuse the whole command, while missing one that is there only costs
        // the artwork. The length is the playlist's own word, which is what a
        // playlist is FOR.
        if let Some(list) = self.showing_list() {
            let entry = list.entries.get(self.playing)?;
            let ronoctua_inspect::playlist::Where::Local(path) = &entry.at else {
                return None;
            };
            return Some(crate::motion::Reel {
                path: path.clone(),
                // A FRAME NOBODY DRAWS, AND STILL BIG ENOUGH TO STOP ITS OWN
                // CHILD. The panel shows the list, so the picture is thrown
                // away — but the frames are the clock and the pause, and a
                // frame smaller than a pipe pauses nothing at all. See
                // `motion::PIPE_BYTES` and `Panel::preview_key`.
                // THE PLAYLIST'S KEY, not the entry's: `Panel::played` drops a
                // beat whose key is not the one that was asked for, and what
                // was asked for is the file in front. Frames from a child that
                // has been replaced are dropped by the ERA, in the thread,
                // which is what that mechanism is for.
                key: l.key.clone(),
                how: ronoctua_inspect::motion::Source::Ffmpeg,
                show: ronoctua_inspect::motion::Show::Sound { cover: false },
                fit: ronoctua_inspect::motion::Fit::UNSEEN,
                fps: ronoctua_inspect::facts::WAVES_FPS,
                from: self.transport.from,
                sound: self.transport.sound,
                background: letterbox(),
                wave: ink(ronoctua_tokens::color::SURFACE_WAVE),
                wave_height: ronoctua_tokens::space::FILES_WAVEHEIGHT,
                wave_foot: self.controls_foot(),
                inside: None,
            });
        }
        let moving = l.moving.as_ref()?;
        Some(crate::motion::Reel {
            path: l.path.clone(),
            key: l.key.clone(),
            how: moving.how,
            show: moving.show,
            fit,
            fps: moving.shape.fps,
            from: self.transport.from,
            // AND SOUND IS REFUSED WHERE THERE IS NONE TO HEAR. Measured
            // against the real binary: ffmpeg refuses the WHOLE command when
            // the audio output cannot be filled, so asking for sound on a
            // silent film would stop the picture.
            sound: self.transport.sound && moving.shape.sound,
            background: letterbox(),
            wave: ink(ronoctua_tokens::color::SURFACE_WAVE),
            wave_height: ronoctua_tokens::space::FILES_WAVEHEIGHT,
            wave_foot: self.controls_foot(),
            // WHERE THE FILM IS, when the picture is a photograph with one in
            // it. `None` for an ordinary film, which IS the file.
            inside: moving.inside.clone(),
        })
    }

    /// What the player should be told, given everything that has changed.
    ///
    /// `None` is "nothing has changed". One question, one place, one
    /// comparison: the door asks once a frame and passes the answer straight
    /// on. Enumerating the ways a film can need restarting — the cursor moved,
    /// the panel was hidden, the window was resized, the sound was asked for,
    /// somebody scrubbed — is how a case gets missed, and every one of them is
    /// a field of [`crate::motion::Reel`].
    pub fn wants_motion(&mut self) -> Option<crate::motion::Ask> {
        let now = self.wish();
        let ask = match (&self.told, &now) {
            (None, None) => None,
            (Some(_), None) => {
                // AND THE FRAME GOES WITH IT ONLY WHEN THE FILM WENT.
                //
                // A film that ENDED keeps its last picture and its play glyph,
                // because pressing play is how one watches it again. A cursor
                // that MOVED must not: one file's picture under another file's
                // name is the one failure a preview must not have.
                if !self.plays_here() {
                    self.frame = None;
                }
                Some(crate::motion::Ask::Stop)
            }
            (None, Some(w)) => Some(crate::motion::Ask::Play(w.clone(), self.transport.paused)),
            (Some(was), Some(w)) if was == w => None,
            // A RESPAWN OF THE SAME FILM PICKS UP WHERE THE PICTURE IS.
            //
            // `from` is where the last child was TOLD to start, and it only
            // moves when somebody seeks — which is right, or the wish would
            // change thirty times a second. But a respawn for another reason —
            // the panel resized, `C-w` given or taken back, the sound switched
            // — would then start the film at that old mark again. The owner
            // photographed it: *"ao trocar para o modo largo, ou retornar dele,
            // o vídeo retrocede em vez de continuar no exato último tempo"*.
            (Some(was), Some(w)) if was.path == w.path && was.key == w.key => {
                self.transport.from = self.transport.at;
                // AND THE WISH IS REBUILT AT THE NEW MARK, so what is recorded
                // as told is what was actually asked for.
                let w = self.wish()?;
                Some(crate::motion::Ask::Play(w, self.transport.paused))
            }
            (Some(_), Some(w)) => Some(crate::motion::Ask::Play(w.clone(), self.transport.paused)),
        };
        // AND PAUSING IS NOT A RESPAWN. It is not part of the wish at all: the
        // child stays alive, blocked on a pipe nobody is draining, at no cost —
        // against the 108 to 242 milliseconds a seek costs, measured.
        if ask.is_none() && now.is_some() {
            if self.transport.paused != self.holding {
                self.holding = self.transport.paused;
                return Some(if self.holding {
                    crate::motion::Ask::Pause
                } else {
                    crate::motion::Ask::Resume
                });
            }
            return None;
        }
        if let Some(crate::motion::Ask::Play(w, held)) = &ask {
            // AND THE PLAYER WAS TOLD THE HOLD, so `wants_motion` must not go
            // on believing it is running — or it would send a Resume nobody
            // asked for on the very next frame.
            self.holding = *held;
            // WHAT WAS ACTUALLY ASKED FOR, and not what was wished a moment
            // earlier: the branch above may have moved the mark.
            self.told = Some(w.clone());
        } else if ask.is_some() {
            self.holding = false;
            self.told = None;
        }
        ask
    }

    /// What SHOULD be drawn of a shape, as a value.
    ///
    /// `None` is "nothing at all": no shape under the cursor, the preview
    /// hidden, or a panel not measured yet.
    fn wish_turn(&self) -> Option<crate::stage::Turn> {
        let l = self.looked.as_ref()?;
        self.showing_now()?;
        if !l.turns {
            return None;
        }
        // THE SAME QUANTISED BOX THE FILM ASKS FOR, and for the same reason:
        // dragging a window's edge changes the panel sixty times a second, and
        // a size that followed it exactly would redraw the piece on every one.
        let fit = self.preview_fit()?;
        Some(crate::stage::Turn {
            path: l.path.clone(),
            mime: l.mime,
            key: l.key.clone(),
            width: fit.width,
            height: fit.height,
            pose: self.pose,
            paint: Self::paint_of(),
        })
    }

    /// Points the globe at whatever the cursor is on, in the box it was given.
    ///
    /// The DOOR measures the box and calls this, exactly as `measured_preview`
    /// already works: the picture that is drawn is the picture that was
    /// measured, and a second opinion about the size is a texture the GPU has
    /// at one size and the layout at another.
    /// `from` is how many pixels of the picture's TOP have scrolled away — the
    /// band's rows are content and they move, and the picture keeps its SIZE
    /// while they do. See [`crate::globe::paint`].
    pub fn measured_globe(&self, size: (u32, u32), from: u32, theme: usize) {
        let at = self.looked.as_ref().and_then(|l| l.at);
        let key = self
            .looked
            .as_ref()
            .map_or(String::new(), |l| l.key.clone());
        let portuguese = crate::globe::tongue(&self.texts) == ronoctua_atlas::Tongue::Portuguese;
        self.globe
            .borrow_mut()
            .looking_at(at, &key, size, from, theme, portuguese);
    }

    /// What the globe wants drawn, when it is not what was last asked for.
    pub fn wants_globe(&mut self) -> Option<crate::globe::Ask> {
        self.globe.borrow_mut().wants()
    }

    /// A picture or an address coming back.
    pub fn saw_globe(&mut self, beat: &crate::globe::Beat) -> bool {
        // THE SHEET IS REMEMBERED and the address is part of it, so an answer
        // arriving has to forget what was built before it.
        if self.globe.borrow_mut().saw(beat) {
            *self.remembered.borrow_mut() = None;
            return true;
        }
        false
    }

    /// The globe's picture, when there is one for what is being shown.
    #[must_use]
    pub fn globe_frame(&self) -> Option<ronoctua_render::image::Rgba> {
        self.globe.borrow().frame().cloned()
    }

    /// Its texture's name.
    #[must_use]
    pub fn globe_key(&self) -> Option<String> {
        self.globe.borrow().key()
    }

    /// Whether there is a globe on screen at all — as a band, or as the
    /// expanded sheet's first column.
    ///
    /// BOTH SHAPES, because the keys that turn and frame it mean the same thing
    /// in either: asking only about the band killed `C-k`, `C-j` and `C-r` in
    /// the one place the picture is biggest.
    #[must_use]
    pub fn globe_here(&self) -> bool {
        self.globe_rows() > 0 || self.globe_columns() > 0
    }

    /// Whether the pointer is resting on the globe, told by the door.
    ///
    /// The door has already resolved a target to draw the band; this is that
    /// answer remembered where it was found, so the wheel does not have to ask
    /// a second time with less to go on.
    pub fn pointing_globe(&self, on: bool) {
        self.pointing_at_the_globe.set(on);
    }

    /// Turns the globe by a drag, in pixels.
    pub fn pan_globe(&mut self, right: f32, down: f32) -> bool {
        self.globe.borrow_mut().pan(right, down)
    }

    /// The difference between what is drawn and what should be.
    ///
    /// One comparison in one place, the arrangement [`Panel::wants_motion`]
    /// already argues for: enumerating the ways a picture can need redrawing —
    /// the file changed, the panel resized, the piece was turned, zoomed,
    /// reframed — is how one of them gets missed.
    pub fn wants_turn(&mut self) -> Option<crate::stage::Ask> {
        let now = self.wish_turn();
        let ask = match (&self.told_turn, &now) {
            (None, None) => None,
            (Some(_), None) => Some(crate::stage::Ask::Stop),
            (Some(was), Some(w)) if was == w => None,
            (_, Some(w)) => Some(crate::stage::Ask::Show(Box::new(w.clone()))),
        };
        if ask.is_some() {
            self.told_turn = now;
        }
        ask
    }

    /// The stage had something to say.
    ///
    /// Dropped when it is not about the file the cursor is on — the same rule
    /// [`Panel::played`] keeps, and for the same reason.
    pub fn drew(&mut self, beat: crate::stage::Beat) {
        let about = match &beat {
            crate::stage::Beat::Drawn(f) => f.key.clone(),
            crate::stage::Beat::Trouble { key, .. } => key.clone(),
        };
        if self.asked.as_deref() != Some(about.as_str()) {
            return;
        }
        match beat {
            crate::stage::Beat::Drawn(f) => {
                self.shape_trouble = None;
                self.turned = Some(*f);
            }
            crate::stage::Beat::Trouble { why, .. } => {
                self.turned = None;
                self.shape_trouble = Some(why);
            }
        }
    }

    /// What went wrong with the shape, when something did.
    ///
    /// **Refusal is a value, not an absence** — the rule the rest of the panel
    /// keeps. A `.stl` that will not parse says so instead of leaving a blank
    /// panel waiting for a frame that is never coming.
    #[must_use]
    pub fn shape_said(&self) -> Option<Explanation> {
        let why = self.shape_trouble.as_ref()?;
        self.showing_now()?;
        Some(Explanation {
            key: why.key(),
            args: why.args(),
        })
    }

    /// Turns the shape as a hand moving ACROSS THE SCREEN would, in radians.
    ///
    /// `right` and `down` are the directions the hand went, not angles about
    /// an axis — which is the whole reason this function exists rather than the
    /// doors reaching for `Pose::turned` themselves.
    ///
    /// # Why `right` is negated
    ///
    /// Because `solid::axes` puts the CAMERA at `yaw`, so a larger yaw walks
    /// the camera anticlockwise and everything on screen slides LEFT — which is
    /// why `files-turn-left` adds. A drag is the opposite gesture: the hand
    /// takes the piece WITH it, so pulling left must turn it left, and that is
    /// a SMALLER yaw. The doors had the sign the other way round and the owner
    /// reported it from the screen — *"ao arrastar para a esquerda, o objeto
    /// está girando para a direita"* — which is exactly what a convention
    /// spelled out at two call sites and nowhere testable produces.
    ///
    /// `down` is NOT negated, and it is the same rule: pulling down tips the
    /// top of the piece towards the viewer, which is more of it seen from
    /// above, which is a HIGHER elevation.
    ///
    /// `false` when there is no shape to turn, so the caller can let the
    /// gesture mean whatever it means elsewhere.
    pub fn spin(&mut self, right: f32, down: f32) -> bool {
        if !self.shape_here() {
            return false;
        }
        self.pose = self.pose.turned(-right, down);
        true
    }

    /// Moves the shape — or the globe — nearer or further.
    ///
    /// ONE NOTATION, ONE ID, and the FOCUSED PANE decides which subject: a
    /// shape is in the preview panel and a globe is in the info panel, so the
    /// two contexts are disjoint by construction and no key is ambiguous.
    pub fn zoom(&mut self, by: f32) -> bool {
        if matches!(self.around.get().keyboard, Pane::Info) && self.globe_here() {
            return self.globe.borrow_mut().zoom(by);
        }
        if !self.shape_here() {
            return false;
        }
        self.pose = self.pose.scaled(by);
        true
    }

    /// Puts the shape — or the globe — back the way it is first shown.
    pub fn reframe(&mut self) -> bool {
        if matches!(self.around.get().keyboard, Pane::Info) && self.globe_here() {
            return self.globe.borrow_mut().reframe();
        }
        if !self.shape_here() {
            return false;
        }
        self.pose = ronoctua_inspect::solid::Pose::default();
        true
    }

    /// The frame a moving file is showing, if one is.
    #[must_use]
    pub fn moving_frame(&self) -> Option<&ronoctua_render::image::Rgba> {
        self.showing_now()?;
        let l = self.looked.as_ref()?;
        let f = self.frame.as_ref()?;
        (f.key == l.key).then_some(&f.rgba)
    }

    /// How large a frame the preview wants, in pixels.
    ///
    /// From the panel's OWN box, quantised — see [`ronoctua_inspect::motion::Fit`].
    /// The player used to decode at a fixed 512 whatever the panel was, which
    /// is 576 KiB a frame against 83 for a four-line strip: seven times the
    /// traffic to the graphics card for a picture nobody could see the
    /// difference in.
    fn preview_fit(&self) -> Option<ronoctua_inspect::motion::Fit> {
        let (wide, tall) = self.drawn_preview.get();
        #[allow(clippy::cast_precision_loss)]
        let by = self.preview_scale.get().max(1) as f32;
        ronoctua_inspect::motion::Fit::of(wide * by, tall * by)
    }

    /// The scale of the screen the preview is drawn on — see
    /// [`Panel::preview_scale`]. Told by the door at every frame, as the box
    /// is.
    pub fn preview_at_scale(&self, scale: i32) {
        self.preview_scale.set(scale.max(1));
    }

    /// The scale last told — see [`Panel::preview_at_scale`].
    #[must_use]
    pub fn preview_scale_now(&self) -> i32 {
        self.preview_scale.get()
    }

    /// The box the door is actually drawing the picture into, in pixels.
    ///
    /// # It has to be the DOOR's number and not an estimate from cells
    ///
    /// The frame is decoded at the box's own shape, so ffmpeg's letterbox is
    /// the panel's own air and the picture is framed once. Estimating the box
    /// as `cells × cell_width` gets the shape WRONG — the door then takes
    /// `files::picture_air` off each side, and a box narrower on each axis
    /// is a different aspect. The owner photographed the result: *"a imagem do
    /// GIF está distorcida"*.
    ///
    /// A `Cell`, for the reason `drawn_lines` is one: the painter holds the
    /// view immutably, and this is written where the box is decided.
    pub fn measured_preview(&self, wide: f32, tall: f32) {
        if wide > 1.0 && tall > 1.0 {
            self.drawn_preview.set((wide, tall));
        }
    }

    /// The film's timeline, when there is a film with a length.
    pub(crate) fn timeline(&self) -> Option<ronoctua_design::Scroll> {
        self.looked.as_ref()?;
        self.showing_now()?;
        let seconds = self.playing_seconds()?;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let total = (seconds * 10.0).round().max(1.0) as usize;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let offset = (self.transport.at.max(0.0) * 10.0).round() as usize;
        Some(ronoctua_design::Scroll {
            total,
            // ONE TENTH OF A SECOND WIDE, which is what makes the cursor a MARK
            // and not a page: `Scroll::thumb` never draws one shorter than a
            // cell, so the shortest film and the longest get the same handle.
            lines: 1,
            offset: offset.min(total.saturating_sub(1)),
            top: 0,
        })
    }

    /// Whether the preview has been given the whole region.
    ///
    /// # It is DERIVED, and that is what makes it undo itself
    ///
    /// Not a mode that is stored and switched off: the film ending, the cursor
    /// moving, `C-v` hiding the preview — each of them would be a case somebody
    /// has to remember to undo, and the one that gets forgotten leaves the
    /// manager showing a blank region. Asking the question every frame has no
    /// such case.
    #[must_use]
    pub fn whole(&self) -> bool {
        self.wide().is_some()
    }

    /// WHICH panel has the whole region, and `None` when neither has.
    ///
    /// # It is DERIVED, and that is what makes it undo itself
    ///
    /// Not a mode that is stored and switched off: the film ending, the cursor
    /// moving, `C-v` hiding the preview — each of them would be a case somebody
    /// has to remember to undo, and the one that gets forgotten leaves the
    /// manager showing a blank region. Asking the question every frame has no
    /// such case.
    ///
    /// ANY PREVIEW, and not only a film. The owner asked for it: *"fazer o
    /// modo largo funcionar em todos os preview, de todos os arquivos"* — a
    /// photograph is exactly as worth a whole region as a film, and so is a
    /// page of code. And the FACTS are worth one too, which is what he asked
    /// for next: expanded, they stop being a column twenty-four cells wide and
    /// become as many columns as the screen can hold.
    ///
    /// What it still needs is something to BE about: with no file under the
    /// cursor the region would be given to an empty panel, and the key would
    /// have lost the listing for nothing. And the panel has to be ASKED for —
    /// `C-v` and `C-i` hide them, and a hidden panel handed the region would be
    /// a blank screen.
    #[must_use]
    pub fn wide(&self) -> Option<Which> {
        let which = self.theatre?;
        (self.panel_on(which) && self.subject.borrow().is_some()).then_some(which)
    }

    /// Whether a panel was asked for at all — `C-v` and `C-i`.
    #[must_use]
    pub const fn panel_on(&self, which: Which) -> bool {
        match which {
            Which::Preview => self.preview_on,
            Which::Info => self.info_on,
        }
    }

    /// Whether the cursor is on a film the preview is showing.
    ///
    /// # It is not "is there a frame"
    ///
    /// A film that has ENDED keeps its last frame and its controls, so one can
    /// press play and watch it again — so a frame is not the question. A film
    /// that STARVED has neither, and it is the one case where there is nothing
    /// left to transport.
    ///
    /// The five keys and the whole-region wish all ask this, which is what
    /// makes them refuse together: five conditions written five times is five
    /// chances for one of them to say yes where the others say no.
    ///
    /// It answers for a SOUND as well, which is why it is no longer called
    /// `film_here`: a sound plays, has a timeline, is sought and is paused by
    /// the same five keys, and the only thing that differs is where its
    /// picture comes from.
    pub(crate) fn plays_here(&self) -> bool {
        self.showing_now().is_some()
            && self
                .looked
                .as_ref()
                // A PLAYLIST PLAYS TOO, and through the same five keys: what
                // it names is what is heard, and the transport is about the
                // entry rather than about the file naming it.
                .is_some_and(|l| l.moving.is_some() || l.list.is_some())
            && !matches!(self.trouble, Some(crate::motion::Trouble::Starved))
    }

    /// Which section the pointer is resting in.
    ///
    /// The doors say so where they already resolve a [`Target`]; it decides
    /// whether the transport's two buttons are drawn. The timeline is NOT
    /// hidden with them — the owner's choice, and the right one: a track that
    /// appeared under the pointer would move the cursor out from under it in
    /// the instant of the click.
    pub fn pointing(&self, pane: Option<Pane>) {
        self.pointing.set(pane);
    }

    /// The letters behind the cursor, when the file has any and no picture.
    ///
    /// It answers `None` the moment the cursor leaves the file, exactly as the
    /// picture does — the panel is about the focused entry, and a panel still
    /// showing the last one is the failure a preview must not have.
    #[must_use]
    pub fn showing_text(&self) -> Option<&ronoctua_inspect::text::Read> {
        self.showing_now()?;
        self.looked.as_ref()?.text.as_ref()
    }

    /// One row of a playlist: the mark, the name, and how long it says it runs.
    ///
    /// # The mark's cell is always there
    ///
    /// Reserved whether or not this is the entry being played, so the rows do
    /// not shift sideways as the queue advances — the argument `info_fields`
    /// already makes about the scrollbar's cell, one panel over.
    ///
    /// Hierarchy is colour, never size and never a rule: the entry being
    /// played is `TEXT_PRIMARY` against `TEXT_SECONDARY` for the rest, and one
    /// that cannot be played from here is `TEXT_DIM` with the reason on the
    /// message line.
    fn entry_fields(
        &self,
        entry: &ronoctua_inspect::playlist::Entry,
        row: usize,
        at: u16,
        room: u16,
    ) -> Vec<Field> {
        use ronoctua_inspect::playlist::Where;
        const MARK: u16 = 2;
        let here = matches!(entry.at, Where::Local(_));
        let colour = if row == self.playing && here {
            ronoctua_tokens::color::TEXT_PRIMARY
        } else if here {
            ronoctua_tokens::color::TEXT_SECONDARY
        } else {
            ronoctua_tokens::color::TEXT_DIM
        };
        // WHAT THE PLAYLIST WROTE, and not what it resolved to: somebody who
        // wrote `../Album/01.flac` should see that. The resolved path goes on
        // the message line, which is the register `files-fact-says` uses.
        let said = entry.title.clone().unwrap_or_else(|| entry.written.clone());
        let clock = entry.seconds.map(clock_of).unwrap_or_default();
        let tail = width_of(&clock);
        let for_name = room.saturating_sub(MARK).saturating_sub(tail);
        // AND A BACKING FIELD ACROSS THE WHOLE ROW, so `files::View::hit` answers for
        // the air as well as for the letters — a row one can only click on its
        // text is a row that misses the click that was aimed at it.
        let mut out = vec![Field {
            at,
            text: " ".repeat(usize::from(room)),
            color: colour,
            hit: Hit::Track(row),
            explain: None,
            ink: Ink::Grid,
        }];
        if row == self.playing && here {
            out.push(Field {
                at,
                text: ronoctua_tokens::icon::PLAY.to_string(),
                color: colour,
                hit: Hit::Track(row),
                explain: None,
                ink: Ink::Grid,
            });
        }
        out.push(Field {
            at: at.saturating_add(MARK),
            text: ronoctua_render::layout::elide_end(&said, for_name),
            color: colour,
            hit: Hit::Track(row),
            // WHERE IT ACTUALLY POINTS, on the message line. The row shows
            // what the playlist WROTE; this is what that resolved to, which is
            // the half somebody needs when a name does not find a file.
            // THE WHOLE NAME AND WHERE IT POINTS, un-elided.
            //
            // The row cuts what does not fit; this is where the rest of it is,
            // which is the register `files-fact-says` already keeps for the
            // same reason one panel over. It is also why a playlist needs no
            // sideways bar — see `Panel::preview_widest`.
            explain: Some(Explanation {
                key: match (&entry.at, entry.title.is_some()) {
                    (Where::Remote(_), _) => ronoctua_i18n::key::FILES_ENTRY_NOT_HERE,
                    (Where::Refused, _) => ronoctua_i18n::key::FILES_ENTRY_REFUSED,
                    (Where::Local(_), true) => ronoctua_i18n::key::FILES_ENTRY_TITLED,
                    (Where::Local(_), false) => ronoctua_i18n::key::FILES_ENTRY_AT,
                },
                args: {
                    let path = match &entry.at {
                        Where::Local(p) => p.display().to_string(),
                        Where::Remote(url) => url.clone(),
                        Where::Refused => entry.written.clone(),
                    };
                    match &entry.title {
                        Some(name) if matches!(entry.at, Where::Local(_)) => {
                            vec![("name", name.clone()), ("path", path)]
                        }
                        _ => vec![("path", path)],
                    }
                },
            }),
            ink: Ink::Grid,
        });
        if tail > 0 {
            out.push(Field {
                at: at.saturating_add(room).saturating_sub(tail),
                text: clock,
                color: ronoctua_tokens::color::TEXT_DIM,
                hit: Hit::Track(row),
                explain: None,
                ink: Ink::Grid,
            });
        }
        out
    }

    /// The list of things to play under the cursor, if the panel is showing one.
    ///
    /// `showing_now` first, for the reason its twin has: an answer that arrived
    /// for a file the cursor has already left is not about what is on screen.
    #[must_use]
    pub fn showing_list(&self) -> Option<&ronoctua_inspect::playlist::List> {
        self.showing_now()?;
        self.looked.as_ref()?.list.as_ref()
    }

    /// How many ROWS the preview has.
    ///
    /// A picture is one and a sentence is one; text is its lines, plus the row
    /// that says the file goes on. That `1` is what keeps a picture from
    /// scrolling with no branch anywhere: `slide` computes `last = 1 - room`,
    /// which is zero, so the gesture is swallowed by arithmetic instead of by
    /// a special case.
    #[must_use]
    pub fn preview_total(&self) -> usize {
        if let Some(l) = self.showing_list() {
            return l.entries.len() + usize::from(l.cut);
        }
        if let Some(t) = self.showing_text() {
            return t.lines.len() + usize::from(t.cut);
        }
        // AND A REFUSAL IS AS TALL AS IT READS. It answered `1` while the
        // sentence was cut at the panel's width; now that it wraps, the bar
        // has to know — a panel four rows tall with a five-row sentence is
        // exactly what a scrollbar is for.
        self.refusal_lines()
    }

    /// How many rows the refusal's sentence takes at the panel's width.
    ///
    /// `0` when there is no sentence to draw, which is what a picture, a film
    /// and a shape answer: their rows are the panel's own.
    fn refusal_lines(&self) -> usize {
        let room = self.drawn_panel.get();
        if room == 0 {
            // NOT DRAWN YET is one row, which is what it was before any of
            // this: a number nobody has measured must not claim a scrollbar.
            return 1;
        }
        let said = match self.looked.as_ref().and_then(|l| l.why.as_ref()) {
            Some(why) => {
                let args: Vec<(&str, String)> = why.args();
                let borrowed: Vec<(&str, &str)> =
                    args.iter().map(|(k, v)| (*k, v.as_str())).collect();
                self.texts.t_with(why.key(), &borrowed)
            }
            None => self.texts.t(NO_PREVIEW),
        };
        ronoctua_render::layout::wrap(&said, room, REFUSAL_CEILING)
            .len()
            .max(1)
    }

    /// The widest line the preview holds, in cells.
    ///
    /// What sideways scrolling stops at. Measured in CLUSTERS, because that is
    /// the ruler `Field::covers`, `elide_end` and both painters use — counting
    /// bytes would let an accented line claim columns it does not fill.
    #[must_use]
    pub fn preview_widest(&self) -> usize {
        // A LIST NEVER READS SIDEWAYS, and that is the listing's own rule
        // rather than a shortcut.
        //
        // A row of a playlist is a RECORD — a mark, a name and a length, with
        // the length against the right edge — and not a line of text. Sliding
        // a record sideways moves two of its three parts out from under the
        // third. The owner met the half-built version of that: *"há playlists
        // que aparecem com scrollbar horizontal, e ainda mais inúteis, pois ao
        // deslizar tudo para o lado oposto, nada ocorre na tela"* — the bar was
        // offered and the rows never moved, because they never could.
        //
        // What a long name costs is an ellipsis, and what it does not cost is
        // the name: the whole of it is on the message line, where the manager
        // already puts what a row had to cut. See `Panel::entry_fields`.
        if self.showing_list().is_some() {
            return 0;
        }
        self.showing_text().map_or(0, |t| {
            t.lines
                .iter()
                .map(|l| usize::from(ronoctua_render::layout::width(&l.text)))
                .max()
                .unwrap_or(0)
        })
    }

    /// Whether an answer about the focused file is still on its way.
    ///
    /// `asked` names a file and `looked` does not answer for it yet. It is the
    /// difference between "there is nothing to show" and "nothing has arrived",
    /// and the panel draws them differently — the second one draws nothing at
    /// all, because a sentence that is replaced by a picture a fraction of a
    /// second later is a flicker with a message in it.
    fn looking_still(&self) -> bool {
        let Some(asked) = self.asked.as_deref() else {
            return false;
        };
        self.looked.as_ref().is_none_or(|l| l.key != asked)
    }

    /// `Some` when what was looked at is still what the cursor is on.
    fn showing_now(&self) -> Option<()> {
        if !self.preview_on {
            return None;
        }
        let subject = self.subject.borrow();
        let path = &subject.as_ref()?.path;
        (self.looked.as_ref()?.path == *path).then_some(())
    }

    /// What one of the threads came back with, handed to the part it is
    /// about: an answer to [`Panel::looked`], a frame to [`Panel::played`], a
    /// view of a shape to [`Panel::drew`]. Each drops what is not about the
    /// file the panels are on now.
    ///
    /// `true` when it was the inspector's ANSWER — the moment a host asks the
    /// film and the stage again (`Panel::wants_motion`, `Panel::wants_turn`),
    /// because the answer is what says the file moves.
    pub fn heard(&mut self, heard: Heard) -> bool {
        match heard {
            Heard::Looked(looked) => {
                self.looked(*looked);
                true
            }
            Heard::Beat(beat) => {
                self.played(beat);
                false
            }
            Heard::Turned(beat) => {
                self.drew(beat);
                false
            }
            // THE SECOND ANSWER, and it counts as an answer: it changes how
            // many rows the facts have, so the host follows the height again.
            Heard::Changeable { key, amendable } => {
                self.changeable(&key, *amendable);
                true
            }
        }
    }

    /// What can be changed in the file on screen, arriving after its facts.
    /// Dropped when it is about another ask — the cursor moved on, or the
    /// file was written again and asked afresh.
    pub fn changeable(&mut self, key: &str, amendable: crate::inspection_edit::Amendable) {
        if let Some(looked) = self.looked.as_mut()
            && looked.key == key
        {
            looked.amendable = amendable;
        }
    }

    // ── THE KEYS, THE POINTER AND THE WHEEL ─────────────────────────────────
    //
    // What a host's own dispatch hands the panels: an id out of its table, a
    // target its hit test resolved, a gesture over one of the two. Each
    // answers `None` — or `false` — for what is not the panels', and the host
    // goes on with it.

    /// One of the panels' own ids, out of the host's key table.
    ///
    /// `None` for an id that is not the panels': the host's own match goes on
    /// with it. The ids are the manager's (`files::OF_THE_VIEW`), because the
    /// keys are one table — a second one here would be a second place a
    /// binding could drift.
    pub fn key(&mut self, id: &str) -> Option<Step> {
        Some(match id {
            // A SHAPE, TURNED. `spin` answers `false` when there is none, and
            // then the key does nothing rather than something surprising —
            // there is nothing else `S-<left>` could sensibly mean here.
            // THE KEYS SPEAK THE SAME LANGUAGE AS THE HAND. `S-<left>` is
            // "move it left", which is what dragging left is, so both go
            // through `spin` and neither knows which way a yaw counts.
            "files-turn-left" => stepped(self.spin(-SPIN_STEP, 0.0)),
            "files-turn-right" => stepped(self.spin(SPIN_STEP, 0.0)),
            "files-turn-up" => stepped(self.spin(0.0, -SPIN_STEP)),
            "files-turn-down" => stepped(self.spin(0.0, SPIN_STEP)),
            "files-zoom-in" => stepped(self.zoom(ZOOM_STEP)),
            "files-zoom-out" => stepped(self.zoom(1.0 / ZOOM_STEP)),
            "files-reframe" => stepped(self.reframe()),
            // THE GLOBE'S FOUR, in degrees rather than in pixels: a key has no
            // distance, so it moves by a fixed step of the CURRENT view — a
            // tenth of the box, which reads the same near and far.
            "globe-north" => stepped(self.pan_globe(0.0, GLOBE_STEP)),
            "globe-south" => stepped(self.pan_globe(0.0, -GLOBE_STEP)),
            "globe-east" => stepped(self.pan_globe(-GLOBE_STEP, 0.0)),
            "globe-west" => stepped(self.pan_globe(GLOBE_STEP, 0.0)),
            "files-facts-scroll" => self.scroll_facts(),
            "files-preview-scroll" => self.scroll_preview(),
            "files-playlist-previous" => self.step_entry(-1),
            "files-playlist-next" => self.step_entry(1),
            "files-play" => self.play_pause(),
            "files-sound" => self.hear(),
            "files-rewind" => self.seek_by(-SEEK_STEP),
            "files-forward" => self.seek_by(SEEK_STEP),
            "files-theatre" => self.theatre(Which::Preview),
            "files-facts-wide" => self.theatre(Which::Info),
            _ => return None,
        })
    }

    /// A click on one of the panels' own targets — `None` for a target that
    /// is not theirs.
    ///
    /// THROUGH THE KEYS' OWN CODE, every one of them. The rule the manager's
    /// whole pointer keeps: the mouse never reaches anything the keyboard
    /// cannot, and never by a second path.
    pub fn click(&mut self, hit: &Hit) -> Option<Step> {
        Some(match hit {
            // THE TRANSPORT'S TWO, AND THROUGH THE KEYS' OWN CODE.
            Hit::Play => self.play_pause(),
            Hit::Sound => self.hear(),
            Hit::Theatre(which) => self.theatre(*which),
            // THE GLOBE'S PICTURE ANSWERS NOTHING TO A CLICK, because a press
            // on it is the start of a DRAG — resolved before the row, exactly
            // as a press on a shape in the preview is. A click that also acted
            // would act every time somebody let go after turning it.
            Hit::Globe => Step::Nothing,
            // AND THE DOT FRAMES ON ITSELF, through the key's own code.
            Hit::GlobeDot => stepped(self.globe.borrow_mut().frame_on_the_dot()),
            // ONE ENTRY OF A PLAYLIST, and through the same code the keys use.
            Hit::Track(n) => self.play_entry(*n),
            _ => return None,
        })
    }

    /// Scrolls the panel under the pointer, and only it. `false` when the
    /// gesture is not the panels' to take — the facts, when they are not on
    /// screen.
    ///
    /// A picture does not scroll, and saying so is the point: the gesture is
    /// swallowed rather than passed on, so a hand resting over the preview does
    /// not move a list somewhere else on the screen. `room` is the panel's own
    /// height, in lines.
    pub fn wheel(&mut self, which: Which, by: i64, room: usize) -> bool {
        match which {
            // AND THE PREVIEW SCROLLS, now that there is something in it to
            // scroll. A PICTURE STILL SWALLOWS THE GESTURE and there is no
            // branch for that either: `preview_total` is 1, `slide` computes
            // `last = 1 - room` as nought, the offset does not move, and it
            // still answers `true` — so a hand resting over a photograph does
            // not move a list somewhere else on the screen.
            Which::Preview => {
                // A SHAPE TAKES THE WHEEL AND ZOOMS WITH IT. The figure already
                // swallows the gesture — `preview_total` is 1 for a picture, so
                // `slide` computes nothing and returns `true` anyway — and this
                // is the one figure that has somewhere to put it. Up is nearer,
                // which is what every map and every photograph does.
                if self.zoom(WHEEL_ZOOM.powi(-i32::try_from(by).unwrap_or(0))) {
                    return true;
                }
                let total = self.preview_total();
                self.preview_offset = crate::files::slide(self.preview_offset, by, total, room);
                true
            }
            Which::Info => {
                if !self.info_shown() {
                    return false;
                }
                // A FIGURE SWALLOWS THE GESTURE, and this is the one figure in
                // this panel with somewhere to put it. The preview's arm makes
                // the same move for a shape; without it the wheel over the
                // globe would scroll the facts underneath, which is the defect
                // `Panes::section_at` exists to end, one section in.
                // AND IT IS THE POINTER THAT DECIDES, not the focus.
                //
                // `Panel::zoom` asks which pane has the KEYBOARD — right for
                // `C-k`, because the shape and the globe share that notation
                // and the focused pane is what tells them apart. A wheel has no
                // such ambiguity and no such question: it acts where the hand
                // is. Going through `zoom` meant the gesture only worked with
                // the info panel focused, which is not the state anybody scrolls
                // in — the listing has the focus while one reads a photograph.
                if self.pointing_at_the_globe.get()
                    && self.globe_here()
                    && self
                        .globe
                        .borrow_mut()
                        .zoom(WHEEL_ZOOM.powi(-i32::try_from(by).unwrap_or(0)))
                {
                    return true;
                }
                let total = self.info_total();
                self.info_offset = crate::files::slide(
                    self.facts_offset(total),
                    by,
                    total,
                    self.info_room().max(1),
                );
                true
            }
        }
    }

    /// Puts the film where the pointer holding its timeline is, `fraction` of
    /// the way along. `false` when there is no film with a length to hold.
    ///
    /// A seek is not a scroll: it kills a child and starts another.
    pub fn drag_time(&mut self, fraction: f64) -> bool {
        let Some(total) = self.playing_seconds() else {
            return false;
        };
        let Some(bar) = self.timeline() else {
            return false;
        };
        #[allow(clippy::cast_possible_truncation)]
        let at = fraction.clamp(0.0, 1.0) as f32;
        // THROUGH `offset_at`, which is `thumb`'s own inverse — the same
        // call the other two axes make. Tenths of a second in, tenths of a
        // second out.
        #[allow(clippy::cast_precision_loss)]
        let to = bar.offset_at(at, 1.0, 0.0) as f64 / 10.0;
        self.seek_to(to.clamp(0.0, (total - 0.1).max(0.0)));
        true
    }

    /// Puts the preview's first column where the pointer holding its sideways
    /// bar is. `cells` is how wide the preview's box is — the host's layout
    /// says where the box is, and so how wide.
    pub fn drag_across(&mut self, fraction: f64, cells: u16) -> bool {
        let widest = self.preview_widest();
        let bar = ronoctua_design::Scroll {
            total: widest,
            lines: usize::from(cells),
            offset: 0,
            top: 0,
        };
        #[allow(clippy::cast_possible_truncation)]
        let at = fraction.clamp(0.0, 1.0) as f32;
        self.preview_column = bar.offset_at(at, 1.0, 0.0);
        true
    }

    /// Puts a panel's first row where the pointer holding its bar is, in a
    /// panel `room` lines tall.
    ///
    /// THROUGH `Scroll::offset_at`, which is `thumb`'s own inverse — see
    /// `files::View::drag_to`, which hands the panels' two bars here.
    pub fn drag_down(&mut self, which: Which, fraction: f64, room: usize) -> bool {
        let total = match which {
            // THE GRID'S ROWS, which is what the BAR was drawn from: with the
            // facts in columns a row holds one fact out of each, and the blank
            // row at the end is one of them.
            Which::Info => self.info_total(),
            // A PICTURE STILL DOES NOT SCROLL, and there is no branch for it:
            // `preview_total` is 1, so `last` is nought and the drag lands
            // where it started. The rule survives as arithmetic.
            Which::Preview => self.preview_total(),
        };
        let bar = ronoctua_design::Scroll {
            total,
            lines: room,
            offset: 0,
            top: 0,
        };
        #[allow(clippy::cast_possible_truncation)]
        let at = fraction.clamp(0.0, 1.0) as f32;
        let to = bar.offset_at(at, 1.0, 0.0);
        match which {
            Which::Preview => self.preview_offset = to,
            Which::Info => self.info_offset = to,
        }
        true
    }

    // ── WHAT IS DRAWN ─────────────────────────────────────────────────────

    /// A panel was given a box: `room` cells wide and `lines` tall.
    ///
    /// REMEMBERED WHERE IT IS DECIDED, for the keys that page the panels
    /// (`C-.` and `C-,`), for the height a wrapped refusal takes and for the
    /// facts' grid — a key arrives with no box, and the layout is the one place
    /// that knows. Written for every box laid out, drawn or not: the pair's
    /// lines are one number, and the one panel not asked for this frame must
    /// not be left paging against a stale one.
    pub fn laid_out(&self, which: Which, room: u16, lines: usize) {
        // HOW MANY LINES A PANEL WAS GIVEN, remembered for the keys that page
        // them (`C-.` and `C-,`). AND IT IS RECORDED FOR BOTH PANELS, not only
        // the facts: `lines` is one number for the pair, so it is the
        // preview's height too.
        self.drawn_lines
            .set(u16::try_from(lines).unwrap_or(u16::MAX));
        // THE PANEL'S OWN WIDTH, and not the content's.
        //
        // `content_room` is a cell narrower when there is a bar, and whether
        // there is a bar is what `total` is being asked. Measuring the refusal
        // against the narrower one would be asking the answer to decide the
        // question.
        if matches!(which, Which::Preview) {
            self.drawn_panel.set(room);
        } else {
            // AND THE FACTS' OWN, which is a different number whenever only one
            // of the two panels is on. `Panel::grid_of` is what reads it, and a
            // grid measured against the other panel's width is a grid the
            // painter and the scrollbar disagree about.
            self.drawn_info.set(room);
        }
    }

    /// ONE PANEL, AS A SECTION: its box (`at`, `room` cells), its band (`top`,
    /// `lines`), its offset, its bars and its rows.
    ///
    /// The box is the host's to decide — beside a listing, under it or across
    /// the region — and everything inside it is the panel's. `sheet` is the
    /// facts, read once by the caller for both panels: `sheet()` hands back a
    /// clone. Call [`Panel::laid_out`] for the box first.
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn section(
        &self,
        which: Which,
        (at, room): (u16, u16),
        (top, lines): (usize, usize),
        sheet: &[Fact],
        texts: &ronoctua_i18n::Texts,
    ) -> Section {
        let pane = match which {
            Which::Preview => Pane::Preview,
            Which::Info => Pane::Info,
        };
        let total = if matches!(pane, Pane::Info) {
            // THE GLOBE'S ROWS AND THE GRID'S, which together are what
            // there is to scroll: a row of the grid carries one fact
            // out of every column, so the sheet is shorter down the
            // page than it is long. `Panel::info_total` is the one place
            // it is added up, and the bar, the wheel, the paging key
            // and the drag all ask it.
            self.globe_rows()
                .saturating_add(self.grid_of(sheet.len()).2)
        } else {
            self.preview_total()
        };
        // THE BAR'S CELL IS NOT THE CONTENT'S.
        //
        // A scrollbar sits in the LAST cell of its section's span, and
        // the fields were being given that cell too — so a line long
        // enough ran under the thumb and lost its last letters behind
        // it. The owner photographed it. A bar drawn over the content
        // hides the thing it exists to help you reach.
        //
        // The listing and the sidebar never had it, and that is not
        // luck: both inset their columns by `EDGE_RIGHT`, so their bar
        // lands in air. The panels had no such reserve, so they take
        // one here — and only when there IS a bar, because `cut` does
        // not change as one scrolls and a short file must not pay a
        // column for a bar it will never grow.
        let content_room = if total > lines {
            room.saturating_sub(BAR)
        } else {
            room
        };
        // AND THE PREVIEW READS SIDEWAYS TOO.
        //
        // The owner chose scrolling over a silent cut, and a view this
        // system offers only to the keyboard is half a view: he
        // reported the missing bar as soon as the keys worked. The
        // track is the panel's own cells, so this is the same `Scroll`
        // with the axes swapped — `total` is the widest line and
        // `lines` is what fits.
        let across = if matches!(pane, Pane::Preview) {
            let widest = self.preview_widest();
            (widest > usize::from(content_room)).then(|| ronoctua_design::Scroll {
                total: widest,
                lines: usize::from(content_room),
                offset: self.preview_column,
                top: 0,
            })
        } else {
            None
        };
        // AND THE FILM'S TIMELINE IS THE THIRD AXIS, on the same
        // `Scroll`: tenths of a second, from nought to the length of
        // the film. `None` for a GIF, which has no duration and whose
        // frames could not be sought into anyway.
        let time = matches!(pane, Pane::Preview)
            .then(|| self.timeline())
            .flatten();
        // AND ITS LINE IS NOT THE CONTENT'S EITHER, which is the
        // same rule the cell follows, turned ninety degrees: a
        // sideways bar sits on the band's LAST line, so the rows stop
        // one short of it.
        //
        // THE TRANSPORT TAKES ONE THE SAME WAY, and the picture gives
        // it: nothing in this manager draws over its own content, which
        // is the rule the owner set when he photographed a scrollbar
        // sitting on a line of code.
        let transport = matches!(pane, Pane::Preview) && self.plays_here();
        // HOW WIDE THE CLOCK IS, measured once and carried, because the
        // track's length is what depends on it — and the cursor's
        // rectangle and both doors' drags all have to agree about that.
        let clock = time.as_ref().map_or(0, |t| width_of(&clock_text(t)));
        let content_lines = lines
            .saturating_sub(usize::from(across.is_some()))
            .saturating_sub(usize::from(transport));
        // THE GLOBE TAKES LINES OFF THE TOP, the way `across` and
        // `transport` take them off the bottom — the same arithmetic
        // turned the other way up. The rows it takes are REAL rows with
        // real fields, so `hit`, `band_row`, a click and a key all keep
        // going through the machine that already exists rather than
        // through a fourth opinion about where the panel is.
        // THE GLOBE'S ROWS ARE THE FACTS' OWN, built by the same
        // builder as every other line of the panel. They used to be a
        // band chained in FRONT of the content and marked `above`, and
        // that is the pinning `Panel::globe_rows` records the two
        // defects of.
        let mut content: Vec<Row> = Vec::with_capacity(content_lines);
        content.extend((0..content_lines).map(|k| Row {
            fields: match which {
                Which::Preview => self.preview_fields(k, content_lines, at, content_room, texts),
                Which::Info => self.info_fields(k, content_lines, at, content_room, sheet),
            },
            marked: false,
            foot: false,
            focused: false,
        }));
        if transport {
            // A ROW AND NOT A RECTANGLE, unlike the two bars: two of
            // its cells are clickable, so `Section::line_of` and
            // `files::View::hit` resolve them with the arithmetic they already
            // have rather than with a fourth opinion about where the
            // panel is.
            content.push(Row {
                fields: self.transport_fields(at, content_room, time.as_ref()),
                marked: false,
                // AND ANCHORED TO THE FOOT — the one row in this
                // manager that is. See `Row::foot`.
                foot: true,
                focused: false,
            });
        }
        // AND THE EXPAND BUTTON, ON THE PANEL'S LAST LINE — the one
        // the eye calls the bottom-right corner.
        //
        // IT USED TO BE PUSHED BY THE ROW BUILDER, at `content_lines -
        // 1`, and `content_lines` is the panel MINUS the rows added
        // after it: the film's transport, and the bar that scrolls a
        // long value sideways. So on any panel that had one the button
        // sat a line or two ABOVE the floor while the column's rule ran
        // past it — the owner photographed both corners floating and
        // said so twice. The builders' own comment already promised
        // this line, *"over whatever that line holds"*; what it could
        // not see from inside a per-row loop was which line that was.
        //
        // Pushed LAST for the reason it always was: `lit_field` is an
        // `rposition`, so the cell goes to the button rather than to
        // whatever is underneath it, and what the eye sees lit is by
        // construction what the click takes.
        //
        // NOT WITH A PLAYER: there it rides the transport's row beside
        // the clock, which is where the owner put it.
        let corner = match which {
            Which::Preview => {
                !self.plays_here()
                    && (!self.theatre_covers(content_lines) || self.pointing_at_preview())
            }
            Which::Info => self.pointing_at_info(),
        };
        if corner
            && content_room > 0
            && self.has_a_subject()
            && let Some(last) = content.last_mut()
        {
            last.fields
                .push(self.theatre_field(at.saturating_add(content_room).saturating_sub(1), which));
        }
        Section {
            pane,
            across,
            time,
            // WHICH ROW OF `content` THE TRANSPORT IS, counted after
            // the title has been pushed — the title is a row like any
            // other as far as this index is concerned, and computing
            // it from `content_lines` instead put the film's controls
            // one line above themselves.
            transport: transport.then(|| content.len().saturating_sub(1)),
            clock,
            // THE SECTION KEEPS ITS WHOLE SPAN. Shrinking `to` was the
            // first attempt and it moved the bar instead of the text:
            // the bar is drawn at `to - 1`, so the gap simply reopened
            // one cell to the left. What shrinks is the room the
            // FIELDS are given, below.
            from: at,
            to: at.saturating_add(room),
            top,
            lines,
            offset: if matches!(pane, Pane::Info) {
                // KEPT INSIDE WHAT THERE IS TO SCROLL. See
                // `Panel::facts_offset`: the GRID changes under a stored
                // offset, and nothing writes when it does.
                self.facts_offset(total)
            } else {
                self.preview_offset
            },
            total,
            // AND A PANEL HAS NO CHROME LINES AT ALL.
            //
            // The globe's were the one exception and they are gone: a
            // pinned band makes `Section::visible` shorter than the
            // panel while `Room::panel` — what the DRAG divides by — is
            // the panel, and the owner felt the two disagree under his
            // hand. See `Panel::globe_rows`.
            above: 0,
            grid: None,
            content,
        }
    }

    /// THE PANELS THAT ARE ON SCREEN, side by side across `cols` cells and
    /// `lines` lines — the preview first — for a host with nothing beside
    /// them. `None` for a panel that is not: turned off (`C-v`, `C-i`), put
    /// aside by the other being given the region (`C-w`), or covered by
    /// something modal ([`Panel::around`]).
    ///
    /// The boxes are the ones the manager gives its strip (`files::Panes`):
    /// one arithmetic for where two panels sit next to each other, so the
    /// remainder cell goes where it goes there, and one panel alone takes the
    /// width. Which panels are on is [`Panel::preview_shown`] and
    /// [`Panel::info_shown`], the answers every key, click and paint of the
    /// panels already asks — so what this lays out is what they act on.
    ///
    /// NO DOOR ASKS FOR THIS YET: the manager lays its panels out with its
    /// own `Panes` ([`Panel::section`], one box at a time), and the search's
    /// detail stacks the two (`fast_access_detail::file_sections`). It stays
    /// for the first door that puts both side by side with nothing beside
    /// them, with its tests.
    #[must_use]
    #[allow(dead_code)]
    pub fn sections(
        &self,
        cols: u16,
        lines: usize,
        texts: &ronoctua_i18n::Texts,
    ) -> (Option<Section>, Option<Section>) {
        let boxes = Panes {
            sidebar: 0,
            right: 0,
            strip: u16::try_from(lines).unwrap_or(u16::MAX).max(1),
            preview: self.preview_shown(),
            info: self.info_shown(),
            whole: false,
        }
        .boxes(cols);
        let sheet = self.sheet();
        let one = |which: Which| {
            // A PANEL WITH NO BOX IS NOT LAID OUT AT ALL: recording a width
            // of nought for it would page its keys against a panel that is
            // not there, and the one on screen keeps the width it was given.
            let &(_, at, room) = boxes.iter().find(|(w, _, _)| *w == which)?;
            self.laid_out(which, room, lines);
            Some(self.section(which, (at, room), (0, lines), &sheet, texts))
        };
        (one(Which::Preview), one(Which::Info))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-inspection-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create the test directory");
        dir
    }

    fn a_panel() -> Panel {
        Panel::new(
            Rc::new(ronoctua_i18n::Texts::from_env()),
            ronoctua_files::when::Zone::utc(),
        )
    }

    fn entry(dir: &Path, name: &str) -> Entry {
        Entry::of_path(name.to_owned(), &dir.join(name)).expect("lstat the fixture")
    }

    #[test]
    fn the_place_row_says_it_takes_the_place_out_and_not_that_it_changes_it() {
        use crate::inspection_sheet::{Doable, Fact};
        let panel = a_panel();
        let said = |does| {
            panel
                .fact_fields(
                    &Fact {
                        label: "place".to_owned(),
                        value: "-22.97110, -43.18220".to_owned(),
                        does,
                        says: None,
                    },
                    0,
                    60,
                )
                .iter()
                .filter_map(|f| f.explain.as_ref().map(|e| e.key))
                .collect::<Vec<_>>()
        };
        let place = said(Doable::RemoveLocation);
        assert!(!place.is_empty(), "the place row explains nothing");
        assert!(
            place
                .iter()
                .all(|k| *k == ronoctua_i18n::key::FILES_FACT_TAKES_PLACE_OUT),
            "{place:?}"
        );
        assert!(
            said(Doable::Rename)
                .iter()
                .all(|k| *k == ronoctua_i18n::key::FILES_FACT_EDITS)
        );
    }

    #[test]
    fn a_subject_names_the_way_out_by_its_parent_and_owners_by_number() {
        let dir = scratch("way-out");
        std::fs::write(dir.join("a.txt"), b"x").unwrap();
        let roster = Roster::default();
        let file = Subject::of_entry(&dir, &entry(&dir, "a.txt"), &roster, None, 3);
        assert_eq!(file.path, dir.join("a.txt"));
        assert_eq!(file.row, 3);
        // AN EMPTY ROSTER SAYS NUMBERS, never an empty name.
        assert_eq!(file.owner, file.entry.uid.to_string());
        assert_eq!(file.group, file.entry.gid.to_string());
        let mut out = entry(&dir, "a.txt");
        out.name = Entry::PARENT.to_owned();
        let up = Subject::of_entry(&dir, &out, &roster, None, 0);
        assert_eq!(
            up.path,
            dir.parent().expect("the fixture has a parent"),
            "the way out was spelt with `..`"
        );
        assert_eq!(up.joined(), dir.join(".."), "the list's spelling was lost");
    }

    #[test]
    fn a_subject_of_a_path_is_what_lstat_says_and_finds_its_film() {
        let dir = scratch("of-path");
        std::fs::write(dir.join("IMG_1.HEIC"), b"x").unwrap();
        std::fs::write(dir.join("IMG_1.MOV"), b"y").unwrap();
        std::fs::write(dir.join("IMG_2.HEIC"), b"z").unwrap();
        std::fs::write(dir.join("IMG_3.jpg"), b"w").unwrap();
        std::fs::write(dir.join("IMG_3.mp4"), b"v").unwrap();
        let roster = Roster::default();
        let one = Subject::of_path(&dir.join("IMG_1.HEIC"), &roster).expect("it is there");
        assert_eq!(one.path, dir.join("IMG_1.HEIC"));
        assert_eq!(one.folder, dir);
        assert!(matches!(one.entry.kind, Kind::File));
        assert_eq!(
            one.beside,
            Some(dir.join("IMG_1.MOV")),
            "the film beside it"
        );
        let two = Subject::of_path(&dir.join("IMG_2.HEIC"), &roster).expect("it is there");
        assert_eq!(two.beside, None, "a picture with no film was given one");
        let three = Subject::of_path(&dir.join("IMG_3.jpg"), &roster).expect("it is there");
        assert_eq!(
            three.beside,
            Some(dir.join("IMG_3.mp4")),
            "a film spelt in lower case"
        );
        // A FILM IS NOT ITS OWN OTHER HALF, in whatever case it is asked.
        let film = Subject::of_path(&dir.join("IMG_1.MOV"), &roster).expect("it is there");
        assert_eq!(film.beside, None, "the film was paired with itself");
        assert!(Subject::of_path(&dir.join("nothing"), &roster).is_none());
        assert!(Subject::of_path(Path::new("/"), &roster).is_none());
    }

    #[test]
    fn a_host_that_tells_the_same_thing_twice_builds_one_subject() {
        let dir = scratch("follow");
        std::fs::write(dir.join("a.txt"), b"x").unwrap();
        std::fs::write(dir.join("b.txt"), b"y").unwrap();
        let (a, b) = (entry(&dir, "a.txt"), entry(&dir, "b.txt"));
        let roster = Roster::default();
        let panel = a_panel();
        let seen = |entry, row| Seen {
            folder: &dir,
            entry,
            roster: &roster,
            measured: None,
            row,
        };
        panel.follow(Some(seen(&a, 0)), Around::default());
        let first = panel.subject.borrow().clone();
        panel.follow(Some(seen(&a, 0)), Around::default());
        assert_eq!(
            *panel.subject.borrow(),
            first,
            "the same view changed the subject"
        );
        // ANOTHER ROW OF THE SAME FILE IS ANOTHER SUBJECT: the sheet's targets
        // name the row.
        panel.follow(Some(seen(&a, 4)), Around::default());
        assert_eq!(panel.subject.borrow().as_ref().map(|s| s.row), Some(4));
        panel.follow(Some(seen(&b, 4)), Around::default());
        assert_eq!(
            panel.subject.borrow().as_ref().map(|s| s.path.clone()),
            Some(dir.join("b.txt"))
        );
        panel.follow(None, Around::default());
        assert!(
            !panel.has_a_subject(),
            "nothing under the cursor kept a subject"
        );
    }

    #[test]
    fn a_panel_pointed_at_a_path_asks_about_it_once() {
        let dir = scratch("focus");
        std::fs::write(dir.join("notes.txt"), b"one\ntwo\n").unwrap();
        let mut panel = a_panel();
        assert!(panel.wants_look(512).is_none(), "nothing to look at, asked");
        panel.focus(Subject::of_path(&dir.join("notes.txt"), &Roster::default()));
        let ask = panel
            .wants_look(512)
            .expect("a file, and nothing asked yet");
        assert_eq!(ask.path, dir.join("notes.txt"));
        assert_eq!(ask.target, 512);
        assert!(
            panel.wants_look(512).is_none(),
            "the same file was asked twice"
        );
        // AND THE ANSWER IS TAKEN: the preview shows the file's own letters.
        panel.looked(look(ask));
        assert!(
            panel.showing_text().is_some(),
            "the text did not reach the panel"
        );
        assert!(
            panel
                .sheet()
                .iter()
                .any(|f| f.value == dir.join("notes.txt").to_string_lossy()),
            "the sheet does not say where the file is"
        );
    }

    #[test]
    fn with_no_inspector_the_wish_is_never_asked() {
        // ASKING WRITES DOWN WHAT WAS ASKED, and with nobody to answer, the
        // panel would wait for ever — drawn as "not yet" — instead of saying
        // there is no preview.
        let threads = Threads::none();
        let mut asked = false;
        threads.look(|| {
            asked = true;
            None
        });
        assert!(!asked, "a wish was asked with no thread to send it to");
    }

    #[test]
    fn the_two_sections_share_the_width_and_start_at_the_top() {
        let dir = scratch("sections");
        std::fs::write(dir.join("notes.txt"), b"one\ntwo\n").unwrap();
        let panel = a_panel();
        panel.focus(Subject::of_path(&dir.join("notes.txt"), &Roster::default()));
        let texts = ronoctua_i18n::Texts::from_env();
        let (preview, info) = panel.sections(80, 12, &texts);
        let (preview, info) = (
            preview.expect("the preview is on"),
            info.expect("the facts are on"),
        );
        assert_eq!((preview.pane, info.pane), (Pane::Preview, Pane::Info));
        assert_eq!((preview.top, info.top), (0, 0));
        assert_eq!((preview.lines, info.lines), (12, 12));
        assert!(preview.to <= info.from, "the two boxes overlap");
        assert!(info.to <= 80, "a box runs past the width");
        assert!(
            info.content.iter().any(|r| !r.fields.is_empty()),
            "the facts drew nothing"
        );
    }

    #[test]
    fn the_sections_are_the_panels_on_screen_and_a_modal_covers_both() {
        let dir = scratch("sections-on");
        std::fs::write(dir.join("notes.txt"), b"one\ntwo\n").unwrap();
        let mut panel = a_panel();
        panel.focus(Subject::of_path(&dir.join("notes.txt"), &Roster::default()));
        let texts = ronoctua_i18n::Texts::from_env();
        let shared = panel.sections(80, 12, &texts).0.expect("the preview is on");
        // THE FACTS TURNED OFF (`C-i`): no box for them, and the preview
        // alone across the width.
        panel.info_on = false;
        let (preview, info) = panel.sections(80, 12, &texts);
        assert!(info.is_none(), "a panel turned off was laid out");
        let alone = preview.expect("the preview is still on");
        assert!(
            alone.to - alone.from > shared.to - shared.from,
            "the preview alone kept half the width"
        );
        // A MODAL OVER THE PANELS, told by a host with no list: neither is on
        // screen, and neither is laid out.
        panel.info_on = true;
        panel.around(Around {
            covered: true,
            keyboard: Pane::Info,
        });
        assert!(
            !panel.preview_shown() && !panel.info_shown(),
            "a covered panel said it was on screen"
        );
        let (preview, info) = panel.sections(80, 12, &texts);
        assert!(
            preview.is_none() && info.is_none(),
            "covered panels were laid out"
        );
        // AND THE KEYBOARD IS WHERE THE HOST SAID, uncovered or not.
        assert_eq!(panel.around.get().keyboard, Pane::Info);
        panel.around(Around::default());
        let (preview, info) = panel.sections(80, 12, &texts);
        assert!(
            preview.is_some() && info.is_some(),
            "the panels did not come back when the modal left"
        );
    }

    #[test]
    fn a_film_is_asked_for_by_name_and_its_folder_is_never_read() {
        use std::os::unix::fs::PermissionsExt as _;
        // A FOLDER THAT CANNOT BE LISTED, only walked through: `read_dir`
        // fails in it and the `lstat` of a name does not. Finding the film in
        // one is what proves the folder was not read — the host calls this on
        // its loop, and a folder is as many entries as it holds.
        let dir = scratch("unlisted");
        std::fs::write(dir.join("IMG_1.HEIC"), b"x").unwrap();
        std::fs::write(dir.join("IMG_1.MOV"), b"y").unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o300)).unwrap();
        let listed = std::fs::read_dir(&dir).is_ok();
        let found =
            Subject::of_path(&dir.join("IMG_1.HEIC"), &Roster::default()).and_then(|s| s.beside);
        // PUT BACK BEFORE ANY ASSERTION, so a failure leaves a folder the
        // next run can empty.
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        if listed {
            // AS ROOT every folder can be listed, and this would prove
            // nothing; it says so instead of passing on it.
            eprintln!("the unlistable folder was listed (root?): nothing proven here");
            return;
        }
        assert_eq!(
            found,
            Some(dir.join("IMG_1.MOV")),
            "the film was not found without reading its folder"
        );
    }

    #[test]
    fn the_panels_answer_only_for_their_own() {
        let mut panel = a_panel();
        assert!(
            panel.key("files-down").is_none(),
            "the listing's key was taken"
        );
        assert!(
            panel.key("files-play").is_some(),
            "the film's key was refused"
        );
        assert!(
            panel.click(&Hit::Name(0)).is_none(),
            "a row's click was taken"
        );
        assert!(
            panel.click(&Hit::Play).is_some(),
            "the play glyph was refused"
        );
    }

    #[test]
    fn the_sheet_adds_what_the_tools_said_only_about_the_same_file() {
        let dir = scratch("sheet");
        std::fs::write(dir.join("a.txt"), b"x").unwrap();
        let subject = Subject::of_entry(&dir, &entry(&dir, "a.txt"), &Roster::default(), None, 0);
        let texts = ronoctua_i18n::Texts::from_env();
        let zone = ronoctua_files::when::Zone::utc();
        let told = Looked {
            key: "k".to_owned(),
            path: dir.join("a.txt"),
            rgba: None,
            moving: None,
            list: None,
            text: None,
            why: None,
            facts: vec![ronoctua_inspect::facts::Fact {
                label: ronoctua_inspect::facts::Label::Named("Tag".to_owned()),
                value: ronoctua_inspect::facts::Value::Data("value".to_owned()),
            }],
            mime: "text/plain",
            turns: false,
            outlined: 0,
            at: None,
            amendable: crate::inspection_edit::Amendable::default(),
        };
        let plain = crate::inspection_sheet::fs_facts(&subject, &zone, &texts);
        let whole = crate::inspection_sheet::sheet_of(&subject, Some(&told), &zone, &texts);
        assert_eq!(
            whole.len(),
            plain.len() + 1,
            "what the tools said is not there"
        );
        assert_eq!(whole.last().map(|f| f.label.as_str()), Some("Tag"));
        let elsewhere = Looked {
            path: dir.join("b.txt"),
            ..told
        };
        assert_eq!(
            crate::inspection_sheet::sheet_of(&subject, Some(&elsewhere), &zone, &texts),
            plain,
            "another file's facts were put under this one's name"
        );
    }
}
