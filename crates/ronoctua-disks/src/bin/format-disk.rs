//! Writes a filesystem, from a command line — for `disks-vm` and nothing else.
//!
//! # Why a binary and not a unit test
//!
//! [`ronoctua_disks::Manager::format`] is three D-Bus calls with a RACE between
//! them: `Block.Format` writes a partition table, and the `PartitionTable`
//! interface then appears on the object through a uevent that has not happened
//! yet. A unit test would mock the bus and prove the argument order — which is
//! the half that was never in doubt. What needs a running udisks is that the
//! interface ARRIVES, and that the partition comes out typed for the filesystem
//! that was written on it.
//!
//! It is not installed anywhere: `tests/disks-vm.nix` is its only caller, and
//! `nix/rust.nix` extracts it on its own so the VM does not carry the
//! workspace's wgpu, wasm runtime and three embedded fonts to make three calls.
//!
//! Usage:
//!
//! ```text
//! ronoctua-format-disk <node> <filesystem> [label] [--encrypt]
//! ronoctua-format-disk unlock <node>     the cleartext node, on stdout
//! ronoctua-format-disk lock <node>
//! ronoctua-format-disk mount <node>      where it landed, on stdout
//! ronoctua-format-disk eject <node>      unmount, lock, cut the power
//! ronoctua-format-disk rows              the sidebar's rows: node and seal
//! ```
//!
//! A PASSPHRASE COMES ON STDIN, one line — `--encrypt` and `unlock` read it —
//! and never as an argument, exactly as the shell hands it to the same calls:
//! `Manager::format` with a `Secret`, `Manager::unlock`, `Manager::put_away`.
//!
//! The node and not the object path, because a test script knows `/dev/vdb`
//! and would have to ask udisks for the other — which is the thing being
//! tested.

use anyhow::{Context as _, Result, bail};
use ronoctua_consent::keys::Secret;

/// The one line on stdin, without its end.
fn passphrase() -> Result<Secret> {
    let mut line = String::new();
    std::io::stdin()
        .read_line(&mut line)
        .context("read the passphrase from stdin")?;
    let text = line.trim_end_matches(['\n', '\r']);
    if text.is_empty() {
        bail!("no passphrase on stdin");
    }
    Ok(Secret::new(text))
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let manager = ronoctua_disks::Manager::fresh()?;
    let rack = manager.read()?;
    let find = |node: &str| {
        rack.volumes
            .iter()
            .find(|v| v.node == node)
            .with_context(|| {
                format!(
                    "udisks2 does not know {node}; it knows {:?}",
                    rack.volumes.iter().map(|v| &v.node).collect::<Vec<_>>()
                )
            })
    };
    match args.first().map(String::as_str) {
        Some("rows") => {
            for v in ronoctua_disks::listed(&rack.volumes, 0, true) {
                let seal = match ronoctua_disks::seal_of(v, &rack.volumes) {
                    Some(ronoctua_disks::Encrypted::Locked) => "locked",
                    Some(ronoctua_disks::Encrypted::Unlocked) => "unlocked",
                    None => "-",
                };
                println!("{} {seal}", v.node);
            }
            return Ok(());
        }
        Some("unlock") => {
            let target = find(args.get(1).context("unlock <node>")?)?;
            let clear = manager
                .unlock(&target.at, &passphrase()?)
                .map_err(|e| anyhow::anyhow!("{:?}: {e:#}", ronoctua_disks::refusal(&e)))?;
            let again = manager.read()?;
            let node = again
                .volumes
                .iter()
                .find(|v| v.at == clear)
                .map_or(clear.clone(), |v| v.node.clone());
            println!("{node}");
            return Ok(());
        }
        Some("lock") => {
            let target = find(args.get(1).context("lock <node>")?)?;
            return manager.put_away(&target.at, true);
        }
        Some("mount") => {
            let target = find(args.get(1).context("mount <node>")?)?;
            println!("{}", manager.mount(&target.at)?);
            return Ok(());
        }
        Some("eject") => {
            let target = find(args.get(1).context("eject <node>")?)?;
            return manager.put_away(&target.at, false);
        }
        _ => {}
    }

    let usage = "usage: ronoctua-format-disk <node> <filesystem> [label] [--encrypt]";
    let encrypt = args.iter().any(|a| a == "--encrypt");
    let words: Vec<&String> = args.iter().filter(|a| *a != "--encrypt").collect();
    let node = words.first().context(usage)?;
    let filesystem = words.get(1).context(usage)?;
    let label = words.get(2).map_or("", |l| l.as_str());
    let target = find(node)?;

    // THE SAME GUARD THE HOST APPLIES, and it is here so that a mistyped node
    // in a test script cannot erase the machine the test is running on.
    if let Some(why) = ronoctua_disks::needed(target) {
        bail!("{node} is a volume the system needs ({why:?})");
    }

    let secret = if encrypt { Some(passphrase()?) } else { None };
    manager
        .format(
            &target.at,
            filesystem,
            label,
            target.whole(),
            secret.as_ref(),
        )
        .with_context(|| format!("format {node} as {filesystem}"))?;
    println!(
        "formatted: {node} -> {filesystem} `{label}`{}",
        if encrypt { " inside LUKS2" } else { "" }
    );
    Ok(())
}
