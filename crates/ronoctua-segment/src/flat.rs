//! The FlatBuffer reader, and only as much of it as a TFLite model needs.
//!
//! # The format, in the five facts this file relies on
//!
//! 1. The buffer starts with a `u32` offset to the ROOT table.
//! 2. A table starts with an `i32` that is SUBTRACTED from the table's position
//!    to find its vtable: `vtable = table - soffset`.
//! 3. A vtable is `u16 vtable_size, u16 table_size`, then one `u16` per field,
//!    the field's offset from the TABLE start — `0` meaning absent, and a slot
//!    past `vtable_size` meaning absent too (a newer writer, an older field).
//! 4. A reference field (string, vector, table) holds a `u32` that is added to
//!    THE FIELD'S OWN position, never the table's.
//! 5. A vector is a `u32` length then the elements; a vector of tables holds
//!    one `u32` per element, each relative to the element's own position.
//!
//! Every read is bounds-checked and answers `None` rather than panicking: the
//! model is a file on disk, and a truncated one must be a refusal, not a crash
//! of the process that owns a camera.

/// A position inside the buffer, with the buffer.
#[derive(Clone, Copy)]
pub(crate) struct Table<'a> {
    bytes: &'a [u8],
    at: usize,
}

fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

fn i32_at(b: &[u8], at: usize) -> Option<i32> {
    Some(i32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// The root table of a buffer.
pub(crate) fn root(bytes: &[u8]) -> Option<Table<'_>> {
    let at = u32_at(bytes, 0)? as usize;
    (at < bytes.len()).then_some(Table { bytes, at })
}

impl<'a> Table<'a> {
    /// Where field `id` lives, or `None` when the writer left it out.
    fn field(&self, id: usize) -> Option<usize> {
        let soffset = i32_at(self.bytes, self.at)?;
        let vtable = usize::try_from(self.at as i64 - i64::from(soffset)).ok()?;
        let size = usize::from(u16_at(self.bytes, vtable)?);
        let slot = 4 + 2 * id;
        if slot + 2 > size {
            return None;
        }
        let off = usize::from(u16_at(self.bytes, vtable + slot)?);
        (off != 0).then_some(self.at + off)
    }

    /// A scalar byte field, or its schema default.
    pub(crate) fn u8(&self, id: usize, default: u8) -> u8 {
        self.field(id)
            .and_then(|at| self.bytes.get(at).copied())
            .unwrap_or(default)
    }

    /// A scalar `i32` field, or its schema default.
    pub(crate) fn i32(&self, id: usize, default: i32) -> i32 {
        self.field(id)
            .and_then(|at| i32_at(self.bytes, at))
            .unwrap_or(default)
    }

    /// A scalar `u32` field, or its schema default.
    pub(crate) fn u32(&self, id: usize, default: u32) -> u32 {
        self.field(id)
            .and_then(|at| u32_at(self.bytes, at))
            .unwrap_or(default)
    }

    /// A scalar boolean field, or its schema default.
    pub(crate) fn bool(&self, id: usize, default: bool) -> bool {
        self.field(id)
            .and_then(|at| self.bytes.get(at).copied())
            .map_or(default, |b| b != 0)
    }

    /// Where a reference field points (fact 4).
    fn target(&self, id: usize) -> Option<usize> {
        let at = self.field(id)?;
        let rel = u32_at(self.bytes, at)? as usize;
        let to = at.checked_add(rel)?;
        (to < self.bytes.len()).then_some(to)
    }

    /// A sub-table field.
    pub(crate) fn table(&self, id: usize) -> Option<Table<'a>> {
        Some(Table {
            bytes: self.bytes,
            at: self.target(id)?,
        })
    }

    /// A string field.
    pub(crate) fn str(&self, id: usize) -> Option<&'a str> {
        std::str::from_utf8(self.bytes_of(id)?).ok()
    }

    /// A `[ubyte]` field (also what a string is underneath).
    pub(crate) fn bytes_of(&self, id: usize) -> Option<&'a [u8]> {
        let at = self.target(id)?;
        let len = u32_at(self.bytes, at)? as usize;
        self.bytes.get(at + 4..at + 4 + len)
    }

    /// An `[int]` field.
    pub(crate) fn ints(&self, id: usize) -> Option<Vec<i32>> {
        let at = self.target(id)?;
        let len = u32_at(self.bytes, at)? as usize;
        (0..len)
            .map(|i| i32_at(self.bytes, at + 4 + 4 * i))
            .collect()
    }

    /// A `[Table]` field (fact 5).
    pub(crate) fn tables(&self, id: usize) -> Option<Vec<Table<'a>>> {
        let at = self.target(id)?;
        let len = u32_at(self.bytes, at)? as usize;
        (0..len)
            .map(|i| {
                let slot = at + 4 + 4 * i;
                let to = slot.checked_add(u32_at(self.bytes, slot)? as usize)?;
                (to < self.bytes.len()).then_some(Table {
                    bytes: self.bytes,
                    at: to,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A hand-assembled buffer: root → table with one `i32` field (id 1) and
    /// field 0 absent. Proves facts 1–3 without a model on disk.
    #[test]
    fn a_hand_built_table_reads_back() {
        let mut b = vec![0u8; 32];
        // root offset → table at 16
        b[0..4].copy_from_slice(&16u32.to_le_bytes());
        // vtable at 8: size 8 (two slots), table size 8, field0 = 0, field1 = 4
        b[8..10].copy_from_slice(&8u16.to_le_bytes());
        b[10..12].copy_from_slice(&8u16.to_le_bytes());
        b[12..14].copy_from_slice(&0u16.to_le_bytes());
        b[14..16].copy_from_slice(&4u16.to_le_bytes());
        // table at 16: soffset = 16 - 8 = 8
        b[16..20].copy_from_slice(&8i32.to_le_bytes());
        b[20..24].copy_from_slice(&(-7i32).to_le_bytes());
        let t = root(&b).expect("root");
        assert_eq!(t.i32(1, 0), -7);
        assert_eq!(t.i32(0, 42), 42, "an absent field is its default");
        assert_eq!(t.i32(5, 3), 3, "a slot past the vtable is absent too");
    }

    #[test]
    fn a_truncated_buffer_is_refused_and_not_a_panic() {
        assert!(root(&[]).is_none());
        assert!(root(&[200, 0, 0, 0]).is_none());
    }
}
