//! The QR of the copied content.
//!
//! # Why a MATRIX, and not an image
//!
//! Because what draws is the quad pipeline the footer already has — one quad
//! per dark module. Generating a PNG would require a decoder, a texture and a
//! pipeline that does not exist today (that is the image wave, still to come), just to
//! draw black squares on a light background.
//!
//! # Why the module size is not here
//!
//! It comes from the height available in the region, which has a ceiling (invariant 14) and
//! changes with the screen. A fixed number here would give a QR that does not fit on a low
//! screen or that comes out tiny on a high one — and illegible is the same as absent,
//! because nobody can point the camera at it.

/// A QR matrix: `side × side`, `true` = dark module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Matrix {
    /// How many modules per edge.
    pub side: usize,
    /// The modules, in reading order.
    pub modules: Vec<bool>,
}

impl Matrix {
    /// The module at `(x, y)`. Outside the matrix it is light.
    #[must_use]
    pub fn dark(&self, x: usize, y: usize) -> bool {
        if x >= self.side || y >= self.side {
            return false;
        }
        self.modules[y * self.side + x]
    }
}

/// Generates the QR of a text.
///
/// `None` when the text does not fit in a QR — the limit depends on the content, and
/// hovers around a few thousand characters. Refusing is better than
/// truncating: half an address read by the camera leads nowhere, and the
/// error would only show up on the phone of whoever scanned it.
#[must_use]
pub fn generate(text: &str) -> Option<Matrix> {
    // `Low` correction: the content here is read off a screen, centimetres away and
    // with no dirt and no crease. High correction would spend modules — that is, a bigger
    // QR for the same text — to survive damage that is not going to exist.
    let code = qrcode::QrCode::with_error_correction_level(text, qrcode::EcLevel::L).ok()?;
    let side = code.width();
    let modules = code
        .to_colors()
        .into_iter()
        .map(|c| c == qrcode::Color::Dark)
        .collect();
    Some(Matrix { side, modules })
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_generated_qr_has_the_shape_of_a_qr() {
        // There is no way to READ the code back without a decoder — and bringing
        // one in just for the test would be bringing a dependency to prove what the
        // structure already proves. What is asserted here is what an invalid QR
        // would not have: an odd side >= 21, module count = side^2, and the
        // three position patterns in the corners.
        let m = super::generate("https://example.org").expect("texto curto tem de caber");
        assert!(m.side >= 21 && m.side % 2 == 1, "lado invalido: {}", m.side);
        assert_eq!(m.modules.len(), m.side * m.side);

        // THE THREE EYES. Every QR has a 7x7 pattern in the top
        // left, top right and bottom left corners — and none in the
        // bottom right. It is the cheapest signature of "this is a QR".
        let eye = |ox: usize, oy: usize| {
            // Dark border, light ring, dark core.
            m.dark(ox, oy)
                && m.dark(ox + 6, oy)
                && !m.dark(ox + 1, oy + 1)
                && m.dark(ox + 3, oy + 3)
        };
        assert!(eye(0, 0), "sem olho no canto superior esquerdo");
        assert!(eye(m.side - 7, 0), "sem olho no canto superior direito");
        assert!(eye(0, m.side - 7), "sem olho no canto inferior esquerdo");
    }

    #[test]
    fn text_bigger_than_fits_is_refused() {
        // REFUSE, and do not truncate. Half an address read by the camera leads
        // nowhere, and the error would only show up on the phone of whoever scanned it.
        let huge = "x".repeat(10_000);
        assert!(super::generate(&huge).is_none());
    }

    #[test]
    fn different_contents_give_different_matrices() {
        let a = super::generate("a").unwrap();
        let b = super::generate("b").unwrap();
        assert_ne!(a, b);
    }
}
