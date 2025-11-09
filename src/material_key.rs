use std::collections::BTreeSet;
use std::fmt;
use std::fmt::Write as _;
use std::str::FromStr;

use crate::transform::{Transform, TransformSet};
use shakmaty::{Bitboard, Chess, Color, Position, PositionErrorKinds, Role, Square};

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
            PieceDescriptor::DarkBishop => "Bd",
            PieceDescriptor::LightBishop => "Bl",
            PieceDescriptor::Knight => "N",
            PieceDescriptor::Pawn => "P",
        }
    }

    pub(crate) fn role(self) -> Role {
        match self {
            PieceDescriptor::King => Role::King,
            PieceDescriptor::Queen => Role::Queen,
            PieceDescriptor::Rook => Role::Rook,
            PieceDescriptor::DarkBishop | PieceDescriptor::LightBishop => Role::Bishop,
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
            "Bd" => Some(PieceDescriptor::DarkBishop),
            "Bl" => Some(PieceDescriptor::LightBishop),
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
    PieceDescriptor::DarkBishop,
    PieceDescriptor::LightBishop,
    PieceDescriptor::Knight,
    PieceDescriptor::Pawn,
];

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MaterialKey {
    /// Piece counts indexed by color then piece descriptor.
    /// By convention the strong side is always first and it is encoded as white
    /// whenever we convert to a position.
    pub(crate) counts: [[u8; PIECES.len()]; 2],
    pawn_bitboards: [Bitboard; 2],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaterialError {
    MismatchedMaterial,
    IndexOutOfBounds,
    InvalidPosition(PositionErrorKinds),
}

impl MaterialKey {
    pub(crate) fn new(counts: [[u8; PIECES.len()]; 2], pawn_bitboards: [Bitboard; 2]) -> Self {
        let mut key = Self {
            counts,
            pawn_bitboards,
        };
        key.sync_pawn_counts();
        key.canonicalize();
        key
    }

    fn sync_pawn_counts(&mut self) {
        let pawn_idx = PieceDescriptor::Pawn as usize;
        for color_idx in 0..2 {
            self.counts[color_idx][pawn_idx] =
                self.pawn_bitboards[color_idx].into_iter().count() as u8;
        }
    }

    /// Parse a [`MaterialKey`] from its textual representation.
    ///
    /// # Syntax
    /// The expected form is `<white pieces>v<black pieces>`, where each side is
    /// a sequence of piece tokens and exactly one `v` separates the two
    /// sides.
    ///
    /// # Piece tokens and colors
    /// Supported tokens are `K`, `Q`, `R`, `Bl`, `Bd`, and `N` for king,
    /// queen, rook, light-squared bishop, dark-squared bishop, and knight
    /// respectively. Pawn locations are represented explicitly by algebraic
    /// coordinates such as `e4` or `h7`. Coordinates must come after all
    /// piece tokens for a side and are emitted in lexicographic order.
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

        if parts.next().is_some() {
            return None;
        }

        let mut counts = [[0u8; PIECES.len()]; 2];
        let mut pawns = [Bitboard::EMPTY; 2];

        fn parse_side(
            out_counts: &mut [[u8; PIECES.len()]; 2],
            out_pawns: &mut [Bitboard; 2],
            s: &str,
            color: Color,
        ) -> Option<()> {
            let color_idx = match color {
                Color::White => 0,
                Color::Black => 1,
            };

            let mut i = 0;
            let bytes = s.as_bytes();
            let mut parsing_pawns = false;

            while i < bytes.len() {
                match bytes[i] as char {
                    'B' => {
                        if parsing_pawns || i + 1 >= bytes.len() {
                            return None;
                        }
                        let token = match bytes[i + 1] as char {
                            'd' => {
                                i += 2;
                                "Bd"
                            }
                            'l' => {
                                i += 2;
                                "Bl"
                            }
                            _ => return None,
                        };
                        let pd = PieceDescriptor::from_token(token)?;
                        out_counts[color_idx][pd as usize] += 1;
                    }
                    'K' | 'Q' | 'R' | 'N' => {
                        if parsing_pawns {
                            return None;
                        }
                        let pd = PieceDescriptor::from_token(&s[i..i + 1])?;
                        out_counts[color_idx][pd as usize] += 1;
                        i += 1;
                    }
                    'a'..='h' => {
                        parsing_pawns = true;
                        if i + 1 >= bytes.len() {
                            return None;
                        }
                        let square_str = &s[i..i + 2];
                        let square = Square::from_str(square_str).ok()?;
                        let bb = Bitboard::from_square(square);
                        if !(out_pawns[color_idx] & bb).is_empty() {
                            return None;
                        }
                        out_pawns[color_idx] |= bb;
                        i += 2;
                    }
                    _ => return None,
                }
            }

            Some(())
        }

        parse_side(&mut counts, &mut pawns, white, Color::White)?;
        parse_side(&mut counts, &mut pawns, black, Color::Black)?;

        Some(Self::new(counts, pawns))
    }

    pub fn non_pawn_piece_count(&self) -> u32 {
        let pawn_idx = PieceDescriptor::Pawn as usize;
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
        self.sync_pawn_counts();

        if Self::strong_color_from_counts(&self.counts) == Color::Black {
            self.swap_colors();
        }

        if self.should_swap_bishops() {
            self.flip_bishop_colors();
        }

        self.canonicalize_pawns();
        self.sync_pawn_counts();
    }

    fn canonicalize_pawns(&mut self) {
        if !self.has_pawns() {
            return;
        }

        let original = self.pawn_bitboards;
        let counts = self.counts;
        let mut best_bitboards = original;
        let mut best_string = format_material(&counts, &original);

        for &transform in self.allowed_transforms().iter() {
            let transformed = [
                transform_bitboard(original[0], transform),
                transform_bitboard(original[1], transform),
            ];
            let serialized = format_material(&counts, &transformed);
            if serialized < best_string {
                best_string = serialized;
                best_bitboards = transformed;
            }
        }

        self.pawn_bitboards = best_bitboards;
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
        !(self.pawn_bitboards[0].is_empty() && self.pawn_bitboards[1].is_empty())
    }

    pub(crate) fn has_bishops(&self) -> bool {
        let light_idx = PieceDescriptor::LightBishop as usize;
        let dark_idx = PieceDescriptor::DarkBishop as usize;
        self.counts[0][light_idx] > 0
            || self.counts[1][light_idx] > 0
            || self.counts[0][dark_idx] > 0
            || self.counts[1][dark_idx] > 0
    }

    fn swap_colors(&mut self) {
        self.counts.swap(0, 1);
        self.pawn_bitboards.swap(0, 1);
    }

    /// Determines which color has the stronger material based on piece counts.
    ///
    /// By design, this matches the logic used by syzygy tablebases.
    ///
    /// The first factor is the total piece count.
    /// Then, it's which side has the strongest piece.
    /// Finally, in case of a tie, White is considered stronger.
    fn strong_color_from_counts(counts: &[[u8; PIECES.len()]; 2]) -> Color {
        let compare = |white: u8, black: u8| -> Option<Color> {
            if white > black {
                Some(Color::White)
            } else if black > white {
                Some(Color::Black)
            } else {
                None
            }
        };

        let queen_idx = PieceDescriptor::Queen as usize;
        if let Some(color) = compare(counts[0][queen_idx], counts[1][queen_idx]) {
            return color;
        }

        let rook_idx = PieceDescriptor::Rook as usize;
        if let Some(color) = compare(counts[0][rook_idx], counts[1][rook_idx]) {
            return color;
        }

        let light_idx = PieceDescriptor::LightBishop as usize;
        let dark_idx = PieceDescriptor::DarkBishop as usize;
        let white_bishops = counts[0][light_idx] + counts[0][dark_idx];
        let black_bishops = counts[1][light_idx] + counts[1][dark_idx];
        if let Some(color) = compare(white_bishops, black_bishops) {
            return color;
        }

        let knight_idx = PieceDescriptor::Knight as usize;
        if let Some(color) = compare(counts[0][knight_idx], counts[1][knight_idx]) {
            return color;
        }

        let pawn_idx = PieceDescriptor::Pawn as usize;
        if let Some(color) = compare(counts[0][pawn_idx], counts[1][pawn_idx]) {
            return color;
        }

        Color::White
    }

    pub(crate) fn transform_set(&self) -> TransformSet {
        match (!self.has_pawns(), self.has_bishops()) {
            (true, false) => TransformSet::Full,
            (true, true) => TransformSet::Rotations,
            (false, false) => TransformSet::Horizontal,
            (false, true) => TransformSet::Identity,
        }
    }

    pub(crate) fn allowed_transforms(&self) -> &'static [Transform] {
        self.transform_set().transforms()
    }

    pub(crate) fn child_material_keys(&self) -> Vec<MaterialKey> {
        let mut children = BTreeSet::new();
        let pawn_idx = PieceDescriptor::Pawn as usize;

        for color_idx in 0..2 {
            let opponent = 1 - color_idx;
            for piece_idx in 0..PIECES.len() {
                if piece_idx == PieceDescriptor::King as usize {
                    continue;
                }

                if piece_idx == pawn_idx {
                    if self.counts[opponent][pawn_idx] == 0 {
                        continue;
                    }
                    for square in self.pawn_bitboards[opponent] {
                        let mut counts = self.counts;
                        let mut pawns = self.pawn_bitboards;
                        counts[opponent][pawn_idx] -= 1;
                        pawns[opponent] = remove_square(pawns[opponent], square);
                        children.insert(MaterialKey::new(counts, pawns));
                    }
                } else if self.counts[opponent][piece_idx] > 0 {
                    let mut counts = self.counts;
                    counts[opponent][piece_idx] -= 1;
                    children.insert(MaterialKey::new(counts, self.pawn_bitboards));
                }
            }
        }

        let promo_targets = [
            PieceDescriptor::Queen,
            PieceDescriptor::Rook,
            PieceDescriptor::LightBishop,
            PieceDescriptor::DarkBishop,
            PieceDescriptor::Knight,
        ];

        for color_idx in 0..2 {
            let opponent = 1 - color_idx;
            if self.pawn_bitboards[color_idx].is_empty() {
                continue;
            }

            for pawn_square in self.pawn_bitboards[color_idx] {
                let mut base_counts = self.counts;
                let mut base_pawns = self.pawn_bitboards;
                base_counts[color_idx][pawn_idx] -= 1;
                base_pawns[color_idx] = remove_square(base_pawns[color_idx], pawn_square);

                for target in promo_targets {
                    let mut counts = base_counts;
                    let pawns = base_pawns;
                    counts[color_idx][target as usize] += 1;
                    children.insert(MaterialKey::new(counts, pawns));

                    for capture_idx in 0..PIECES.len() {
                        if capture_idx == PieceDescriptor::King as usize {
                            continue;
                        }

                        if capture_idx == pawn_idx {
                            if self.counts[opponent][pawn_idx] == 0 {
                                continue;
                            }
                            for capture_square in self.pawn_bitboards[opponent] {
                                let mut capture_counts = counts;
                                let mut capture_pawns = pawns;
                                capture_counts[opponent][pawn_idx] -= 1;
                                capture_pawns[opponent] =
                                    remove_square(capture_pawns[opponent], capture_square);
                                children.insert(MaterialKey::new(capture_counts, capture_pawns));
                            }
                        } else if self.counts[opponent][capture_idx] > 0 {
                            let mut capture_counts = counts;
                            capture_counts[opponent][capture_idx] -= 1;
                            children.insert(MaterialKey::new(capture_counts, pawns));
                        }
                    }
                }
            }
        }

        children.into_iter().collect()
    }

    pub fn from_position(position: &Chess) -> Option<Self> {
        let mut counts = [[0u8; PIECES.len()]; 2];
        let mut pawn_bitboards = [Bitboard::EMPTY; 2];

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
                if piece.role == Role::Pawn {
                    pawn_bitboards[color_idx] |= Bitboard::from_square(square);
                }
            }
        }

        Some(MaterialKey::new(counts, pawn_bitboards))
    }

    pub(crate) fn pawn_bitboard(&self, color: Color) -> Bitboard {
        match color {
            Color::White => self.pawn_bitboards[0],
            Color::Black => self.pawn_bitboards[1],
        }
    }
}

impl fmt::Display for MaterialKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let serialized = format_material(&self.counts, &self.pawn_bitboards);
        f.write_str(&serialized)
    }
}

fn transform_square(square: Square, transform: Transform) -> Square {
    let idx = u32::from(square) as i8;
    let file = idx % 8;
    let rank = idx / 8;

    let (new_file, new_rank) = match transform {
        Transform::Identity => (file, rank),
        Transform::FlipHorizontal => (7 - file, rank),
        Transform::FlipVertical => (file, 7 - rank),
        Transform::Rotate90 => (rank, 7 - file),
        Transform::Rotate180 => (7 - file, 7 - rank),
        Transform::Rotate270 => (7 - rank, file),
        Transform::MirrorMain => (rank, file),
        Transform::MirrorAnti => (7 - rank, 7 - file),
    };

    Square::new((new_rank * 8 + new_file) as u32)
}

fn transform_bitboard(bitboard: Bitboard, transform: Transform) -> Bitboard {
    if matches!(transform, Transform::Identity) {
        return bitboard;
    }

    let mut result = Bitboard::EMPTY;
    for square in bitboard {
        result |= Bitboard::from_square(transform_square(square, transform));
    }
    result
}

fn remove_square(bitboard: Bitboard, square: Square) -> Bitboard {
    bitboard & !Bitboard::from_square(square)
}

fn format_material(counts: &[[u8; PIECES.len()]; 2], pawns: &[Bitboard; 2]) -> String {
    let mut out = String::new();

    for color_idx in 0..2 {
        if color_idx == 1 {
            out.push('v');
        }

        let side = &counts[color_idx];

        for _ in 0..side[PieceDescriptor::King as usize] {
            out.push_str(PieceDescriptor::King.token());
        }

        for _ in 0..side[PieceDescriptor::Queen as usize] {
            out.push_str(PieceDescriptor::Queen.token());
        }

        for _ in 0..side[PieceDescriptor::Rook as usize] {
            out.push_str(PieceDescriptor::Rook.token());
        }

        let light_idx = PieceDescriptor::LightBishop as usize;
        let dark_idx = PieceDescriptor::DarkBishop as usize;
        for _ in 0..side[dark_idx] {
            out.push_str(PieceDescriptor::DarkBishop.token());
        }
        for _ in 0..side[light_idx] {
            out.push_str(PieceDescriptor::LightBishop.token());
        }

        for _ in 0..side[PieceDescriptor::Knight as usize] {
            out.push_str(PieceDescriptor::Knight.token());
        }

        let mut pawn_squares: Vec<Square> = pawns[color_idx].into_iter().collect();
        pawn_squares.sort_unstable_by_key(|sq| sq.to_u32());
        for square in pawn_squares {
            let _ = write!(out, "{}", square);
        }
    }

    out
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
    fn canonicalizes_mirrored_pawn_structure() {
        assert_eq!(material_key("Kf2g2vK"), "Kb2c2vK");
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
    fn rejects_legacy_pawn_token() {
        assert!(MaterialKey::from_string("KPvK").is_none());
    }

    #[test]
    fn child_keys_for_ke2vk() {
        let key = MaterialKey::from_string("Ke2vK").unwrap();
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
        assert_eq!(key.to_string(), "Kd2vK");
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

        let key = MaterialKey::from_string("Ke2vK").unwrap();
        assert_eq!(
            key.allowed_transforms(),
            [Identity, FlipHorizontal].as_slice()
        );
    }

    #[test]
    fn allowed_transforms_with_pawns_and_bishops() {
        use crate::transform::Transform::*;

        let key = MaterialKey::from_string("KBdvKe2").unwrap();
        assert_eq!(key.allowed_transforms(), [Identity].as_slice());
    }
}
