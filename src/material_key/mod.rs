mod hb_piece;
mod pawn_structure;

use crate::material_key::pawn_structure::PawnStructure;
use shakmaty::{Bitboard, Chess, Color, Position, Role, Square};
use std::fmt;

pub use hb_piece::{HbPiece, HbPieceRole};

/// Represents a material configuration, e.g. `KQvK`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MaterialKey {
    /// Piece counts indexed by color then piece descriptor.
    /// By convention the strong side is always first and it is encoded as white
    /// whenever we convert to a position.
    pub counts: [[u8; HbPieceRole::ALL.len()]; 2],
    pub pawns: PawnStructure,
}

impl MaterialKey {
    pub fn new(counts: [[u8; HbPieceRole::ALL.len()]; 2], pawns: PawnStructure) -> Self {
        let mut key = Self { counts, pawns };
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
    /// Supported tokens are `K`, `Q`, `R`, `Bl`, `Bd` and `N` for king,
    /// queen, rook, light-squared bishop, dark-squared bishop and knight respectively.
    /// Pieces appearing before the separator are interpreted as white, while those after
    /// it are treated as black.
    ///
    /// # Pawn tokens
    /// Pawn squares are a sequence of lowercase file/rank pairs (`a1`..`h8`).
    /// Pieces appearing before the separator are interpreted as white, while those after
    /// it are treated as black.
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
        let mut pawn_bitboards = [Bitboard::EMPTY, Bitboard::EMPTY];

        fn push_pieces(
            out: &mut [[u8; HbPieceRole::ALL.len()]; 2],
            pawns: &mut [Bitboard; 2],
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
                    'a'..='h' => {
                        if i + 1 >= bytes.len() {
                            return None;
                        }
                        let square = Square::from_ascii(&bytes[i..i + 2]).ok()?;
                        let occupied = pawns[0] | pawns[1];
                        if occupied.contains(square) {
                            return None;
                        }
                        pawns[color_idx].add(square);
                        i += 2;
                        continue;
                    }
                    _ => return None,
                };

                let pd = HbPieceRole::from_token(token)?;
                out[color_idx][pd as usize] += 1;
            }

            Some(())
        }

        push_pieces(&mut counts, &mut pawn_bitboards, white, Color::White)?;
        push_pieces(&mut counts, &mut pawn_bitboards, black, Color::Black)?;

        let pawns = PawnStructure::new(pawn_bitboards[0], pawn_bitboards[1]);

        Some(Self::new(counts, pawns))
    }

    pub fn non_pawn_piece_count(&self) -> u32 {
        self.counts
            .iter()
            .map(|side| side.iter().sum::<u8>())
            .sum::<u8>() as u32
    }

    pub fn total_piece_count(&self) -> u32 {
        self.counts
            .iter()
            .map(|side| side.iter().sum::<u8>())
            .sum::<u8>() as u32
            + self.pawns.occupied().count() as u32
    }

    fn canonicalize(&mut self) {
        // Ensure that the stronger side is white.
        if Self::strong_color_from_counts(&self.counts) == Color::Black {
            self.mirror_sides();
        }

        // Canonicalize bishop colors if needed.
        if self.should_swap_bishops() {
            self.mirror_left_to_right();
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

    /// Mirror the board left-to-right (kingside-to-queenside)
    fn mirror_left_to_right(&mut self) {
        // Flip the color of the bishops on the board.
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
        self.pawns.occupied().any()
    }

    pub fn has_bishops(&self) -> bool {
        let light_idx = HbPieceRole::LightBishop as usize;
        let dark_idx = HbPieceRole::DarkBishop as usize;
        self.counts[0][light_idx] > 0
            || self.counts[1][light_idx] > 0
            || self.counts[0][dark_idx] > 0
            || self.counts[1][dark_idx] > 0
    }

    /// Mirror the sides of the board (white-to-black and black-to-white)
    fn mirror_sides(&mut self) {
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

        // TODO: do we need to compare pawn counts here?

        Color::White
    }

    pub fn child_material_keys(&self) -> Vec<MaterialKey> {
        let mut children = Vec::new();

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
                // TODO: handle pawn movements
                children.push(MaterialKey::new(counts, self.pawns.clone()));
            }
        }

        // Promotions (with and without capture).
        // TODO: handle pawn promotions

        children
    }

    pub fn from_position(position: &Chess) -> Option<Self> {
        let mut counts = [[0u8; HbPieceRole::ALL.len()]; 2];
        for square in Square::ALL {
            if let Some(piece) = position.board().piece_at(square) {
                if piece.role == Role::Pawn {
                    continue;
                }

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
                    Role::Pawn => unreachable!(),
                };
                counts[color_idx][piece_idx] += 1;
            }
        }

        Some(MaterialKey::new(
            counts,
            PawnStructure::from_board(position.board()),
        ))
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
            let color = match color_idx {
                0 => Color::White,
                1 => Color::Black,
                _ => unreachable!(),
            };

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

            for square in self.pawns.0[color] {
                write!(f, "{}", square.to_string())?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_debug_snapshot;
    use shakmaty::{CastlingMode, fen::Fen};

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
    fn parses_kd2vkh7() {
        let key = MaterialKey::from_string("Kd2vKh7").unwrap();
        assert_eq!(key.to_string(), "Kd2vKh7");
    }

    #[test]
    fn child_material_keys_for_ke4vkn() {
        let key = MaterialKey::from_string("Ke4vKN").unwrap();
        let children = key
            .child_material_keys()
            .into_iter()
            .map(|k| k.to_string())
            .collect::<Vec<String>>();

        assert_debug_snapshot!(children, @r#"
        [
            "Ke4vK",
        ]
        "#);
    }

    #[test]
    fn material_key_from_position() {
        let position = "8/4k3/8/8/8/8/3P4/4K3 w - - 0 1"
            .parse::<Fen>()
            .unwrap()
            .into_position(CastlingMode::Standard)
            .unwrap();
        let key = MaterialKey::from_position(&position).unwrap();
        assert_eq!(key.to_string(), "Kd2vK");
    }
}
