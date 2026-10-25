//! The person's mirrors, asked for the downloads a build could not fetch
//! from their origin — a part's (`ronoctua-parts realize`) or the system's
//! ([`crate::apply`]).
//!
//! The person names them in their options (`download-mirrors`,
//! `ronoctua_person::options`), which reach this machine signed, through the
//! root applier, as `<machine>/person/options.json` — the one copy read here.
//! Each missing flat download is asked of each mirror in order, by its hash
//! and then by its name ([`Download::mirror_urls`]), with `nix store
//! prefetch-file` and the hash the flake pins: a mirror can only bring the
//! very bytes the distro declared. A source the person named is their
//! consent.

use std::path::Path;

use ronoctua_person::frontier::Download;
use ronoctua_person::options::Options;

use crate::{argv, run, say};

/// The mirrors the person named, as the copy of their options in the
/// machine directory `dir` says — none without one, or one that does not
/// read.
#[must_use]
pub fn named(dir: &Path) -> Vec<String> {
    let path = dir
        .join(ronoctua_person::names::MACHINE_PERSON)
        .join(ronoctua_person::names::OPTIONS);
    std::fs::read(path)
        .ok()
        .and_then(|bytes| ronoctua_person::parse::<Options>(&bytes).ok())
        .map(|o| o.download_mirrors)
        .unwrap_or_default()
}

/// Each download in `missing` fetched by `nix` from the first of `mirrors`
/// that has it: how many came.
#[must_use]
pub fn fetched(nix: &Path, missing: &[Download], mirrors: &[String]) -> usize {
    let mut came = 0;
    for download in missing {
        for url in download.mirror_urls(mirrors) {
            let Ok(said) = run::capture(
                nix,
                &argv::prefetch(&download.name, &download.hash, false, &url),
            ) else {
                continue;
            };
            if !said.ok() {
                continue;
            }
            match store_path_of(&said.stdout) {
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

/// The store path `nix store prefetch-file --json` named.
///
/// # Errors
///
/// What Nix answered that does not name one, in words.
pub fn store_path_of(json: &str) -> Result<String, String> {
    let json: serde_json::Value =
        serde_json::from_str(json).map_err(|_| "nix answered nothing it could read".to_owned())?;
    json.get("storePath")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| "nix named no store path".to_owned())
}
