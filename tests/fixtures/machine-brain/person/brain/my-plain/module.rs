//! A person's module of the machine fixture `tests/fixtures/machine-brain/`:
//! it draws nothing and runs nothing — what a machine compiles of it is its
//! manifest.

use crate::module::{Birth, Manifest, Module};

/// The entry every module has.
#[must_use]
pub fn new(_birth: &Birth) -> impl Module + use<> {
    Fixture
}

/// The module: no state.
struct Fixture;

impl Module for Fixture {
    fn manifest(&self) -> &'static Manifest {
        &super::MANIFEST
    }
}
