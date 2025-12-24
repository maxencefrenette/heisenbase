use std::collections::BTreeSet;
use std::fmt;

use shakmaty::{Chess, Color, Position, Role, Square};

mod hb_piece;

pub use hb_piece::{HbPiece, HbPieceRole};

/// Represents a material configuration, e.g. `KQvK`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MaterialKey {
    /// Piece counts indexed by color then piece descriptor.
    /// By convention the strong side is always first and it is encoded as white
    /// whenever we convert to a position.
    pub counts: [[u8; HbPieceRole::ALL.len()]; 2],
}

impl MaterialKey {
    pub(crate) fn new(counts: [[u8; HbPieceRole::ALL.len()]; 2]) -> Self {
        let mut key = Self { counts };
        key.canonicalize();
        key
    }

    /// Parse a [`MaterialKey`] from its textual representation.
    ///
    /// # Syntax
    /// The expected form is `<white pieces>v<black pieces>`, where each side is
    /// a sequence of piece tokens and exactly one `v` separates the two
    /// sides.
    ///
    /// # Piece tokens and colors
    /// Supported tokens are `K`, `Q`, `R`, `Bl`, `Bd`, `N` and `P` for king,
    /// queen, rook, light-squared bishop, dark-squared bishop, knight and pawn
    /// respectively. Pieces appearing before the separator are interpreted as
    /// white, while those after it are treated as black.
    ///
    /// # Use cases
    /// This is primarily useful for tests and simple user interfaces that need
    /// to describe a set of pieces without board coordinates.
    ///
    /// # Errors
    /// Returns `None` if the string is malformed, contains unsupported
    /// tokens, has a missing or extra separator, or is otherwise ambiguous.
    pub fn from_string(s: &str) -> Option<Self> {
        let mut parts = s.split('v');
        let white = parts.next()?;
        let black = parts.next()?;

        // Only one separator is allowed.
        if parts.next().is_some() {
            return None;
        }

        let mut counts = [[0u8; HbPieceRole::ALL.len()]; 2];

        fn push_pieces(
            out: &mut [[u8; HbPieceRole::ALL.len()]; 2],
            s: &str,
            color: Color,
        ) -> Option<()> {
            let color_idx = match color {
                Color::White => 0,
                Color::Black => 1,
            };

            let bytes = s.as_bytes();
            let mut i = 0;
            while i < bytes.len() {
                let token = match bytes[i] as char {
                    'B' => {
                        if i + 1 >= bytes.len() {
                            return None;
                        }
                        match bytes[i + 1] as char {
                            'l' => {
                                i += 2;
                                "Bl"
                            }
                            'd' => {
                                i += 2;
                                "Bd"
                            }
                            _ => return None,
                        }
                    }
                    'K' => {
                        i += 1;
                        "K"
                    }
                    'Q' => {
                        i += 1;
                        "Q"
                    }
                    'R' => {
                        i += 1;
                        "R"
                    }
                    'N' => {
                        i += 1;
                        "N"
                    }
                    'P' => {
                        i += 1;
                        "P"
                    }
                    _ => return None,
                };

                let pd = HbPieceRole::from_token(token)?;
                out[color_idx][pd as usize] += 1;
            }

            Some(())
        }

        push_pieces(&mut counts, white, Color::White)?;
        push_pieces(&mut counts, black, Color::Black)?;

        Some(Self::new(counts))
    }

    pub fn non_pawn_piece_count(&self) -> u32 {
        let pawn_idx = HbPieceRole::Pawn as usize;
        self.counts
            .iter()
            .map(|side| {
                side.iter()
                    .enumerate()
                    .filter(|(idx, _)| *idx != pawn_idx)
                    .map(|(_, &count)| count as u32)
                    .sum::<u32>()
            })
            .sum()
    }

    pub fn total_piece_count(&self) -> u32 {
        self.counts
            .iter()
            .map(|side| side.iter().map(|&count| count as u32).sum::<u32>())
            .sum()
    }

    fn canonicalize(&mut self) {
        // Ensure that the stronger side is white.
        if Self::strong_color_from_counts(&self.counts) == Color::Black {
            self.swap_colors();
        }

        // Canonicalize bishop colors if needed.
        if self.should_swap_bishops() {
            self.flip_bishop_colors();
        }
    }

    fn should_swap_bishops(&self) -> bool {
        if self.has_pawns() {
            return false;
        }

        if !self.has_bishops() {
            return false;
        }

        let current = Self::flatten_counts(&self.counts);
        let swapped_counts = Self::swapped_bishop_counts(&self.counts);
        let swapped = Self::flatten_counts(&swapped_counts);
        swapped < current
    }

    fn flip_bishop_colors(&mut self) {
        let light_idx = HbPieceRole::LightBishop as usize;
        let dark_idx = HbPieceRole::DarkBishop as usize;
        for color_idx in 0..2 {
            let light = self.counts[color_idx][light_idx];
            let dark = self.counts[color_idx][dark_idx];
            self.counts[color_idx][light_idx] = dark;
            self.counts[color_idx][dark_idx] = light;
        }
    }

    fn swapped_bishop_counts(
        counts: &[[u8; HbPieceRole::ALL.len()]; 2],
    ) -> [[u8; HbPieceRole::ALL.len()]; 2] {
        let mut swapped = *counts;
        let light_idx = HbPieceRole::LightBishop as usize;
        let dark_idx = HbPieceRole::DarkBishop as usize;
        for color_idx in 0..2 {
            swapped[color_idx][light_idx] = counts[color_idx][dark_idx];
            swapped[color_idx][dark_idx] = counts[color_idx][light_idx];
        }
        swapped
    }

    fn flatten_counts(
        counts: &[[u8; HbPieceRole::ALL.len()]; 2],
    ) -> [u8; HbPieceRole::ALL.len() * 2] {
        let mut flat = [0u8; HbPieceRole::ALL.len() * 2];
        for color_idx in 0..2 {
            for piece_idx in 0..HbPieceRole::ALL.len() {
                flat[color_idx * HbPieceRole::ALL.len() + piece_idx] = counts[color_idx][piece_idx];
            }
        }
        flat
    }

    pub fn has_pawns(&self) -> bool {
        let pawn_idx = HbPieceRole::Pawn as usize;
        self.counts[0][pawn_idx] > 0 || self.counts[1][pawn_idx] > 0
    }

    pub fn has_bishops(&self) -> bool {
        let light_idx = HbPieceRole::LightBishop as usize;
        let dark_idx = HbPieceRole::DarkBishop as usize;
        self.counts[0][light_idx] > 0
            || self.counts[1][light_idx] > 0
            || self.counts[0][dark_idx] > 0
            || self.counts[1][dark_idx] > 0
    }

    fn swap_colors(&mut self) {
        self.counts.swap(0, 1);
    }

    /// Determines which color has the stronger material based on piece counts.
    ///
    /// By design, this matches the logic used by syzygy tablebases.
    ///
    /// The first factor is the total piece count.
    /// Then, it's which side has the strongest piece.
    /// Finally, In case of a tie, White is considered stronger.
    fn strong_color_from_counts(counts: &[[u8; HbPieceRole::ALL.len()]; 2]) -> Color {
        let compare = |white: u8, black: u8| -> Option<Color> {
            if white > black {
                Some(Color::White)
            } else if black > white {
                Some(Color::Black)
            } else {
                None
            }
        };

        let queen_idx = HbPieceRole::Queen as usize;
        if let Some(color) = compare(counts[0][queen_idx], counts[1][queen_idx]) {
            return color;
        }

        let rook_idx = HbPieceRole::Rook as usize;
        if let Some(color) = compare(counts[0][rook_idx], counts[1][rook_idx]) {
            return color;
        }

        let light_idx = HbPieceRole::LightBishop as usize;
        let dark_idx = HbPieceRole::DarkBishop as usize;
        let white_bishops = counts[0][light_idx] + counts[0][dark_idx];
        let black_bishops = counts[1][light_idx] + counts[1][dark_idx];
        if let Some(color) = compare(white_bishops, black_bishops) {
            return color;
        }

        let knight_idx = HbPieceRole::Knight as usize;
        if let Some(color) = compare(counts[0][knight_idx], counts[1][knight_idx]) {
            return color;
        }

        let pawn_idx = HbPieceRole::Pawn as usize;
        if let Some(color) = compare(counts[0][pawn_idx], counts[1][pawn_idx]) {
            return color;
        }

        Color::White
    }

    pub fn child_material_keys(&self) -> Vec<MaterialKey> {
        let mut children = BTreeSet::new();

        // Captures: any move that removes an opponent piece (except the king).
        for color_idx in 0..2 {
            let opponent = 1 - color_idx;
            for piece_idx in 0..HbPieceRole::ALL.len() {
                if piece_idx == HbPieceRole::King as usize {
                    continue;
                }
                if self.counts[opponent][piece_idx] == 0 {
                    continue;
                }
                let mut counts = self.counts;
                counts[opponent][piece_idx] -= 1;
                children.insert(MaterialKey::new(counts));
            }
        }

        // Promotions (with and without capture).
        let promo_targets = [
            HbPieceRole::Queen,
            HbPieceRole::Rook,
            HbPieceRole::LightBishop,
            HbPieceRole::DarkBishop,
            HbPieceRole::Knight,
        ];
        let pawn_idx = HbPieceRole::Pawn as usize;
        for color_idx in 0..2 {
            if self.counts[color_idx][pawn_idx] == 0 {
                continue;
            }
            let opponent = 1 - color_idx;
            for target in promo_targets {
                let target_idx = target as usize;
                let mut promo_counts = self.counts;
                promo_counts[color_idx][pawn_idx] -= 1;
                promo_counts[color_idx][target_idx] += 1;
                children.insert(MaterialKey::new(promo_counts));

                for capture_idx in 0..HbPieceRole::ALL.len() {
                    if capture_idx == HbPieceRole::King as usize {
                        continue;
                    }
                    if self.counts[opponent][capture_idx] == 0 {
                        continue;
                    }
                    let mut capture_counts = promo_counts;
                    capture_counts[opponent][capture_idx] -= 1;
                    children.insert(MaterialKey::new(capture_counts));
                }
            }
        }

        children.into_iter().collect()
    }

    pub fn from_position(position: &Chess) -> Option<Self> {
        let mut counts = [[0u8; HbPieceRole::ALL.len()]; 2];
        for square in Square::ALL {
            if let Some(piece) = position.board().piece_at(square) {
                let color_idx = match piece.color {
                    Color::White => 0,
                    Color::Black => 1,
                };
                let piece_idx = match piece.role {
                    Role::King => HbPieceRole::King as usize,
                    Role::Queen => HbPieceRole::Queen as usize,
                    Role::Rook => HbPieceRole::Rook as usize,
                    Role::Bishop => {
                        if square.is_light() {
                            HbPieceRole::LightBishop as usize
                        } else {
                            HbPieceRole::DarkBishop as usize
                        }
                    }
                    Role::Knight => HbPieceRole::Knight as usize,
                    Role::Pawn => HbPieceRole::Pawn as usize,
                };
                counts[color_idx][piece_idx] += 1;
            }
        }

        Some(MaterialKey::new(counts))
    }

    pub fn pieces(&self) -> impl Iterator<Item = HbPiece> {
        (0..2).flat_map(move |color_index| {
            let color = match color_index {
                0 => Color::White,
                1 => Color::Black,
                _ => unreachable!(),
            };

            HbPieceRole::ALL.iter().copied().flat_map(move |piece| {
                (0..self.counts[color_index][piece as usize])
                    .map(move |_| HbPiece { role: piece, color })
            })
        })
    }
}

impl fmt::Display for MaterialKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn write_bishops(f: &mut fmt::Formatter<'_>, light: u8, dark: u8) -> fmt::Result {
            for _ in 0..dark {
                write!(f, "{}", HbPieceRole::DarkBishop.token())?;
            }
            for _ in 0..light {
                write!(f, "{}", HbPieceRole::LightBishop.token())?;
            }
            Ok(())
        }

        let light_idx = HbPieceRole::LightBishop as usize;
        let dark_idx = HbPieceRole::DarkBishop as usize;

        for color_idx in 0..2 {
            if color_idx == 1 {
                write!(f, "v")?;
            }

            // Manually emit pieces in canonical order to keep output stable.
            let counts = &self.counts[color_idx];

            // Always write the king first.
            for _ in 0..counts[HbPieceRole::King as usize] {
                write!(f, "{}", HbPieceRole::King.token())?;
            }

            for _ in 0..counts[HbPieceRole::Queen as usize] {
                write!(f, "{}", HbPieceRole::Queen.token())?;
            }

            for _ in 0..counts[HbPieceRole::Rook as usize] {
                write!(f, "{}", HbPieceRole::Rook.token())?;
            }

            let light = counts[light_idx];
            let dark = counts[dark_idx];
            if light > 0 || dark > 0 {
                write_bishops(f, light, dark)?;
            }

            for _ in 0..counts[HbPieceRole::Knight as usize] {
                write!(f, "{}", HbPieceRole::Knight.token())?;
            }

            for _ in 0..counts[HbPieceRole::Pawn as usize] {
                write!(f, "{}", HbPieceRole::Pawn.token())?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shakmaty::{CastlingMode, fen::Fen};
    use std::collections::BTreeSet;

    fn material_key(s: &str) -> String {
        MaterialKey::from_string(s).unwrap().to_string()
    }

    #[test]
    fn parses_kqvk() {
        assert_eq!(material_key("KQvK"), "KQvK");
    }

    #[test]
    fn parses_light_and_dark_bishops() {
        assert_eq!(material_key("KBdBlvK"), "KBdBlvK");
    }

    #[test]
    fn canonicalizes_bishop_colors_in_material_key_1() {
        assert_eq!(material_key("KBlvKBd"), "KBdvKBl");
    }

    #[test]
    fn canonicalizes_bishop_colors_in_material_key_2() {
        assert_eq!(material_key("KBlBlBdvK"), "KBdBdBlvK");
    }

    #[test]
    fn rejects_invalid_char() {
        assert!(MaterialKey::from_string("KXvK").is_none());
    }

    #[test]
    fn rejects_missing_separator() {
        assert!(MaterialKey::from_string("KQK").is_none());
    }

    #[test]
    fn child_keys_for_kpvk() {
        let key = MaterialKey::from_string("KPvK").unwrap();
        let children: BTreeSet<String> = key
            .child_material_keys()
            .into_iter()
            .map(|k| k.to_string())
            .collect();
        let expected: BTreeSet<String> = ["KvK", "KQvK", "KRvK", "KBdvK", "KNvK"]
            .into_iter()
            .map(String::from)
            .collect();
        assert_eq!(children, expected);
    }

    #[test]
    fn material_key_from_position() {
        let position = "8/4k3/8/8/8/8/3P4/4K3 w - - 0 1"
            .parse::<Fen>()
            .unwrap()
            .into_position(CastlingMode::Standard)
            .unwrap();
        let key = MaterialKey::from_position(&position).unwrap();
        assert_eq!(key.to_string(), "KPvK");
    }
}
