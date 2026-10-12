//! The declaration's orders: declare, forget, decline, bring back, refresh
//! the pins, reconcile.
//!
//! # One chain, in this order
//!
//! ```text
//! declare     install HERE NOW by today's path → pin → the mirror → staged + applied → pushed
//! forget      the mirror → staged + applied → the copy by hand removed → rebuilt → reconciled → pushed
//! decline     the mirror → staged + applied → rebuilt → reconciled → pushed
//! undecline   the mirror → staged + applied → rebuilt → reconciled → pushed
//! refresh     newer pins → the mirror → staged + applied → pushed
//! ```
//!
//! A DECLARATION IS INSTALLED NOW and not at the next update: the person
//! clicked a program to have it, and a declaration that waited a week for a
//! rebuild would be a button that did nothing. The program goes in the way an
//! install always did (the source's own module, in its lane); the declaration
//! is what keeps it — the update builds the declared copy, and the reconciler
//! then takes the hand-made one away ([`crate::reconcile`]).
//!
//! A REMOVAL, A DECLINE AND A RETURN NEED THE SYSTEM REBUILT, because the
//! declared copy is part of the system: `ronoctua-update-apply.service` builds
//! the machine directory without moving its inputs ([`crate::stage::rebuild`]).
//! A declaration needs no rebuild of its own: what it brought is already here.
//!
//! # Lanes
//!
//! A source's lane is held only around what touches that source (the
//! install, the removal, the reconcile), and the declaration's own lane only
//! around reading, changing and staging the mirror — never across a rebuild,
//! which can take as long as a system build does.
//!
//! # What runs in a rehearsal
//!
//! Nothing that changes anything: the `.#dev` instance shares the person's
//! home and machine with the real session, and a test click there must not
//! declare a program on the person's other computers.

use std::io::Write as _;

use ronoctua_person::declared::{
    AndroidApp, AndroidImage as PersonImage, AppImageApp, Entry, FlatpakApp, NixApp, WebApp,
};

use crate::declaration::{self as decl, Edit};
use crate::order::{AndroidImage, Consent, Order};
use crate::progress::{Phase, Report};
use crate::stage::{self, Published};
use crate::{AppKey, Context, Done, Failure, Outcome, Refusal, Source};

/// Run one of the declaration's orders on this machine.
///
/// # Errors
///
/// The order's refusal, as a value.
pub fn run(ctx: &Context, order: &Order, report: &mut dyn Report) -> Done {
    run_on(ctx, order, &decl::this_host(), &crate::github::Net, report)
}

/// [`run`], on the machine `host`, over `net` — a test's.
///
/// # Errors
///
/// As [`run`].
pub fn run_on(
    ctx: &Context,
    order: &Order,
    host: &str,
    net: &dyn crate::github::Fetch,
    report: &mut dyn Report,
) -> Done {
    if let Order::Reconcile { source } = order {
        return crate::reconcile::run(ctx, *source, report);
    }
    let host = decl::host_of(host)?;
    if ctx.rehearse && order.changes_something() {
        let _ = writeln!(
            std::io::stderr(),
            "rehearse: {} {}",
            order.word(),
            crate::cli::name_of(order)
        );
        return Ok(Outcome {
            key: crate::cli::key_of(order),
            rehearsed: true,
            ..Outcome::default()
        });
    }
    let edit = Edit {
        at: decl::now_stamp(),
        on: host.clone(),
    };
    match order {
        Order::Declare {
            key,
            consent,
            machines,
        } => declare(ctx, key, *consent, machines.as_deref(), &edit, net, report),
        Order::Forget { key, keep_data } => forget(ctx, key, *keep_data, report),
        Order::Decline { id, machines } => decline(ctx, id, machines.clone(), &edit, report),
        Order::Undecline { id } => undecline(ctx, id, &host, report),
        Order::RefreshPins => refresh_pins(ctx, &host, net, report),
        _ => Err(Failure::refused(Refusal::UnknownItem)),
    }
}

/// A source's lane, saying so when it waits.
fn lane(
    ctx: &Context,
    source: Source,
    report: &mut dyn Report,
) -> Result<crate::lock::Lane, Failure> {
    crate::lock::take(&crate::lock::directory(&ctx.roots), source, || {
        report.step(Phase::Waiting, 0, 0, source.word());
    })
    .map_err(|e| Failure::trouble(format!("could not take the {} lane: {e}", source.word())))
}

/// The declaration's lane.
fn declaration_lane(ctx: &Context, report: &mut dyn Report) -> Result<crate::lock::Lane, Failure> {
    crate::lock::take_declaration(&crate::lock::directory(&ctx.roots), || {
        report.step(Phase::Waiting, 0, 0, crate::lock::DECLARATION);
    })
    .map_err(|e| Failure::trouble(format!("could not take the declaration's lane: {e}")))
}

/// Machines as a declaration may name them: `None` (every one), or a list of
/// distinct hostnames that is not empty.
fn machines_of(machines: Option<&[String]>) -> Result<Option<Vec<String>>, Failure> {
    let Some(hosts) = machines else {
        return Ok(None);
    };
    let mut out: Vec<String> = Vec::new();
    for h in hosts {
        decl::host_of(h)?;
        if !out.contains(h) {
            out.push(h.clone());
        }
    }
    if out.is_empty() {
        return Err(Failure {
            refusal: Some(Refusal::MachineUnnamed),
            trouble: "no machine named (none means every machine)".into(),
        });
    }
    Ok(Some(out))
}

/// Change the mirror under the declaration's lane, and hand it to root.
///
/// A busy machine (the update holds it) is no failure of the change: the
/// mirror holds it, and the machine takes it at the apply AFTER that
/// update's — which staged the mirror before it took the machine. The
/// screen says so from the files: what the declaration lost and the system
/// still carries is LEAVING (`Declarations::leaving`), and a decline the
/// system has not built waits.
fn change_and_apply(
    ctx: &Context,
    report: &mut dyn Report,
    change: impl FnOnce(&Context) -> Result<(), Failure>,
) -> Result<stage::Applied, Failure> {
    let _held = declaration_lane(ctx, report)?;
    report.step(Phase::Declaring, 0, 0, "");
    change(ctx)?;
    stage::apply(ctx, report)
}

/// Carry the declaration to the repository; what does not go now goes with
/// the next sync, and the journal says why.
fn publish(ctx: &Context, report: &mut dyn Report) {
    match stage::publish(ctx, report) {
        Published::Pushed(said) => tracing::info!(report = %said, "the declaration was pushed"),
        Published::Later(why) => {
            tracing::info!(%why, "the declaration travels with the next sync");
        }
    }
}

/// Whether a program is on this machine already, by its source's own list
/// (a hand-made copy) or as a declared copy the system applied.
fn here(ctx: &Context, key: &AppKey) -> bool {
    let applied = decl::id_of(key).is_some_and(|id| {
        decl::person_applied(&ctx.roots).is_some_and(|r| r.applied.contains(&id))
            && decl::read_declared(&ctx.roots).is_ok_and(|d| d.entry(&id).is_some())
    });
    if applied {
        return true;
    }
    match key {
        AppKey::Nix { attr } => crate::nix::manifest(ctx).is_ok_and(|m| m.find(attr).is_some()),
        AppKey::Flatpak { .. } => crate::flatpak::installed(ctx).iter().any(|i| i.key == *key),
        AppKey::AppImage { key } => crate::appimage::sidecar(&ctx.roots, key).is_some(),
        AppKey::Web { slug } => crate::web::read(&ctx.roots, slug).is_some(),
        AppKey::Android { package } => {
            crate::android::record(&ctx.roots).contains_key(package)
                || crate::android::installed(ctx, &crate::android::status(ctx))
                    .iter()
                    .any(|i| i.key == *key)
        }
        AppKey::System { .. } => true,
    }
}

/// Declare a program for the person: here now, then kept.
fn declare(
    ctx: &Context,
    key: &AppKey,
    consent: Consent,
    machines: Option<&[String]>,
    edit: &Edit,
    net: &dyn crate::github::Fetch,
    report: &mut dyn Report,
) -> Done {
    let source = key
        .source()
        .ok_or_else(|| Failure::refused(Refusal::DeclaredInTheFlake))?;
    let machines = machines_of(machines)?;
    let id = decl::id_of(key).ok_or_else(|| Failure::refused(Refusal::DeclaredInTheFlake))?;
    // DECLARED ALREADY — on the person's other computers, say — the entry IS
    // the declaration, and only where it goes changes. A declaration that
    // does not read is refused before anything is installed.
    if let Some(entry) = decl::read_declared(&ctx.roots)
        .map_err(|why| decl::unreadable(decl::File::Declared, why))?
        .entry(&id)
        .cloned()
    {
        return extend(ctx, key, entry, machines, edit, report);
    }
    // HERE NOW, AND PINNED, in the source's lane.
    let (entry, desktop_ids) = {
        let _lane = lane(ctx, source, report)?;
        let mut desktop_ids = Vec::new();
        if !here(ctx, key) {
            // A WEB APP is made before it is declared (its editor): there is
            // nothing to install by a slug alone.
            if source == Source::Web {
                return Err(Failure::refused(Refusal::UnknownItem));
            }
            let install = Order::Install {
                key: key.clone(),
                consent,
            };
            let done =
                crate::cli::module(ctx, crate::cli::Route::Source(source), &install, report)?;
            desktop_ids = done.desktop_ids;
        }
        report.step(Phase::Declaring, 0, 0, key.id());
        (
            entry_of(ctx, key, consent, machines, edit, net, report)?,
            desktop_ids,
        )
    };
    change_and_apply(ctx, report, |ctx| {
        decl::change_declared(&ctx.roots, |doc| {
            decl::upsert(doc, entry.clone());
            Ok(())
        })
        .map(|_| ())
    })?;
    publish(ctx, report);
    Ok(Outcome {
        key: Some(key.clone()),
        desktop_ids,
        amount: None,
        rehearsed: false,
    })
}

/// `entry` going to `machines`, edited now on this machine — nothing else
/// of it moves: its pin, its icon, what it was built from.
fn restamped(mut entry: Entry, machines: Option<Vec<String>>, edit: &Edit) -> Entry {
    macro_rules! set {
        ($a:expr) => {{
            $a.machines = machines;
            $a.edited_at = edit.at.clone();
            $a.edited_on.clone_from(&edit.on);
        }};
    }
    match &mut entry {
        Entry::Nix(a) => set!(a),
        Entry::Flatpak(a) => set!(a),
        Entry::AppImage(a) => set!(a),
        Entry::Web(a) => set!(a),
        Entry::Android(a) => set!(a),
    }
    entry
}

/// Declare again what is declared already: only where it goes changes —
/// "also on this computer", or every computer of the person's.
///
/// Nothing is installed by its key: an AppImage would come at its newest
/// release instead of its pin (and the pin would move for every machine),
/// and a web app has nothing to install by a slug. When this computer is
/// newly among the entry's and the program is not here, the machine is
/// rebuilt now, and what arrives is the declared copy — the one every other
/// computer of the person's has.
fn extend(
    ctx: &Context,
    key: &AppKey,
    entry: Entry,
    machines: Option<Vec<String>>,
    edit: &Edit,
    report: &mut dyn Report,
) -> Done {
    let source = key
        .source()
        .ok_or_else(|| Failure::refused(Refusal::DeclaredInTheFlake))?;
    let was_here = entry.applies_to(&edit.on);
    let next = restamped(entry, machines, edit);
    let now_here = next.applies_to(&edit.on);
    let applied = change_and_apply(ctx, report, |ctx| {
        decl::change_declared(&ctx.roots, |doc| {
            decl::upsert(doc, next.clone());
            Ok(())
        })
        .map(|_| ())
    })?;
    // A BUSY MACHINE has not taken the change: its update builds it later,
    // and a rebuild now would build the copy root has, without this one.
    if now_here && !was_here && applied != stage::Applied::Busy && !here(ctx, key) {
        stage::rebuild(ctx, report)?;
        crate::reconcile::run(ctx, Some(source), report)?;
    }
    publish(ctx, report);
    Ok(Outcome {
        key: Some(key.clone()),
        ..Outcome::default()
    })
}

/// Change a DECLARED web app — what the editor's save is when the slug is
/// in the person's declaration ([`crate::cli`] routes it here).
///
/// Written here first, by today's path: the person's own entry, made when
/// the system's declared one is the only one, so the change shows at once
/// (it shadows the declared copy, as the launcher reads them). Then declared
/// again from what was written, keeping where it goes, staged and pushed:
/// every computer of the person's builds the new copy at its update, and the
/// reconciler takes the person's entry away once this machine's system
/// carries the same app ([`crate::reconcile::waits_for_build`]).
///
/// # Errors
///
/// [`Refusal::UnknownItem`] for a slug the declaration does not name,
/// [`Refusal::DeclarationUnreadable`], and the web module's and the
/// staging's refusals.
pub fn edit_declared_web(
    ctx: &Context,
    slug: &str,
    spec: &crate::order::WebSpec,
    report: &mut dyn Report,
) -> Done {
    edit_declared_web_on(ctx, slug, spec, &decl::this_host(), report)
}

/// [`edit_declared_web`], on the machine `host` — a test's.
///
/// # Errors
///
/// As [`edit_declared_web`].
pub fn edit_declared_web_on(
    ctx: &Context,
    slug: &str,
    spec: &crate::order::WebSpec,
    host: &str,
    report: &mut dyn Report,
) -> Done {
    let key = AppKey::Web {
        slug: slug.to_owned(),
    };
    let id = decl::id_of(&key).ok_or_else(|| Failure::refused(Refusal::UnknownItem))?;
    let entry = decl::read_declared(&ctx.roots)
        .map_err(|why| decl::unreadable(decl::File::Declared, why))?
        .entry(&id)
        .cloned()
        .ok_or_else(|| Failure::refused(Refusal::UnknownItem))?;
    if ctx.rehearse {
        let _ = writeln!(
            std::io::stderr(),
            "rehearse: write the web app {slug} here and declare it again"
        );
        return Ok(Outcome {
            key: Some(key),
            rehearsed: true,
            ..Outcome::default()
        });
    }
    let edit = Edit {
        at: decl::now_stamp(),
        on: decl::host_of(host)?,
    };
    let (written, next) = {
        let _lane = lane(ctx, Source::Web, report)?;
        let written = crate::web::write_declared(ctx, slug, spec, report)?;
        report.step(Phase::Declaring, 0, 0, slug);
        let next = entry_of(
            ctx,
            &key,
            Consent::default(),
            entry.machines().map(<[String]>::to_vec),
            &edit,
            &crate::github::Net,
            report,
        )?;
        (written, next)
    };
    change_and_apply(ctx, report, |ctx| {
        decl::change_declared(&ctx.roots, |doc| {
            decl::upsert(doc, next.clone());
            Ok(())
        })
        .map(|_| ())
    })?;
    publish(ctx, report);
    Ok(written)
}

/// The entry that declares `key`, built from what is on this machine.
fn entry_of(
    ctx: &Context,
    key: &AppKey,
    consent: Consent,
    machines: Option<Vec<String>>,
    edit: &Edit,
    net: &dyn crate::github::Fetch,
    report: &mut dyn Report,
) -> Result<Entry, Failure> {
    let id = decl::id_of(key).ok_or_else(|| Failure::refused(Refusal::DeclaredInTheFlake))?;
    let at = edit.at.clone();
    let on = edit.on.clone();
    Ok(match key {
        AppKey::Nix { attr } => {
            // THE CONSENT THE MACHINES NEED: an unfree package declared
            // without it is skipped by every one of them.
            let unfree = crate::nix::vet(ctx, attr, consent)?
                || crate::prefs::read(&ctx.roots.config).unfree.contains(attr);
            Entry::Nix(NixApp {
                id,
                machines,
                edited_at: at,
                edited_on: on,
                attr: attr.clone(),
                unfree,
            })
        }
        AppKey::Flatpak { id: app_id } => Entry::Flatpak(FlatpakApp {
            id,
            machines,
            edited_at: at,
            edited_on: on,
            origin: crate::flatpak::origin_of(ctx, app_id)
                .unwrap_or_else(|| crate::flatpak::REMOTE.to_owned()),
            app_id: app_id.clone(),
        }),
        AppKey::AppImage { key: k } => {
            let s = crate::appimage::sidecar(&ctx.roots, k)
                .ok_or_else(|| Failure::refused(Refusal::UnknownItem))?;
            let (asset, pattern, repo) =
                crate::pins::appimage::asset_of(ctx, &s, net, crate::appimage::now())?;
            let mine = crate::pins::appimage::is_here(&s, &asset)
                .then_some((s.file.as_path(), s.sha256.as_str()));
            let pin = crate::pins::appimage::pin(ctx, k, &asset, mine, at.clone(), report)?;
            Entry::AppImage(AppImageApp {
                id,
                machines,
                edited_at: at,
                edited_on: on,
                key: k.clone(),
                name: s.name.clone(),
                pin,
                follows: Some(crate::pins::appimage::follows(&repo, &pattern)),
                icon: s
                    .icon
                    .as_deref()
                    .and_then(|p| decl::icon_of_file(&ctx.roots, p)),
            })
        }
        AppKey::Web { slug } => {
            let w = crate::web::read(&ctx.roots, slug)
                .ok_or_else(|| Failure::refused(Refusal::UnknownItem))?;
            let url = crate::web::canonical_url(&w.url).map_err(Failure::refused)?;
            Entry::Web(WebApp {
                id,
                machines,
                edited_at: at,
                edited_on: on,
                slug: slug.clone(),
                name: w.name.clone(),
                url,
                profile: w.profile.clone(),
                icon: w
                    .icon_file
                    .as_deref()
                    .and_then(|p| decl::icon_of_file(&ctx.roots, p)),
                category: w
                    .category
                    .filter(|c| crate::web::CATEGORIES.contains(&c.as_str()))
                    .unwrap_or_else(|| crate::web::FALLBACK_CATEGORY.to_owned()),
            })
        }
        AppKey::Android { package } => {
            let recorded = crate::android::record(&ctx.roots);
            if recorded
                .get(package)
                .is_some_and(|r| r.source == crate::android::Came::File)
            {
                return Err(Failure {
                    refusal: Some(Refusal::NothingToPin),
                    trouble: format!("{package} came from a file"),
                });
            }
            let (pin, from) = crate::pins::android(&ctx.roots, package)?;
            let image = match crate::android::status(ctx).image.unwrap_or_default() {
                AndroidImage::Gapps => PersonImage::Gapps,
                AndroidImage::Vanilla => PersonImage::Vanilla,
            };
            Entry::Android(AndroidApp {
                id,
                machines,
                edited_at: at,
                edited_on: on,
                package: package.clone(),
                pin,
                from,
                image,
            })
        }
        AppKey::System { .. } => return Err(Failure::refused(Refusal::DeclaredInTheFlake)),
    })
}

/// Remove a program for good: here, and from the declaration.
fn forget(ctx: &Context, key: &AppKey, keep_data: bool, report: &mut dyn Report) -> Done {
    let source = key
        .source()
        .ok_or_else(|| Failure::refused(Refusal::DeclaredInTheFlake))?;
    let id = decl::id_of(key).ok_or_else(|| Failure::refused(Refusal::DeclaredInTheFlake))?;
    let was_applied = decl::person_applied(&ctx.roots).is_some_and(|r| r.applied.contains(&id));
    // A DECLARATION THAT DOES NOT READ is refused, never taken for one that
    // does not name it: removed here alone, the next update would bring it
    // back.
    let was_declared = decl::read_declared(&ctx.roots)
        .map_err(|why| decl::unreadable(decl::File::Declared, why))?
        .entry(&id)
        .is_some();
    if was_declared {
        change_and_apply(ctx, report, |ctx| {
            decl::change_declared(&ctx.roots, |doc| {
                decl::without(doc, &id);
                Ok(())
            })
            .map(|_| ())
        })?;
    }
    // THE COPY BY HAND, when there is one — a Flatpak nix-flatpak installed
    // is the declared copy, and the rebuild takes it.
    {
        let _lane = lane(ctx, source, report)?;
        let by_hand = match key {
            AppKey::Flatpak { id } => {
                !crate::flatpak::declared(&ctx.roots).contains(id)
                    && crate::flatpak::installed(ctx).iter().any(|i| i.key == *key)
            }
            AppKey::Android { package } => {
                // THE LEDGER'S is the declared copy: the reconciler takes it.
                !crate::reconcile::ledger(&ctx.roots)
                    .android
                    .iter()
                    .any(|k| k.package == *package)
                    && here(ctx, key)
            }
            AppKey::Nix { attr } => crate::nix::manifest(ctx).is_ok_and(|m| m.find(attr).is_some()),
            AppKey::AppImage { key } => crate::appimage::sidecar(&ctx.roots, key).is_some(),
            AppKey::Web { slug } => crate::web::read(&ctx.roots, slug).is_some(),
            AppKey::System { .. } => false,
        };
        if by_hand {
            let remove = Order::Remove {
                key: key.clone(),
                keep_data,
            };
            crate::cli::module(ctx, crate::cli::Route::Source(source), &remove, report)?;
        }
    }
    if was_applied {
        let rebuilt = stage::rebuild(ctx, report)?;
        if rebuilt == stage::Rebuilt::Switched
            && !keep_data
            && let AppKey::Flatpak { id } = key
        {
            erase_flatpak_data(ctx, id);
        }
    }
    crate::reconcile::run(ctx, Some(source), report)?;
    if was_declared {
        publish(ctx, report);
    }
    Ok(Outcome {
        key: Some(key.clone()),
        ..Outcome::default()
    })
}

/// What a Flatpak saved (`~/.var/app/<id>`), once the app itself is gone —
/// the second, danger-coloured answer of "remove for good".
fn erase_flatpak_data(ctx: &Context, id: &str) {
    if !crate::flatpak::valid_id(id)
        || crate::flatpak::installed(ctx)
            .iter()
            .any(|i| i.key == AppKey::Flatpak { id: id.to_owned() })
    {
        return;
    }
    let dir = ctx.roots.home.join(".var/app").join(id);
    if let Err(e) = std::fs::remove_dir_all(&dir)
        && e.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!(error = %e, dir = %dir.display(), "what the app saved was not erased");
    }
}

/// Keep one of the distro's programs uninstalled, here or everywhere.
fn decline(
    ctx: &Context,
    id: &str,
    machines: Option<Vec<String>>,
    edit: &Edit,
    report: &mut dyn Report,
) -> Done {
    let machines = machines_of(machines.as_deref())?;
    // AN ESSENTIAL ONE IS REFUSED HERE, with its reason, before anything is
    // written: Nix would keep it anyway, and say so.
    if let Some(app) =
        decl::distro_apps(&ctx.roots).and_then(|d| d.apps.into_iter().find(|a| a.id == id))
        && let Some(reason) = app.essential
    {
        return Err(Failure {
            refusal: Some(Refusal::Essential),
            trouble: reason,
        });
    }
    let here_too = machines
        .as_ref()
        .is_none_or(|hosts| hosts.contains(&edit.on));
    change_and_apply(ctx, report, |ctx| {
        decl::change_declined(&ctx.roots, |doc| {
            decl::decline(doc, id, machines.clone(), edit);
            Ok(())
        })
        .map(|_| ())
    })?;
    if here_too {
        stage::rebuild(ctx, report)?;
        crate::reconcile::run(ctx, None, report)?;
    }
    publish(ctx, report);
    Ok(Outcome::default())
}

/// Bring a declined program of the distro back, on every machine.
fn undecline(ctx: &Context, id: &str, host: &str, report: &mut dyn Report) -> Done {
    let was_here = decl::read_declined(&ctx.roots)
        .map_err(|why| decl::unreadable(decl::File::Declined, why))?
        .apps
        .into_iter()
        .find(|x| x.id == id)
        .map(|d| d.applies_to(host));
    let Some(was_here) = was_here else {
        return Err(Failure::refused(Refusal::UnknownItem));
    };
    change_and_apply(ctx, report, |ctx| {
        decl::change_declined(&ctx.roots, |doc| {
            decl::undecline(doc, id);
            Ok(())
        })
        .map(|_| ())
    })?;
    if was_here {
        stage::rebuild(ctx, report)?;
        crate::reconcile::run(ctx, None, report)?;
    }
    publish(ctx, report);
    Ok(Outcome::default())
}

/// Look for newer pins, and hand the ones that moved to the machine. They
/// are BUILT at the next update, whose verification tries each first.
fn refresh_pins(
    ctx: &Context,
    host: &str,
    net: &dyn crate::github::Fetch,
    report: &mut dyn Report,
) -> Done {
    // THE DOWNLOADS RUN WITH NO LANE HELD; the pins are written to the file
    // as it is then, under the declaration's lane, and staged with it.
    let newer = crate::pins::newer(ctx, net, host, report)?;
    let mut moved = 0;
    if !newer.is_empty() {
        change_and_apply(ctx, report, |ctx| {
            moved = crate::pins::take(&ctx.roots, &newer)?;
            Ok(())
        })?;
    }
    if moved > 0 {
        publish(ctx, report);
    }
    Ok(Outcome {
        amount: Some(moved),
        ..Outcome::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Roots, Tools};

    fn scratch(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-software-declare-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A machine with a fake `nix` (a profile that grows with every `add`)
    /// and a fake `systemctl` that answers as root's applier and the
    /// rebuild would, writing down every call.
    fn machine(root: &std::path::Path) -> (Context, std::path::PathBuf) {
        let roots = Roots::under(root);
        let bin = root.join("bin");
        let log = root.join("calls.log");
        let profile = root.join("profile.txt");
        crate::write_executable(
            &bin.join("nix"),
            &format!(
                "#!/bin/sh\n\
                 echo \"nix $*\" >> {log}\n\
                 case \"$2\" in\n\
                 add) for a in \"$@\"; do last=$a; done; echo \"${{last#nixpkgs#}}\" >> {profile} ;;\n\
                 list) printf '{{\"version\":3,\"elements\":{{'; sep=''; \
                       if [ -e {profile} ]; then while read -r a; do printf '%s\"%s\":{{\"active\":true,\"attrPath\":\"legacyPackages.x86_64-linux.%s\",\"originalUrl\":\"flake:nixpkgs\",\"storePaths\":[]}}' \"$sep\" \"$a\" \"$a\"; sep=','; done < {profile}; fi; printf '}}}}\\n' ;;\n\
                 esac\n",
                log = log.display(),
                profile = profile.display()
            ),
        );
        crate::write_executable(
            &bin.join("systemctl"),
            &format!(
                "#!/bin/sh\n\
                 echo \"systemctl $*\" >> {log}\n\
                 case \"$*\" in\n\
                 *ronoctua-person-apply.service) n=$(cat {staged}/nonce); \
                   printf '{{\"format\":1,\"at\":\"2026-10-05T12:00:00Z\",\"nonce\":\"%s\",\"outcome\":\"applied\",\"refused\":null,\"ignored\":[],\"left-out\":[]}}' \"$n\" > {verdict} ;;\n\
                 esac\n",
                log = log.display(),
                staged = roots.system.staged().display(),
                verdict = roots.system.verdict().display()
            ),
        );
        let ctx = Context {
            tools: Tools {
                nix: Some(bin.join("nix")),
                systemctl: Some(bin.join("systemctl")),
                ..Tools::default()
            },
            roots,
            prefs: crate::prefs::Preferences::default(),
            rehearse: false,
        };
        (ctx, log)
    }

    /// DECLARE INSTALLS NOW AND RECORDS: the program goes in by today's path
    /// (`nix profile add`), the declaration holds it with this machine's name
    /// and the time, the staging root reads holds it too, and root's applier
    /// was started for it — no rebuild, which a declaration does not need.
    ///
    /// FAILS IF any of those does not hold.
    #[test]
    fn declare_installs_now_and_records() {
        let root = scratch("declare");
        let (ctx, log) = machine(&root);
        let key = AppKey::Nix {
            attr: "hello".into(),
        };
        let order = Order::Declare {
            key: key.clone(),
            consent: Consent::default(),
            machines: None,
        };
        let done = run_on(
            &ctx,
            &order,
            "noctua",
            &crate::github::canned::Canned::new(Vec::new()),
            &mut Vec::new(),
        )
        .unwrap();
        assert_eq!(done.key, Some(key));
        let calls = std::fs::read_to_string(&log).unwrap();
        assert!(
            calls.contains("nix profile add") && calls.contains("nixpkgs#hello"),
            "{calls}"
        );
        assert!(
            calls.contains(&format!(
                "systemctl start --no-ask-password {}",
                ronoctua_person::root::APPLY_UNIT
            )),
            "{calls}"
        );
        assert!(
            !calls.contains(ronoctua_machine::UPDATE_APPLY_UNIT),
            "a declaration rebuilt the system: {calls}"
        );
        let declared = decl::read_declared(&ctx.roots).unwrap();
        let entry = declared.entry("nix:hello").expect("declared");
        assert_eq!(entry.edited_on(), "noctua");
        assert_eq!(entry.machines(), None);
        let staged = std::fs::read_to_string(
            ctx.roots
                .system
                .staged()
                .join("person")
                .join(ronoctua_person::names::DECLARED),
        )
        .unwrap();
        assert!(staged.contains("\"nix:hello\""), "{staged}");

        // DECLARED AGAIN, ALREADY HERE: nothing is installed twice.
        std::fs::write(&log, b"").unwrap();
        run_on(
            &ctx,
            &order,
            "noctua",
            &crate::github::canned::Canned::new(Vec::new()),
            &mut Vec::new(),
        )
        .unwrap();
        let calls = std::fs::read_to_string(&log).unwrap();
        assert!(!calls.contains("profile add"), "{calls}");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// FAILS IF: forgetting does not take the entry out of the declaration,
    /// does not remove the copy by hand, or rebuilds a system the program
    /// was never applied to.
    #[test]
    fn forgetting_takes_it_from_the_declaration_and_from_here() {
        let root = scratch("forget");
        let (ctx, log) = machine(&root);
        let key = AppKey::Nix {
            attr: "hello".into(),
        };
        run_on(
            &ctx,
            &Order::Declare {
                key: key.clone(),
                consent: Consent::default(),
                machines: None,
            },
            "noctua",
            &crate::github::canned::Canned::new(Vec::new()),
            &mut Vec::new(),
        )
        .unwrap();
        std::fs::write(&log, b"").unwrap();
        run_on(
            &ctx,
            &Order::Forget {
                key,
                keep_data: true,
            },
            "noctua",
            &crate::github::canned::Canned::new(Vec::new()),
            &mut Vec::new(),
        )
        .unwrap();
        let calls = std::fs::read_to_string(&log).unwrap();
        assert!(calls.contains("nix profile remove"), "{calls}");
        assert!(
            !calls.contains(ronoctua_machine::UPDATE_APPLY_UNIT),
            "{calls}"
        );
        assert!(decl::read_declared(&ctx.roots).unwrap().apps.is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// FAILS IF: an essential program of the distro can be declined, a
    /// decline elsewhere rebuilds this machine, or a decline here does not.
    #[test]
    fn a_decline_is_refused_for_what_the_distro_needs_and_rebuilds_only_here() {
        let root = scratch("decline");
        let (ctx, log) = machine(&root);
        let reports = &ctx.roots.system.reports;
        std::fs::create_dir_all(reports).unwrap();
        std::fs::write(
            reports.join(ronoctua_person::names::DISTRO_APPS),
            r#"{"format":1,"apps":[
              {"id":"zsh","source":"nix","desktop-ids":[],"essential":"the-login-shell","group":"base","on-this-machine":true,"declined-here":false},
              {"id":"gimp","source":"nix","desktop-ids":["gimp.desktop"],"essential":null,"group":"graphics","on-this-machine":true,"declined-here":false}]}"#,
        )
        .unwrap();
        let canned = crate::github::canned::Canned::new(Vec::new());
        let refused = run_on(
            &ctx,
            &Order::Decline {
                id: "zsh".into(),
                machines: None,
            },
            "noctua",
            &canned,
            &mut Vec::new(),
        )
        .unwrap_err();
        assert_eq!(refused.refusal, Some(Refusal::Essential));
        assert!(decl::read_declined(&ctx.roots).unwrap().apps.is_empty());
        run_on(
            &ctx,
            &Order::Decline {
                id: "gimp".into(),
                machines: Some(vec!["laptop".into()]),
            },
            "noctua",
            &canned,
            &mut Vec::new(),
        )
        .unwrap();
        assert!(
            !std::fs::read_to_string(&log)
                .unwrap()
                .contains(ronoctua_machine::UPDATE_APPLY_UNIT)
        );
        run_on(
            &ctx,
            &Order::Decline {
                id: "gimp".into(),
                machines: Some(vec!["noctua".into()]),
            },
            "noctua",
            &canned,
            &mut Vec::new(),
        )
        .unwrap();
        assert!(
            std::fs::read_to_string(&log)
                .unwrap()
                .contains(ronoctua_machine::UPDATE_APPLY_UNIT)
        );
        let declined = decl::read_declined(&ctx.roots).unwrap();
        assert_eq!(
            declined.apps[0].machines,
            Some(vec!["laptop".into(), "noctua".into()])
        );
        // BROUGHT BACK: the decline goes; one that is not there is said.
        run_on(
            &ctx,
            &Order::Undecline { id: "gimp".into() },
            "noctua",
            &canned,
            &mut Vec::new(),
        )
        .unwrap();
        assert!(decl::read_declined(&ctx.roots).unwrap().apps.is_empty());
        assert_eq!(
            run_on(
                &ctx,
                &Order::Undecline { id: "gimp".into() },
                "noctua",
                &canned,
                &mut Vec::new()
            )
            .unwrap_err()
            .refusal,
            Some(Refusal::UnknownItem)
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    fn web_entry(slug: &str, machines: Option<Vec<String>>) -> Entry {
        Entry::Web(WebApp {
            id: format!("web:{slug}"),
            machines,
            edited_at: ronoctua_person::Stamp::parse("2026-10-03T09:30:00Z").unwrap(),
            edited_on: "laptop".into(),
            slug: slug.into(),
            name: "Mail".into(),
            url: "https://mail.example.org/".into(),
            profile: "Default".into(),
            icon: None,
            category: "Network".into(),
        })
    }

    /// "ALSO ON THIS COMPUTER", for entries declared on another computer
    /// only: a web app (nothing installs it by a slug) and an AppImage (by
    /// its key it would come at its newest release, and the pin would move
    /// for every machine).
    ///
    /// FAILS IF: the order is refused, anything is installed by its key or
    /// asked of the network, the entry changes beyond where it goes and its
    /// stamps, or the machine is not rebuilt now that it is among the
    /// entry's.
    #[test]
    fn declaring_what_is_declared_elsewhere_moves_only_where_it_goes() {
        use ronoctua_person::declared::Pin;
        let root = scratch("extend");
        let (ctx, log) = machine(&root);
        let at = ronoctua_person::Stamp::parse("2026-10-03T09:30:00Z").unwrap();
        let obsidian = Entry::AppImage(AppImageApp {
            id: "appimage:obsidian".into(),
            machines: Some(vec!["laptop".into()]),
            edited_at: at.clone(),
            edited_on: "laptop".into(),
            key: "obsidian".into(),
            name: "Obsidian".into(),
            pin: Pin {
                url: "https://github.com/obsidianmd/obsidian-releases/releases/download/v1.6.7/Obsidian-1.6.7.AppImage".into(),
                hash: "sha256-47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU=".into(),
                version: "v1.6.7".into(),
                pinned_at: at,
            },
            follows: None,
            icon: None,
        });
        decl::change_declared(&ctx.roots, |d| {
            decl::upsert(d, web_entry("mail", Some(vec!["laptop".into()])));
            decl::upsert(d, obsidian.clone());
            Ok(())
        })
        .unwrap();
        let canned = crate::github::canned::Canned::new(Vec::new());
        run_on(
            &ctx,
            &Order::Declare {
                key: AppKey::Web {
                    slug: "mail".into(),
                },
                consent: Consent::default(),
                machines: Some(vec!["laptop".into(), "noctua".into()]),
            },
            "noctua",
            &canned,
            &mut Vec::new(),
        )
        .unwrap();
        run_on(
            &ctx,
            &Order::Declare {
                key: AppKey::AppImage {
                    key: "obsidian".into(),
                },
                consent: Consent::default(),
                machines: None,
            },
            "noctua",
            &canned,
            &mut Vec::new(),
        )
        .unwrap();
        let doc = decl::read_declared(&ctx.roots).unwrap();
        let mail = doc.entry("web:mail").unwrap();
        assert_eq!(
            mail.machines(),
            Some(&["laptop".to_owned(), "noctua".to_owned()][..])
        );
        assert_eq!(mail.edited_on(), "noctua");
        assert_eq!(mail.settled(), {
            let Entry::Web(mut w) = web_entry("mail", Some(vec!["laptop".into(), "noctua".into()]))
            else {
                unreachable!()
            };
            w.edited_on = "noctua".into();
            Entry::Web(w).settled()
        });
        let Some(Entry::AppImage(a)) = doc.entry("appimage:obsidian") else {
            panic!("the AppImage left the declaration");
        };
        let Entry::AppImage(was) = &obsidian else {
            unreachable!()
        };
        assert_eq!(a.pin, was.pin, "the pin moved");
        assert_eq!(a.machines, None);
        assert!(canned.asked.borrow().is_empty(), "the network was asked");
        let calls = std::fs::read_to_string(&log).unwrap();
        assert!(!calls.contains("profile add"), "{calls}");
        assert!(
            calls.contains(ronoctua_machine::UPDATE_APPLY_UNIT),
            "not rebuilt: {calls}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// CHANGING A DECLARED WEB APP declares it again.
    ///
    /// FAILS IF: the editor's change of a declared web app — whose own entry
    /// here the reconciler took away — is refused, is written here only (the
    /// reconciler would take it away and the declaration keep the old
    /// address), or does not keep where the entry goes; or a slug the
    /// declaration does not name is changed this way.
    #[test]
    fn changing_a_declared_web_app_declares_it_again() {
        let root = scratch("web-edit");
        let (mut ctx, _log) = machine(&root);
        ctx.tools.chrome = Some(root.join("bin/google-chrome-stable"));
        decl::change_declared(&ctx.roots, |d| {
            decl::upsert(
                d,
                web_entry("mail", Some(vec!["laptop".into(), "noctua".into()])),
            );
            Ok(())
        })
        .unwrap();
        let png = root.join("icon.png");
        std::fs::write(&png, b"\x89PNG\r\n\x1a\nan-icon").unwrap();
        let spec = crate::order::WebSpec {
            name: "Post".into(),
            url: "https://post.example.org/".into(),
            profile: "Default".into(),
            icon: crate::order::IconChoice::File { path: png.clone() },
            prepared: Some(png),
            category: Some("Office".into()),
        };
        assert!(crate::web::read(&ctx.roots, "mail").is_none());
        let done = edit_declared_web_on(&ctx, "mail", &spec, "noctua", &mut Vec::new()).unwrap();
        assert_eq!(
            done.key,
            Some(AppKey::Web {
                slug: "mail".into()
            })
        );
        let mine = crate::web::read(&ctx.roots, "mail").expect("written here");
        assert_eq!(mine.url, "https://post.example.org/");
        let doc = decl::read_declared(&ctx.roots).unwrap();
        let Some(Entry::Web(w)) = doc.entry("web:mail") else {
            panic!("no longer declared");
        };
        assert_eq!(
            (w.name.as_str(), w.url.as_str(), w.category.as_str()),
            ("Post", "https://post.example.org/", "Office")
        );
        assert_eq!(w.machines, Some(vec!["laptop".into(), "noctua".into()]));
        assert!(w.icon.is_some(), "the icon is kept by its content");
        assert_eq!(
            edit_declared_web_on(&ctx, "other", &spec, "noctua", &mut Vec::new())
                .unwrap_err()
                .refusal,
            Some(Refusal::UnknownItem)
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// FAILS IF: a rehearsal writes the declaration or runs anything.
    #[test]
    fn a_rehearsal_declares_nothing() {
        let root = scratch("rehearsal");
        let (mut ctx, log) = machine(&root);
        ctx.rehearse = true;
        let done = run_on(
            &ctx,
            &Order::Declare {
                key: AppKey::Nix {
                    attr: "hello".into(),
                },
                consent: Consent::default(),
                machines: None,
            },
            "noctua",
            &crate::github::canned::Canned::new(Vec::new()),
            &mut Vec::new(),
        )
        .unwrap();
        assert!(done.rehearsed);
        assert!(!log.exists());
        assert!(!decl::mirror(&ctx.roots).exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_machine_a_declaration_cannot_name_is_refused() {
        assert_eq!(
            machines_of(Some(&["Not A Host".to_owned()]))
                .unwrap_err()
                .refusal,
            Some(Refusal::MachineUnnamed)
        );
        assert_eq!(
            machines_of(Some(&[])).unwrap_err().refusal,
            Some(Refusal::MachineUnnamed)
        );
        assert_eq!(
            machines_of(Some(&["a".to_owned(), "a".to_owned()])).unwrap(),
            Some(vec!["a".to_owned()])
        );
        assert_eq!(machines_of(None).unwrap(), None);
    }
}
