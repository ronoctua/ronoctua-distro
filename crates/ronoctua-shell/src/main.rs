//! `ronoctua-shell` — the binary: the one program every door of the shell is.
//!
//! # What it is
//!
//! The composition, and nothing else: the one crate that names every
//! feature. It installs what every door of the process reads — the theme,
//! the date's shape, the size of the text, the keymap every crate's keys are
//! assembled into (`keymap::CONTRIBUTIONS`), the player's pages — and runs
//! the door the first word names: its own subcommands, then every crate's
//! windows by their tables ([`DOORS`]), and, with no word, the footer, handed
//! the regions it hosts by their ids ([`COMPOSITION`]). Each window, each
//! view, the footer itself, is a crate of its own; a line of one recompiles
//! that crate and this one.
//!
//! # What it does NOT do
//!
//! - **It does not tile windows.** What does that is the compositor
//!   (invariant 4). One `xdg_toplevel` per native buffer, and the compositor
//!   decides where they go.
//! - **It does not talk to the compositor through anything other than IPC or
//!   layer-shell** (invariant 3). There is no fork and no patch of it, and no
//!   crate of ours but `ronoctua-scroll` knows which compositor it is.

#![deny(unsafe_code)]
#![warn(missing_docs)]

use anyhow::{Context as _, Result};
use ronoctua_shell_door::{column, launch, right_column};
use ronoctua_shell_kit::doors::{Door, Words};
use ronoctua_shell_kit::{actions, bindings, fold, ipc, language, theme, toolkits, when};
use ronoctua_tokens as tokens;

mod key;
mod keymap;
mod photograph;
// THE REGIONS THE FOOTER HOSTS (`region::Slots`): one line per region.
mod regions;
// THE CENTRE'S VAULT FLOW as another door holds it — the file manager's
// window, for `m` and `l` on its vault's row (`vault::Flow`).
mod vault_flow;

/// THE PLAYER'S PAGES WRITTEN ABOVE IT, in the order its view asks them —
/// the ranking's look before the catalogue's, as the view asked them before
/// the seam (`player_seam`). The binary is the one place that names both.
const PLAYER_PAGES: &[&dyn ronoctua_shell_player::player_seam::Page] = &[
    &ronoctua_shell_video_metrics::player_ranking::PAGE,
    &ronoctua_shell_catalogue::player_catalog::PAGE,
];

/// WHAT THE FOOTER HOSTS, AND WHAT ITS CAMERA PLANTS: the regions the binary
/// registers (`regions.rs`) with the residents beside them, and the
/// features' own fixtures the footer's camera plants by their words — the
/// LLM accounts' marks over the room's files, the software centre's reading
/// of the room. The footer names none of them.
static COMPOSITION: ronoctua_shell_footer::Composition = ronoctua_shell_footer::Composition {
    regions: regions::REGIONS,
    residents: regions::RESIDENTS,
    plantings: ronoctua_shell_footer::footer_photograph::Plantings {
        accounts_marks: ronoctua_shell_agents::agents::with_accounts_marks,
        software: ronoctua_shell_software::software_window::camera::planted_in_the_footer,
    },
};

/// EVERY CRATE'S WINDOWS, by the word that names each on the command line
/// (`ronoctua-shell <door> …`): one table per crate, sorted, each written
/// beside the windows it opens (`kit::doors::Door`). A door that needs what
/// two crates make — the file manager's window and the vault flow of the
/// security centre, a buffer and the queue's words, the column's two tenants
/// — is a subcommand of this binary's own (`main`).
const DOORS: &[&[Door]] = &[
    ronoctua_shell_agents::DOORS,
    ronoctua_shell_audio::DOORS,
    ronoctua_shell_belongings::DOORS,
    ronoctua_shell_bluetooth::DOORS,
    ronoctua_shell_calc::DOORS,
    ronoctua_shell_camera::DOORS,
    ronoctua_shell_catalogue::DOORS,
    ronoctua_shell_clipboard::DOORS,
    ronoctua_shell_containers::DOORS,
    ronoctua_shell_display::DOORS,
    ronoctua_shell_fast_access::DOORS,
    ronoctua_shell_footer::DOORS,
    ronoctua_shell_git::DOORS,
    ronoctua_shell_history::DOORS,
    ronoctua_shell_icons::DOORS,
    ronoctua_shell_incus::DOORS,
    ronoctua_shell_lifeline::DOORS,
    ronoctua_shell_locale::DOORS,
    ronoctua_shell_manual::DOORS,
    ronoctua_shell_network::DOORS,
    ronoctua_shell_notifications::DOORS,
    ronoctua_shell_overlays::DOORS,
    ronoctua_shell_player::DOORS,
    ronoctua_shell_pointers::DOORS,
    ronoctua_shell_power::DOORS,
    ronoctua_shell_security::DOORS,
    ronoctua_shell_software::DOORS,
    ronoctua_shell_stick::DOORS,
    ronoctua_shell_syncthing::DOORS,
    ronoctua_shell_terminal::DOORS,
    ronoctua_shell_update::DOORS,
    ronoctua_shell_video_metrics::DOORS,
];

/// EVERY KEY THIS BINARY MAY SAY: every crate's it links that speaks
/// (`key::sets`, each owning its own) and every DISTRO brain module's, whose
/// words the footer says through the module's manifest
/// (`ronoctua_brain::compiled::key_sets`). What the start guard,
/// `check-texts` and the census ask of the texts (`ronoctua_i18n::Keys::of`).
/// A person's module is not here: this binary is the same with or without
/// one.
fn every_key(into: &mut Vec<ronoctua_i18n::Set>) {
    key::sets(into);
    ronoctua_brain::compiled::key_sets(into);
}

fn main() -> Result<()> {
    // BEFORE the subscriber: building `tracing` already costs time, and it counts
    // towards 5.2's 200 ms target as much as everything else.
    // The value does not matter here; it is the call that ANCHORS the clock.
    let _ = ronoctua_render::start();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ronoctua_shell=info,ronoctua_render=info".into()),
        )
        .with_writer(std::io::stderr)
        .init();

    // THE DATE'S SHAPE, before any door: it is a process-wide value the way
    // the theme (below) is, so that a date resolves through it wherever one
    // is written. See `when::prefs::current`.
    when::prefs::adopt();
    // AND THE SIZE OF THE TEXT, on the same footing and for the same reason.
    // It is not the theme's business — a palette and a size are two choices —
    // but they are both read at start because they are both properties of
    // the process rather than of any one door.
    toolkits::resolve_at_start();
    // AND THE KEYMAP, which every door teaches its keys from: installed now
    // and built on the first question, in the process that asks
    // (`bindings::taught`). The binary is the one place that names every
    // crate's keys (`keymap::CONTRIBUTIONS`).
    bindings::install(keymap::build);
    // AND THE PLAYER'S PAGES WRITTEN ABOVE IT — the ranking of the video
    // catalogue, channels by theme — which every player's view of this
    // process holds (`player_seam::install`): the binary is the one place
    // that names them.
    ronoctua_shell_player::player_seam::install(PLAYER_PAGES);

    let subcommand = std::env::args().nth(1);
    // AND THE THEME, before any door: every process of this binary — the
    // footer, the windows, a buffer's host — draws through the same three
    // sources, read once here. Every other short-lived subcommand pays one
    // file read.
    //
    // NOT FOR THE FOUR RELAYS, which draw nothing: they write a line to a
    // socket and exit. The compositor starts them on a keypress by this
    // binary's own path (`export-binds`), unwrapped and with ITS environment,
    // and the theme is read off the palettes (`RONOCTUA_THEMES`): a session
    // without them, or with an older build's, would panic on every chord.
    if !matches!(
        subcommand.as_deref(),
        Some("leader" | "command" | "buffer-keymap" | "buffer-command")
    ) {
        theme::resolve_at_start();
    }

    // THE TEXTS, before anything speaks. The sentences are data read at run
    // time (`RONOCTUA_TEXTS`, which the wrapper sets) and this binary was
    // compiled with the key names only, so whether every key is in them is
    // asked HERE, once, of the language this process is about to speak: a
    // missing directory or another build's texts is one English line and exit
    // 78 (`ronoctua_i18n::refuse`), never a screen of `[key]` or a panic in the
    // middle of a door. The four relays say no word — they write a line to the
    // footer's socket and exit, and `leader` is on a keypress's path — and
    // `check-texts` says what is wrong by itself. EVERY KEY is every crate's
    // this binary links that speaks and every distro module's (`every_key`).
    //
    // AND THE BRAIN'S CATALOGUE, at the same moment and for the same reason
    // in marks: what the footer draws of the brain's modules — their glyphs,
    // the actions the keymap registers, their notifications — is data read at
    // run time too (`RONOCTUA_BRAIN_CATALOGUE`), and a missing file, another
    // build's (its keys not in these texts) or notifications the book cannot
    // read are one English line and exit 78 (`catalogue::refuse`), never a
    // keymap short of every module's action. `check-catalogue` says what is
    // wrong by itself; and the lifeline, which comes up because the footer
    // died, draws no module's mark and registers no keymap, so it is not
    // refused for data it never reads.
    if !matches!(
        subcommand.as_deref(),
        Some(
            "leader"
                | "command"
                | "buffer-keymap"
                | "buffer-command"
                | "check-texts"
                | "check-catalogue"
        )
    ) {
        match ronoctua_i18n::Texts::checked(
            &language::resolve().0,
            &ronoctua_i18n::Keys::of(every_key),
        ) {
            Err(missing) => ronoctua_i18n::refuse(&missing),
            // THE LIFELINE'S DOORS, by its own table: never named here, so
            // its word stays its crate's (`every-door-is-dispatched`).
            Ok(_)
                if ronoctua_shell_lifeline::DOORS
                    .iter()
                    .any(|door| subcommand.as_deref() == Some(door.name)) => {}
            Ok(texts) => {
                // ITS KEYS ASKED OF THE TEXTS ABOUT TO BE SPOKEN — and only
                // the file read for a language that is none (a refusal the
                // texts' own guard owns, never one of the catalogue's).
                let read = match texts.as_ref() {
                    Some(texts) => ronoctua_brain::catalogue::checked(|key| texts.has(key)),
                    None => ronoctua_brain::catalogue::load(),
                };
                if let Err(missing) = read {
                    ronoctua_brain::catalogue::refuse(&missing);
                }
                if let Err(why) = ronoctua_notify::brain::checked() {
                    eprintln!("{}: {why}", program_name());
                    std::process::exit(i32::from(ronoctua_brain::catalogue::EXIT_MISSING));
                }
            }
        }
    }

    // THE WORDS AFTER THE SUBCOMMAND, none of them read yet: each door makes
    // text of the ones it takes and no others (`Words`), as each arm read its
    // own before the doors had tables — a word that is not UTF-8 stops only a
    // door that reads it, never `opener`, which reads them as the system's.
    let words = Words::of_this_process();
    match subcommand.as_deref() {
        // It tells an ALREADY-RUNNING footer. It is the compositor that sees the
        // leader's key — see the top of the footer's `leader.rs`.
        Some("leader") => return ipc::send_it("leader").context("tell the leader"),
        // RUNNING A COMMAND BY NAME, from outside the process.
        //
        // It is what section 13 asks for — every feature addressable by name
        // — and it is what the emergency binds use: the `Ctrl+Shift+L` that
        // restores the keyboard cannot depend on the leader, precisely because
        // it exists for when the leader does not respond.
        //
        // It does NOT execute here: it sends to the footer that is already running. A
        // process lasting an instant has no surface, does not have the registry
        // loaded and would have nowhere to echo the result.
        Some("command") => {
            let mut args = std::env::args().skip(2);
            let id = args
                .next()
                .context("usage: ronoctua-shell command <id> [argument]")?;
            let rest: Vec<String> = args.collect();
            let line = if rest.is_empty() {
                format!("command {id}")
            } else {
                format!("command {id} {}", rest.join(" "))
            };
            let sent = ipc::send_it(&line);
            // A KEY THE FOOTER ASKED THE COMPOSITOR FOR, and no footer to take
            // it: the keys go back to the windows (`fold::nobody_took`).
            if sent.is_err() {
                fold::nobody_took(&id);
            }
            return sent.with_context(|| format!("send `{id}`"));
        }
        // WHAT A BUFFER DECLARES, asked of the buffer's own process.
        //
        // The footer cannot resolve a buffer's keymap: a buffer is a window of
        // its own (invariant 4) and holds the keyboard while focused, so the
        // bindings live over there. These two subcommands are that boundary made
        // reachable by hand — the same shape as `command <id>` above, and the
        // way to tell "the buffer is not publishing" apart from "the footer is
        // not asking" without a screenshot.
        Some("buffer-keymap") => {
            let pid = std::env::args()
                .nth(2)
                .context("usage: ronoctua-shell buffer-keymap <pid>")?
                .parse::<u32>()
                .context("the pid has to be a number")?;
            for (keys, id) in ipc::buffer_keymap(pid).context("read the buffer's keymap")? {
                println!("{keys} {id}");
            }
            return Ok(());
        }
        Some("buffer-command") => {
            let mut args = std::env::args().skip(2);
            let pid = args
                .next()
                .context("usage: ronoctua-shell buffer-command <pid> <id>")?
                .parse::<u32>()
                .context("the pid has to be a number")?;
            let id = args
                .next()
                .context("usage: ronoctua-shell buffer-command <pid> <id>")?;
            return ipc::buffer_command(pid, &id)
                .with_context(|| format!("send `{id}` to buffer {pid}"));
        }
        // It opens a native buffer in a window of its OWN. What decides where it
        // goes is the compositor (invariant 4) — the shell does not tile.
        Some("buffer") => {
            let path = std::env::args()
                .nth(2)
                .context("usage: ronoctua-shell buffer <component.wasm> [file]")?;
            // THE FILE IS OPTIONAL, and belongs to the HOST. The component never
            // learns a path: it receives the content through `Input::Paste` and
            // asks to write with `Effect::Save`, which is what lets an editor
            // exist without giving a buffer a filesystem (invariant 11).
            let file = std::env::args().nth(3);
            // AND THE QUEUE IN WORDS, which are the work's: the host of
            // buffers stands on the door alone (`buffer::QueueWords`).
            return ronoctua_shell_buffers::buffer::host_it(
                std::path::Path::new(&path),
                file.as_deref().map(std::path::Path::new),
                ronoctua_shell_work::jobs::queue_of,
            )
            .context("host the buffer");
        }
        // It emits the CHORD binds, for the compositor's config.
        //
        // It exists so the chord does not become a second table: what it does
        // is in `actions.rs`, and the command text comes from there. The
        // prefix is an argument because the leader is `Mod` in production and
        // `Alt` in the nested loop.
        Some("export-binds") => {
            let prefix = std::env::args()
                .nth(2)
                .context("usage: ronoctua-shell export-binds <prefix>")?;
            // THIS BINARY, BY NAME, first: a deed that runs one of our
            // commands (`wm-fold-keys-hold`) is text fixed at build time, and
            // it names the binary through this variable.
            println!("{}", actions::shell_variable());
            for chord in actions::full_chords() {
                for (key, said) in chord.binds_for(&prefix) {
                    // `Shift+1` becomes `Mod+Shift+1`: the prefix goes in
                    // front, and the Shift already came with the key.
                    println!(
                        "bindsym{}{} {prefix}+{key} {said}",
                        repeat_of(said),
                        region_of(key)
                    );
                }
            }
            // THE EMERGENCY BINDS come out WITHOUT the leader's prefix — and that is what
            // makes them useful when it is the leader that is broken. They come from the
            // same table for the same reason as the chords: written by hand in the
            // two configs, one day they would diverge, and the day one finds out is
            // the day nothing works any more.
            for (key, said, _) in actions::emergency() {
                println!("bindsym{}{} {key} {said}", repeat_of(&said), region_of(key));
            }
            // AND THE MEDIA KEYS, bare for a different reason — see
            // `actions::media`. They come out of the same table for the same
            // reason as everything above: a key written in two configs is a
            // key that one day differs between them.
            for (key, said, _) in actions::media() {
                println!("bindsym{}{} {key} {said}", repeat_of(&said), region_of(key));
            }
            // THE COMPOSITOR'S MODES, from the same table the footer draws
            // their keys from — see `actions::modes`. Inside one the keys are
            // bare, and `Esc` always leaves.
            for mode in actions::modes() {
                println!("mode \"{}\" {{", mode.name);
                for k in &mode.keys {
                    println!("    bindsym {} {}", k.key, k.said);
                }
                println!("    bindsym Escape mode \"default\"");
                println!("}}");
            }
            for line in actions::gestures(&prefix) {
                println!("{line}");
            }
            return Ok(());
        }
        // The window rules, for the same reason as the binds above: the
        // monitor's `app_id` is set in `terminal.rs` and read by the
        // compositor's config, and a string written at both ends drifts. The
        // two configs (`nix/modules/scroll.nix` and `nix/dev.nix`) take this in.
        Some("export-window-rules") => {
            print!("{}", ronoctua_shell_terminal::terminal::window_rules());
            return Ok(());
        }
        // A NATIVE DOOR IN A PNG, with no compositor and no VM — the
        // player's window and its column at every size, walked by keys and
        // clicks from a state fixture (`photograph.rs`). What `nix run .#png
        // -- --door …` runs.
        Some("photograph") => return photograph::run(&words.all()),
        // `ronoctua-shell column [<tenant>] [--on <workspace>] [--size <size>]`
        // — a right column (`column`), on the workspace named or the one in
        // front, at the size named or its tenant's own: its two tenants are
        // two crates, and the binary is where both are named.
        Some("column") => {
            let said = words.all();
            let opening = column::Opening::parse(&said[..]).with_context(|| {
                format!(
                    "usage: ronoctua-shell column [{}] [--on <workspace>] [--size {}]",
                    right_column::Tenant::ALL
                        .map(right_column::Tenant::name)
                        .join(" | "),
                    column::Size::ALL.map(column::Size::name).join(" | ")
                )
            })?;
            return match opening.tenant {
                right_column::Tenant::FastAccess => {
                    ronoctua_shell_fast_access::fast_access_window::host_column(&opening, None)
                        .context("the search's column")
                }

                right_column::Tenant::Player => {
                    ronoctua_shell_player::player_window::host_column(&opening)
                        .context("the player's column")
                }
            };
        }
        // `ronoctua-shell files [path]` — the manager in a window of its own,
        // the SAME `files::View` the footer draws from, handed the vault's
        // flow of the security centre, which the files crate cannot name
        // (`vault_flow`).
        Some("files") => {
            let where_ = words.first().map(std::path::PathBuf::from);
            return ronoctua_shell_files::files_window::host_it(
                where_.as_deref(),
                vault_flow::flow(),
            )
            .context("host the manager");
        }
        // EVERY KEY THIS BINARY WAS COMPILED WITH, IN EVERY LANGUAGE OF THE
        // TEXTS IT IS POINTED AT (`RONOCTUA_TEXTS`) — the start guard's
        // question, asked of all of them and said in full. It brings nothing
        // up; `every-key-exists-at-run-time` runs it against the build's own
        // texts and against broken copies.
        Some("check-texts") => {
            let mut refused = false;
            let keys = ronoctua_i18n::Keys::of(every_key);
            for language in ronoctua_i18n::LANGUAGES {
                match ronoctua_i18n::Texts::checked(language, &keys) {
                    Ok(Some(_)) => {}
                    Ok(None) => {
                        eprintln!("{language} is a declared language and its texts do not load");
                        refused = true;
                    }
                    // NO DIRECTORY AT ALL is one sentence, not one per language.
                    Err(ronoctua_i18n::Missing::Unset) => {
                        eprintln!("{}", ronoctua_i18n::Missing::Unset);
                        refused = true;
                        break;
                    }
                    Err(missing) => {
                        eprintln!("{missing}");
                        refused = true;
                    }
                }
            }
            if refused {
                std::process::exit(i32::from(ronoctua_i18n::EXIT_MISSING));
            }
            println!(
                "texts OK: {} keys of {} crates and {} brain modules, each in {} languages ({}), at {}",
                keys.len(),
                ronoctua_i18n::Keys::of(key::sets).crates().count(),
                ronoctua_brain::compiled::IDS.len(),
                ronoctua_i18n::LANGUAGES.len(),
                ronoctua_i18n::LANGUAGES.join(", "),
                std::env::var(ronoctua_i18n::ENV).unwrap_or_default()
            );
            return Ok(());
        }
        // WHAT THE FOOTER DRAWS OF THE BRAIN'S MODULES, AS IT IS POINTED AT
        // (`RONOCTUA_BRAIN_CATALOGUE`) — the start guard's question, asked
        // of every language and said in full: the file read, every key its
        // modules need in each language's texts, and its notifications as
        // the book reads them. It brings nothing up; the catalogue's checks
        // run it against the build's own file and against broken copies.
        Some("check-catalogue") => {
            let catalogue = match ronoctua_brain::catalogue::load() {
                Ok(c) => c,
                Err(missing) => ronoctua_brain::catalogue::refuse(&missing),
            };
            let mut refused = false;
            let keys = ronoctua_i18n::Keys::of(every_key);
            for language in ronoctua_i18n::LANGUAGES {
                match ronoctua_i18n::Texts::checked(language, &keys) {
                    Ok(Some(texts)) => {
                        let lacking = catalogue.lacking(|k| texts.has(k));
                        if !lacking.is_empty() {
                            eprintln!(
                                "{language}: {}",
                                ronoctua_brain::catalogue::Missing::Keys {
                                    file: std::env::var_os(ronoctua_brain::catalogue::ENV)
                                        .unwrap_or_default()
                                        .into(),
                                    keys: lacking,
                                }
                            );
                            refused = true;
                        }
                    }
                    Ok(None) => {
                        eprintln!("{language} is a declared language and its texts do not load");
                        refused = true;
                    }
                    Err(missing) => {
                        eprintln!("{missing}");
                        refused = true;
                    }
                }
            }
            let notified = match ronoctua_notify::brain::checked() {
                Ok(entries) => entries.len(),
                Err(why) => {
                    eprintln!("{why}");
                    refused = true;
                    0
                }
            };
            if refused {
                std::process::exit(i32::from(ronoctua_brain::catalogue::EXIT_MISSING));
            }
            println!(
                "catalogue OK: {} modules, {} marks, {} actions, {} events ({} notifications), \
                 {} glyphs, every key in {} languages, at {}",
                catalogue.modules.len(),
                catalogue
                    .modules
                    .iter()
                    .map(|m| m.marks.len())
                    .sum::<usize>(),
                catalogue.actions().count(),
                catalogue.events().count(),
                notified,
                catalogue.glyphs.len(),
                ronoctua_i18n::LANGUAGES.len(),
                std::env::var(ronoctua_brain::catalogue::ENV).unwrap_or_default()
            );
            return Ok(());
        }
        // It checks the keymap without bringing anything up. It exists so whoever edits their own
        // keymap gets the same verdict `nix flake check` gives.
        Some("check-keymap") => {
            // THE THREE SETS, each built through `Tree::build_it`: a collision
            // in any of them — the folded ones included — fails here, named
            // with the set it is in (`bindings::Trees::build`).
            let (trees, registry) = keymap::build().context("keymap")?;
            // REACHABILITY, and not only collisions. A command with no key at all does not
            // collide with anything — and that is how it goes unnoticed. It is
            // asked of the FULL tree, which holds every sequence the other two
            // fold away.
            let reachable = keymap::reachable(&trees.full);
            // The compositor's 141 MIRRORED actions are reachable by NAME
            // and by name only — giving them keys would spend the whole keymap on
            // what is almost never used. It is recognisable by its prefix.
            //
            // AND THE ONES EVERY CRATE NAMES AS REACHED BY NAME ONLY
            // (`keymap::by_name_only`), each with its reason beside it. It is
            // that list and not a literal here because the TEST beside it
            // reads the same list, and two literals would be two answers the
            // day one of them moved.
            //
            // AND EVERY BRAIN MODULE'S ACTION (`leader::a_module_action`),
            // generated from the modules' folders, for the reason written
            // there.
            let by_name_only = keymap::by_name_only();
            let orphans: Vec<&str> = registry
                .all_ones()
                .map(|c| c.id)
                .filter(|id| {
                    !id.starts_with("wm-")
                        && !by_name_only.contains(id)
                        && !ronoctua_shell_footer::leader::a_module_action(id)
                        && !reachable.contains(id)
                })
                .collect();
            anyhow::ensure!(
                orphans.is_empty(),
                "commands with no key at all (no sequence, no chord, no emergency): {}",
                orphans.join(", ")
            );
            // `--dump`: THE WHOLE KEYMAP AS TEXT, after the verdict — every
            // sequence of the three sets, the chords, every command the
            // registry holds and every brain action. Sorted wherever the
            // order is only the order of registration, so two builds that
            // assemble the same keymap differently print the same lines: it
            // is what proves a refactor of the keymap moved no key.
            if std::env::args().skip(2).any(|a| a == "--dump") {
                for line in keymap_dump(&trees, &registry) {
                    println!("{line}");
                }
            }
            println!(
                "keymap OK: {} bindings ({} in the footer's set, {} in the window's), \
                 {} chords, {} commands, all reachable",
                trees.full.amount(),
                trees.footer.amount(),
                trees.window.amount(),
                actions::chords().len(),
                registry.amount()
            );
            return Ok(());
        }

        // `ronoctua-shell opener <file>…` — what opening each file reaches,
        // one line per file: the entry's id, `editor`, or `nothing <type>`.
        // The answer both file doors act on (`launch::opener_of`), over the
        // catalogue and the `mimeapps.list` defaults of this environment —
        // what `models-open-in-orca` asks, with no window.
        Some("opener") => {
            let paths: Vec<std::path::PathBuf> =
                std::env::args_os().skip(2).map(Into::into).collect();
            anyhow::ensure!(!paths.is_empty(), "usage: ronoctua-shell opener <file>...");
            let catalogue: launch::Catalogue = std::sync::Arc::new(ronoctua_apps::scan("en_GB"));
            for path in &paths {
                let said = match launch::opener_of(&catalogue, path) {
                    launch::Opener::App(entry) => entry.id,
                    launch::Opener::Editor => "editor".to_owned(),
                    launch::Opener::Nothing(of_type) => format!("nothing {of_type}"),
                };
                println!("{}\t{said}", path.display());
            }
            return Ok(());
        }

        // EVERY CRATE'S WINDOWS, by their tables ([`DOORS`]).
        Some(name) => {
            if let Some(door) = ronoctua_shell_kit::doors::door_named(DOORS, name) {
                return (door.run)(&words);
            }
        }
        None => {}
    }

    tracing::info!(
        face = tokens::font::FAMILY_MONO,
        size = tokens::text::size(),
        cell = tokens::metrics::cell_width(tokens::text::size()),
        line = tokens::metrics::line_height(tokens::text::size()),
        footer = tokens::metrics::footer_height(tokens::text::size()),
        "bringing the footer up"
    );

    ronoctua_shell_footer::surface::run(&COMPOSITION).context("the footer's surface failed")
}

/// The lines of `check-keymap --dump`, one fact a line, tab-separated:
///
/// - `<set> <sequence> group <label>` and `<set> <sequence> command <id>
///   <label> <argument|->`, for every node of the footer's, the window's and
///   the full set, walked through [`ronoctua_keymap::Tree::resolve`] in the
///   tree's own key order;
/// - `chord <shows> <command> <label>`, for every chord which-key lists;
/// - `command <id> <label>`, for every command the registry holds, by id;
/// - `by-name-only <id>`, the commands reached by name alone;
/// - `brain-action <module> <id> <command> <label> row=<bool> asks=<key|->`,
///   every brain module's action, by command.
fn keymap_dump(trees: &bindings::Trees, registry: &ronoctua_cmd::Registry) -> Vec<String> {
    fn walk(
        tree: &ronoctua_keymap::Tree,
        set: &str,
        seq: &mut Vec<ronoctua_keymap::Key>,
        out: &mut Vec<String>,
    ) {
        let ronoctua_keymap::Resolution::Prefix(entries) = tree.resolve(seq) else {
            return;
        };
        for entry in entries {
            seq.push(entry.sym);
            let at = ronoctua_keymap::notation(seq);
            match tree.resolve(seq) {
                ronoctua_keymap::Resolution::Command(id, argument) => out.push(format!(
                    "{set}\t{at}\tcommand\t{id}\t{}\t{}",
                    entry.label,
                    argument.unwrap_or("-")
                )),
                _ => {
                    out.push(format!("{set}\t{at}\tgroup\t{}", entry.label));
                    walk(tree, set, seq, out);
                }
            }
            seq.pop();
        }
    }
    let mut out = Vec::new();
    for (set, name) in [
        (bindings::Set::Footer, "footer"),
        (bindings::Set::Window, "window"),
        (bindings::Set::Full, "full"),
    ] {
        walk(trees.of(set), name, &mut Vec::new(), &mut out);
    }
    for chord in actions::chords() {
        out.push(format!(
            "chord\t{}\t{}\t{}",
            chord.shows,
            chord.command,
            chord.label()
        ));
    }
    let mut commands: Vec<String> = registry
        .all_ones()
        .map(|c| format!("command\t{}\t{}", c.id, c.label))
        .collect();
    commands.sort();
    out.extend(commands);
    let mut by_name: Vec<String> = keymap::by_name_only()
        .iter()
        .map(|id| format!("by-name-only\t{id}"))
        .collect();
    by_name.sort();
    out.extend(by_name);
    let mut actions: Vec<String> = ronoctua_brain::catalogue::catalogue()
        .actions()
        .map(|(module, a)| {
            format!(
                "brain-action\t{}\t{}\t{}\t{}\trow={}\tasks={}",
                module.id,
                a.id,
                a.command,
                a.label,
                a.row,
                a.asks.as_deref().unwrap_or("-")
            )
        })
        .collect();
    actions.sort();
    out.extend(actions);
    out
}

/// This program's name, as a refusal at its start says it first.
fn program_name() -> String {
    std::env::args_os()
        .next()
        .map(std::path::PathBuf::from)
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_default()
}

/// The bind's `repeat` property, when it needs to be stated.
///
/// # Why it is not always `false`
///
/// A bind repeats while the key is held, and that is **desirable** for what
/// the compositor applies: holding `LEADER+←` keeps moving the focus, as
/// holding an arrow in an editor keeps moving. What must not repeat is what
/// **starts a process** — holding `LEADER+RET` would open a queue of
/// terminals, and holding the alternative leader would reopen the sequence on
/// every repeat, which is the defect that was being fixed.
///
/// The rule comes from the command itself, and not from an extra column in
/// the table: if it starts with `exec`, it creates a process.
///
/// THE FLAG GOES BEFORE THE KEY, which is why this returns a prefix rather
/// than a suffix: `bindsym --no-repeat Ctrl+Alt+space exec …`.
fn repeat_of(said: &str) -> &'static str {
    if said.starts_with("exec ") {
        " --no-repeat"
    } else {
        ""
    }
}

/// A MOUSE BIND REACHES THE TITLE BAR AND NOTHING ELSE, unless it says so.
///
/// That is this compositor's default (`scroll.5`: "mouse bindings are only
/// triggered when over the title bar"), and this distro draws NO title bar —
/// `default_border pixel` is the whole decoration, and invariant 15 is why.
/// So the two wheel chords would be bound to a region that does not exist on
/// screen: the config would load clean, the manual would teach them, and
/// scrolling with the leader held would do nothing anywhere.
///
/// `--whole-window` also makes them work over an empty workspace, which is
/// where somebody scrolling between workspaces most often is.
///
/// BY THE KEY AND NOT BY THE COMMAND, unlike `repeat_of` above: what decides
/// is whether the thing pressed is a BUTTON, and the compositor's own spelling
/// for one is `button1`…`button9` or a `BTN_` event name.
fn region_of(key: &str) -> &'static str {
    let button = key
        .rsplit('+')
        .next()
        .is_some_and(|k| k.starts_with("button") || k.starts_with("BTN_"));
    if button { " --whole-window" } else { "" }
}

#[cfg(test)]
mod tests {
    //! THE FOUNDATION'S GUARDS, IN THIS BINARY'S TESTS. `cfg(test)` is true in
    //! a crate's own test build only: what the kit keeps for tests — a test
    //! that reaches the machine fails, a copy stays in the tests — reaches
    //! these through its `test-support`, named in `[dev-dependencies]`, or
    //! it does not, and a test of the footer acts on the person's session.

    /// FAILS IF: a test of this binary that reaches the machine acts on it
    /// instead of failing — the kit's guard compiled out of these tests.
    #[test]
    #[should_panic(expected = "a test reached the machine")]
    fn a_test_of_the_binary_that_reaches_the_machine_fails() {
        ronoctua_shell_kit::daemon::about_to_act("the binary's own guard test");
    }

    /// FAILS IF: a copy made in this binary's tests is handed to `wl-copy`,
    /// over the clipboard of whoever runs the suite, instead of to the tests'
    /// own record.
    #[test]
    fn a_copy_in_the_binarys_tests_stays_in_the_tests() {
        ronoctua_shell_kit::clipboard_copy::COPIED.with(|c| c.borrow_mut().clear());
        ronoctua_shell_kit::clipboard_copy::copy_to_clipboard("kept in the tests")
            .expect("a test's copy is recorded");
        let copied = ronoctua_shell_kit::clipboard_copy::COPIED.with(|c| c.borrow().clone());
        assert_eq!(copied, vec![(None, b"kept in the tests".to_vec())]);
    }

    /// FAILS IF: two crates' tables claim one word — the dispatch takes the
    /// first table listed, and the other window could never be opened — or
    /// a word no table has opens a door instead of falling to the footer.
    #[test]
    fn every_door_is_one_word_and_the_dispatch_finds_it() {
        let mut seen = std::collections::BTreeMap::new();
        for (at, table) in crate::DOORS.iter().enumerate() {
            for door in *table {
                if let Some(first) = seen.insert(door.name, at) {
                    panic!("`{}` is the word of tables {first} and {at}", door.name);
                }
                let found = ronoctua_shell_kit::doors::door_named(crate::DOORS, door.name)
                    .unwrap_or_else(|| panic!("`{}` is not found by its word", door.name));
                assert_eq!(found.name, door.name);
            }
        }
        assert!(seen.len() >= 30, "{} doors in the tables", seen.len());
        assert!(ronoctua_shell_kit::doors::door_named(crate::DOORS, "nowhere").is_none());
        assert!(ronoctua_shell_kit::doors::door_named(&[], "stick").is_none());
    }

    /// FAILS IF: a key a crate of this binary was compiled with, or a distro
    /// brain module's, is not in the texts the tests read — in any declared
    /// language — or a crate's or a module's keys are collected twice, or a
    /// crate that speaks under the binary is not reached by its `key::sets`,
    /// or a module's keys are not asked at all (each crate owns its keys now,
    /// and a module's are its manifest's: only the binary knows them all, so
    /// the census is here).
    #[test]
    fn every_key_of_every_crate_is_in_every_language() {
        let keys = ronoctua_i18n::Keys::of(crate::every_key);
        let crates: Vec<&str> = keys.crates().collect();
        let mut once = crates.clone();
        once.sort_unstable();
        once.dedup();
        assert_eq!(
            once.len(),
            crates.len(),
            "a crate's keys were collected twice: {crates:?}"
        );
        for reached in [
            "ronoctua-i18n",
            "ronoctua-shell-kit",
            "ronoctua-shell-door",
            "ronoctua-shell-footer",
            "ronoctua-shell-stick",
        ] {
            assert!(
                crates.contains(&reached),
                "`{reached}` is not among {crates:?}"
            );
        }
        for module in ronoctua_brain::compiled::MANIFESTS {
            assert!(
                crates.contains(&module.id),
                "the brain module `{}` is not among {crates:?}",
                module.id
            );
            assert!(
                keys.iter().any(|k| k == module.name_key),
                "`{}`, the name of the brain module `{}`, is not asked",
                module.name_key,
                module.id
            );
        }
        assert!(keys.len() > 5000, "only {} keys", keys.len());
        for language in ronoctua_i18n::LANGUAGES {
            if let Err(missing) = ronoctua_i18n::Texts::checked(language, &keys) {
                panic!("{missing}");
            }
        }
    }

    /// FAILS IF: the two crates that read the brain's catalogue — the brain's
    /// library its modules, the notification book its notifications — read
    /// it under two names or in two formats: the start guard would pass one
    /// and the book refuse the other (or read another file). Both are linked
    /// here, and only here are they asked together.
    #[test]
    fn the_brains_catalogue_is_one_file_in_one_format() {
        assert_eq!(
            ronoctua_brain::catalogue::ENV,
            ronoctua_notify::brain::VARIABLE
        );
        assert_eq!(
            ronoctua_brain::catalogue::VERSION,
            ronoctua_notify::brain::VERSION
        );
        // AND THE BOOK HOLDS EVERY NOTIFICATION THE CATALOGUE DECLARES, by
        // the event's tag: what the footer's events are recognised by.
        let book = ronoctua_notify::Book::distro();
        let declared: Vec<&str> = ronoctua_brain::catalogue::catalogue()
            .events()
            .filter(|(_, e)| e.notification.is_some())
            .map(|(_, e)| e.tag.as_str())
            .collect();
        assert!(!declared.is_empty(), "no module declares a notification");
        for tag in declared {
            assert!(book.entry(tag).is_some(), "the book does not know `{tag}`");
        }
    }
}

#[cfg(test)]
mod player_pages {
    //! THE WHOLE PLAYER, ITS PAGES PLUGGED IN — what no crate of it can ask
    //! alone: the player's view reaches channels by theme and the ranking
    //! only through the pages written above it (`player_seam`), and the
    //! binary is the one place that names the view and both pages
    //! (`PLAYER_PAGES`).

    use ronoctua_shell_catalogue::player_catalog::CatalogueView as _;
    use ronoctua_shell_video_metrics::player_ranking::{RankingView as _, fixture::video};

    /// FAILS IF: a page of the player exists that no row opens — a page
    /// reachable only by a command is one the pointer never reaches
    /// (invariant 19) — a row that opens a page is labelled by another key
    /// than the one the page is titled by, or the pages the binary installs
    /// are not the ones that make every page reachable.
    #[test]
    fn every_page_of_the_player_is_reached_by_a_row() {
        // A CATALOGUE OF ITS OWN: a listing is reached from a row of channels
        // by theme (`player_catalog`).
        let catalogue =
            std::env::temp_dir().join(format!("ronoctua-pages-catalog-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&catalogue);
        ronoctua_shell_catalogue::player_catalog::write_fixture(&catalogue).expect("a catalogue");
        ronoctua_shell_player::player_seam::with_pages(crate::PLAYER_PAGES, || {
            ronoctua_shell_player::player::every_page_is_reached_by_a_row(&|v| {
                v.catalogue_at(catalogue.clone());
                v.catalog_mut().source = Some("archive".into());
                // AND A RANKING HANDED ITS VIDEOS: a video's own page is
                // reached from the `⋯` of its row (`player_ranking`).
                v.ranking_mut().given(vec![video(1)], 1);
                v.ranking_mut().chosen = Some(video(1).id);
            });
        });
        let _ = std::fs::remove_dir_all(&catalogue);
    }
}
