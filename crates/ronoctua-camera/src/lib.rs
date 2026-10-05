//! The one boundary with V4L2: which cameras exist, what they can do, their
//! controls, and the one door that opens one for frames.
//!
//! # What a camera IS here
//!
//! A node under `/sys/class/video4linux` whose `device_caps` says it captures
//! video. That rule is the whole of "generic": one USB webcam measured on
//! 2026-09-23 exposed FOUR nodes — colour, infrared, and two of metadata, all
//! with the same card name and bus — and the capability bit is the only thing
//! that tells a capture node from a metadata one on every driver. Nothing in
//! this crate names a vendor, a model or a laptop.
//!
//! # And which one is WHICH
//!
//! [`Identity`] is `card` + `bus_info` + the node's INTERFACE (the last part
//! of its sysfs device path — `3-1:1.0` and `3-1:1.2` are the colour and the
//! infrared of one USB camera). `/dev/videoN` is never an identity: the kernel
//! hands the numbers out in plug order, and a setting saved for `video0` would
//! land on whatever took that number next.
//!
//! # Kinds
//!
//! A camera whose every format is greyscale is [`Kind::Infrared`] — the face
//! login sensor, not something a person films with — and is never chosen as
//! the default on its own. A node our own `v4l2loopback` owns is
//! [`Kind::Virtual`] and is NEVER a source: filming our own output is a
//! feedback loop. Any OTHER loopback (a phone app, `v4l2-relayd` for a MIPI
//! camera, OBS) is a camera like any other — that is how cameras only a
//! vendor HAL can drive reach this system at all.

// DENY and not forbid: `v4l2.rs` is the one module allowed to hold `unsafe`,
// and `forbid` cannot be lifted for a module.
#![deny(unsafe_code)]

pub mod capture;
pub mod frames;
pub mod prefs;
pub mod watch;

#[allow(unsafe_code)]
mod v4l2;

use std::{
    fs::{self, File, OpenOptions},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

/// The card label our own virtual camera carries (`nix/modules/studio.nix`
/// sets it on the module), which is how it is told apart from every other
/// loopback.
pub const OUR_LABEL: &str = "ronoctua";

/// Where the kernel lists video devices.
pub const SYS: &str = "/sys/class/video4linux";

/// Which camera this is, in the device's own words.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Identity(pub String);

impl Identity {
    /// `card|bus|interface`, the three parts the kernel reports.
    pub fn of(card: &str, bus: &str, interface: &str) -> Self {
        Self(format!("{card}|{bus}|{interface}"))
    }

    /// The card part, which is what a person reads as the camera's name.
    pub fn card(&self) -> &str {
        self.0.split('|').next().unwrap_or(&self.0)
    }
}

/// What kind of camera a node is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    /// A colour camera.
    Colour,
    /// Every format it declares is greyscale: an infrared sensor.
    Infrared,
    /// Our own loopback — the studio's output, never an input.
    Virtual,
}

/// A frame rate as the device states it, `numerator / denominator` SECONDS
/// per frame (V4L2's interval), kept exact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Interval {
    /// Seconds' numerator.
    pub num: u32,
    /// Seconds' denominator.
    pub den: u32,
}

impl Interval {
    /// Frames per second, rounded to a hundredth.
    pub fn fps(&self) -> f32 {
        if self.num == 0 {
            0.0
        } else {
            self.den as f32 / self.num as f32
        }
    }
}

/// One frame size and the rates it runs at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Size {
    /// Pixels across.
    pub width: u32,
    /// Pixels down.
    pub height: u32,
    /// Discrete rates, fastest first.
    pub intervals: Vec<Interval>,
}

/// One pixel format.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Format {
    /// The four characters, e.g. `MJPG`, `YUYV`, `NV12`, `H264`, `GREY`.
    pub fourcc: String,
    /// The driver's own description.
    pub description: String,
    /// Compressed on the wire (MJPEG, H.264): cheap on USB bandwidth, which is
    /// what lets a USB 2 camera give 1080p at 30.
    pub compressed: bool,
    /// Discrete sizes, largest first. A stepwise device lists a ladder of
    /// common sizes inside its range — see [`ladder`].
    pub sizes: Vec<Size>,
}

/// A control the driver declares.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Control {
    /// V4L2 id.
    pub id: u32,
    /// The driver's name, e.g. `Brightness`.
    pub name: String,
    /// A stable key made of the name (`brightness`), which is what the
    /// preferences file stores — ids differ between drivers for private
    /// controls, names in lower kebab-case do not.
    pub key: String,
    /// What kind of value.
    pub kind: ControlKind,
    /// Smallest value.
    pub min: i32,
    /// Largest value.
    pub max: i32,
    /// Step.
    pub step: i32,
    /// Factory value.
    pub default: i32,
    /// Current value.
    pub value: i32,
    /// Another control decides it right now (a manual exposure while auto
    /// exposure is on): drawn, and refusing.
    pub inactive: bool,
    /// The driver will not take a write (a hardware privacy shutter).
    pub read_only: bool,
    /// Menu items as `(value, name)`, for a menu control.
    pub menu: Vec<(i32, String)>,
}

/// The kinds of control a person can turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControlKind {
    /// A number in a range.
    Integer,
    /// On or off.
    Boolean,
    /// One of named choices.
    Menu,
    /// One of integers (`INTEGER_MENU`), drawn as the numbers.
    IntegerMenu,
}

/// One camera.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Camera {
    /// Who it is.
    pub identity: Identity,
    /// Where it is right now.
    pub node: PathBuf,
    /// The card name.
    pub card: String,
    /// The driver, e.g. `uvcvideo`.
    pub driver: String,
    /// The bus, e.g. `usb-0000:c4:00.4-1`.
    pub bus: String,
    /// What kind.
    pub kind: Kind,
    /// Its formats, in the driver's order.
    pub formats: Vec<Format>,
    /// Its controls.
    pub controls: Vec<Control>,
    /// Whether ffmpeg can stream it. A multi-planar node (some SoC and MIPI
    /// pipelines) is listed but cannot be opened by the V4L2 input ffmpeg has
    /// — it is drawn, and refusing, with the reason.
    pub streamable: bool,
}

impl Camera {
    /// The privacy shutter's state, when the device has one: `Some(true)`
    /// means the lens is covered.
    pub fn shuttered(&self) -> Option<bool> {
        self.controls
            .iter()
            .find(|c| c.key == "privacy")
            .map(|c| c.value != 0)
    }
}

/// A mode to open a camera in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mode {
    /// The pixel format's four characters.
    pub fourcc: String,
    /// Pixels across.
    pub width: u32,
    /// Pixels down.
    pub height: u32,
    /// The rate.
    pub interval: Interval,
}

impl Mode {
    /// Frames per second.
    pub fn fps(&self) -> f32 {
        self.interval.fps()
    }
}

/// ffmpeg's `-input_format` for a fourcc, and `None` for one it has no
/// name for — such a format is drawn and not chosen.
pub fn input_format(fourcc: &str) -> Option<&'static str> {
    Some(match fourcc {
        "MJPG" | "JPEG" => "mjpeg",
        "H264" => "h264",
        "YUYV" => "yuyv422",
        "UYVY" => "uyvy422",
        "YVYU" => "yvyu422",
        "NV12" => "nv12",
        "NV21" => "nv21",
        "YU12" => "yuv420p",
        "YV12" => "yuv420p",
        "422P" => "yuv422p",
        "GREY" => "gray",
        "Y16 " => "gray16le",
        "RGB3" => "rgb24",
        "BGR3" => "bgr24",
        "RGBP" => "rgb565le",
        "BA81" => "bayer_bggr8",
        "GBRG" => "bayer_gbrg8",
        "GRBG" => "bayer_grbg8",
        "RGGB" => "bayer_rggb8",
        _ => return None,
    })
}

fn is_grey(fourcc: &str) -> bool {
    matches!(fourcc, "GREY" | "Y10 " | "Y12 " | "Y16 " | "Y8I " | "Y12I")
}

/// The four characters of a V4L2 pixel format code.
pub fn fourcc_text(code: u32) -> String {
    code.to_le_bytes().iter().map(|b| char::from(*b)).collect()
}

/// A ladder of common sizes inside a stepwise range — what a device that
/// answers with a range rather than a list is offered as.
pub fn ladder(min: (u32, u32), max: (u32, u32)) -> Vec<(u32, u32)> {
    const COMMON: [(u32, u32); 9] = [
        (3840, 2160),
        (2560, 1440),
        (1920, 1080),
        (1280, 720),
        (1024, 576),
        (960, 540),
        (800, 600),
        (640, 480),
        (320, 240),
    ];
    let mut out: Vec<(u32, u32)> = COMMON
        .into_iter()
        .filter(|(w, h)| *w >= min.0 && *w <= max.0 && *h >= min.1 && *h <= max.1)
        .collect();
    if out.is_empty() {
        out.push(max);
    }
    out
}

/// The mode a camera opens in when nobody chose one — and the mode a chosen
/// one falls back to when the device no longer offers it.
///
/// # The rule, which is the whole of "works on any camera"
///
/// 1. Only formats ffmpeg can name.
/// 2. At least 24 frames a second if ANY mode reaches it — a person reads a
///    stutter before they read a resolution.
/// 3. Among those, the largest area up to 1080p. Above that the decode and the
///    effects cost more than a call or a recording needs; a larger mode is
///    still one key away in the settings.
/// 4. Compressed before raw at the same size: a USB 2 camera gives 1080p30
///    only as MJPEG (measured: this laptop's YUYV is 5 fps at 1080p).
/// 5. The faster rate.
///
/// A saved `wanted` wins when the device still offers exactly it.
pub fn choose_mode(camera: &Camera, wanted: Option<&Mode>) -> Option<Mode> {
    let mut all = Vec::new();
    for f in &camera.formats {
        if input_format(&f.fourcc).is_none() {
            continue;
        }
        for s in &f.sizes {
            for i in &s.intervals {
                all.push((f, s, *i));
            }
        }
    }
    if let Some(w) = wanted
        && all.iter().any(|(f, s, i)| {
            f.fourcc == w.fourcc && s.width == w.width && s.height == w.height && *i == w.interval
        })
    {
        return Some(w.clone());
    }
    let smooth = all.iter().any(|(_, _, i)| i.fps() >= 23.9);
    all.into_iter()
        .filter(|(_, _, i)| !smooth || i.fps() >= 23.9)
        .max_by(|a, b| {
            let area = |s: &Size| u64::from(s.width) * u64::from(s.height);
            let capped = |s: &Size| area(s).min(1920 * 1080);
            capped(a.1)
                .cmp(&capped(b.1))
                .then(a.0.compressed.cmp(&b.0.compressed))
                .then(area(b.1).cmp(&area(a.1)))
                .then(a.2.fps().total_cmp(&b.2.fps()))
        })
        .map(|(f, s, i)| Mode {
            fourcc: f.fourcc.clone(),
            width: s.width,
            height: s.height,
            interval: i,
        })
}

/// The camera a person gets when nobody chose one: the first colour camera
/// that streams, in node order — never an infrared sensor or our own output.
pub fn default_camera<'a>(cameras: &'a [Camera], chosen: Option<&Identity>) -> Option<&'a Camera> {
    if let Some(c) = chosen.and_then(|id| {
        cameras
            .iter()
            .find(|c| &c.identity == id && c.streamable && c.kind != Kind::Virtual)
    }) {
        return Some(c);
    }
    cameras
        .iter()
        .find(|c| c.kind == Kind::Colour && c.streamable)
}

fn open(node: &Path) -> std::io::Result<File> {
    // NON-BLOCKING: a query must never wait on a device that is streaming for
    // someone else. Read-write because `S_CTRL` is conventionally asked of a
    // writable descriptor; read-only is the fallback for a node whose mode
    // gives only that, where queries still work.
    OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(rustix::fs::OFlags::NONBLOCK.bits() as i32)
        .open(node)
        .or_else(|_| {
            OpenOptions::new()
                .read(true)
                .custom_flags(rustix::fs::OFlags::NONBLOCK.bits() as i32)
                .open(node)
        })
}

/// The interface part of a node's identity.
fn interface_of(sys: &Path) -> String {
    fs::read_link(sys.join("device"))
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_default()
}

fn kebab(name: &str) -> String {
    let mut out = String::new();
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_owned()
}

fn read_formats(fd: &File, kind: u32) -> Vec<Format> {
    let mut formats = Vec::new();
    for n in 0..64 {
        let Some(desc) = v4l2::enum_fmt(fd, kind, n) else {
            break;
        };
        let fourcc = fourcc_text(desc.pixelformat);
        let mut sizes = Vec::new();
        for k in 0..128 {
            let Some(sz) = v4l2::enum_sizes(fd, desc.pixelformat, k) else {
                break;
            };
            let pairs = if sz.kind == v4l2::FRMSIZE_DISCRETE {
                vec![(sz.union[0], sz.union[1])]
            } else {
                // stepwise / continuous: min_w, max_w, step_w, min_h, max_h, step_h
                ladder((sz.union[0], sz.union[3]), (sz.union[1], sz.union[4]))
            };
            for (w, h) in pairs {
                let mut intervals = Vec::new();
                for j in 0..64 {
                    let Some(iv) = v4l2::enum_intervals(fd, desc.pixelformat, w, h, j) else {
                        break;
                    };
                    if iv.kind == v4l2::FRMIVAL_DISCRETE {
                        intervals.push(Interval {
                            num: iv.union[0],
                            den: iv.union[1],
                        });
                    } else {
                        // Stepwise: offer the fastest (min interval) and 30, 15 inside.
                        let (min, max) = (
                            Interval {
                                num: iv.union[0],
                                den: iv.union[1],
                            },
                            Interval {
                                num: iv.union[2],
                                den: iv.union[3],
                            },
                        );
                        intervals.push(min);
                        for fps in [30u32, 15] {
                            let f = fps as f32;
                            if f < min.fps() && f > max.fps() {
                                intervals.push(Interval { num: 1, den: fps });
                            }
                        }
                        break;
                    }
                }
                if intervals.is_empty() {
                    // A driver that enumerates no interval streams at whatever
                    // it streams; 30 is what ffmpeg asks and most answer.
                    intervals.push(Interval { num: 1, den: 30 });
                }
                intervals.sort_by(|a, b| b.fps().total_cmp(&a.fps()));
                intervals.dedup();
                sizes.push(Size {
                    width: w,
                    height: h,
                    intervals,
                });
            }
            if sz.kind != v4l2::FRMSIZE_DISCRETE {
                break;
            }
        }
        sizes.sort_by(|a, b| {
            (u64::from(b.width) * u64::from(b.height))
                .cmp(&(u64::from(a.width) * u64::from(a.height)))
        });
        sizes.dedup_by(|a, b| a.width == b.width && a.height == b.height);
        formats.push(Format {
            fourcc,
            description: v4l2::text(&desc.description),
            compressed: desc.flags & v4l2::FMT_COMPRESSED != 0,
            sizes,
        });
    }
    formats
}

fn read_controls(fd: &File) -> Vec<Control> {
    let mut out = Vec::new();
    let mut after = 0u32;
    for _ in 0..256 {
        let Some(q) = v4l2::next_ctrl(fd, after) else {
            break;
        };
        after = q.id;
        if q.flags & v4l2::CTRL_DISABLED != 0 {
            continue;
        }
        let kind = match q.kind {
            1 => ControlKind::Integer,
            2 => ControlKind::Boolean,
            3 => ControlKind::Menu,
            9 => ControlKind::IntegerMenu,
            // Buttons, classes, 64-bit, strings and compound controls: nothing
            // a person turns with an arrow key.
            _ => continue,
        };
        let mut menu = Vec::new();
        if matches!(kind, ControlKind::Menu | ControlKind::IntegerMenu) {
            for i in q.minimum..=q.maximum.min(q.minimum + 64) {
                if let Some(m) = v4l2::query_menu(fd, q.id, i as u32) {
                    let name = if kind == ControlKind::Menu {
                        v4l2::text(&{ m.name })
                    } else {
                        let bytes = { m.name };
                        i64::from_le_bytes(bytes[..8].try_into().unwrap_or([0; 8])).to_string()
                    };
                    menu.push((i, name));
                }
            }
        }
        let name = v4l2::text(&q.name);
        out.push(Control {
            id: q.id,
            key: kebab(&name),
            name,
            kind,
            min: q.minimum,
            max: q.maximum,
            step: q.step.max(1),
            default: q.default_value,
            value: v4l2::get_ctrl(fd, q.id).unwrap_or(q.default_value),
            inactive: q.flags & v4l2::CTRL_INACTIVE != 0,
            read_only: q.flags & v4l2::CTRL_READ_ONLY != 0,
            menu,
        });
    }
    out
}

/// Reads one node. `None` for a node that is not a capture device (metadata,
/// output, a codec) or that cannot be opened at all.
pub fn read(node: &Path) -> Option<Camera> {
    let name = node.file_name()?.to_string_lossy().into_owned();
    let sys = Path::new(SYS).join(&name);
    let fd = open(node).ok()?;
    let cap = v4l2::query_cap(&fd).ok()?;
    let caps = if cap.capabilities & v4l2::CAP_DEVICE_CAPS != 0 {
        cap.device_caps
    } else {
        cap.capabilities
    };
    let single = caps & v4l2::CAP_CAPTURE != 0;
    let multi = caps & v4l2::CAP_CAPTURE_MPLANE != 0;
    if !single && !multi {
        return None;
    }
    let card = v4l2::text(&cap.card);
    let bus = v4l2::text(&cap.bus_info);
    let driver = v4l2::text(&cap.driver);
    // A MULTI-PLANAR NODE'S FORMATS ARE READ TOO, under its own buffer
    // type: what it offers is drawn — its sizes and rates — beside the
    // sentence that says ffmpeg's V4L2 input cannot open it
    // (`camera-about-cannot-stream`). It was a camera with no formats at all.
    let formats = read_formats(
        &fd,
        if single {
            v4l2::BUF_TYPE_CAPTURE
        } else {
            v4l2::BUF_TYPE_CAPTURE_MPLANE
        },
    );
    let controls = read_controls(&fd);
    let ours = driver.contains("loopback") && card == OUR_LABEL;
    let kind = if ours {
        Kind::Virtual
    } else if !formats.is_empty() && formats.iter().all(|f| is_grey(&f.fourcc)) {
        Kind::Infrared
    } else {
        Kind::Colour
    };
    Some(Camera {
        identity: Identity::of(&card, &bus, &interface_of(&sys)),
        node: node.to_owned(),
        card,
        driver,
        bus,
        kind,
        streamable: single && formats.iter().any(|f| input_format(&f.fourcc).is_some()),
        formats,
        controls,
    })
}

/// Every camera, in node order.
pub fn scan() -> Vec<Camera> {
    let Ok(dir) = fs::read_dir(SYS) else {
        return Vec::new();
    };
    let mut nodes: Vec<(u32, PathBuf)> = dir
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let n: u32 = name.strip_prefix("video")?.parse().ok()?;
            Some((n, Path::new("/dev").join(name)))
        })
        .collect();
    nodes.sort();
    nodes.iter().filter_map(|(_, p)| read(p)).collect()
}

/// Sets a control by its stable key. The driver clamps and may refuse (an
/// inactive control), and the refusal is the answer.
pub fn set_control(camera: &Camera, key: &str, value: i32) -> anyhow::Result<()> {
    let c = camera
        .controls
        .iter()
        .find(|c| c.key == key)
        .ok_or_else(|| anyhow::anyhow!("{} has no control {key}", camera.card))?;
    if c.read_only {
        anyhow::bail!("{key} is read-only");
    }
    let fd = open(&camera.node)?;
    v4l2::set_ctrl(&fd, c.id, value.clamp(c.min, c.max)).map_err(|e| anyhow::anyhow!("{key}: {e}"))
}

/// Puts every saved control back, in the driver's order — which matters: an
/// `auto_exposure` set to manual has to land before the `exposure_time` it
/// unlocks, and the driver lists them in that order.
pub fn restore(camera: &Camera, saved: &std::collections::BTreeMap<String, i32>) -> Vec<String> {
    let mut refused = Vec::new();
    for c in &camera.controls {
        if let Some(v) = saved.get(&c.key)
            && *v != c.value
            && set_control(camera, &c.key, *v).is_err()
        {
            refused.push(c.key.clone());
        }
    }
    refused
}

#[cfg(test)]
mod tests {
    use super::*;

    fn size(w: u32, h: u32, fps: &[u32]) -> Size {
        Size {
            width: w,
            height: h,
            intervals: fps.iter().map(|f| Interval { num: 1, den: *f }).collect(),
        }
    }

    /// The laptop camera measured on 2026-09-23, as the driver reported it.
    fn measured() -> Camera {
        Camera {
            identity: Identity::of("Integrated RGB Camera", "usb-0000:c4:00.4-1", "3-1:1.0"),
            node: "/dev/video0".into(),
            card: "Integrated RGB Camera".into(),
            driver: "uvcvideo".into(),
            bus: "usb-0000:c4:00.4-1".into(),
            kind: Kind::Colour,
            formats: vec![
                Format {
                    fourcc: "MJPG".into(),
                    description: String::new(),
                    compressed: true,
                    sizes: vec![
                        size(1920, 1080, &[30]),
                        size(1280, 720, &[30]),
                        size(640, 480, &[30]),
                    ],
                },
                Format {
                    fourcc: "YUYV".into(),
                    description: String::new(),
                    compressed: false,
                    sizes: vec![
                        size(1920, 1080, &[5]),
                        size(1280, 720, &[10]),
                        size(640, 480, &[30]),
                    ],
                },
            ],
            controls: Vec::new(),
            streamable: true,
        }
    }

    #[test]
    fn a_usb_camera_opens_at_1080p_mjpeg() {
        let m = choose_mode(&measured(), None).expect("a mode");
        assert_eq!((m.fourcc.as_str(), m.width, m.height), ("MJPG", 1920, 1080));
        assert_eq!(m.fps(), 30.0);
    }

    #[test]
    fn a_raw_only_camera_takes_the_largest_smooth_size() {
        let mut c = measured();
        c.formats.remove(0);
        let m = choose_mode(&c, None).expect("a mode");
        assert_eq!(
            (m.width, m.height),
            (640, 480),
            "5 fps at 1080p is not a camera, it is a slideshow"
        );
    }

    #[test]
    fn a_4k_camera_is_opened_at_1080p_by_default() {
        let mut c = measured();
        c.formats[0].sizes.insert(0, size(3840, 2160, &[30]));
        let m = choose_mode(&c, None).expect("a mode");
        assert_eq!((m.width, m.height), (1920, 1080));
    }

    #[test]
    fn a_slow_camera_still_opens() {
        let mut c = measured();
        c.formats = vec![Format {
            fourcc: "YUYV".into(),
            description: String::new(),
            compressed: false,
            sizes: vec![size(1280, 720, &[10]), size(640, 480, &[15])],
        }];
        let m = choose_mode(&c, None).expect("a mode");
        assert_eq!(
            (m.width, m.height),
            (1280, 720),
            "with nothing smooth, the largest"
        );
    }

    #[test]
    fn a_saved_mode_wins_while_the_device_offers_it() {
        let c = measured();
        let wanted = Mode {
            fourcc: "YUYV".into(),
            width: 640,
            height: 480,
            interval: Interval { num: 1, den: 30 },
        };
        assert_eq!(choose_mode(&c, Some(&wanted)), Some(wanted.clone()));
        let gone = Mode {
            width: 800,
            ..wanted
        };
        assert_eq!(
            choose_mode(&c, Some(&gone)).map(|m| m.width),
            Some(1920),
            "a vanished mode falls back"
        );
    }

    #[test]
    fn a_format_ffmpeg_cannot_name_is_never_chosen() {
        let mut c = measured();
        c.formats = vec![Format {
            fourcc: "ZZZZ".into(),
            description: String::new(),
            compressed: false,
            sizes: vec![size(640, 480, &[30])],
        }];
        assert_eq!(choose_mode(&c, None), None);
    }

    #[test]
    fn the_default_is_never_infrared_nor_our_own_output() {
        let mut ir = measured();
        ir.kind = Kind::Infrared;
        ir.identity = Identity::of("x", "y", "1.2");
        let mut ours = measured();
        ours.kind = Kind::Virtual;
        ours.identity = Identity::of(OUR_LABEL, "platform:v4l2loopback-000", "");
        let colour = measured();
        let all = [ours.clone(), ir.clone(), colour.clone()];
        assert_eq!(
            default_camera(&all, None).map(|c| &c.identity),
            Some(&colour.identity)
        );
        assert_eq!(
            default_camera(&all, Some(&ir.identity)).map(|c| &c.identity),
            Some(&ir.identity),
            "chosen by hand, infrared is allowed"
        );
        assert_eq!(
            default_camera(&all, Some(&ours.identity)).map(|c| &c.identity),
            Some(&colour.identity),
            "our own output never"
        );
    }

    #[test]
    fn a_stepwise_range_is_offered_as_common_sizes() {
        assert_eq!(
            ladder((320, 240), (1280, 720)),
            vec![
                (1280, 720),
                (1024, 576),
                (960, 540),
                (800, 600),
                (640, 480),
                (320, 240)
            ]
        );
        assert_eq!(ladder((100, 100), (200, 150)), vec![(200, 150)]);
    }

    #[test]
    fn control_names_become_stable_keys() {
        assert_eq!(
            kebab("White Balance Temperature, Auto"),
            "white-balance-temperature-auto"
        );
        assert_eq!(kebab("Brightness"), "brightness");
    }

    #[test]
    fn a_fourcc_reads_as_its_four_characters() {
        assert_eq!(fourcc_text(u32::from_le_bytes(*b"MJPG")), "MJPG");
    }
}
