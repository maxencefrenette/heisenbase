use crate::material_key::{MaterialError, MaterialKey, PIECES};
use shakmaty::{Bitboard, CastlingMode, Chess, Color, FromSetup, Piece, Position, Setup, Square};

#[derive(Clone, Copy)]
struct PieceGroup {
    piece: Piece,
    count: u8,
    light: Option<bool>,
}

fn piece_groups(key: &MaterialKey) -> Vec<PieceGroup> {
    let mut groups = Vec::new();
    for (color_idx, &color) in [Color::White, Color::Black].iter().enumerate() {
        for (i, pd) in PIECES.iter().enumerate() {
            let count = key.counts[color_idx][i];
            if count > 0 {
                groups.push(PieceGroup {
                    piece: Piece {
                        role: pd.role(),
                        color,
                    },
                    count,
                    light: pd.light(),
                });
            }
        }
    }
    groups
}

/// Total number of mappable positions for a material configuration.
///
/// Each index corresponds to a unique permutation where all pieces appear
/// on distinct squares. Identical pieces are treated as indistinguishable,
/// so their placements are counted combinatorially.
pub fn total_positions(key: &MaterialKey) -> usize {
    let mut squares: Vec<Square> = (0..64).map(|i| Square::new(i as u32)).collect();
    let mut total = 1usize;
    for group in piece_groups(key) {
        let allowed: Vec<usize> = squares
            .iter()
            .enumerate()
            .filter(|&(_, sq)| match group.light {
                Some(true) => sq.is_light(),
                Some(false) => sq.is_dark(),
                None => true,
            })
            .map(|(i, _)| i)
            .collect();
        let base = n_choose_k(allowed.len(), group.count as usize);
        total *= base;
        for idx in allowed.iter().take(group.count as usize).rev() {
            squares.remove(*idx);
        }
    }
    // Also account for which side is to move.
    total * 2
}

/// Convert an index into a [`Chess`] position.
///
/// Every index less than [`total_positions`] corresponds to a unique arrangement
/// of the pieces described by the material key.  The mapping is purely
/// combinatorial and intentionally ignores the rules of play, so some indices
/// yield setups that are unreachable or illegal under normal chess rules — for
/// example, kings adjacent to one another or a side to move that can
/// immediately capture the opposing king.  When [`shakmaty`] rejects such a
/// placement, this function returns [`Err(MaterialError::InvalidPosition)`].
///
/// Indices greater than or equal to `total_positions()` return
/// [`Err(MaterialError::IndexOutOfBounds)`].
pub fn index_to_position(key: &MaterialKey, mut pos_index: usize) -> Result<Chess, MaterialError> {
    if pos_index >= total_positions(key) {
        return Err(MaterialError::IndexOutOfBounds);
    }

    // Extract side to move from the index.
    let turn = if pos_index % 2 == 0 {
        Color::White
    } else {
        Color::Black
    };
    pos_index /= 2;

    let mut setup = Setup::empty();
    setup.turn = turn;
    let mut squares: Vec<Square> = (0..64).map(|i| Square::new(i as u32)).collect();

    for group in piece_groups(key) {
        let allowed: Vec<usize> = squares
            .iter()
            .enumerate()
            .filter(|&(_, sq)| match group.light {
                Some(true) => sq.is_light(),
                Some(false) => sq.is_dark(),
                None => true,
            })
            .map(|(i, _)| i)
            .collect();
        let base = n_choose_k(allowed.len(), group.count as usize);
        let group_index = pos_index % base;
        pos_index /= base;
        let rel_indices = unrank_combination(allowed.len(), group.count as usize, group_index);
        let mut chosen_indices: Vec<usize> = rel_indices.iter().map(|&r| allowed[r]).collect();
        let chosen_squares: Vec<Square> = chosen_indices.iter().map(|&i| squares[i]).collect();
        chosen_indices.sort_unstable();
        for idx in chosen_indices.iter().rev() {
            squares.remove(*idx);
        }
        for square in chosen_squares {
            setup.board.set_piece_at(square, group.piece);
        }
    }

    Chess::from_setup(setup, CastlingMode::Standard)
        .map_err(|e| MaterialError::InvalidPosition(e.kinds()))
}

/// Convert a [`Chess`] position back into its index within the material mapping.
///
/// The position must contain exactly the pieces described by the key, each
/// appearing on a distinct square. Positions with mismatched material are
/// outside the mapping and return an error.
pub fn position_to_index(key: &MaterialKey, position: &Chess) -> Result<usize, MaterialError> {
    let mut board_counts = [[0u8; PIECES.len()]; 2];
    for (color_idx, &color) in [Color::White, Color::Black].iter().enumerate() {
        for (i, pd) in PIECES.iter().enumerate() {
            let mask = match pd.light() {
                Some(true) => Bitboard::LIGHT_SQUARES,
                Some(false) => Bitboard::DARK_SQUARES,
                None => Bitboard::FULL,
            };
            let piece = Piece {
                role: pd.role(),
                color,
            };
            let count = (position.board().by_piece(piece) & mask)
                .into_iter()
                .count() as u8;
            board_counts[color_idx][i] = count;
        }
    }
    if board_counts != key.counts {
        return Err(MaterialError::MismatchedMaterial);
    }

    let groups = piece_groups(key);
    let mut index = 0usize;
    let mut multiplier = 1usize;
    let mut squares: Vec<Square> = (0..64).map(|i| Square::new(i as u32)).collect();

    for group in groups {
        let allowed: Vec<usize> = squares
            .iter()
            .enumerate()
            .filter(|&(_, sq)| match group.light {
                Some(true) => sq.is_light(),
                Some(false) => sq.is_dark(),
                None => true,
            })
            .map(|(i, _)| i)
            .collect();
        let base = n_choose_k(allowed.len(), group.count as usize);

        let mut piece_indices: Vec<usize> = {
            let mask = match group.light {
                Some(true) => Bitboard::LIGHT_SQUARES,
                Some(false) => Bitboard::DARK_SQUARES,
                None => Bitboard::FULL,
            };
            let bb = position.board().by_piece(group.piece) & mask;
            let mut v = Vec::with_capacity(group.count as usize);
            for sq in bb {
                let idx = allowed
                    .iter()
                    .position(|&i| squares[i] == sq)
                    .expect("piece square must exist");
                v.push(idx);
            }
            v
        };
        piece_indices.sort();
        let group_index = rank_combination(allowed.len(), piece_indices.as_slice());
        index += group_index * multiplier;
        let mut remove_indices: Vec<usize> = piece_indices.iter().map(|&ai| allowed[ai]).collect();
        remove_indices.sort_unstable();
        for idx in remove_indices.iter().rev() {
            squares.remove(*idx);
        }
        multiplier *= base;
    }

    let turn_index = match position.turn() {
        Color::White => 0usize,
        Color::Black => 1usize,
    };
    Ok(index * 2 + turn_index)
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

    fn roundtrip_random_indices(mk: MaterialKey, seed: u64) {
        let mut rng = StdRng::seed_from_u64(seed);
        for _ in 0..10 {
            let index = rng.gen_range(0..total_positions(&mk));
            if let Ok(pos) = index_to_position(&mk, index) {
                let roundtrip = position_to_index(&mk, &pos).unwrap();
                assert_eq!(index, roundtrip);
            }
        }
    }

    #[test]
    fn total_positions_without_overlap() {
        let mk = MaterialKey::from_string("KQvK").unwrap();
        assert_eq!(total_positions(&mk), 64 * 63 * 62 * 2);
    }

    #[test]
    fn total_positions_with_duplicates() {
        let mk = MaterialKey::from_string("KNNvK").unwrap();
        assert_eq!(total_positions(&mk), 64 * (63 * 62 / 2) * 61 * 2);
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
        let mk = MaterialKey::from_string("KRvKBd").unwrap();
        roundtrip_random_indices(mk, 2);
    }

    #[test]
    fn roundtrip_kqvkr() {
        let mk = MaterialKey::from_string("KQvKR").unwrap();
        roundtrip_random_indices(mk, 3);
    }

    #[test]
    fn roundtrip_kbnvkq() {
        let mk = MaterialKey::from_string("KBdNvKQ").unwrap();
        roundtrip_random_indices(mk, 4);
    }

    #[test]
    fn roundtrip_knnvk() {
        let mk = MaterialKey::from_string("KNNvK").unwrap();
        roundtrip_random_indices(mk, 5);
    }

    #[test]
    fn index_to_position_out_of_bounds() {
        let mk = MaterialKey::from_string("KQvK").unwrap();
        let index = total_positions(&mk);
        assert!(matches!(
            index_to_position(&mk, index),
            Err(MaterialError::IndexOutOfBounds)
        ));
    }

    #[test]
    fn index_to_position_invalid_position() {
        let mk = MaterialKey::from_string("KvK").unwrap();
        let mut found_invalid = false;
        for idx in 0..total_positions(&mk) {
            match index_to_position(&mk, idx) {
                Err(MaterialError::InvalidPosition(_)) => {
                    found_invalid = true;
                    break;
                }
                Err(MaterialError::IndexOutOfBounds) => {
                    panic!("index {} unexpectedly out of bounds", idx)
                }
                Ok(_) => {}
                Err(MaterialError::MismatchedMaterial) => unreachable!(),
            }
        }
        assert!(found_invalid);
    }

    #[test]
    fn exhaustive_roundtrip_kvk() {
        let mk = MaterialKey::from_string("KvK").unwrap();
        for idx in 0..total_positions(&mk) {
            if let Ok(pos) = index_to_position(&mk, idx) {
                let roundtrip = position_to_index(&mk, &pos).unwrap();
                assert_eq!(idx, roundtrip);
            }
        }
    }

    #[test]
    fn side_to_move_affects_index() {
        use shakmaty::{CastlingMode, Chess, Color, Piece, Role, Setup, Square};

        let mk = MaterialKey::from_string("KvK").unwrap();
        let mut setup = Setup::empty();
        setup.board.set_piece_at(
            Square::E1,
            Piece {
                role: Role::King,
                color: Color::White,
            },
        );
        setup.board.set_piece_at(
            Square::E8,
            Piece {
                role: Role::King,
                color: Color::Black,
            },
        );

        setup.turn = Color::White;
        let white_pos = Chess::from_setup(setup.clone(), CastlingMode::Standard).unwrap();
        let white_index = position_to_index(&mk, &white_pos).unwrap();

        setup.turn = Color::Black;
        let black_pos = Chess::from_setup(setup, CastlingMode::Standard).unwrap();
        let black_index = position_to_index(&mk, &black_pos).unwrap();

        assert_eq!(black_index, white_index + 1);
        assert_eq!(white_index % 2, 0);
        assert_eq!(black_index % 2, 1);

        let reconstructed = index_to_position(&mk, white_index + 1).unwrap();
        assert_eq!(reconstructed.board(), white_pos.board());
        assert_eq!(reconstructed.turn(), Color::Black);
    }

    #[test]
    fn position_to_index_mismatched_material() {
        use shakmaty::{CastlingMode, Chess, Color, Piece, Role, Setup, Square};

        let mk = MaterialKey::from_string("KQvK").unwrap();
        let mut setup = Setup::empty();
        setup.board.set_piece_at(
            Square::E1,
            Piece {
                role: Role::King,
                color: Color::White,
            },
        );
        setup.board.set_piece_at(
            Square::E8,
            Piece {
                role: Role::King,
                color: Color::Black,
            },
        );
        setup.turn = Color::White;

        let pos = Chess::from_setup(setup, CastlingMode::Standard).unwrap();
        assert!(matches!(
            position_to_index(&mk, &pos),
            Err(MaterialError::MismatchedMaterial)
        ));
    }
}
