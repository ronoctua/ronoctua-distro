//! A region's streets: downloaded, baked, and left as one file.
//!
//! # Why the queue and not the shell
//!
//! Because of the two numbers this delivery measured: the download is between
//! six megabytes (Andorra) and two thousand one hundred (Brazil's south-east),
//! and the baking reads every road of a country. Neither belongs in an event
//! loop that is also drawing. The queue already survives the footer restarting,
//! already knows how to give a job up, and already answers "how far along".
//!
//! # Why ONE job and not two
//!
//! `Work::Media` is the precedent: it downloads and then remuxes, in one job,
//! because the middle state — a file on disk that is nobody's — is exactly
//! what a queue exists to never leave lying about. A person who gives this up
//! half way gets the piece removed, the same promise every other row makes.

use std::sync::Arc;

use crate::{
    job::{Stage, Work},
    ledger::Shared,
};
use ronoctua_files::watch::{Control, Flow};

/// Runs one region to the end, or to whatever the switch says.
///
/// The shape is [`crate::aria2::run`]'s, because the first half IS that: the
/// download goes through aria2, which is what follows the `-latest-` redirect.
pub fn run(shared: &Arc<Shared>, id: u64, switch: &Arc<Control>, work: Work) {
    let Work::Atlas {
        region,
        label,
        url,
        into,
    } = &work
    else {
        tracing::error!("the atlas engine got a job that is not a region");
        shared.touch(id, |j| j.stage = Stage::Failed);
        return;
    };
    match carry(shared, id, switch, region, label, url, into) {
        Ok(()) => shared.touch(id, |j| j.stage = Stage::Done),
        Err(e) => {
            let cancelled = switch.flow() == Flow::Stop;
            let text = format!("{e:#}");
            shared.touch(id, |j| {
                j.stage = if cancelled {
                    Stage::Cancelled
                } else {
                    Stage::Failed
                };
                if !cancelled {
                    j.trouble = Some(text);
                }
            });
        }
    }
}

fn carry(
    shared: &Arc<Shared>,
    id: u64,
    switch: &Arc<Control>,
    region: &str,
    label: &str,
    url: &str,
    into: &std::path::Path,
) -> anyhow::Result<()> {
    // THE SCRATCH IS INSIDE THE REGION'S OWN DIRECTORY, so giving up removes
    // one directory and leaves nothing anywhere else — and so a download and
    // the pack it becomes are never on two filesystems, where the rename at
    // the end would stop being atomic.
    let dir = into.join(ronoctua_atlas_packs::flat(region));
    std::fs::create_dir_all(&dir)?;
    let scratch = dir.join("download");
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch)?;

    let down = crate::aria2::carry(shared, id, switch, url, &scratch, None);
    if let Err(e) = down {
        let _ = std::fs::remove_dir_all(&scratch);
        return Err(e);
    }

    // WHAT CAME DOWN, BY WHAT IT IS AND NOT BY WHAT IT WAS ASKED FOR: the URL
    // is a redirect, so the name on disk carries the DATE Geofabrik published,
    // which is this pack's version and the only one there is.
    let zip = std::fs::read_dir(&scratch)?
        .flatten()
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|e| e == "zip"))
        .ok_or_else(|| anyhow::anyhow!("the download left no archive"))?;
    let dated = zip
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();

    // THE ROW KEEPS RUNNING AND THE TOTAL GOES BACK TO UNKNOWN.
    //
    // Baking is not a download and has no honest percentage — it is one pass
    // over a hundred megabytes of geometry — so the bar goes away and the row
    // says the stage and the name, which is what `Job::total == 0` already
    // means everywhere else in this queue.
    shared.touch(id, |j| {
        j.stage = Stage::Running;
        j.done = 0;
        j.total = 0;
        j.now = dated.clone();
    });

    let raw = std::fs::read(&zip)?;
    let baked = ronoctua_atlas_packs::bake::pack(&raw, region, label, &dated)
        .map_err(|t| anyhow::anyhow!("{t:?}"))?;
    drop(raw);
    ronoctua_xdg::write_atomically(&dir, ronoctua_atlas_packs::FILE, &baked)?;
    // AND THE DOWNLOAD GOES. It is half a gigabyte whose whole purpose was to
    // become the file beside it, and a queue that left it there would double
    // what every region costs on disk without saying so anywhere.
    let _ = std::fs::remove_dir_all(&scratch);
    Ok(())
}
