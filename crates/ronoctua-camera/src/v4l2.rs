//! The V4L2 ioctls, and the only file in the tree that speaks them.
//!
//! # The `unsafe`, and what makes it sound
//!
//! Every call is `rustix::ioctl::ioctl` with an `Updater` over a `#[repr(C)]`
//! struct, and the soundness rests on ONE fact per struct: its size and field
//! offsets are the kernel's. `#[repr(C)]` promises to ARRANGE fields, never to
//! match a header (the trap AGENTS.md records from the libtorrent FFI), so the
//! sizes below are asserted at compile time AND compared against the C
//! compiler's `sizeof` over `linux/videodev2.h` by `v4l2-layout-matches-c`.
//! A wrong size is a kernel writing past the struct, and there is no symptom.
//!
//! # What is read, and why only read
//!
//! Capabilities, formats, frame sizes and intervals, controls and their menus
//! — and `S_CTRL`, the one write, which sets a control (brightness, exposure).
//! Streaming is NOT here: ffmpeg streams (`capture.rs`), and a second streamer
//! would be a second thing to be wrong about buffer queues.

use std::os::fd::AsFd;

use rustix::ioctl::{Opcode, Updater, ioctl, opcode};

/// `struct v4l2_capability`.
#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct Capability {
    pub driver: [u8; 16],
    pub card: [u8; 32],
    pub bus_info: [u8; 32],
    pub version: u32,
    pub capabilities: u32,
    pub device_caps: u32,
    pub reserved: [u32; 3],
}

/// `struct v4l2_fmtdesc`.
#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct FmtDesc {
    pub index: u32,
    pub kind: u32,
    pub flags: u32,
    pub description: [u8; 32],
    pub pixelformat: u32,
    pub mbus_code: u32,
    pub reserved: [u32; 3],
}

/// `struct v4l2_frmsizeenum`, with the union as six words: `discrete` is the
/// first two, `stepwise` all six.
#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct FrmSizeEnum {
    pub index: u32,
    pub pixel_format: u32,
    pub kind: u32,
    pub union: [u32; 6],
    pub reserved: [u32; 2],
}

/// `struct v4l2_frmivalenum`, the union as six words: `discrete` is one
/// fraction (numerator, denominator), `stepwise` three of them.
#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct FrmIvalEnum {
    pub index: u32,
    pub pixel_format: u32,
    pub width: u32,
    pub height: u32,
    pub kind: u32,
    pub union: [u32; 6],
    pub reserved: [u32; 2],
}

/// `struct v4l2_queryctrl`.
#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct QueryCtrl {
    pub id: u32,
    pub kind: u32,
    pub name: [u8; 32],
    pub minimum: i32,
    pub maximum: i32,
    pub step: i32,
    pub default_value: i32,
    pub flags: u32,
    pub reserved: [u32; 2],
}

/// `struct v4l2_querymenu` — PACKED in the header, which is why it is 44 and
/// not 48: the `__s64` in the union would otherwise align it.
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub(crate) struct QueryMenu {
    pub id: u32,
    pub index: u32,
    pub name: [u8; 32],
    pub reserved: u32,
}

/// `struct v4l2_control`.
#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct Control {
    pub id: u32,
    pub value: i32,
}

// THE SIZES, at compile time. The same numbers the C check prints.
const _: () = assert!(size_of::<Capability>() == 104);
const _: () = assert!(size_of::<FmtDesc>() == 64);
const _: () = assert!(size_of::<FrmSizeEnum>() == 44);
const _: () = assert!(size_of::<FrmIvalEnum>() == 52);
const _: () = assert!(size_of::<QueryCtrl>() == 68);
const _: () = assert!(size_of::<QueryMenu>() == 44);
const _: () = assert!(size_of::<Control>() == 8);

const QUERYCAP: Opcode = opcode::read::<Capability>(b'V', 0);
const ENUM_FMT: Opcode = opcode::read_write::<FmtDesc>(b'V', 2);
const G_CTRL: Opcode = opcode::read_write::<Control>(b'V', 27);
const S_CTRL: Opcode = opcode::read_write::<Control>(b'V', 28);
const QUERYCTRL: Opcode = opcode::read_write::<QueryCtrl>(b'V', 36);
const QUERYMENU: Opcode = opcode::read_write::<QueryMenu>(b'V', 37);
const ENUM_FRAMESIZES: Opcode = opcode::read_write::<FrmSizeEnum>(b'V', 74);
const ENUM_FRAMEINTERVALS: Opcode = opcode::read_write::<FrmIvalEnum>(b'V', 75);

/// `V4L2_BUF_TYPE_VIDEO_CAPTURE`.
pub(crate) const BUF_TYPE_CAPTURE: u32 = 1;
/// `V4L2_CAP_VIDEO_CAPTURE`.
pub(crate) const CAP_CAPTURE: u32 = 0x0000_0001;
/// `V4L2_CAP_VIDEO_CAPTURE_MPLANE`.
pub(crate) const CAP_CAPTURE_MPLANE: u32 = 0x0000_1000;
/// `V4L2_CAP_DEVICE_CAPS`: `device_caps` is filled in.
pub(crate) const CAP_DEVICE_CAPS: u32 = 0x8000_0000;
/// `V4L2_FMT_FLAG_COMPRESSED`.
pub(crate) const FMT_COMPRESSED: u32 = 0x0001;
/// `V4L2_FRMSIZE_TYPE_DISCRETE`.
pub(crate) const FRMSIZE_DISCRETE: u32 = 1;
/// `V4L2_FRMIVAL_TYPE_DISCRETE`.
pub(crate) const FRMIVAL_DISCRETE: u32 = 1;
/// `V4L2_CTRL_FLAG_NEXT_CTRL`.
pub(crate) const NEXT_CTRL: u32 = 0x8000_0000;
/// `V4L2_CTRL_FLAG_DISABLED`.
pub(crate) const CTRL_DISABLED: u32 = 0x0001;
/// `V4L2_CTRL_FLAG_READ_ONLY`.
pub(crate) const CTRL_READ_ONLY: u32 = 0x0004;
/// `V4L2_CTRL_FLAG_INACTIVE`.
pub(crate) const CTRL_INACTIVE: u32 = 0x0010;

fn zeroed<T: Copy>() -> T {
    // SAFETY: every struct above is plain integers and byte arrays, for which
    // all-zero is a valid value — and it is what the kernel expects of the
    // fields a caller does not set.
    unsafe { std::mem::zeroed() }
}

/// `VIDIOC_QUERYCAP`.
pub(crate) fn query_cap(fd: impl AsFd) -> rustix::io::Result<Capability> {
    let mut v: Capability = zeroed();
    // SAFETY: QUERYCAP writes a `v4l2_capability`, whose layout is asserted above.
    unsafe { ioctl(fd, Updater::<QUERYCAP, Capability>::new(&mut v))? };
    Ok(v)
}

/// `VIDIOC_ENUM_FMT` for index `n`; `None` past the last.
pub(crate) fn enum_fmt(fd: impl AsFd, n: u32) -> Option<FmtDesc> {
    let mut v: FmtDesc = zeroed();
    v.index = n;
    v.kind = BUF_TYPE_CAPTURE;
    // SAFETY: ENUM_FMT reads and writes a `v4l2_fmtdesc`, layout asserted above.
    unsafe { ioctl(fd, Updater::<ENUM_FMT, FmtDesc>::new(&mut v)) }.ok()?;
    Some(v)
}

/// `VIDIOC_ENUM_FRAMESIZES`.
pub(crate) fn enum_sizes(fd: impl AsFd, fourcc: u32, n: u32) -> Option<FrmSizeEnum> {
    let mut v: FrmSizeEnum = zeroed();
    v.index = n;
    v.pixel_format = fourcc;
    // SAFETY: as above, `v4l2_frmsizeenum`.
    unsafe { ioctl(fd, Updater::<ENUM_FRAMESIZES, FrmSizeEnum>::new(&mut v)) }.ok()?;
    Some(v)
}

/// `VIDIOC_ENUM_FRAMEINTERVALS`.
pub(crate) fn enum_intervals(
    fd: impl AsFd,
    fourcc: u32,
    width: u32,
    height: u32,
    n: u32,
) -> Option<FrmIvalEnum> {
    let mut v: FrmIvalEnum = zeroed();
    v.index = n;
    v.pixel_format = fourcc;
    v.width = width;
    v.height = height;
    // SAFETY: as above, `v4l2_frmivalenum`.
    unsafe { ioctl(fd, Updater::<ENUM_FRAMEINTERVALS, FrmIvalEnum>::new(&mut v)) }.ok()?;
    Some(v)
}

/// `VIDIOC_QUERYCTRL` with `NEXT_CTRL`: the control after `after`.
pub(crate) fn next_ctrl(fd: impl AsFd, after: u32) -> Option<QueryCtrl> {
    let mut v: QueryCtrl = zeroed();
    v.id = after | NEXT_CTRL;
    // SAFETY: as above, `v4l2_queryctrl`.
    unsafe { ioctl(fd, Updater::<QUERYCTRL, QueryCtrl>::new(&mut v)) }.ok()?;
    Some(v)
}

/// `VIDIOC_QUERYMENU`: a menu item's name; `None` for a gap (drivers skip
/// indices, as the measured `auto_exposure` does: 1 and 3, not 0 and 2).
pub(crate) fn query_menu(fd: impl AsFd, id: u32, index: u32) -> Option<QueryMenu> {
    let mut v: QueryMenu = zeroed();
    v.id = id;
    v.index = index;
    // SAFETY: as above, the packed `v4l2_querymenu`.
    unsafe { ioctl(fd, Updater::<QUERYMENU, QueryMenu>::new(&mut v)) }.ok()?;
    Some(v)
}

/// `VIDIOC_G_CTRL`.
pub(crate) fn get_ctrl(fd: impl AsFd, id: u32) -> Option<i32> {
    let mut v = Control { id, value: 0 };
    // SAFETY: as above, `v4l2_control`.
    unsafe { ioctl(fd, Updater::<G_CTRL, Control>::new(&mut v)) }.ok()?;
    Some(v.value)
}

/// `VIDIOC_S_CTRL` — the one write this file makes.
pub(crate) fn set_ctrl(fd: impl AsFd, id: u32, value: i32) -> rustix::io::Result<()> {
    let mut v = Control { id, value };
    // SAFETY: as above, `v4l2_control`.
    unsafe { ioctl(fd, Updater::<S_CTRL, Control>::new(&mut v)) }
}

/// A NUL-padded byte array to a string, lossily.
pub(crate) fn text(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opcodes_are_the_headers() {
        // The numbers `videodev2.h` produces on x86_64 and aarch64, printed by
        // a C compiler on 2026-09-23 — a second witness beside the sizes.
        assert_eq!(QUERYCAP, 0x8068_5600);
        assert_eq!(ENUM_FMT, 0xc040_5602);
        assert_eq!(QUERYCTRL, 0xc044_5624);
        assert_eq!(QUERYMENU, 0xc02c_5625);
        assert_eq!(G_CTRL, 0xc008_561b);
        assert_eq!(S_CTRL, 0xc008_561c);
        assert_eq!(ENUM_FRAMESIZES, 0xc02c_564a);
        assert_eq!(ENUM_FRAMEINTERVALS, 0xc034_564b);
    }

    #[test]
    fn a_padded_name_reads_without_its_nuls() {
        let mut b = [0u8; 16];
        b[..5].copy_from_slice(b"uvcvi");
        assert_eq!(text(&b), "uvcvi");
    }
}
