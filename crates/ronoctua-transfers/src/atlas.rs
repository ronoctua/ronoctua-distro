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
    //
    // AND THE SWITCH DECIDES AT THE SEAM: a pause that landed as the download
    // ended is a held row, and the job HOLDS here, between its halves, until
    // it is let go. Written `Running` and baked at once, the row said it ran
    // while the person held it, and the baking did not wait.
    shared.under_way(id, |j| {
        j.done = 0;
        j.total = 0;
        j.now = dated.clone();
    });
    let baked = bake(switch, &zip, &dir, region, label, &dated, &mut || {});
    // AND THE DOWNLOAD GOES, however the baking ended. It is half a gigabyte
    // whose whole purpose was to become the file beside it: a queue that left
    // it there would double what every region costs on disk without saying
    // so anywhere, and a region given up mid-baking would leave its piece —
    // the promise every other row keeps.
    let _ = std::fs::remove_dir_all(&scratch);
    baked
}

/// The download at `zip` baked into the region's pack in `dir` — asking the
/// switch before it reads the archive and between every bite of the baking
/// (`bake::pack`'s `go_on`), and telling `at` before each of those bites.
///
/// THE SWITCH IS THE BAKING'S TOO: it blocks while the job is held, so the
/// row's `Paused` is a baking that waits, and a stop ends the baking at its
/// next boundary with no pack written. Before, `bake::pack` was one call the
/// switch never reached: a pause asked during it drew a held row over a
/// baking that ran on to `Done`. `at` is for a test that has to act at one
/// exact boundary; [`carry`] passes one nobody reads.
fn bake(
    switch: &Control,
    zip: &std::path::Path,
    dir: &std::path::Path,
    region: &str,
    label: &str,
    dated: &str,
    at: &mut dyn FnMut(),
) -> anyhow::Result<()> {
    switch.allowed()?;
    let raw = std::fs::read(zip)?;
    let baked = ronoctua_atlas_packs::bake::pack(&raw, region, label, dated, &mut || {
        at();
        switch.allowed().is_ok()
    })
    .map_err(|t| anyhow::anyhow!("{t:?}"))?;
    drop(raw);
    ronoctua_xdg::write_atomically(dir, ronoctua_atlas_packs::FILE, &baked)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::job::Engine;
    use std::{path::PathBuf, sync::mpsc, time::Duration};

    /// A HANG GUARD and never a wait for something to happen: a baking that
    /// never asks would otherwise hang the build. Reaching it is the failure.
    const GUARD: Duration = Duration::from_secs(30);

    /// A `.shp` of two named ways, their `.dbf` and a STORED zip of the two —
    /// the least a region's download is that the baking bakes, written here
    /// so the test fetches nothing (`ronoctua-atlas-packs`' own tests say
    /// why). Its baking asks five times: before each of the two layers, before
    /// each of the two records, and before the write.
    #[allow(clippy::cast_possible_truncation)]
    fn download() -> Vec<u8> {
        const WIDTH: usize = 40;
        let ways: [(&str, [(f64, f64); 2]); 2] = [
            ("Rua Augusta", [(38.71, -9.14), (38.71, -9.13)]),
            ("Rua da Prata", [(38.72, -9.14), (38.72, -9.13)]),
        ];
        let mut shp = vec![0_u8; 100];
        shp[0..4].copy_from_slice(&9994_u32.to_be_bytes());
        let mut dbf = vec![0_u8; 32];
        dbf[0] = 0x03;
        dbf[4..8].copy_from_slice(&(ways.len() as u32).to_le_bytes());
        dbf[8..10].copy_from_slice(&65_u16.to_le_bytes());
        dbf[10..12].copy_from_slice(&((WIDTH + 1) as u16).to_le_bytes());
        let mut field = vec![0_u8; 32];
        field[..4].copy_from_slice(b"name");
        field[11] = b'C';
        field[16] = WIDTH as u8;
        dbf.extend_from_slice(&field);
        dbf.push(0x0D);
        for (n, (name, points)) in ways.iter().enumerate() {
            let mut body = 3_u32.to_le_bytes().to_vec();
            body.extend_from_slice(&[0_u8; 32]);
            for count in [1_u32, 2, 0] {
                body.extend_from_slice(&count.to_le_bytes());
            }
            for (lat, lon) in points {
                body.extend_from_slice(&lon.to_le_bytes());
                body.extend_from_slice(&lat.to_le_bytes());
            }
            shp.extend_from_slice(&(n as u32 + 1).to_be_bytes());
            shp.extend_from_slice(&((body.len() / 2) as u32).to_be_bytes());
            shp.extend_from_slice(&body);
            dbf.push(b' ');
            let mut cell = vec![b' '; WIDTH];
            cell[..name.len()].copy_from_slice(name.as_bytes());
            dbf.extend_from_slice(&cell);
        }
        let members = [
            ("gis_osm_roads_free_1.shp", shp),
            ("gis_osm_roads_free_1.dbf", dbf),
        ];
        let (mut out, mut central) = (Vec::new(), Vec::new());
        for (name, body) in &members {
            let size = (body.len() as u32).to_le_bytes();
            let at = (out.len() as u32).to_le_bytes();
            out.extend_from_slice(b"PK\x03\x04");
            out.extend_from_slice(&[0_u8; 14]);
            out.extend_from_slice(&size);
            out.extend_from_slice(&size);
            out.extend_from_slice(&(name.len() as u16).to_le_bytes());
            out.extend_from_slice(&[0_u8; 2]);
            out.extend_from_slice(name.as_bytes());
            out.extend_from_slice(body);
            central.extend_from_slice(b"PK\x01\x02");
            central.extend_from_slice(&[0_u8; 16]);
            central.extend_from_slice(&size);
            central.extend_from_slice(&size);
            central.extend_from_slice(&(name.len() as u16).to_le_bytes());
            central.extend_from_slice(&[0_u8; 12]);
            central.extend_from_slice(&at);
            central.extend_from_slice(name.as_bytes());
        }
        let dir_at = (out.len() as u32).to_le_bytes();
        out.extend_from_slice(&central);
        out.extend_from_slice(b"PK\x05\x06");
        out.extend_from_slice(&[0_u8; 6]);
        out.extend_from_slice(&(members.len() as u16).to_le_bytes());
        out.extend_from_slice(&(central.len() as u32).to_le_bytes());
        out.extend_from_slice(&dir_at);
        out.extend_from_slice(&[0_u8; 2]);
        out
    }

    /// How a held baking is let go.
    #[derive(Clone, Copy)]
    enum Then {
        Resume,
        Cancel,
    }

    /// A region's job at the seam, as `carry` leaves it — under way, its
    /// download in the region's directory — baked on a thread of its own and
    /// HELD AT THE BAKING'S `ask`-th ASK: the thread waits there while
    /// the test pauses the job, and the switch holds it from there until the
    /// test does `then`. Gives back how the baking ended, how many asks it
    /// had made when it was let go and in all, and the region's directory.
    fn held_at(ask: usize, then: Then) -> (anyhow::Result<()>, usize, usize, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-transfers-atlas-{ask}-{}-{}",
            matches!(then, Then::Resume),
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let zip = dir.join("x-260914-free.shp.zip");
        std::fs::write(&zip, download()).unwrap();

        let shared = Arc::new(Shared::default());
        let work = Work::Atlas {
            region: "x".to_owned(),
            label: "X".to_owned(),
            url: String::new(),
            into: dir.clone(),
        };
        let (id, switch) = shared.take(Engine::Atlas, work);
        shared.under_way(id, |_| {});

        let asks = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let (reached, at_the_ask) = mpsc::channel::<()>();
        let (go, let_go) = mpsc::channel::<()>();
        let worker = {
            let (asks, dir) = (Arc::clone(&asks), dir.clone());
            std::thread::spawn(move || {
                bake(&switch, &zip, &dir, "x", "X", "x.zip", &mut || {
                    let n = asks.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                    if n == ask {
                        let _ = reached.send(());
                        let _ = let_go.recv();
                    }
                })
            })
        };

        at_the_ask
            .recv_timeout(GUARD)
            .expect("the baking never asked");
        shared.pause(id);
        assert_eq!(shared.until(id, |_| true).stage, Stage::Paused);
        go.send(()).unwrap();
        // THE SWITCH HOLDS THE BAKING NOW, at this very ask: nothing above
        // waits for it to reach anything, because a held baking reaches
        // nothing until it is let go.
        let when_let_go = asks.load(std::sync::atomic::Ordering::SeqCst);
        match then {
            Then::Resume => shared.resume(id),
            Then::Cancel => shared.cancel(id),
        }
        let ended = worker.join().expect("the baking's thread");
        let all = asks.load(std::sync::atomic::Ordering::SeqCst);
        (ended, when_let_go, all, dir)
    }

    /// FAILS IF: the baking does not stop where the switch says stop — a
    /// region given up while its baking was held is baked anyway and its
    /// pack written — at ANY of the baking's asks. Before, the baking was one
    /// call the switch never reached, and a held row sat over a baking that
    /// ran on to the end.
    #[test]
    fn a_region_held_mid_baking_and_given_up_bakes_nothing() {
        for ask in 1..=5 {
            let (ended, _, all, dir) = held_at(ask, Then::Cancel);
            let why = ended.expect_err("given up mid-baking, it baked on to the end");
            assert_eq!(format!("{why:#}"), "GivenUp", "at ask {ask}");
            assert_eq!(all, ask, "the baking asked again after a stop at ask {ask}");
            assert!(
                !dir.join(ronoctua_atlas_packs::FILE).exists(),
                "given up at ask {ask}, a pack was written"
            );
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    /// FAILS IF: a baking held at an ask moves on before it is let go, or
    /// does not finish its pack once it is.
    #[test]
    fn a_region_held_mid_baking_finishes_its_pack_when_let_go() {
        for ask in 1..=5 {
            let (ended, when_let_go, all, dir) = held_at(ask, Then::Resume);
            ended.expect("the baking, let go");
            assert_eq!(when_let_go, ask, "a held baking moved on to another ask");
            assert_eq!(all, 5, "the baking's asks");
            ronoctua_atlas_packs::Pack::open(&dir.join(ronoctua_atlas_packs::FILE))
                .expect("the pack the baking wrote");
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}
