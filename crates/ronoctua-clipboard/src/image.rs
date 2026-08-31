//! Decoding the copied image.
//!
//! # Why here, and not in the renderer
//!
//! Because it does not depend on the GPU. Decoding a PNG is turning bytes into
//! pixels — the same nature as [`crate::qr`], which turns text into a
//! matrix. Being here, "a 2×1 PNG becomes four bytes per pixel" can be
//! proved without a compositor, without Vulkan and without a window.
//!
//! What **draws** is `ronoctua_render::image`, and that is where the shape the
//! GPU consumes lives.
//!
//! # PNG and JPEG only
//!
//! They are what comes out of a screenshot and out of a browser. The `image` crate brings a
//! dozen decoders; loading them all into the process that has to
//! draw in one frame would be paying for formats nobody pastes.

/// The image MIME types the watcher accepts, in ORDER OF PREFERENCE.
///
/// PNG first: it is lossless and it is what screenshots use. JPEG only afterwards,
/// because when both are offered the PNG is the faithful version.
pub const TYPES: [&str; 3] = ["image/png", "image/jpeg", "image/jpg"];

/// Does this offer carry an image?
#[must_use]
pub fn is_image(kinds: &[String]) -> bool {
    kinds
        .iter()
        .any(|t| TYPES.iter().any(|i| t.eq_ignore_ascii_case(i)))
}

/// The preferred image type among those offered.
#[must_use]
pub fn preferred_type(kinds: &[String]) -> Option<&'static str> {
    TYPES
        .into_iter()
        .find(|i| kinds.iter().any(|t| t.eq_ignore_ascii_case(i)))
}

/// How many pixels the preview agrees to decode.
///
/// Twenty megapixels cover a 4K monitor with room to spare. The ceiling exists because a
/// PNG's header declares the dimensions, and a malformed (or
/// malicious) file can declare 60000×60000 — which is 14 GB of RGBA allocated
/// before any check. It is the same kind of guard as the texture
/// ceiling: refusing is better than dying without saying why.
pub const PIXEL_CEILING: u64 = 20_000_000;

/// Decodes to RGBA8.
///
/// `None` when the bytes are not an image we know how to read, or when it
/// goes past [`PIXEL_CEILING`]. In both cases the history keeps the entry
/// anyway — what is lost is the preview, not the content.
#[must_use]
pub fn decode(bytes: &[u8]) -> Option<ronoctua_render_rgba_fn::Rgba> {
    let reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;

    // THE DIMENSIONS BEFORE THE PIXELS. `into_dimensions` reads only the header; that
    // is what makes it possible to refuse a giant file without allocating for it.
    let (width, height) = reader.into_dimensions().ok()?;
    if u64::from(width) * u64::from(height) > PIXEL_CEILING {
        return None;
    }

    let image = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()?
        .to_rgba8();
    Some(ronoctua_render_rgba_fn::Rgba {
        width: image.width(),
        height: image.height(),
        pixels: image.into_raw(),
    })
}

/// The shape the renderer consumes.
///
/// Re-exported from `ronoctua-render` so this crate does **not** depend on it: what
/// this module returns is pixels, and pixels are not the GPU.
pub mod ronoctua_render_rgba_fn {
    /// Pixels ready for the GPU.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Rgba {
        /// Width in pixels.
        pub width: u32,
        /// Height in pixels.
        pub height: u32,
        /// `width × height × 4` bytes, RGBA8.
        pub pixels: Vec<u8>,
    }
}

#[cfg(test)]
mod tests {
    /// A actual 2×1 PNG: one opaque red pixel and one opaque blue one.
    ///
    /// Written out in bytes, and not generated, because what we want to prove is that we
    /// **decode a file** — generating it with the same crate that reads it would prove
    /// that it talks to itself.
    fn png_2x1() -> Vec<u8> {
        use image::ImageEncoder as _;
        let mut out = Vec::new();
        let pixels: [u8; 8] = [255, 0, 0, 255, 0, 0, 255, 255];
        image::codecs::png::PngEncoder::new(&mut out)
            .write_image(&pixels, 2, 1, image::ExtendedColorType::Rgba8)
            .expect("encode the fixture");
        out
    }

    #[test]
    fn decodes_png_into_rgba() {
        let r = super::decode(&png_2x1()).expect("a valid PNG has to decode");
        assert_eq!((r.width, r.height), (2, 1));
        let expected = usize::try_from(r.width * r.height * 4).unwrap();
        assert_eq!(r.pixels.len(), expected, "RGBA is four bytes per pixel");
        assert_eq!(&r.pixels[..4], &[255, 0, 0, 255], "the first pixel is red");
    }

    #[test]
    fn junk_does_not_become_an_image() {
        // REFUSE, and do not draw noise. The history keeps the entry
        // either way; what is lost is the preview.
        assert!(super::decode(b"I am not a png").is_none());
        assert!(super::decode(&[]).is_none());
    }

    #[test]
    fn the_image_types_are_recognised_case_insensitively() {
        let kinds = vec!["TEXT/PLAIN".to_owned(), "Image/PNG".to_owned()];
        assert!(super::is_image(&kinds));
        assert_eq!(super::preferred_type(&kinds), Some("image/png"));
        // PNG BEATS JPEG when both are offered: it is the faithful version.
        let both = vec!["image/jpeg".to_owned(), "image/png".to_owned()];
        assert_eq!(super::preferred_type(&both), Some("image/png"));
        assert!(!super::is_image(&["text/plain".to_owned()]));
    }
}
