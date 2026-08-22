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
//! Usage: `ronoctua-format-disk <node> <filesystem> [label]`
//!
//! The node and not the object path, because a test script knows `/dev/vdb`
//! and would have to ask udisks for the other — which is the thing being
//! tested.

use anyhow::{Context as _, Result, bail};

fn main() -> Result<()> {
    let node = std::env::args()
        .nth(1)
        .context("uso: ronoctua-format-disk <no> <sistema-de-arquivos> [rotulo]")?;
    let filesystem = std::env::args()
        .nth(2)
        .context("uso: ronoctua-format-disk <no> <sistema-de-arquivos> [rotulo]")?;
    let label = std::env::args().nth(3).unwrap_or_default();

    let manager = ronoctua_disks::Manager::fresh()?;
    let rack = manager.read()?;
    let Some(target) = rack.volumes.iter().find(|v| v.node == node) else {
        bail!(
            "o udisks2 nao conhece {node}; conhece {:?}",
            rack.volumes.iter().map(|v| &v.node).collect::<Vec<_>>()
        );
    };

    // THE SAME GUARD THE HOST APPLIES, and it is here so that a mistyped node
    // in a test script cannot erase the machine the test is running on.
    if let Some(why) = ronoctua_disks::needed(target) {
        bail!("{node} e um volume de que o sistema precisa ({why:?})");
    }

    manager
        .format(&target.at, &filesystem, &label, target.whole())
        .with_context(|| format!("formatar {node} como {filesystem}"))?;
    println!("formatado: {node} -> {filesystem} `{label}`");
    Ok(())
}
