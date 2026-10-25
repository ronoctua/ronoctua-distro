//! The person's mirrors, asked for the downloads a part's build could not
//! fetch from their origin.
//!
//! The person names them in their options (`download-mirrors`,
//! `ronoctua_person::options`), which reach this machine signed, through the
//! root applier, as `<machine>/person/options.json` — the one copy read here.
//! Each missing flat download is asked of each mirror in order, by its hash
//! and then by its name (`ronoctua_parts::frontier::Download::mirror_urls`),
//! with `nix store prefetch-file` and the hash the flake pins: a mirror can
//! only bring the very bytes the distro declared. A source the person named
//! is their consent.

use std::path::Path;

use ronoctua_parts::frontier::Download;
use ronoctua_person::options::Options;

use crate::disk;
use crate::say;

/// The machine directory, baked by the build (`nix/machine.nix`'s one name).
const MACHINE_DIR: Option<&str> = option_env!("RONOCTUA_MACHINE_DIR");

/// The mirrors the person named, as the machine's copy of their options
/// says — none without one, or one that does not read.
pub fn named() -> Vec<String> {
    let Some(dir) = MACHINE_DIR else {
        return Vec::new();
    };
    let path = Path::new(dir)
        .join(ronoctua_person::names::MACHINE_PERSON)
        .join(ronoctua_person::names::OPTIONS);
    std::fs::read(path)
        .ok()
        .and_then(|bytes| ronoctua_person::parse::<Options>(&bytes).ok())
        .map(|o| o.download_mirrors)
        .unwrap_or_default()
}

/// Each download in `missing` fetched from the first of `mirrors` that has
/// it: how many came.
pub fn fetched(nix: &str, missing: &[Download], mirrors: &[String]) -> usize {
    let mut came = 0;
    for download in missing {
        for url in download.mirror_urls(mirrors) {
            let said = disk::run(&ronoctua_parts::argv::prefetch(
                nix,
                &download.name,
                &download.hash,
                false,
                &url,
            ));
            if !said.ok {
                continue;
            }
            match crate::handin::store_path_of(&said.out) {
                Ok(out) if out == download.out => {
                    say(6, &format!("{}: brought from {url}", download.name));
                    came += 1;
                    break;
                }
                Ok(out) => say(4, &format!("{}: {url} became {out}", download.name)),
                Err(e) => say(4, &format!("{}: {url}: {e}", download.name)),
            }
        }
    }
    came
}
