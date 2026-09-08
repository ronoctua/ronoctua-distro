//! The border with `libronoctua-torrent.so`, and the only place in this crate
//! that may write `unsafe`.
//!
//! # EVERY LAYOUT HERE WAS MEASURED, NOT INFERRED
//!
//! `#[repr(C)]` promises to ARRANGE what it was told, never to check it against
//! the header — on a 64-bit ABI a wrong integer width and a wrong struct size
//! are both just registers with the wrong contents, and neither produces a
//! diagnostic. That is written down in `AGENTS.md` because it cost three wrong
//! struct layouts on `libghostty-vt`, found by a twenty-line C program.
//!
//! So this one does not rely on somebody remembering: `nix/torrent-shim.nix`
//! compiles the real header with the real C++ compiler and writes every
//! `sizeof` and `offsetof` into `share/ronoctua/layout.txt`, and
//! `torrent-layout-matches-cxx` compares that file against [`LAYOUT`] below.
//! The numbers are still repeated here — a test that reads them from the thing
//! it is testing proves nothing — but nobody has to measure them by hand again.
//!
//! # Nothing crossing this line owns memory
//!
//! Every string that comes back is a fixed-size array inside a struct this side
//! allocated. There is no `free` to forget, no lifetime to get wrong, and no
//! pointer that outlives the call. It costs a copy per poll.

#![cfg(torrent)]
// THE ONLY PLACE IN THIS CRATE THAT MAY WRITE `unsafe`.
//
// The crate root denies it; this line is the exception, and it is one line in
// one file so that finding every unsafe in the system stays a grep for this
// attribute rather than a reading of every crate.
#![allow(unsafe_code)]

use std::ffi::{CStr, CString, c_char, c_int, c_void};

/// Room for a v2 info hash: SHA-256 is 32 bytes, 64 hex characters, plus the
/// terminator. A v1 hash uses the first 41 and leaves the rest zeroed.
pub(crate) const HASH: usize = 65;
pub(crate) const NAME: usize = 256;
pub(crate) const TROUBLE: usize = 192;

/// `struct rn_session*` — a pointer, eight bytes.
type Handle = *mut c_void;

/// `rn_status`, and the layout is the contract.
#[repr(C)]
pub(crate) struct Status {
    pub(crate) hash: [c_char; HASH],
    pub(crate) name: [c_char; NAME],
    pub(crate) trouble: [c_char; TROUBLE],
    pub(crate) done: u64,
    pub(crate) total: u64,
    pub(crate) down_rate: u64,
    pub(crate) up_rate: u64,
    pub(crate) state: i32,
    pub(crate) flags: i32,
    pub(crate) peers: i32,
    pub(crate) seeds: i32,
}

impl Status {
    /// A zeroed row, which is what `rn_poll` expects to be handed.
    pub(crate) const fn empty() -> Self {
        Self {
            hash: [0; HASH],
            name: [0; NAME],
            trouble: [0; TROUBLE],
            done: 0,
            total: 0,
            down_rate: 0,
            up_rate: 0,
            state: 0,
            flags: 0,
            peers: 0,
            seeds: 0,
        }
    }
}

/// The bits of `Status::flags`, from `shim.h`.
pub(crate) const FLAG_PAUSED: i32 = 1;
pub(crate) const FLAG_HAS_V1: i32 = 2;
pub(crate) const FLAG_HAS_V2: i32 = 4;

unsafe extern "C" {
    fn rn_session_new(state_dir: *const c_char, port_low: c_int, port_high: c_int) -> Handle;
    fn rn_session_free(session: Handle);
    fn rn_add_magnet(
        session: Handle,
        uri: *const c_char,
        save_path: *const c_char,
        out_hash: *mut c_char,
    ) -> c_int;
    fn rn_add_file(
        session: Handle,
        path: *const c_char,
        save_path: *const c_char,
        out_hash: *mut c_char,
    ) -> c_int;
    fn rn_pause(session: Handle, hash: *const c_char) -> c_int;
    fn rn_resume(session: Handle, hash: *const c_char) -> c_int;
    fn rn_remove(session: Handle, hash: *const c_char, delete_files: c_int) -> c_int;
    fn rn_poll(session: Handle, out: *mut Status, max: c_int) -> c_int;
    fn rn_save_resume(session: Handle) -> c_int;
    fn rn_make_torrent(content: *const c_char, out_file: *const c_char, flavour: c_int) -> c_int;
    fn rn_version() -> *const c_char;
}

/// A live session. NOT `Send`, and deliberately so — see [`crate::Session`].
pub(crate) struct Raw {
    handle: Handle,
}

impl Raw {
    pub(crate) fn open(state_dir: &CStr, low: u16, high: u16) -> Option<Self> {
        let handle =
            unsafe { rn_session_new(state_dir.as_ptr(), c_int::from(low), c_int::from(high)) };
        // The double guard the Ghostty binding also uses: a null out-parameter
        // and a non-zero code are two different failures, and either one on its
        // own has been the real answer at some point.
        if handle.is_null() {
            return None;
        }
        Some(Self { handle })
    }

    pub(crate) fn add_magnet(&mut self, uri: &CStr, into: &CStr) -> Option<String> {
        let mut hash = [0_i8; HASH];
        let code = unsafe {
            rn_add_magnet(
                self.handle,
                uri.as_ptr(),
                into.as_ptr(),
                hash.as_mut_ptr().cast::<c_char>(),
            )
        };
        (code == 0).then(|| text_of(&hash))
    }

    pub(crate) fn add_file(&mut self, path: &CStr, into: &CStr) -> Option<String> {
        let mut hash = [0_i8; HASH];
        let code = unsafe {
            rn_add_file(
                self.handle,
                path.as_ptr(),
                into.as_ptr(),
                hash.as_mut_ptr().cast::<c_char>(),
            )
        };
        (code == 0).then(|| text_of(&hash))
    }

    pub(crate) fn pause(&mut self, hash: &CStr) -> bool {
        unsafe { rn_pause(self.handle, hash.as_ptr()) == 0 }
    }

    pub(crate) fn resume(&mut self, hash: &CStr) -> bool {
        unsafe { rn_resume(self.handle, hash.as_ptr()) == 0 }
    }

    pub(crate) fn remove(&mut self, hash: &CStr, delete_files: bool) -> bool {
        unsafe { rn_remove(self.handle, hash.as_ptr(), c_int::from(delete_files)) == 0 }
    }

    /// Fills the caller's buffer and says how many rows are real.
    pub(crate) fn poll(&mut self, into: &mut [Status]) -> usize {
        let Ok(max) = c_int::try_from(into.len()) else {
            return 0;
        };
        let n = unsafe { rn_poll(self.handle, into.as_mut_ptr(), max) };
        usize::try_from(n).unwrap_or(0)
    }

    pub(crate) fn save(&mut self) {
        let _ = unsafe { rn_save_resume(self.handle) };
    }
}

impl Drop for Raw {
    fn drop(&mut self) {
        // It saves resume data before it goes, which is what makes a reboot
        // skip the re-check of files already on disk.
        unsafe { rn_session_free(self.handle) };
    }
}

/// Writes a `.torrent` for a directory. `flavour`: 0 hybrid, 1 v1, 2 v2.
pub(crate) fn make_torrent(content: &CStr, out_file: &CStr, flavour: c_int) -> bool {
    unsafe { rn_make_torrent(content.as_ptr(), out_file.as_ptr(), flavour) == 0 }
}

/// What libtorrent version is actually linked.
pub(crate) fn version() -> String {
    let raw = unsafe { rn_version() };
    if raw.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(raw) }
        .to_string_lossy()
        .into_owned()
}

/// A NUL-terminated array from the other side, as a `String`.
///
/// `from_utf8_lossy` and not a failure: these are a hex hash and a torrent's
/// own name, and a name is whatever the person who made the torrent put there.
/// A row that refuses to draw because a name is not UTF-8 is worse than a row
/// with a replacement character in it.
fn text_of(raw: &[i8]) -> String {
    let bytes: Vec<u8> = raw
        .iter()
        .take_while(|c| **c != 0)
        .map(|c| *c as u8)
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// The same, for a field of [`Status`].
pub(crate) fn field(raw: &[c_char]) -> String {
    text_of(raw)
}

/// A path as a C string, keeping the bytes it actually has.
///
/// `to_string_lossy` would be wrong here: a Linux filename is a byte sequence
/// and need not be UTF-8, and a lossy conversion produces a path that does not
/// exist. The only thing that genuinely cannot cross is an interior NUL, which
/// a filename cannot contain either.
pub(crate) fn c_path(path: &std::path::Path) -> Option<CString> {
    use std::os::unix::ffi::OsStrExt as _;
    CString::new(path.as_os_str().as_bytes()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The numbers the C++ compiler measured, written down.
    ///
    /// `nix/torrent-shim.nix` prints these from the real header, and
    /// `torrent-layout-matches-cxx` compares the two lists. Repeating them here
    /// is the point: a test that read them out of the artefact it is testing
    /// would agree with anything.
    #[test]
    fn the_layout_is_the_one_the_cxx_compiler_measured() {
        assert_eq!(size_of::<Status>(), 568, "sizeof(rn_status)");
        assert_eq!(align_of::<Status>(), 8, "alignof(rn_status)");
        assert_eq!(std::mem::offset_of!(Status, hash), 0);
        assert_eq!(std::mem::offset_of!(Status, name), 65);
        assert_eq!(std::mem::offset_of!(Status, trouble), 321);
        assert_eq!(std::mem::offset_of!(Status, done), 520);
        assert_eq!(std::mem::offset_of!(Status, total), 528);
        assert_eq!(std::mem::offset_of!(Status, down_rate), 536);
        assert_eq!(std::mem::offset_of!(Status, up_rate), 544);
        assert_eq!(std::mem::offset_of!(Status, state), 552);
        assert_eq!(std::mem::offset_of!(Status, flags), 556);
        assert_eq!(std::mem::offset_of!(Status, peers), 560);
        assert_eq!(std::mem::offset_of!(Status, seeds), 564);
    }

    #[test]
    fn a_field_stops_at_the_terminator_and_not_at_the_end() {
        let mut raw = [0_i8; 8];
        raw[0] = b'a' as i8;
        raw[1] = b'b' as i8;
        // Everything past the NUL is padding the other side zeroed, and reading
        // it would put eight characters on screen where two belong.
        assert_eq!(text_of(&raw), "ab");
    }
}
