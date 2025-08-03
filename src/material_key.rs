use std::fmt;

use shakmaty::{CastlingMode, Chess, Color, FromSetup, Piece, Position, Role, Setup, Square};

/// Represents a material configuration, e.g. `KQvK`, and provides the
/// bidirectional mapping between permutations of those pieces and compact
/// indices.
///
/// The mapping only covers legal placements where every piece occupies a
/// unique square. Identical pieces are treated as indistinguishable, so the
/// mapping enumerates combinations of their placements without ambiguity. Any
/// arrangement with overlapping pieces is considered invalid and is never
/// assigned an index.
const ROLES: [Role; 6] = [
    Role::King,
    Role::Queen,
    Role::Rook,
    Role::Bishop,
    Role::Knight,
    Role::Pawn,
];

fn role_index(role: Role) -> usize {
    match role {
        Role::King => 0,
        Role::Queen => 1,
        Role::Rook => 2,
        Role::Bishop => 3,
        Role::Knight => 4,
        Role::Pawn => 5,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MaterialKey {
    /// Piece counts indexed by color then role.
    counts: [[u8; 6]; 2],
}

impl MaterialKey {
    /// Parse a [`MaterialKey`] from its textual representation.
    ///
    /// # Syntax
    /// The expected form is `<white pieces>v<black pieces>`, where each side is
    /// a sequence of piece characters and exactly one `v` separates the two
    /// sides.
    ///
    /// # Piece characters and colors
    /// Supported characters are `K`, `Q`, `R`, `B`, `N` and `P` for king,
    /// queen, rook, bishop, knight and pawn respectively. All characters must
    /// be uppercase. Pieces appearing before the separator are interpreted as
    /// white, while those after it are treated as black.
    ///
    /// # Use cases
    /// This is primarily useful for tests and simple user interfaces that need
    /// to describe a set of pieces without board coordinates.
    ///
    /// # Errors
    /// Returns `None` if the string is malformed, contains unsupported
    /// characters, has a missing or extra separator, or is otherwise ambiguous.
    pub fn from_string(s: &str) -> Option<Self> {
        let mut parts = s.split('v');
        let white = parts.next()?;
        let black = parts.next()?;

        // Only one separator is allowed.
        if parts.next().is_some() {
            return None;
        }

        let mut counts = [[0u8; 6]; 2];

        fn push_pieces(out: &mut [[u8; 6]; 2], chars: &str, color: Color) -> Option<()> {
            let color_idx = match color {
                Color::White => 0,
                Color::Black => 1,
            };
            for ch in chars.chars() {
                let role = match ch {
                    'K' => Role::King,
                    'Q' => Role::Queen,
                    'R' => Role::Rook,
                    'B' => Role::Bishop,
                    'N' => Role::Knight,
                    'P' => Role::Pawn,
                    _ => return None,
                };
                let role_idx = role_index(role);
                out[color_idx][role_idx] += 1;
            }
            Some(())
        }

        push_pieces(&mut counts, white, Color::White)?;
        push_pieces(&mut counts, black, Color::Black)?;

        Some(Self { counts })
    }

    /// Total number of mappable positions for this material configuration.
    ///
    /// Each index corresponds to a unique permutation where all pieces appear
    /// on distinct squares. Identical pieces are treated as indistinguishable,
    /// so their placements are counted combinatorially.
    pub fn total_positions(&self) -> usize {
        let mut remaining = 64usize;
        let mut total = 1usize;
        for (_, count) in self.piece_groups() {
            let c = n_choose_k(remaining, count as usize);
            total *= c;
            remaining -= count as usize;
        }
        total
    }

    /// Convert an index into a [`Chess`] position.
    ///
    /// Returns `None` only when the index exceeds the number of mappable
    /// positions. Every index less than [`total_positions`](Self::total_positions)
    /// yields a unique placement with all pieces on distinct squares.
    pub fn index_to_position(&self, mut pos_index: usize) -> Option<Chess> {
        if pos_index >= self.total_positions() {
            return None;
        }

        let mut setup = Setup::empty();
        let mut squares: Vec<Square> = (0..64).map(|i| Square::new(i as u32)).collect();

        for (piece, count) in self.piece_groups() {
            let base = n_choose_k(squares.len(), count as usize);
            let group_index = pos_index % base;
            pos_index /= base;
            let indices = unrank_combination(squares.len(), count as usize, group_index);
            let chosen: Vec<Square> = indices.iter().map(|&i| squares[i]).collect();
            for idx in indices.iter().rev() {
                squares.remove(*idx);
            }
            for square in chosen {
                setup.board.set_piece_at(square, piece);
            }
        }

        Chess::from_setup(setup, CastlingMode::Standard).ok()
    }

    /// Convert a [`Chess`] position back into its index within this material
    /// mapping.
    ///
    /// The position must contain each piece from this key exactly once on a
    /// distinct square. Positions that violate this requirement are outside the
    /// mapping and result in undefined behaviour.
    pub fn position_to_index(&self, position: &Chess) -> usize {
        let mut index = 0usize;
        let mut multiplier = 1usize;
        let mut squares: Vec<Square> = (0..64).map(|i| Square::new(i as u32)).collect();

        for (piece, count) in self.piece_groups() {
            let base = n_choose_k(squares.len(), count as usize);

            let mut piece_indices: Vec<usize> = {
                let bb = position.board().by_piece(piece);
                let mut v = Vec::with_capacity(count as usize);
                for sq in bb {
                    let idx = squares
                        .iter()
                        .position(|&s| s == sq)
                        .expect("piece square must exist");
                    v.push(idx);
                }
                v
            };
            piece_indices.sort();
            assert_eq!(piece_indices.len(), count as usize);
            let group_index = rank_combination(squares.len(), piece_indices.as_slice());
            index += group_index * multiplier;
            for idx in piece_indices.iter().rev() {
                squares.remove(*idx);
            }
            multiplier *= base;
        }

        index
    }
}

impl fmt::Display for MaterialKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for color_idx in 0..2 {
            if color_idx == 1 {
                write!(f, "v")?;
            }

            for (ch, role) in [
                ('K', Role::King),
                ('Q', Role::Queen),
                ('R', Role::Rook),
                ('B', Role::Bishop),
                ('N', Role::Knight),
                ('P', Role::Pawn),
            ] {
                for _ in 0..self.counts[color_idx][role_index(role)] {
                    write!(f, "{}", ch)?;
                }
            }
        }

        Ok(())
    }
}

impl MaterialKey {
    /// Group identical pieces with their multiplicity.
    fn piece_groups(&self) -> Vec<(Piece, u8)> {
        let mut groups = Vec::new();
        for (color_idx, &color) in [Color::White, Color::Black].iter().enumerate() {
            for &role in &ROLES {
                let count = self.counts[color_idx][role_index(role)];
                if count > 0 {
                    groups.push((Piece { role, color }, count));
                }
            }
        }
        groups
    }
}

fn n_choose_k(n: usize, k: usize) -> usize {
    if k > n {
        return 0;
    }
    let k = k.min(n - k);
    let mut result: usize = 1;
    for i in 1..=k {
        result = result * (n - (k - i)) / i;
    }
    result
}

fn unrank_combination(n: usize, k: usize, mut rank: usize) -> Vec<usize> {
    let mut combo = Vec::with_capacity(k);
    let mut x = 0usize;
    for i in 0..k {
        let mut c = x;
        loop {
            let count = n_choose_k(n - c - 1, k - i - 1);
            if count <= rank {
                rank -= count;
                c += 1;
            } else {
                combo.push(c);
                x = c + 1;
                break;
            }
        }
    }
    combo
}

fn rank_combination(n: usize, indices: &[usize]) -> usize {
    let k = indices.len();
    let mut rank = 0usize;
    for (i, &c) in indices.iter().enumerate() {
        let start = if i == 0 { 0 } else { indices[i - 1] + 1 };
        for j in start..c {
            rank += n_choose_k(n - j - 1, k - i - 1);
        }
    }
    rank
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{Rng, SeedableRng, rngs::StdRng};

    #[test]
    fn parses_kqvk() {
        let mk = MaterialKey::from_string("KQvK").unwrap();
        assert_eq!(mk.to_string(), "KQvK");
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
    fn total_positions_without_overlap() {
        let mk = MaterialKey::from_string("KQvK").unwrap();
        assert_eq!(mk.total_positions(), 64 * 63 * 62);
    }

    #[test]
    fn total_positions_with_duplicates() {
        let mk = MaterialKey::from_string("KNNvK").unwrap();
        assert_eq!(mk.total_positions(), 64 * (63 * 62 / 2) * 61);
    }

    fn roundtrip_random_indices(mk: MaterialKey, seed: u64) {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut successes = 0;
        while successes < 10 {
            let index = rng.gen_range(0..mk.total_positions());
            if let Some(pos) = mk.index_to_position(index) {
                let roundtrip = mk.position_to_index(&pos);
                assert_eq!(index, roundtrip);
                successes += 1;
            }
        }
    }

    #[test]
    fn roundtrip_kvk() {
        let mk = MaterialKey::from_string("KvK").unwrap();
        roundtrip_random_indices(mk, 0);
    }

    #[test]
    fn roundtrip_kqvk() {
        let mk = MaterialKey::from_string("KQvK").unwrap();
        roundtrip_random_indices(mk, 1);
    }

    #[test]
    fn roundtrip_krvkb() {
        let mk = MaterialKey::from_string("KRvKB").unwrap();
        roundtrip_random_indices(mk, 2);
    }

    #[test]
    fn roundtrip_kqvkr() {
        let mk = MaterialKey::from_string("KQvKR").unwrap();
        roundtrip_random_indices(mk, 3);
    }

    #[test]
    fn roundtrip_kbnvkq() {
        let mk = MaterialKey::from_string("KBNvKQ").unwrap();
        roundtrip_random_indices(mk, 4);
    }

    #[test]
    fn roundtrip_knnvk() {
        let mk = MaterialKey::from_string("KNNvK").unwrap();
        roundtrip_random_indices(mk, 5);
    }
}
