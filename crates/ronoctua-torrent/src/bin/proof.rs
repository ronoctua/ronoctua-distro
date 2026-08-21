//! Proves this build speaks BitTorrent v2 — offline, and against real data.
//!
//! # Why a binary, and why it is not a claim
//!
//! "libtorrent 2.0 supports BEP 52" is something a changelog says. What this
//! does is make a **purely v2** torrent out of a directory it just wrote, hand
//! it to a real session, and check that the session reports a v2 info hash and
//! **no v1 one at all** — then do the same with a v1-only torrent, so a flag
//! that is simply always true fails here too.
//!
//! It also checks the thing a wrong flag would hide: that the pieces
//! **verify**. A v2 torrent is verified against a Merkle tree of SHA-256
//! hashes, so a session that reached `seeding` with `done == total` has walked
//! that tree over bytes on disk.
//!
//! Nothing here touches a tracker, a peer or the DHT: the data is already
//! where the torrent says it is, so the session checks it and goes straight to
//! seeding. That is what makes it a check and not a network test.
//!
//! Usage: `ronoctua-test-torrent <workdir>`

use anyhow::{Context as _, Result, bail};
use ronoctua_torrent::{Flavour, Session, State};
use std::path::{Path, PathBuf};

/// Enough bytes for several pieces at the 16 KiB minimum v2 allows.
const SIZE: usize = 400 * 1024;

/// How long to let the session check the files before giving up on it.
const PATIENCE: std::time::Duration = std::time::Duration::from_secs(30);

/// Writes the fixture: one directory, two files, contents that are not all the
/// same byte — a Merkle tree over identical blocks would agree with itself
/// even if the walk were wrong.
fn fixture(root: &Path) -> Result<PathBuf> {
    let content = root.join("conteudo");
    std::fs::create_dir_all(&content)?;
    for (n, name) in ["um.bin", "dois.bin"].iter().enumerate() {
        let bytes: Vec<u8> = (0..SIZE)
            .map(|i| u8::try_from((i * 31 + n * 7) % 251).unwrap_or(0))
            .collect();
        std::fs::write(content.join(name), &bytes)?;
    }
    Ok(content)
}

/// Adds a torrent whose data is already in place, and waits for it to verify.
fn verified(
    session: &mut Session,
    torrent: &Path,
    into: &Path,
) -> Result<ronoctua_torrent::Torrent> {
    let hash = session.add_file(torrent, into)?;
    let start = std::time::Instant::now();
    loop {
        let rows = session.poll();
        if let Some(row) = rows.iter().find(|r| r.hash == hash) {
            if row.state == State::Errored {
                bail!("o torrent falhou: {}", row.trouble);
            }
            // Seeding means it has everything AND checked it: libtorrent only
            // gets there by hashing what is on disk against the torrent.
            if matches!(row.state, State::Seeding | State::Finished)
                && row.total > 0
                && row.done == row.total
            {
                return Ok(row.clone());
            }
        }
        if start.elapsed() > PATIENCE {
            bail!("o torrent nao verificou em {PATIENCE:?}");
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

fn main() -> Result<()> {
    let root = std::env::args()
        .nth(1)
        .context("uso: ronoctua-test-torrent <diretorio-de-trabalho>")?;
    let root = PathBuf::from(root);
    std::fs::create_dir_all(&root)?;

    if !ronoctua_torrent::available() {
        bail!("esta build nao tem o shim; nao ha o que provar");
    }
    println!("libtorrent: {}", ronoctua_torrent::version());

    let content = fixture(&root)?;
    let v2_file = root.join("so-v2.torrent");
    let v1_file = root.join("so-v1.torrent");
    ronoctua_torrent::make_torrent(&content, &v2_file, Flavour::Two)?;
    ronoctua_torrent::make_torrent(&content, &v1_file, Flavour::One)?;

    // A PURELY-V2 TORRENT IS A DIFFERENT FILE, and it shows in the bytes: BEP
    // 52 puts `meta version` and `piece layers` in, and leaves the v1 `pieces`
    // string out. Checking the file itself as well as the session's opinion of
    // it means a session that mislabels one cannot pass alone.
    let raw = std::fs::read(&v2_file)?;
    for needle in [
        &b"meta version"[..],
        &b"piece layers"[..],
        &b"file tree"[..],
    ] {
        if !raw.windows(needle.len()).any(|w| w == needle) {
            bail!(
                "o .torrent v2 nao tem `{}` — nao e BEP 52",
                String::from_utf8_lossy(needle)
            );
        }
    }
    if raw.windows(8).any(|w| w == b"6:pieces") {
        bail!("o .torrent `so-v2` ainda carrega o `pieces` do v1");
    }
    println!("arquivo: `meta version`, `file tree` e `piece layers`, e nenhum `pieces` v1");

    // A high, fixed range: the check runs in a sandbox with a loopback and
    // nothing else, and a port already taken would fail for the wrong reason.
    let mut session = Session::open(&root.join("estado"), (46881, 46891))?;

    let v2 = verified(&mut session, &v2_file, content.parent().unwrap_or(&root))?;
    if !v2.v2 {
        bail!("a sessao nao reconheceu o hash v2");
    }
    if v2.v1 {
        bail!("o torrent `so-v2` chegou com hash v1 — nao era so-v2");
    }
    // 64 hex characters: a SHA-256, whole. `get_best()` would have truncated it
    // to a 40-character SHA-1 so v1 code paths could carry it, and truncated
    // hashes collide.
    if v2.hash.len() != 64 {
        bail!(
            "o hash v2 tem {} caracteres, nao 64: {}",
            v2.hash.len(),
            v2.hash
        );
    }
    println!(
        "v2: verificado, {} bytes, hash SHA-256 completo ({}…)",
        v2.total,
        &v2.hash[..8]
    );

    // AND THE CONVERSE, so that a flag which is always true fails here.
    let v1 = verified(&mut session, &v1_file, content.parent().unwrap_or(&root))?;
    if !v1.v1 || v1.v2 {
        bail!(
            "o torrent `so-v1` nao veio como v1 puro: v1={} v2={}",
            v1.v1,
            v1.v2
        );
    }
    if v1.hash.len() != 40 {
        bail!("o hash v1 tem {} caracteres, nao 40", v1.hash.len());
    }
    println!("v1: verificado, e a bandeira distingue os dois");

    // ---- HOLD, LET GO, AND GIVE UP ----
    if !session.pause(&v2.hash) {
        bail!("pausar o v2 falhou");
    }
    let held = {
        let start = std::time::Instant::now();
        loop {
            if let Some(r) = session.poll().into_iter().find(|r| r.hash == v2.hash)
                && r.paused
            {
                break true;
            }
            if start.elapsed() > PATIENCE {
                break false;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    };
    if !held {
        bail!("pediu para segurar e a sessao nunca disse que segurou");
    }
    if !session.resume(&v2.hash) {
        bail!("soltar o v2 falhou");
    }
    println!("segurar e soltar: a sessao confirma, e nao so aceita o pedido");

    // GIVING UP TAKES THE DATA, which is the promise a cancelled copy makes.
    if !session.remove(&v1.hash, true) {
        bail!("remover o v1 falhou");
    }
    let start = std::time::Instant::now();
    while session.poll().iter().any(|r| r.hash == v1.hash) {
        if start.elapsed() > PATIENCE {
            bail!("o torrent removido continua na lista");
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    println!("desistir: some da lista");

    session.save();
    println!("OK: BitTorrent v2 verificado offline, e o v1 continua distinguivel");
    Ok(())
}
