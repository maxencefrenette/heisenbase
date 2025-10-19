use std::collections::BTreeSet;
use std::fmt;

use crate::transform::{Transform, TransformSet};
use shakmaty::{Chess, Color, Position, PositionErrorKinds, Role, Square};

/// Represents a material configuration, e.g. `KQvK`.

#[derive(Clone, Copy)]
pub(crate) enum PieceDescriptor {
    King,
    Queen,
    Rook,
    LightBishop,
    DarkBishop,
    Knight,
    Pawn,
}

impl PieceDescriptor {
    fn token(self) -> &'static str {
        match self {
            PieceDescriptor::King => "K",
            PieceDescriptor::Queen => "Q",
            PieceDescriptor::Rook => "R",
            PieceDescriptor::LightBishop => "Bl",
            PieceDescriptor::DarkBishop => "Bd",
            PieceDescriptor::Knight => "N",
            PieceDescriptor::Pawn => "P",
        }
    }

    pub(crate) fn role(self) -> Role {
        match self {
            PieceDescriptor::King => Role::King,
            PieceDescriptor::Queen => Role::Queen,
            PieceDescriptor::Rook => Role::Rook,
            PieceDescriptor::LightBishop | PieceDescriptor::DarkBishop => Role::Bishop,
            PieceDescriptor::Knight => Role::Knight,
            PieceDescriptor::Pawn => Role::Pawn,
        }
    }

    pub(crate) fn light(self) -> Option<bool> {
        match self {
            PieceDescriptor::LightBishop => Some(true),
            PieceDescriptor::DarkBishop => Some(false),
            _ => None,
        }
    }

    fn from_token(tok: &str) -> Option<Self> {
        match tok {
            "K" => Some(PieceDescriptor::King),
            "Q" => Some(PieceDescriptor::Queen),
            "R" => Some(PieceDescriptor::Rook),
            "Bl" => Some(PieceDescriptor::LightBishop),
            "Bd" => Some(PieceDescriptor::DarkBishop),
            "N" => Some(PieceDescriptor::Knight),
            "P" => Some(PieceDescriptor::Pawn),
            _ => None,
        }
    }
}

pub(crate) const PIECES: [PieceDescriptor; 7] = [
    PieceDescriptor::King,
    PieceDescriptor::Queen,
    PieceDescriptor::Rook,
    PieceDescriptor::LightBishop,
    PieceDescriptor::DarkBishop,
    PieceDescriptor::Knight,
    PieceDescriptor::Pawn,
];

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MaterialKey {
    /// Piece counts indexed by color then piece descriptor.
    pub(crate) counts: [[u8; PIECES.len()]; 2],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaterialError {
    MismatchedMaterial,
    IndexOutOfBounds,
    InvalidPosition(PositionErrorKinds),
}

impl MaterialKey {
    pub(crate) fn new(counts: [[u8; PIECES.len()]; 2]) -> Self {
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

        let mut counts = [[0u8; PIECES.len()]; 2];

        fn push_pieces(out: &mut [[u8; PIECES.len()]; 2], s: &str, color: Color) -> Option<()> {
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

                let pd = PieceDescriptor::from_token(token)?;
                out[color_idx][pd as usize] += 1;
            }

            Some(())
        }

        push_pieces(&mut counts, white, Color::White)?;
        push_pieces(&mut counts, black, Color::Black)?;

        Some(Self::new(counts))
    }

    fn canonicalize(&mut self) {
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
        let light_idx = PieceDescriptor::LightBishop as usize;
        let dark_idx = PieceDescriptor::DarkBishop as usize;
        for color_idx in 0..2 {
            let light = self.counts[color_idx][light_idx];
            let dark = self.counts[color_idx][dark_idx];
            self.counts[color_idx][light_idx] = dark;
            self.counts[color_idx][dark_idx] = light;
        }
    }

    fn swapped_bishop_counts(counts: &[[u8; PIECES.len()]; 2]) -> [[u8; PIECES.len()]; 2] {
        let mut swapped = *counts;
        let light_idx = PieceDescriptor::LightBishop as usize;
        let dark_idx = PieceDescriptor::DarkBishop as usize;
        for color_idx in 0..2 {
            swapped[color_idx][light_idx] = counts[color_idx][dark_idx];
            swapped[color_idx][dark_idx] = counts[color_idx][light_idx];
        }
        swapped
    }

    fn flatten_counts(counts: &[[u8; PIECES.len()]; 2]) -> [u8; PIECES.len() * 2] {
        let mut flat = [0u8; PIECES.len() * 2];
        for color_idx in 0..2 {
            for piece_idx in 0..PIECES.len() {
                flat[color_idx * PIECES.len() + piece_idx] = counts[color_idx][piece_idx];
            }
        }
        flat
    }

    pub(crate) fn has_pawns(&self) -> bool {
        let pawn_idx = PieceDescriptor::Pawn as usize;
        self.counts[0][pawn_idx] > 0 || self.counts[1][pawn_idx] > 0
    }

    pub(crate) fn has_bishops(&self) -> bool {
        let light_idx = PieceDescriptor::LightBishop as usize;
        let dark_idx = PieceDescriptor::DarkBishop as usize;
        self.counts[0][light_idx] > 0
            || self.counts[1][light_idx] > 0
            || self.counts[0][dark_idx] > 0
            || self.counts[1][dark_idx] > 0
    }

    pub(crate) fn strong_color(&self) -> Color {
        let order = [
            PieceDescriptor::Queen as usize,
            PieceDescriptor::Rook as usize,
            PieceDescriptor::LightBishop as usize,
            PieceDescriptor::DarkBishop as usize,
            PieceDescriptor::Knight as usize,
            PieceDescriptor::Pawn as usize,
        ];

        for &idx in &order {
            let white = self.counts[0][idx];
            let black = self.counts[1][idx];
            if white > black {
                return Color::White;
            } else if black > white {
                return Color::Black;
            }
        }

        Color::White
    }

    pub(crate) fn transform_set(&self) -> TransformSet {
        match (!self.has_pawns(), self.has_bishops()) {
            (true, false) => TransformSet::Full,
            (true, true) => TransformSet::Rotations,
            (false, false) => TransformSet::AxisFlips,
            (false, true) => TransformSet::HalfTurn,
        }
    }

    pub(crate) fn allowed_transforms(&self) -> &'static [Transform] {
        self.transform_set().transforms()
    }

    pub(crate) fn child_material_keys(&self) -> Vec<MaterialKey> {
        let mut children = BTreeSet::new();

        // Captures: any move that removes an opponent piece (except the king).
        for color_idx in 0..2 {
            let opponent = 1 - color_idx;
            for piece_idx in 0..PIECES.len() {
                if piece_idx == PieceDescriptor::King as usize {
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
            PieceDescriptor::Queen,
            PieceDescriptor::Rook,
            PieceDescriptor::LightBishop,
            PieceDescriptor::DarkBishop,
            PieceDescriptor::Knight,
        ];
        let pawn_idx = PieceDescriptor::Pawn as usize;
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

                for capture_idx in 0..PIECES.len() {
                    if capture_idx == PieceDescriptor::King as usize {
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

    pub(crate) fn from_position(position: &Chess) -> Option<Self> {
        let mut counts = [[0u8; PIECES.len()]; 2];
        for square in Square::ALL {
            if let Some(piece) = position.board().piece_at(square) {
                let color_idx = match piece.color {
                    Color::White => 0,
                    Color::Black => 1,
                };
                let piece_idx = match piece.role {
                    Role::King => PieceDescriptor::King as usize,
                    Role::Queen => PieceDescriptor::Queen as usize,
                    Role::Rook => PieceDescriptor::Rook as usize,
                    Role::Bishop => {
                        if square.is_light() {
                            PieceDescriptor::LightBishop as usize
                        } else {
                            PieceDescriptor::DarkBishop as usize
                        }
                    }
                    Role::Knight => PieceDescriptor::Knight as usize,
                    Role::Pawn => PieceDescriptor::Pawn as usize,
                };
                counts[color_idx][piece_idx] += 1;
            }
        }

        Some(MaterialKey::new(counts))
    }
}

impl fmt::Display for MaterialKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn write_bishops(f: &mut fmt::Formatter<'_>, light: u8, dark: u8) -> fmt::Result {
            if light < dark {
                for _ in 0..dark {
                    write!(f, "{}", PieceDescriptor::DarkBishop.token())?;
                }
                for _ in 0..light {
                    write!(f, "{}", PieceDescriptor::LightBishop.token())?;
                }
            } else {
                for _ in 0..light {
                    write!(f, "{}", PieceDescriptor::LightBishop.token())?;
                }
                for _ in 0..dark {
                    write!(f, "{}", PieceDescriptor::DarkBishop.token())?;
                }
            }
            Ok(())
        }

        let light_idx = PieceDescriptor::LightBishop as usize;
        let dark_idx = PieceDescriptor::DarkBishop as usize;

        for color_idx in 0..2 {
            if color_idx == 1 {
                write!(f, "v")?;
            }

            for (i, pd) in PIECES.iter().enumerate() {
                if i == light_idx {
                    let light = self.counts[color_idx][light_idx];
                    let dark = self.counts[color_idx][dark_idx];
                    if light == 0 && dark == 0 {
                        continue;
                    }
                    write_bishops(f, light, dark)?;
                    continue;
                } else if i == dark_idx {
                    continue;
                }

                for _ in 0..self.counts[color_idx][i] {
                    write!(f, "{}", pd.token())?;
                }
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

    #[test]
    fn parses_kqvk() {
        let mk = MaterialKey::from_string("KQvK").unwrap();
        assert_eq!(mk.to_string(), "KQvK");
    }

    #[test]
    fn parses_light_and_dark_bishops() {
        let mk = MaterialKey::from_string("BlBdvK").unwrap();
        assert_eq!(mk.to_string(), "BlBdvK");
    }

    #[test]
    fn canonicalizes_bishop_colors_in_material_key() {
        let mk = MaterialKey::from_string("KBlvKBd").unwrap();
        assert_eq!(mk.to_string(), "KBdvKBl");
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

    #[test]
    fn material_key_flips_bishop_colors_1() {
        let key = MaterialKey::from_string("KBlvKBd").unwrap();
        assert_eq!(key.to_string(), "KBdvKBl");
    }

    #[test]
    fn material_key_flips_bishop_colors_2() {
        let key = MaterialKey::from_string("KBlBlBdvK").unwrap();
        assert_eq!(key.to_string(), "KBdBdBlvK");
    }

    #[test]
    fn allowed_transforms_pawnless_no_bishops() {
        use crate::transform::Transform::*;

        let key = MaterialKey::from_string("KQvK").unwrap();
        assert_eq!(
            key.allowed_transforms(),
            [
                Identity,
                FlipHorizontal,
                FlipVertical,
                Rotate90,
                Rotate270,
                Rotate180,
                MirrorMain,
                MirrorAnti
            ]
            .as_slice()
        );
    }

    #[test]
    fn allowed_transforms_pawnless_with_bishops() {
        use crate::transform::Transform::*;

        let key = MaterialKey::from_string("KBdvK").unwrap();
        assert_eq!(
            key.allowed_transforms(),
            [Identity, Rotate90, Rotate180, Rotate270].as_slice()
        );
    }

    #[test]
    fn allowed_transforms_with_pawns_no_bishops() {
        use crate::transform::Transform::*;

        let key = MaterialKey::from_string("KPvK").unwrap();
        assert_eq!(
            key.allowed_transforms(),
            [Identity, FlipHorizontal, FlipVertical, Rotate180].as_slice()
        );
    }

    #[test]
    fn allowed_transforms_with_pawns_and_bishops() {
        use crate::transform::Transform::*;

        let key = MaterialKey::from_string("KBdvKP").unwrap();
        assert_eq!(key.allowed_transforms(), [Identity, Rotate180].as_slice());
    }

    #[test]
    fn transform_set_matches_pawnless_no_bishops() {
        use crate::transform::TransformSet;

        let key = MaterialKey::from_string("KQvK").unwrap();
        assert_eq!(key.transform_set(), TransformSet::Full);
    }

    #[test]
    fn transform_set_matches_pawnless_with_bishops() {
        use crate::transform::TransformSet;

        let key = MaterialKey::from_string("KBdvK").unwrap();
        assert_eq!(key.transform_set(), TransformSet::Rotations);
    }

    #[test]
    fn transform_set_matches_with_pawns_no_bishops() {
        use crate::transform::TransformSet;

        let key = MaterialKey::from_string("KPvK").unwrap();
        assert_eq!(key.transform_set(), TransformSet::AxisFlips);
    }

    #[test]
    fn transform_set_matches_with_pawns_and_bishops() {
        use crate::transform::TransformSet;

        let key = MaterialKey::from_string("KBdvKP").unwrap();
        assert_eq!(key.transform_set(), TransformSet::HalfTurn);
    }
}
