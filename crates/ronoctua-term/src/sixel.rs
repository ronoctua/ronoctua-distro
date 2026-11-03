//! Sixel graphics, decoded to RGBA — the holder's, never a window's.
//!
//! # Why the holder decodes what the engine cannot draw
//!
//! `libghostty-vt` speaks kitty's graphics protocol and not sixel. Rather than
//! a second image model, the holder's inbound filter (`inbound.rs`) takes a
//! `DCS … q … ST` out of the stream, decodes it here, and puts a kitty image
//! in its place — so the engine, every window's mirror and the renderer know
//! one kind of picture, and the program never learns the difference.
//!
//! # What is decoded
//!
//! The raster attributes (`"Pan;Pad;Ph;Pv`: the size), the colour registers
//! (`#Pc;Pu;Px;Py;Pz`, HLS or RGB), repeats (`!Pn`), carriage return (`$`)
//! and new line (`-`), and the background select of the introducer
//! (`P2 = 1`: what was not drawn stays transparent; otherwise it is the
//! terminal's background).
//!
//! A register a program never defined is the TERMINAL's colour of that number
//! — the 16 ANSI colours and the 256-colour table the window gave, its page's
//! roles — rather than a palette of hand-written colours of its own; beyond
//! 255, or before any window gave colours, it paints nothing.
//!
//! The aspect ratio is not applied: pixels are square, as in the modern
//! terminals programs are written against today (an encoder that wants 2:1
//! repeats its rows).
//!
//! # Bounded
//!
//! A sixel is a size a program says; nothing past [`MAX_SIDE_PX`] on a side or
//! [`MAX_PIXELS`] in all is kept — the rest is drawn off the edge, as it would
//! be off a screen.

/// The widest or tallest image kept, in pixels.
pub const MAX_SIDE_PX: usize = 8192;

/// The most pixels an image keeps in all (64 MiB of RGBA).
pub const MAX_PIXELS: usize = 4096 * 4096;

/// How many colour registers there are — xterm's number, and what
/// XTSMGRAPHICS answers.
pub const REGISTERS: usize = 1024;

/// The most numbers a command keeps: a colour takes five, and a command of
/// endless `;` must not grow a list for as long as it is written.
const NUMBERS_MAX: usize = 8;

/// A decoded image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    /// Width in pixels.
    pub width: usize,
    /// Height in pixels.
    pub height: usize,
    /// Four bytes a pixel, row by row; not premultiplied.
    pub rgba: Vec<u8>,
}

/// The command whose numbers are being read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Command {
    None,
    Repeat,
    Colour,
    Raster,
}

/// One sixel being decoded, a byte at a time — so it does not matter how the
/// program's writes cut it.
pub struct Decoder {
    /// `P2 = 1`: what was not drawn stays transparent.
    transparent: bool,
    registers: Vec<Option<[u8; 3]>>,
    palette: Option<Box<[[u8; 3]; 256]>>,
    colour: usize,
    repeat: usize,
    x: usize,
    band: usize,
    /// Packed `0xAARRGGBB`; zero is a pixel nothing drew.
    pixels: Vec<u32>,
    stride: usize,
    rows: usize,
    drawn: (usize, usize),
    raster: (usize, usize),
    command: Command,
    numbers: Vec<u32>,
    digits: Option<u32>,
}

impl Decoder {
    /// A decoder for the introducer's parameters (`P1;P2;P3`), with the
    /// terminal's palette for the registers a program does not define.
    #[must_use]
    pub fn new(params: &[u16], palette: Option<&[[u8; 3]; 256]>) -> Self {
        Self {
            transparent: params.get(1) == Some(&1),
            registers: vec![None; REGISTERS],
            palette: palette.map(|p| Box::new(*p)),
            colour: 0,
            repeat: 1,
            x: 0,
            band: 0,
            pixels: Vec::new(),
            stride: 0,
            rows: 0,
            drawn: (0, 0),
            raster: (0, 0),
            command: Command::None,
            numbers: Vec::new(),
            digits: None,
        }
    }

    /// Feeds the sixel's data — what comes after the `q`.
    pub fn feed(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.byte(b);
        }
    }

    fn byte(&mut self, b: u8) {
        if self.command != Command::None {
            match b {
                b'0'..=b'9' => {
                    let d = u32::from(b - b'0');
                    // Saturating: a number no image could use is still a
                    // number, and clamped where it is used.
                    self.digits = Some(
                        self.digits
                            .unwrap_or(0)
                            .saturating_mul(10)
                            .saturating_add(d)
                            .min(1 << 24),
                    );
                    return;
                }
                b';' => {
                    let n = self.digits.take().unwrap_or(0);
                    if self.numbers.len() < NUMBERS_MAX {
                        self.numbers.push(n);
                    }
                    return;
                }
                _ => self.end_command(),
            }
        }
        match b {
            0x3F..=0x7E => {
                let times = std::mem::replace(&mut self.repeat, 1);
                self.sixel(b - 0x3F, times);
            }
            b'!' => self.command = Command::Repeat,
            b'#' => self.command = Command::Colour,
            b'"' => self.command = Command::Raster,
            b'$' => self.x = 0,
            b'-' => {
                self.x = 0;
                self.band += 1;
            }
            // Anything else — spaces, line breaks a program wrapped its data
            // in — is not sixel data, and is skipped as xterm skips it.
            _ => {}
        }
    }

    /// The numbers of a command are read; do what it says.
    fn end_command(&mut self) {
        if let Some(n) = self.digits.take()
            && self.numbers.len() < NUMBERS_MAX
        {
            self.numbers.push(n);
        }
        let numbers = std::mem::take(&mut self.numbers);
        let at = |i: usize| numbers.get(i).copied().unwrap_or(0) as usize;
        match std::mem::replace(&mut self.command, Command::None) {
            Command::None => {}
            // `!0` is one, as everywhere.
            Command::Repeat => self.repeat = at(0).clamp(1, MAX_SIDE_PX),
            Command::Colour if numbers.is_empty() => {}
            Command::Colour => {
                let register = at(0).min(REGISTERS - 1);
                if numbers.len() >= 5 {
                    let (x, y, z) = (numbers[2], numbers[3], numbers[4]);
                    self.registers[register] = match numbers[1] {
                        1 => Some(hls(x, y, z)),
                        2 => Some([percent(x), percent(y), percent(z)]),
                        // A colour space that does not exist defines nothing.
                        _ => self.registers[register],
                    };
                }
                self.colour = register;
            }
            Command::Raster => {
                if numbers.len() >= 4 {
                    self.raster = (at(2).min(MAX_SIDE_PX), at(3).min(MAX_SIDE_PX));
                    let (w, h) = self.raster;
                    self.reserve(w, h);
                }
            }
        }
    }

    /// The register's colour, or the terminal's of that number, or nothing.
    fn ink(&self) -> Option<[u8; 3]> {
        self.registers[self.colour].or_else(|| {
            let palette = self.palette.as_ref()?;
            palette.get(self.colour).copied()
        })
    }

    /// One sixel — six pixels down, `bits` from the top — `times` across.
    fn sixel(&mut self, bits: u8, times: usize) {
        let top = self.band.saturating_mul(6);
        // Only the columns inside the bound are visited: a repeat past the
        // edge, as many times as a program likes, draws nothing and costs
        // nothing.
        if bits != 0
            && let Some([r, g, b]) = self.ink()
        {
            let packed = 0xFF00_0000 | u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b);
            for x in self.x..self.x.saturating_add(times).min(MAX_SIDE_PX) {
                for bit in 0..6 {
                    if bits & (1 << bit) != 0 {
                        self.put(x, top + bit, packed);
                    }
                }
            }
        }
        self.x = self.x.saturating_add(times).min(MAX_SIDE_PX);
    }

    /// Grows the pixel store to hold `width` by `height`, within the bounds.
    fn reserve(&mut self, width: usize, height: usize) -> bool {
        let (width, height) = (width.min(MAX_SIDE_PX), height.min(MAX_SIDE_PX));
        if width <= self.stride && height <= self.rows {
            return true;
        }
        // In doublings, both ways, so a sixel drawn column by column or band
        // by band — one with no raster attributes says its size only as it
        // goes — does not copy its pixels once per column or once per band.
        // A doubling the bound has no room for falls back to the exact size,
        // which may still fit.
        let grow = |want: usize, have: usize, floor: usize| {
            if want > have {
                want.max(have * 2).clamp(floor, MAX_SIDE_PX)
            } else {
                have
            }
        };
        let (mut stride, mut rows) = (grow(width, self.stride, 64), grow(height, self.rows, 6));
        if stride * rows > MAX_PIXELS {
            stride = width.max(self.stride);
            rows = height.max(self.rows);
        }
        if stride * rows > MAX_PIXELS {
            return false;
        }
        let mut grown = vec![0u32; stride * rows];
        for y in 0..self.rows {
            let from = &self.pixels[y * self.stride..(y + 1) * self.stride];
            grown[y * stride..y * stride + self.stride].copy_from_slice(from);
        }
        self.pixels = grown;
        self.stride = stride;
        self.rows = rows;
        true
    }

    fn put(&mut self, x: usize, y: usize, packed: u32) {
        if x >= MAX_SIDE_PX || y >= MAX_SIDE_PX || !self.reserve(x + 1, (y / 6 + 1) * 6) {
            return;
        }
        self.pixels[y * self.stride + x] = packed;
        self.drawn = (self.drawn.0.max(x + 1), self.drawn.1.max(y + 1));
    }

    /// The image, with what was not drawn as `background` (opaque) unless
    /// the introducer asked for transparency; `None` if it has no pixel at
    /// all — not even a size.
    #[must_use]
    pub fn finish(mut self, background: Option<[u8; 3]>) -> Option<Image> {
        self.end_command();
        let width = self.drawn.0.max(self.raster.0);
        let mut height = self.drawn.1.max(self.raster.1);
        if width == 0 || height == 0 {
            return None;
        }
        height = height.min(MAX_PIXELS / width);
        let ground = match (self.transparent, background) {
            (false, Some([r, g, b])) => [r, g, b, 0xFF],
            _ => [0; 4],
        };
        let mut rgba = Vec::with_capacity(width * height * 4);
        for y in 0..height {
            for x in 0..width {
                let p = if x < self.stride && y < self.rows {
                    self.pixels[y * self.stride + x]
                } else {
                    0
                };
                if p == 0 {
                    rgba.extend_from_slice(&ground);
                } else {
                    rgba.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, p as u8, 0xFF]);
                }
            }
        }
        Some(Image {
            width,
            height,
            rgba,
        })
    }
}

/// A sixel percentage (0–100) as an 8-bit channel.
fn percent(v: u32) -> u8 {
    u8::try_from((v.min(100) * 255 + 50) / 100).unwrap_or(u8::MAX)
}

/// DEC's HLS — hue in degrees with BLUE at 0 (red at 120, green at 240),
/// lightness and saturation in percent — as RGB.
fn hls(h: u32, l: u32, s: u32) -> [u8; 3] {
    // DEC's wheel is the usual one turned by 240°: its blue, 0°, is the
    // usual 240°.
    let hue = f64::from((h % 360 + 240) % 360) / 360.0;
    let (l, s) = (f64::from(l.min(100)) / 100.0, f64::from(s.min(100)) / 100.0);
    if s == 0.0 {
        let v = channel(l);
        return [v, v, v];
    }
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let tone = |t: f64| -> u8 {
        let t = t.rem_euclid(1.0);
        channel(if t < 1.0 / 6.0 {
            (q - p).mul_add(6.0 * t, p)
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            (q - p).mul_add((2.0 / 3.0 - t) * 6.0, p)
        } else {
            p
        })
    };
    [tone(hue + 1.0 / 3.0), tone(hue), tone(hue - 1.0 / 3.0)]
}

/// A fraction 0–1 as an 8-bit channel, rounded.
fn channel(v: f64) -> u8 {
    // Clamped first: the cast of a value in 0..=255 is exact.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let c = (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(params: &[u16], data: &[u8], background: Option<[u8; 3]>) -> Option<Image> {
        let mut d = Decoder::new(params, None);
        d.feed(data);
        d.finish(background)
    }

    fn pixel(img: &Image, x: usize, y: usize) -> [u8; 4] {
        let i = (y * img.width + x) * 4;
        img.rgba[i..i + 4].try_into().unwrap()
    }

    #[test]
    fn a_column_of_six_pixels_is_one_character() {
        // FAILS IF the bits of a sixel are read from the wrong end: `@` is
        // the top pixel, `~` all six.
        let img = decode(&[0, 1], b"#1;2;100;0;0#1@~", None).unwrap();
        assert_eq!((img.width, img.height), (2, 6));
        assert_eq!(pixel(&img, 0, 0), [255, 0, 0, 255]);
        assert_eq!(
            pixel(&img, 0, 1),
            [0, 0, 0, 0],
            "transparent where nothing drew"
        );
        assert_eq!(pixel(&img, 1, 5), [255, 0, 0, 255]);
    }

    #[test]
    fn repeats_lines_and_returns_move_where_the_next_sixel_lands() {
        // FAILS IF `!` repeats the wrong count, `-` does not start a new band
        // at column 0, or `$` does not return within the band.
        let img = decode(&[0, 1], b"#2;2;0;100;0#2!3~-@$#3;2;0;0;100#3A", None).unwrap();
        assert_eq!((img.width, img.height), (3, 8));
        for x in 0..3 {
            assert_eq!(pixel(&img, x, 5), [0, 255, 0, 255], "{x}");
        }
        // Second band: `@` (top) in green, then `$` and `A` (second pixel) blue.
        assert_eq!(pixel(&img, 0, 6), [0, 255, 0, 255]);
        assert_eq!(pixel(&img, 0, 7), [0, 0, 255, 255]);
    }

    #[test]
    fn what_was_not_drawn_is_the_background_unless_asked_transparent() {
        // FAILS IF `P2` is ignored: an image meant to sit on the terminal's
        // ground would show the text under it, or one meant transparent would
        // hide it.
        let ground = Some([10, 20, 30]);
        let opaque = decode(&[0, 0], b"\"1;1;4;6#1;2;100;100;100#1@", ground).unwrap();
        assert_eq!((opaque.width, opaque.height), (4, 6), "the raster's size");
        assert_eq!(pixel(&opaque, 3, 5), [10, 20, 30, 255]);
        let clear = decode(&[0, 1], b"\"1;1;4;6#1;2;100;100;100#1@", ground).unwrap();
        assert_eq!(pixel(&clear, 3, 5), [0, 0, 0, 0]);
    }

    #[test]
    fn hls_has_blue_at_zero_and_red_at_120() {
        // FAILS IF the HLS wheel is the usual one: DEC's starts at blue.
        assert_eq!(hls(0, 50, 100), [0, 0, 255]);
        assert_eq!(hls(120, 50, 100), [255, 0, 0]);
        assert_eq!(hls(240, 50, 100), [0, 255, 0]);
        assert_eq!(hls(77, 100, 0), [255, 255, 255]);
    }

    #[test]
    fn an_undefined_register_is_the_terminals_colour_of_that_number() {
        // FAILS IF a register a program never set paints a colour of ours,
        // or paints at all when there is no terminal colour to give it.
        let mut palette = [[0u8; 3]; 256];
        palette[3] = [7, 8, 9];
        let mut d = Decoder::new(&[0, 1], Some(&palette));
        d.feed(b"#3@#300@#3@");
        let img = d.finish(None).unwrap();
        assert_eq!(pixel(&img, 0, 0), [7, 8, 9, 255]);
        assert_eq!(
            pixel(&img, 1, 0),
            [0, 0, 0, 0],
            "register 300 has no colour"
        );
        assert_eq!(pixel(&img, 2, 0), [7, 8, 9, 255]);
    }

    #[test]
    fn a_command_of_endless_numbers_keeps_a_few() {
        // FAILS IF a command's numbers grow for as long as a program writes
        // `;` — the holder would keep them all.
        let mut d = Decoder::new(&[0, 1], None);
        d.feed(b"#");
        for _ in 0..64 {
            d.feed(&[b';'; 16 << 10]);
            assert!(d.numbers.len() <= NUMBERS_MAX, "{}", d.numbers.len());
        }
        // `$` ends it, and what follows is read as ever.
        d.feed(b"$#1;2;100;0;0#1@");
        let img = d.finish(None).expect("and the decoder goes on");
        assert_eq!(pixel(&img, 0, 0), [255, 0, 0, 255]);
    }

    #[test]
    fn an_image_that_says_its_size_as_it_goes_keeps_what_fits() {
        // FAILS IF growing the pixel store in doublings refuses a size that
        // fits the bound exactly — a sixel with no raster attributes, taller
        // than half the bound and drawn wider at its last band, would lose
        // its last columns.
        let mut data = b"#1;2;100;100;100#1".to_vec();
        data.extend(std::iter::repeat_n(b'-', 400));
        data.extend_from_slice(b"!4097~");
        let img = decode(&[0, 1], &data, None).unwrap();
        assert_eq!((img.width, img.height), (4097, 2406));
        assert_eq!(pixel(&img, 4096, 2405), [255, 255, 255, 255]);
    }

    #[test]
    fn an_image_past_the_bounds_is_cut_and_not_grown() {
        // FAILS IF a program can make the holder allocate whatever size it
        // names.
        let img = decode(&[0, 1], b"\"1;1;99999;99999#1;2;1;1;1#1!99999@", None).unwrap();
        assert!(img.width <= MAX_SIDE_PX && img.width * img.height <= MAX_PIXELS);
        assert!(
            decode(&[], b"", None).is_none(),
            "an empty sixel is no image"
        );
    }
}
