//! The `nix` this program runs.

use std::path::PathBuf;

/// The `nix` this program runs, by store path (`RONOCTUA_NIX`): the
/// machine's own, which is the one that fills its store.
#[derive(Debug, Clone)]
pub struct Nix {
    /// Its path.
    pub program: PathBuf,
}
