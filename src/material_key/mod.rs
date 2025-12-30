mod hb_piece;
mod pawn_structure;
mod piece_counts;

use crate::material_key::pawn_structure::PawnStructure;
use itertools::iproduct;
use shakmaty::{Bitboard, ByColor, Chess, Color, Position, Role, Square};
use std::{cmp::Ordering, collections::BTreeSet, fmt, iter::once};
use winnow::ModalResult;
use winnow::combinator::{alt, eof, fail, repeat, separated_pair, terminated};
use winnow::prelude::*;
use winnow::token::{literal, take};

pub use hb_piece::{HbPiece, HbPieceRole};
pub use piece_counts::PieceCounts;

/// Represents a material configuration, e.g. `KQvK`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MaterialKey {
    pub pawns: PawnStructure,
    pub counts: ByColor<PieceCounts>,
}

impl MaterialKey {
    pub fn new(counts: ByColor<PieceCounts>, pawns: PawnStructure) -> Self {
        let key = Self { counts, pawns };
        key.into_normalized()
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
        fn pawn_square(input: &mut &[u8]) -> ModalResult<Square> {
            Square::from_ascii(take(2usize).parse_next(input)?)
                .or_else(|_| fail.parse_next(input)?)
        }

        fn token(input: &mut &[u8]) -> ModalResult<HbPieceRole> {
            alt((
                'Q'.value(HbPieceRole::Queen),
                'R'.value(HbPieceRole::Rook),
                'N'.value(HbPieceRole::Knight),
                "Bl".value(HbPieceRole::LightBishop),
                "Bd".value(HbPieceRole::DarkBishop),
            ))
            .parse_next(input)
        }

        fn side(input: &mut &[u8]) -> ModalResult<(PieceCounts, Bitboard)> {
            literal("K").parse_next(input)?;

            let tokens: Vec<HbPieceRole> = repeat(0.., token).parse_next(input)?;
            let mut piece_counts = PieceCounts::empty();
            for role in tokens {
                piece_counts[role] += 1;
            }
            let squares: Vec<Square> = repeat(0.., pawn_square).parse_next(input)?;
            let bitboard = squares.into_iter().collect::<Bitboard>();

            Ok((piece_counts, bitboard))
        }

        let mut input = s.as_bytes();
        let ((white_piece_counts, white_pawns), (black_piece_counts, black_pawns)) =
            terminated(separated_pair(side, 'v', side), eof)
                .parse_next(&mut input)
                .ok()?;

        let mut counts = ByColor {
            white: white_piece_counts,
            black: black_piece_counts,
        };
        let pawns = PawnStructure::new(white_pawns, black_pawns).ok()?;

        // Add kings
        counts.white[HbPieceRole::King] += 1;
        counts.black[HbPieceRole::King] += 1;

        Some(Self::new(counts, pawns))
    }

    pub fn non_pawn_piece_count(&self) -> u32 {
        self.counts.iter().map(|side| side.total()).sum::<u8>() as u32
    }

    pub fn total_piece_count(&self) -> u32 {
        self.counts.iter().map(|side| side.total()).sum::<u8>() as u32
            + self.pawns.occupied().count() as u32
    }

    fn swap_bishop_counts(&mut self) {
        self.counts.for_each(|mut side| side.swap_bishops());
    }

    /// Mirror the sides of the board (white-to-black and black-to-white)
    fn into_mirrored_sides(mut self) -> Self {
        std::mem::swap(&mut self.counts.white, &mut self.counts.black);
        self.swap_bishop_counts();
        self.pawns = self.pawns.flip_sides();
        self
    }

    /// Mirror the board left-to-right (kingside-to-queenside)
    fn into_mirrored_left_to_right(mut self) -> Self {
        self.swap_bishop_counts();
        self.pawns = self.pawns.flip_horizontal();
        self
    }

    fn into_normalized(self) -> Self {
        [
            self.clone(),
            self.clone().into_mirrored_sides(),
            self.clone().into_mirrored_left_to_right(),
            self.clone()
                .into_mirrored_sides()
                .into_mirrored_left_to_right(),
        ]
        .into_iter()
        .min()
        .unwrap()
    }

    pub fn child_material_keys(&self) -> BTreeSet<MaterialKey> {
        let mut children = BTreeSet::new();

        // Simple pawn moves
        children.extend(
            self.pawns
                .child_pawn_structures_no_piece_change()
                .into_iter()
                .map(|ps| MaterialKey::new(self.counts.clone(), ps)),
        );

        // Captures: any move that removes an opponent piece (except the king).
        for color in Color::ALL {
            let opponent = color.other();
            // Piece captures
            children.extend(
                iproduct!(
                    once(self.pawns.clone())
                        .chain(self.pawns.child_pawn_structures_with_piece_capture(color)),
                    HbPieceRole::CAPTURABLE,
                )
                .filter_map(|(ps, role)| {
                    if self.counts[opponent][role] == 0 {
                        return None;
                    }

                    let mut counts = self.counts.clone();
                    counts[opponent][role] -= 1;
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
                    if self.counts[color][role] == 0 {
                        return None;
                    }

                    let mut counts = self.counts.clone();
                    counts[color][role] += 1;
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
                    if self.counts[color][role1] == 0 {
                        return None;
                    }

                    if self.counts[opponent][role2] == 0 {
                        return None;
                    }

                    let mut counts = self.counts.clone();
                    counts[color][role1] += 1;
                    counts[opponent][role2] -= 1;
                    Some(MaterialKey::new(counts, ps))
                }),
            );
        }

        children
    }

    pub fn from_position(position: &Chess) -> Option<Self> {
        let mut counts = ByColor::new_with(|_| PieceCounts::empty());
        for square in Square::ALL {
            if let Some(piece) = position.board().piece_at(square) {
                let piece_idx = match piece.role {
                    Role::King => HbPieceRole::King,
                    Role::Queen => HbPieceRole::Queen,
                    Role::Rook => HbPieceRole::Rook,
                    Role::Bishop => {
                        if square.is_light() {
                            HbPieceRole::LightBishop
                        } else {
                            HbPieceRole::DarkBishop
                        }
                    }
                    Role::Knight => HbPieceRole::Knight,
                    Role::Pawn => {
                        continue;
                    }
                };
                counts[piece.color][piece_idx] += 1;
            }
        }

        Some(MaterialKey::new(
            counts,
            PawnStructure::from_board(position.board()),
        ))
    }

    pub fn pieces(&self) -> impl Iterator<Item = HbPiece> {
        Color::ALL.into_iter().flat_map(move |color| {
            HbPieceRole::ALL.iter().copied().flat_map(move |piece| {
                (0..self.counts[color][piece]).map(move |_| HbPiece { role: piece, color })
            })
        })
    }
}

impl fmt::Display for MaterialKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for color in Color::ALL {
            if color == Color::Black {
                write!(f, "v")?;
            }

            // Manually emit pieces in canonical order to keep output stable.
            let counts = self.counts[color];

            for role in HbPieceRole::ALL {
                for _ in 0..counts[role] {
                    write!(f, "{}", role.token())?;
                }
            }

            for square in self.pawns.0[color] {
                write!(f, "{}", square)?;
            }
        }

        Ok(())
    }
}

impl PartialOrd for MaterialKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MaterialKey {
    fn cmp(&self, other: &Self) -> Ordering {
        // This is used to normalize the material key, so we compare with a specific ordering to
        // ensure that normalized material keys visually look nice.
        self.pawns
            .0
            .black
            .cmp(&other.pawns.0.black)
            .then_with(|| self.pawns.0.white.cmp(&other.pawns.0.white))
            .then_with(|| self.counts.white.cmp(&other.counts.white).reverse())
            .then_with(|| self.counts.black.cmp(&other.counts.black).reverse())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::{assert_debug_snapshot, assert_snapshot};
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
        assert_snapshot!(material_key("KQvK"), @"KQvK");
    }

    #[test]
    fn parses_light_and_dark_bishops() {
        assert_snapshot!(material_key("KBdBlvK"), @"KBdBlvK");
    }

    #[test]
    fn canonicalizes_bishop_colors_in_material_key_1() {
        assert_snapshot!(material_key("KBlvKBd"), @"KBlvKBd");
    }

    #[test]
    fn canonicalizes_bishop_colors_in_material_key_2() {
        assert_snapshot!(material_key("KBlBlBdvK"), @"KBdBlBlvK");
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
    fn rejects_overlapping_pawns() {
        assert!(MaterialKey::from_string("Ke4vKe4").is_none());
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
            "KNvK",
            "Kd4vK",
            "Kc5vK",
            "Kd5vKN",
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
            "KBlvK",
            "Kd3vK",
            "KBld4vK",
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
