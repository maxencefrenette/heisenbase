mod hb_piece;
mod pawn_structure;

use crate::material_key::pawn_structure::PawnStructure;
use itertools::iproduct;
use shakmaty::{Bitboard, Chess, Color, Position, Role, Square};
use std::{cmp::Ordering, fmt, iter::once};

pub use hb_piece::{HbPiece, HbPieceRole};

/// Represents a material configuration, e.g. `KQvK`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MaterialKey {
    pub pawns: PawnStructure,
    /// Piece counts indexed by color then piece descriptor.
    /// By convention the strong side is encoded as white when pawn structures
    /// are symmetric under a vertical flip; otherwise pawn-structure ordering
    /// takes precedence and the stronger side may appear second.
    pub counts: [[u8; HbPieceRole::ALL.len()]; 2],
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
    /// # Errors
    /// Returns `None` if the string is malformed, contains unsupported
    /// tokens, has a missing or extra separator, or is otherwise ambiguous.
    pub fn from_string(s: &str) -> Option<Self> {
        let (white, black) = s.split_once('v')?;
        if black.contains('v') {
            return None;
        }

        let mut counts = [[0u8; HbPieceRole::ALL.len()]; 2];
        let mut pawn_bitboards = [Bitboard::EMPTY, Bitboard::EMPTY];
        let mut occupied = Bitboard::EMPTY;

        fn push_pieces(
            out: &mut [[u8; HbPieceRole::ALL.len()]; 2],
            pawns: &mut [Bitboard; 2],
            occupied: &mut Bitboard,
            s: &str,
            color_idx: usize,
        ) -> Option<()> {
            let bytes = s.as_bytes();
            let mut i = 0;
            while i < bytes.len() {
                match bytes[i] {
                    b'K' => {
                        out[color_idx][HbPieceRole::King as usize] += 1;
                        i += 1;
                    }
                    b'Q' => {
                        out[color_idx][HbPieceRole::Queen as usize] += 1;
                        i += 1;
                    }
                    b'R' => {
                        out[color_idx][HbPieceRole::Rook as usize] += 1;
                        i += 1;
                    }
                    b'N' => {
                        out[color_idx][HbPieceRole::Knight as usize] += 1;
                        i += 1;
                    }
                    b'B' => {
                        let next = *bytes.get(i + 1)?;
                        let role = match next {
                            b'l' => HbPieceRole::LightBishop,
                            b'd' => HbPieceRole::DarkBishop,
                            _ => return None,
                        };
                        out[color_idx][role as usize] += 1;
                        i += 2;
                    }
                    b'a'..=b'h' => {
                        if i + 1 >= bytes.len() {
                            return None;
                        }
                        let square = Square::from_ascii(&bytes[i..i + 2]).ok()?;
                        if occupied.contains(square) {
                            return None;
                        }
                        pawns[color_idx].add(square);
                        occupied.add(square);
                        i += 2;
                    }
                    _ => return None,
                }
            }

            Some(())
        }

        push_pieces(&mut counts, &mut pawn_bitboards, &mut occupied, white, 0)?;
        push_pieces(&mut counts, &mut pawn_bitboards, &mut occupied, black, 1)?;

        let king_idx = HbPieceRole::King as usize;
        if counts[0][king_idx] != 1 || counts[1][king_idx] != 1 {
            return None;
        }

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
        let order = self.pawns.0.white.cmp(&self.pawns.0.black.flip_vertical());
        match order {
            Ordering::Less => {
                self.mirror_sides();
                return;
            }
            Ordering::Equal => (),
            Ordering::Greater => {
                return;
            }
        }

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
        if !self.has_bishops() {
            return false;
        }

        if !self.pawns.is_symmetric_horizontal() {
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

        self.pawns = self.pawns.flip_horizontal();
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
        self.pawns = self.pawns.flip_sides();
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

        Color::White
    }

    pub fn child_material_keys(&self) -> Vec<MaterialKey> {
        let mut children = Vec::new();

        // Simple pawn moves
        children.extend(
            self.pawns
                .child_pawn_structures_no_piece_change()
                .into_iter()
                .map(|ps| MaterialKey::new(self.counts, ps)),
        );

        // Captures: any move that removes an opponent piece (except the king).
        for color in [Color::White, Color::Black] {
            let opponent = color.other();
            let color_idx = match color {
                Color::White => 0,
                Color::Black => 1,
            };
            let opponent_idx = match opponent {
                Color::White => 0,
                Color::Black => 1,
            };

            // Piece captures
            children.extend(
                iproduct!(
                    once(self.pawns.clone())
                        .chain(self.pawns.child_pawn_structures_with_piece_capture(color)),
                    HbPieceRole::CAPTURABLE,
                )
                .filter_map(|(ps, role)| {
                    if self.counts[opponent_idx][role as usize] == 0 {
                        return None;
                    }

                    let mut counts = self.counts;
                    counts[opponent_idx][role as usize] -= 1;
                    Some(MaterialKey::new(counts, ps))
                }),
            );

            // Promotions without piece captures
            children.extend(
                iproduct!(
                    self.pawns.child_pawn_structures_with_promotion(color),
                    HbPieceRole::CAPTURABLE,
                )
                .filter_map(|(ps, role)| {
                    if self.counts[color_idx][role as usize] == 0 {
                        return None;
                    }

                    let mut counts = self.counts;
                    counts[color_idx][role as usize] += 1;
                    Some(MaterialKey::new(counts, ps))
                }),
            );

            // Promotions with piece captures
            children.extend(
                iproduct!(
                    self.pawns.child_pawn_structures_with_promotion(color),
                    HbPieceRole::CAPTURABLE,
                    HbPieceRole::CAPTURABLE,
                )
                .filter_map(|(ps, role1, role2)| {
                    if self.counts[color_idx][role1 as usize] == 0 {
                        return None;
                    }

                    if self.counts[opponent_idx][role2 as usize] == 0 {
                        return None;
                    }

                    let mut counts = self.counts;
                    counts[color_idx][role1 as usize] += 1;
                    counts[opponent_idx][role2 as usize] -= 1;
                    Some(MaterialKey::new(counts, ps))
                }),
            );
        }

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
    use proptest::{prelude::*, string::string_regex};
    use shakmaty::{CastlingMode, fen::Fen};

    fn material_key(s: &str) -> String {
        MaterialKey::from_string(s).unwrap().to_string()
    }

    fn material_key_string_strategy() -> impl Strategy<Value = String> {
        string_regex("K(Q|R|Bl|Bd|N){0,2}([a-h][2-7]){0,3}vK(Q|R|Bl|Bd|N){0,2}([a-h][2-7]){0,3}")
            .unwrap()
            .prop_filter("valid material key", |value| {
                MaterialKey::from_string(value).is_some()
            })
    }

    proptest! {
        #[test]
        fn roundtrip_parsing(key in material_key_string_strategy()) {
            let parsed = MaterialKey::from_string(&key).expect("filtered to valid material keys");
            let rendered = parsed.to_string();
            let reparsed = MaterialKey::from_string(&rendered)
                .expect("rendered material keys should parse");
            prop_assert_eq!(&parsed, &reparsed);
            prop_assert_eq!(rendered, reparsed.to_string());
        }
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
    fn rejects_missing_king() {
        assert!(MaterialKey::from_string("QvK").is_none());
    }

    #[test]
    fn rejects_empty_string() {
        assert!(MaterialKey::from_string("").is_none());
    }

    #[test]
    fn rejects_incomplete_pawn_square() {
        assert!(MaterialKey::from_string("KavK").is_none());
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
            "Ke5vKN",
            "Ke4vK",
            "Kf5vK",
            "Kd5vK",
        ]
        "#);
    }

    #[test]
    fn child_material_keys_for_kqvk() {
        let key = MaterialKey::from_string("KQvK").unwrap();
        let children = key
            .child_material_keys()
            .into_iter()
            .map(|k| k.to_string())
            .collect::<Vec<String>>();

        assert_debug_snapshot!(children, @r#"
        [
            "KvK",
        ]
        "#);
    }

    #[test]
    fn child_material_keys_for_kbld3vk() {
        let key = MaterialKey::from_string("KBld3vK").unwrap();
        let children = key
            .child_material_keys()
            .into_iter()
            .map(|k| k.to_string())
            .collect::<Vec<String>>();

        assert_debug_snapshot!(children, @r#"
        [
            "KBld4vK",
            "Kd3vK",
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

    #[test]
    fn non_pawn_piece_count_includes_kings() {
        assert_eq!(
            MaterialKey::from_string("KQvK")
                .unwrap()
                .non_pawn_piece_count(),
            3
        );
    }
}
