//! Covers as bytes: what they are, how big, and how one gets into a tag.

use std::io::{Cursor, Read as _};
use std::path::Path;

use crate::sheet::{COVER_MOST_BYTES, Why};

/// An image format, as its first bytes say.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ImageKind {
    /// JPEG.
    Jpeg,
    /// PNG.
    Png,
    /// GIF.
    Gif,
    /// WebP.
    WebP,
    /// BMP.
    Bmp,
    /// TIFF.
    Tiff,
}

impl ImageKind {
    /// Its MIME type.
    pub(crate) const fn mime(self) -> &'static str {
        match self {
            Self::Jpeg => "image/jpeg",
            Self::Png => "image/png",
            Self::Gif => "image/gif",
            Self::WebP => "image/webp",
            Self::Bmp => "image/bmp",
            Self::Tiff => "image/tiff",
        }
    }

    /// The extension a file of it takes.
    pub(crate) const fn extension(self) -> &'static str {
        match self {
            Self::Jpeg => "jpg",
            Self::Png => "png",
            Self::Gif => "gif",
            Self::WebP => "webp",
            Self::Bmp => "bmp",
            Self::Tiff => "tif",
        }
    }

    /// The kind a MIME type names, for a picture whose bytes did not say.
    pub(crate) fn of_mime(mime: &str) -> Option<Self> {
        match mime.to_ascii_lowercase().as_str() {
            "image/jpeg" | "image/jpg" => Some(Self::Jpeg),
            "image/png" => Some(Self::Png),
            "image/gif" => Some(Self::Gif),
            "image/webp" => Some(Self::WebP),
            "image/bmp" => Some(Self::Bmp),
            "image/tiff" => Some(Self::Tiff),
            _ => None,
        }
    }
}

/// What the first bytes say an image is.
///
/// The bytes and not the declared MIME type: an APIC frame that says
/// `image/jpg`, `jpeg` or nothing at all is common, and the extension of an
/// extracted cover has to be what the file really is.
pub(crate) fn sniff(bytes: &[u8]) -> Option<ImageKind> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(ImageKind::Jpeg)
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(ImageKind::Png)
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some(ImageKind::Gif)
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some(ImageKind::WebP)
    } else if bytes.starts_with(b"BM") {
        Some(ImageKind::Bmp)
    } else if bytes.starts_with(b"II*\0") || bytes.starts_with(b"MM\0*") {
        Some(ImageKind::Tiff)
    } else {
        None
    }
}

/// Width and height out of the header alone — nothing is decoded.
pub(crate) fn dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?
        .into_dimensions()
        .ok()
}

/// The widest side a cover may be decoded at when it has to be converted.
///
/// A converted cover is decoded whole into memory; eight thousand pixels a
/// side is 256 MiB of RGBA, which is already more than a player should
/// hold for a thumbnail, and past it the file is not a cover.
const DECODE_MOST_PX: u32 = 8192;

/// An image file made fit for a tag: its bytes and what they are.
///
/// JPEG and PNG go in AS THEY ARE — every tag format and every player reads
/// both, and re-encoding a JPEG would lose quality for nothing. Anything else
/// the `image` crate reads (WebP from a browser, GIF, BMP, TIFF) is decoded
/// and written as PNG: ID3v2 would carry a WebP, but no car stereo would draw
/// it.
///
/// # Errors
///
/// [`Why::Gone`] when the file cannot be opened, [`Why::TooBig`] past
/// [`COVER_MOST_BYTES`] (on disk or once converted), [`Why::NotAnImage`] for
/// anything else.
pub(crate) fn for_tags(path: &Path) -> Result<(Vec<u8>, ImageKind), Why> {
    let file = std::fs::File::open(path).map_err(|_| Why::Gone)?;
    let mut bytes = Vec::new();
    file.take(COVER_MOST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Why::Gone)?;
    if bytes.len() as u64 > COVER_MOST_BYTES {
        return Err(Why::TooBig {
            bytes: std::fs::metadata(path).map_or(bytes.len() as u64, |m| m.len()),
            most_bytes: COVER_MOST_BYTES,
        });
    }
    let kind = sniff(&bytes).ok_or(Why::NotAnImage)?;
    if matches!(kind, ImageKind::Jpeg | ImageKind::Png) {
        // A header that does not parse is a file that only STARTS like an
        // image; the tag would carry garbage every player then refuses.
        dimensions(&bytes).ok_or(Why::NotAnImage)?;
        return Ok((bytes, kind));
    }
    let mut reader = image::ImageReader::new(Cursor::new(&bytes))
        .with_guessed_format()
        .map_err(|_| Why::NotAnImage)?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(DECODE_MOST_PX);
    limits.max_image_height = Some(DECODE_MOST_PX);
    reader.limits(limits);
    let picture = reader.decode().map_err(|_| Why::NotAnImage)?;
    let mut png = Vec::new();
    picture
        .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|_| Why::NotAnImage)?;
    if png.len() as u64 > COVER_MOST_BYTES {
        return Err(Why::TooBig {
            bytes: png.len() as u64,
            most_bytes: COVER_MOST_BYTES,
        });
    }
    Ok((png, ImageKind::Png))
}

#[cfg(test)]
pub(crate) mod fixture {
    //! Tiny images, made here so no binary lives in the tree.

    use std::io::Cursor;

    /// A `w`×`h` image in a format, one colour.
    pub(crate) fn image(w: u32, h: u32, format: image::ImageFormat) -> Vec<u8> {
        let img = image::RgbImage::from_pixel(w, h, image::Rgb([200, 30, 60]));
        let mut out = Vec::new();
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut Cursor::new(&mut out), format)
            .expect("an image this small encodes");
        out
    }
}

#[cfg(test)]
mod tests {
    use super::{ImageKind, dimensions, fixture, for_tags, sniff};
    use crate::sheet::Why;

    #[test]
    fn the_bytes_say_what_an_image_is() {
        let png = fixture::image(3, 2, image::ImageFormat::Png);
        let jpeg = fixture::image(3, 2, image::ImageFormat::Jpeg);
        assert_eq!(sniff(&png), Some(ImageKind::Png));
        assert_eq!(sniff(&jpeg), Some(ImageKind::Jpeg));
        assert_eq!(sniff(b"hello"), None);
        assert_eq!(dimensions(&png), Some((3, 2)));
    }

    #[test]
    fn a_webp_goes_in_as_png_and_a_jpeg_goes_in_as_it_is() {
        let dir = std::env::temp_dir().join(format!("ronoctua-tags-pic-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a temp dir");
        let webp = dir.join("a.webp");
        std::fs::write(&webp, fixture::image(4, 4, image::ImageFormat::WebP)).expect("written");
        let (bytes, kind) = for_tags(&webp).expect("a webp is an image");
        assert_eq!(kind, ImageKind::Png);
        assert_eq!(dimensions(&bytes), Some((4, 4)));

        let jpeg = dir.join("b.jpg");
        let original = fixture::image(5, 5, image::ImageFormat::Jpeg);
        std::fs::write(&jpeg, &original).expect("written");
        assert_eq!(for_tags(&jpeg), Ok((original, ImageKind::Jpeg)));

        let text = dir.join("c.png");
        std::fs::write(&text, b"\x89PNG\r\n\x1a\nnot really").expect("written");
        assert_eq!(for_tags(&text), Err(Why::NotAnImage));
        assert_eq!(for_tags(&dir.join("absent.png")), Err(Why::Gone));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
