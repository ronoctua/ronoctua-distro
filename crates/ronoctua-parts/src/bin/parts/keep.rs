//! The downloads of a part that is here, kept.
//!
//! After a part arrives, every download its build used — the fixed-output
//! derivations of its derivation's graph that are in the store — is rooted
//! under `<KEPT>/<id>/<name>`. A wrapper's change then builds again from what
//! is here instead of fetching it again (Chrome's pool drops old `.deb`s),
//! and a stick can carry them to another machine (`export`). The set is
//! replaced whole each time the part arrives, so a download the part stopped
//! using is let go.

use std::fs;
use std::path::Path;

use ronoctua_parts::names;

use crate::disk;
use crate::{program, say};

/// Roots `id`'s downloads, as `drv`'s graph names them.
pub fn downloads(id: &str, drv: &str) {
    let shown = disk::run(&[
        program("RONOCTUA_NIX"),
        "derivation".to_owned(),
        "show".to_owned(),
        "--recursive".to_owned(),
        drv.to_owned(),
    ]);
    let Ok(shown) = serde_json::from_str::<serde_json::Value>(&shown.out) else {
        say(
            4,
            &format!("{id}: its downloads were not kept: the graph did not read"),
        );
        return;
    };
    let table = shown.get("derivations").unwrap_or(&shown);
    let Some(table) = table.as_object() else {
        return;
    };
    let nix_store = crate::beside_nix("nix-store");
    let mut kept = Vec::new();
    for (key, d) in table {
        if d.pointer("/outputs/out/hash").is_none() {
            continue;
        }
        let path = if key.starts_with("/nix/store/") {
            key.clone()
        } else {
            format!("/nix/store/{key}")
        };
        let said = disk::run(&[
            nix_store.clone(),
            "--query".to_owned(),
            "--outputs".to_owned(),
            path,
        ]);
        if let Some(out) = said.out.lines().next().map(str::trim)
            && Path::new(out).exists()
        {
            kept.push(out.to_owned());
        }
    }
    let folder = Path::new(names::KEPT).join(id);
    let next = Path::new(names::KEPT).join(format!(".{id}.{}", std::process::id()));
    let _ = fs::remove_dir_all(&next);
    let made = (|| -> std::io::Result<()> {
        fs::create_dir_all(&next)?;
        for out in &kept {
            let name = out.rsplit('/').next().unwrap_or(out);
            std::os::unix::fs::symlink(out, next.join(name))?;
        }
        let _ = fs::remove_dir_all(&folder);
        fs::rename(&next, &folder)
    })();
    if let Err(e) = made {
        let _ = fs::remove_dir_all(&next);
        say(4, &format!("{id}: its downloads were not kept: {e}"));
    }
}
