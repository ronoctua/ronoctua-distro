//! Accents off, so `calculo` finds `Cálculo`.
//!
//! # Why 1:1, and not a actual normalisation
//!
//! Because the search returns the **positions** of the letters that matched, and the drawing
//! lights up exactly those positions (`runs_of`, in the footer's `surface`). If the folding
//! changed the length — `ß` → `ss`, `æ` → `ae` — the positions would belong to one
//! ruler and the text to another, and the highlight would light the wrong letter
//! **without getting anything else wrong**: the list would still be right and only the emphasis would lie.
//!
//! It is the same rule text clipping learned in `layout::elide_end`, from
//! a similar defect: one unit, from beginning to end.
//!
//! That is why the table is `char → char` and nothing more. A character with no
//! single-character equivalent stays as it is — worse would be for it to shift everything.

/// A character without an accent, if there is a single-character one.
#[must_use]
pub const fn folded_char(c: char) -> char {
    match c {
        'á' | 'à' | 'â' | 'ã' | 'ä' | 'å' => 'a',
        'Á' | 'À' | 'Â' | 'Ã' | 'Ä' | 'Å' => 'A',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'É' | 'È' | 'Ê' | 'Ë' => 'E',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'Í' | 'Ì' | 'Î' | 'Ï' => 'I',
        'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
        'Ó' | 'Ò' | 'Ô' | 'Õ' | 'Ö' => 'O',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'Ú' | 'Ù' | 'Û' | 'Ü' => 'U',
        'ç' => 'c',
        'Ç' => 'C',
        'ñ' => 'n',
        'Ñ' => 'N',
        'ý' | 'ÿ' => 'y',
        'Ý' => 'Y',
        otherwise => otherwise,
    }
}

/// The folded, lower-cased string — the form searching runs over.
#[must_use]
pub fn fold(s: &str) -> String {
    // `flat_map` over `to_lowercase` would be the correct thing for the whole of Unicode and
    // would break 1:1 (the Turkish `İ` becomes two). `to_ascii_lowercase` after the
    // folding solves our two languages while keeping the ruler.
    s.chars()
        .map(|c| folded_char(c).to_ascii_lowercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::fold;

    #[test]
    fn the_accent_goes_and_the_case_lowers() {
        // pt-BR: the accent IS the subject — these are what folding folds.
        assert_eq!(fold("Cálculo"), "calculo");
        assert_eq!(fold("Configurações"), "configuracoes");
        assert_eq!(fold("Área de Trabalho"), "area de trabalho");
    }

    /// THE INVARIANT the highlighting depends on: a single ruler.
    ///
    /// If someone one day swaps the table for a actual normalisation,
    /// this test fails — and it fails BEFORE the emphasis starts lighting the wrong
    /// letter on screen, which is a defect nobody connects to the folding.
    #[test]
    fn the_folding_preserves_the_character_count() {
        for s in [
            "Cálculo",
            "Configurações",
            "Ámbar Ñandú Çedilha",
            "no accent at all",
            "ÀÁÂÃÄÅÈÉÊËÌÍÎÏÒÓÔÕÖÙÚÛÜÝ",
        ] {
            assert_eq!(
                fold(s).chars().count(),
                s.chars().count(),
                "the fold changed the length of {s:?}"
            );
        }
    }
}
