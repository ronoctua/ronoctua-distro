//! The Ghostty engine, over `libghostty-vt`'s C API.
//!
//! # What this is for
//!
//! A second implementation of [`Engine`], so `term-differential` compares two
//! engines instead of comparing one against a golden file. The difference is the
//! whole point: a golden file catches OUR regressions; two engines disagreeing
//! catches a wrong reading of the VT specification.
//!
//! # The whole surface, in four calls
//!
//! `ghostty_terminal_new` and `_free` for the lifetime, `_vt_write` to feed,
//! `_resize` to resize, and three chained calls to read one cell: `_grid_ref`
//! gives a reference at a point, `ghostty_grid_ref_cell` turns it into a cell,
//! and `ghostty_cell_get` reads the codepoint out of it.
//!
//! # EVERY LAYOUT HERE WAS MEASURED, NOT INFERRED
//!
//! This binding was written once from the headers and it compiled, linked and
//! returned a blank grid. Three of its struct layouts were wrong, and none of
//! the three produces a diagnostic — on a 64-bit ABI a wrong integer width and a
//! wrong struct size are both just registers with the wrong contents.
//!
//! What found them was a twenty-line C program: the C compiler checks these
//! shapes, and `#[repr(C)]` can only promise to lay out what it was told. The
//! sizes below come from `sizeof` in that program, and the test at the bottom
//! keeps them true.
//!
//! # It only exists when the library does
//!
//! The module is behind `cfg(ghostty_vt)`, which `build.rs` emits when
//! `RONOCTUA_GHOSTTY_VT` is set. A `cargo check` without the Nix build compiles
//! the crate with the alacritty engine alone rather than failing at link time in
//! whatever binary pulled it in.
#![cfg(ghostty_vt)]
// THE ONLY PLACE IN THE PROJECT THAT MAY WRITE `unsafe`.
//
// The crate root denies it; this line is the exception, and it is one line in
// one file so that finding every unsafe in the system is a grep for this
// attribute rather than a reading of every crate.
#![allow(unsafe_code)]

use std::ffi::c_void;

use crate::{CellBox, Engine, Grid};

/// `struct GhosttyTerminalImpl*` — a pointer, eight bytes.
type GhosttyTerminal = *mut c_void;

/// `GhosttyCell` is a `uint64_t`, and NOT a pointer.
///
/// Declaring it as a pointer costs nothing on a 64-bit ABI — same width, same
/// register — so nothing crashes and nothing warns. What arrives on the other
/// side is a number read as an address.
type GhosttyCell = u64;

/// Zero is success. The rest are failures this harness cannot act on
/// differently: whatever went wrong, the answer is to leave the cell empty.
type GhosttyResult = i32;

/// `GHOSTTY_POINT_TAG_ACTIVE` — the area where the cursor moves.
///
/// Not `SCREEN` and not `HISTORY`: the canonical [`Grid`] is what is on screen
/// now, and scrollback would make the two engines disagree about size before
/// they could disagree about content. Read from the declaration order in
/// `point.h`, not from a sorted grep.
const POINT_TAG_ACTIVE: u32 = 0;

/// `GHOSTTY_CELL_DATA_CODEPOINT`, likewise read in declaration order.
const CELL_DATA_CODEPOINT: u32 = 1;

/// `GHOSTTY_TERMINAL_DATA_CURSOR_X` and `_CURSOR_Y`.
///
/// The cursor is NOT part of the grid on this side: it is asked of the terminal,
/// not read out of a cell. Leaving it at the origin made the differential
/// disagree on every case that moves the cursor — fifteen of fifteen — which
/// reads as two engines that agree about nothing rather than as one engine that
/// was never asked.
const TERMINAL_DATA_CURSOR_X: u32 = 3;
const TERMINAL_DATA_CURSOR_Y: u32 = 4;

/// `GhosttyPointCoordinate`.
#[repr(C)]
#[derive(Clone, Copy)]
struct PointCoordinate {
    x: u32,
    y: u32,
}

/// `GhosttyPointValue` — a UNION, and this is where the first binding went wrong.
///
/// It is sixteen bytes, not eight: the coordinate uses the first half and the
/// rest is padding upstream reserved for ABI compatibility. Declaring the point
/// as a flat `{ tag, x, y }` gives twelve bytes where the library expects
/// twenty-four, so every field after the tag arrives shifted — and the symptom
/// is not a crash, it is a grid that comes back blank.
#[repr(C)]
union PointValue {
    coordinate: PointCoordinate,
    _padding: [u64; 2],
}

/// `GhosttyPoint` — twenty-four bytes.
#[repr(C)]
struct Point {
    tag: u32,
    value: PointValue,
}

impl Point {
    /// A point in the active area.
    const fn active(x: u32, y: u32) -> Self {
        Self {
            tag: POINT_TAG_ACTIVE,
            value: PointValue {
                coordinate: PointCoordinate { x, y },
            },
        }
    }
}

/// `GhosttyGridRef` — twenty-four bytes, and a SIZED STRUCT.
///
/// The caller fills `size` with `sizeof` before the call; it is how upstream
/// versions the layout without breaking callers. A zeroed struct claims to be
/// zero bytes long, the library believes it, and the result is a `SIGSEGV` with
/// no message.
#[repr(C)]
struct GridRef {
    size: usize,
    node: *mut c_void,
    x: u16,
    y: u16,
}

impl GridRef {
    /// The only valid starting state: zeroed, with `size` filled in.
    const fn empty() -> Self {
        Self {
            size: size_of::<Self>(),
            node: std::ptr::null_mut(),
            x: 0,
            y: 0,
        }
    }
}

unsafe extern "C" {
    fn ghostty_terminal_new(
        allocator: *const c_void,
        out: *mut GhosttyTerminal,
        cols: u16,
        rows: u16,
    ) -> GhosttyResult;
    fn ghostty_terminal_free(terminal: GhosttyTerminal);
    fn ghostty_terminal_vt_write(terminal: GhosttyTerminal, data: *const u8, len: usize);
    fn ghostty_terminal_resize(
        terminal: GhosttyTerminal,
        cols: u16,
        rows: u16,
        cell_width_px: u32,
        cell_height_px: u32,
    ) -> GhosttyResult;
    fn ghostty_terminal_grid_ref(
        terminal: GhosttyTerminal,
        point: Point,
        out: *mut GridRef,
    ) -> GhosttyResult;
    fn ghostty_grid_ref_cell(reference: *const GridRef, out: *mut GhosttyCell) -> GhosttyResult;
    fn ghostty_cell_get(cell: GhosttyCell, data: u32, out: *mut c_void) -> GhosttyResult;
    fn ghostty_terminal_get(
        terminal: GhosttyTerminal,
        data: u32,
        out: *mut c_void,
    ) -> GhosttyResult;
}

/// The Ghostty engine.
pub struct Ghostty {
    terminal: GhosttyTerminal,
    columns: u16,
    lines: u16,
}

impl Ghostty {
    /// Brings a terminal up.
    ///
    /// # Errors
    ///
    /// The library refused to allocate one.
    pub fn fresh(columns: u16, lines: u16) -> Result<Self, &'static str> {
        let mut terminal: GhosttyTerminal = std::ptr::null_mut();
        // A NULL allocator asks the library for its default one, which the
        // header documents. Handing it a Rust allocator would mean matching
        // Zig's alignment contract for a harness that allocates one terminal.
        let result =
            unsafe { ghostty_terminal_new(std::ptr::null(), &raw mut terminal, columns, lines) };
        if result != 0 || terminal.is_null() {
            return Err("libghostty-vt recusou criar o terminal");
        }
        Ok(Self {
            terminal,
            columns,
            lines,
        })
    }

    /// One `uint32_t` field of the terminal, or zero when it cannot be read.
    fn field(&self, data: u32) -> u32 {
        let mut out: u32 = 0;
        if unsafe { ghostty_terminal_get(self.terminal, data, (&raw mut out).cast::<c_void>()) }
            != 0
        {
            return 0;
        }
        out
    }

    /// The codepoint at a position, or `None` where there is nothing.
    fn codepoint_at(&self, x: u16, y: u16) -> Option<char> {
        let point = Point::active(u32::from(x), u32::from(y));
        let mut reference = GridRef::empty();
        if unsafe { ghostty_terminal_grid_ref(self.terminal, point, &raw mut reference) } != 0 {
            return None;
        }
        let mut cell: GhosttyCell = 0;
        if unsafe { ghostty_grid_ref_cell(&raw const reference, &raw mut cell) } != 0 {
            return None;
        }
        let mut codepoint: u32 = 0;
        if unsafe {
            ghostty_cell_get(
                cell,
                CELL_DATA_CODEPOINT,
                (&raw mut codepoint).cast::<c_void>(),
            )
        } != 0
        {
            return None;
        }
        char::from_u32(codepoint)
    }
}

impl Drop for Ghostty {
    fn drop(&mut self) {
        unsafe { ghostty_terminal_free(self.terminal) };
    }
}

impl Engine for Ghostty {
    fn name(&self) -> &'static str {
        "ghostty"
    }

    fn feed(&mut self, bytes: &[u8]) {
        unsafe { ghostty_terminal_vt_write(self.terminal, bytes.as_ptr(), bytes.len()) };
    }

    fn grid(&self) -> Grid {
        let mut grid = Grid::fresh_one(self.columns, self.lines);
        for y in 0..self.lines {
            for x in 0..self.columns {
                if let Some(c) = self.codepoint_at(x, y)
                    && c != '\0'
                {
                    grid.put_at(y, x, CellBox { c });
                }
            }
        }
        grid.cursor = (
            u16::try_from(self.field(TERMINAL_DATA_CURSOR_Y)).unwrap_or(0),
            u16::try_from(self.field(TERMINAL_DATA_CURSOR_X)).unwrap_or(0),
        );
        grid
    }

    fn resize(&mut self, columns: u16, lines: u16) {
        // THE CELL SIZE IN PIXELS IS NOT OURS TO KNOW, and the differential
        // never asks: it exists so the library can answer questions about pixel
        // geometry that this harness does not put. Any non-zero pair does.
        let result = unsafe { ghostty_terminal_resize(self.terminal, columns, lines, 1, 1) };
        if result == 0 {
            self.columns = columns;
            self.lines = lines;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_layouts_are_the_ones_the_c_compiler_measured() {
        // THESE FOUR NUMBERS ARE THE BINDING. Every one of them was wrong on the
        // first attempt, and not one produced a diagnostic — a wrong size is
        // just a register with the wrong contents. They come from `sizeof` in a
        // C program compiled against the real headers.
        assert_eq!(size_of::<Point>(), 24, "GhosttyPoint");
        assert_eq!(size_of::<GridRef>(), 24, "GhosttyGridRef");
        assert_eq!(size_of::<GhosttyCell>(), 8, "GhosttyCell");
        assert_eq!(size_of::<GhosttyTerminal>(), 8, "GhosttyTerminal");
    }

    #[test]
    fn it_writes_two_letters_and_reads_them_back() {
        // THE TEST THAT DECIDES EVERYTHING ELSE. If the binding is wrong — the
        // wrong tag, the wrong enum value, a struct of the wrong shape — it
        // shows here, on two characters, instead of showing up as two engines
        // mysteriously disagreeing over a whole screen.
        let mut engine = Ghostty::fresh(10, 2).expect("o terminal tem de subir");
        engine.feed(b"hi");
        let grid = engine.grid();
        assert_eq!(grid.cell(0, 0).c, 'h', "grade: {grid:?}");
        assert_eq!(grid.cell(0, 1).c, 'i', "grade: {grid:?}");
    }
}
