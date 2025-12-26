#[cfg(test)]
use shakmaty::Board;
use shakmaty::{Bitboard, File, Rank};

/// Represents the pawn structure of a position, i.e. the pawns on the board.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PawnStructure {
    pub pawn_bitboards: [Bitboard; 2],
}

impl PawnStructure {
    #[cfg(test)]
    pub fn new(white_pawns: Bitboard, black_pawns: Bitboard) -> Self {
        Self {
            pawn_bitboards: [white_pawns, black_pawns],
        }
    }

    #[cfg(test)]
    pub fn occupied(&self) -> Bitboard {
        self.pawn_bitboards[0] | self.pawn_bitboards[1]
    }

    pub fn flip_sides(&self) -> Self {
        Self {
            pawn_bitboards: [
                self.pawn_bitboards[1].flip_vertical(),
                self.pawn_bitboards[0].flip_vertical(),
            ],
        }
    }

    #[cfg(test)]
    pub fn to_board(&self) -> Board {
        use shakmaty::{ByColor, ByRole, Role};

        let mut by_role = ByRole::default();
        by_role[Role::Pawn] = self.occupied();
        let by_color = ByColor {
            white: self.pawn_bitboards[0],
            black: self.pawn_bitboards[1],
        };
        Board::try_from_bitboards(by_role, by_color).unwrap()
    }

    /// Returns the pawn structures that can be reached from this pawn structure by moving a pawn,
    /// without capturing or promoting a piece.
    #[allow(dead_code)]
    pub fn child_pawn_structures_no_piece_changes(&self) -> Vec<PawnStructure> {
        fn one_sided(ps: &PawnStructure) -> impl Iterator<Item = PawnStructure> {
            let can_be_moved_one_square = (Bitboard::FULL
                .without(Bitboard::BACKRANKS)
                .without(Bitboard::from_rank(Rank::Seventh)))
                & ps.pawn_bitboards[0]
                & !ps.pawn_bitboards[1].shift(-8);
            let can_be_moved_two_squares = Bitboard::from_rank(Rank::Second)
                & ps.pawn_bitboards[0]
                & !ps.pawn_bitboards[1].shift(-8)
                & !ps.pawn_bitboards[1].shift(-16);

            can_be_moved_one_square
                .into_iter()
                .map(|square| {
                    let mut child = ps.clone();
                    child.pawn_bitboards[0].discard(square);
                    child.pawn_bitboards[0].add(square.offset(8).unwrap());
                    child
                })
                .chain(can_be_moved_two_squares.into_iter().map(|square| {
                    let mut child = ps.clone();
                    child.pawn_bitboards[0].discard(square);
                    child.pawn_bitboards[0].add(square.offset(16).unwrap());
                    child
                }))
        }

        one_sided(&self)
            .chain(one_sided(&self.flip_sides()).map(|ps| ps.flip_sides()))
            .collect()
    }

    /// Returns the pawn structures that can be reached from this pawn structure by capturing a piece
    /// without promoting a pawn.
    #[allow(dead_code)]
    pub fn child_pawn_structures_with_piece_captures(&self) -> Vec<PawnStructure> {
        fn one_sided(ps: &PawnStructure) -> impl Iterator<Item = PawnStructure> {
            let can_capture_left = (Bitboard::FULL
                .without(Bitboard::BACKRANKS)
                .without(Bitboard::from_rank(Rank::Seventh)))
            .without(Bitboard::from_file(File::A))
                & ps.pawn_bitboards[0]
                & !ps.pawn_bitboards[1].shift(-7);
            let can_capture_right = (Bitboard::FULL
                .without(Bitboard::BACKRANKS)
                .without(Bitboard::from_rank(Rank::Seventh)))
            .without(Bitboard::from_file(File::H))
                & ps.pawn_bitboards[0]
                & !ps.pawn_bitboards[1].shift(-9);

            can_capture_right
                .into_iter()
                .map(|square| {
                    let mut child = ps.clone();
                    child.pawn_bitboards[0].discard(square);
                    child.pawn_bitboards[0].add(square.offset(9).unwrap());
                    child
                })
                .chain(can_capture_left.into_iter().map(|square| {
                    let mut child = ps.clone();
                    child.pawn_bitboards[0].discard(square);
                    child.pawn_bitboards[0].add(square.offset(7).unwrap());
                    child
                }))
        }

        one_sided(&self)
            .chain(one_sided(&self.flip_sides()).map(|ps| ps.flip_sides()))
            .collect()
    }

    /// Returns the pawn structures that can be reached from this pawn structure by promoting a pawn.
    #[allow(dead_code)]
    pub fn child_pawn_structures_with_promotions(&self) -> Vec<PawnStructure> {
        unimplemented!()
    }

    /// Returns the pawn structures that can be reached from this pawn structure by capturing a piece which
    /// results in a promotion.
    #[allow(dead_code)]
    pub fn child_pawn_structures_with_piece_captures_and_promotions(&self) -> Vec<PawnStructure> {
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_debug_snapshot;
    use shakmaty::{Bitboard, Board, Square};

    #[test]
    fn empty_pawn_structure_generates_no_moves() {
        let parent = PawnStructure::new(Bitboard::EMPTY, Bitboard::EMPTY);
        assert_debug_snapshot!(parent.to_board(), @"
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        ");
        assert_debug_snapshot!(
            parent
                .child_pawn_structures_no_piece_changes()
                .into_iter()
                .map(|ps| ps.to_board())
                .collect::<Vec<Board>>(), @"[]"
        );
    }

    #[test]
    fn child_pawn_structures_generates_moves_for_both_colors() {
        let parent = PawnStructure::new(
            Bitboard::from_square(Square::E2),
            Bitboard::from_square(Square::E7),
        );
        assert_debug_snapshot!(parent.to_board(), @"
        . . . . . . . .
        . . . . p . . .
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        . . . . P . . .
        . . . . . . . .
        ");
        assert_debug_snapshot!(parent.child_pawn_structures_no_piece_changes().into_iter().map(|ps| ps.to_board()).collect::<Vec<Board>>(), @"
        [
            . . . . . . . .
            . . . . p . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . P . . .
            . . . . . . . .
            . . . . . . . .
            ,
            . . . . . . . .
            . . . . p . . .
            . . . . . . . .
            . . . . . . . .
            . . . . P . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            ,
            . . . . . . . .
            . . . . . . . .
            . . . . p . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . P . . .
            . . . . . . . .
            ,
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . p . . .
            . . . . . . . .
            . . . . . . . .
            . . . . P . . .
            . . . . . . . .
            ,
        ]
        ");
    }

    #[test]
    fn child_pawn_structures_blocked_by_opponent_generates_no_moves() {
        let parent = PawnStructure::new(
            Bitboard::from_square(Square::E2),
            Bitboard::from_square(Square::E3),
        );
        assert_debug_snapshot!(parent.to_board(), @"
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        . . . . p . . .
        . . . . P . . .
        . . . . . . . .
        ");
        assert_debug_snapshot!(
            parent
                .child_pawn_structures_no_piece_changes()
                .into_iter()
                .map(|ps| ps.to_board())
                .collect::<Vec<Board>>(), @"[]"
        );
    }

    #[test]
    fn child_pawn_structures_blocked_double_step_still_allows_single_step() {
        let parent = PawnStructure::new(
            Bitboard::from_square(Square::E2),
            Bitboard::from_square(Square::E4),
        );
        assert_debug_snapshot!(parent.to_board(), @"
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        . . . . p . . .
        . . . . . . . .
        . . . . P . . .
        . . . . . . . .
        ");
        assert_debug_snapshot!(
            parent
                .child_pawn_structures_no_piece_changes()
                .into_iter()
                .map(|ps| ps.to_board())
                .collect::<Vec<Board>>(), @"
        [
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . p . . .
            . . . . P . . .
            . . . . . . . .
            . . . . . . . .
            ,
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . p . . .
            . . . . P . . .
            . . . . . . . .
            ,
        ]
        "
        );
    }

    #[test]
    fn child_pawn_structures_seventh_rank_generates_no_moves() {
        let parent = PawnStructure::new(
            Bitboard::from_square(Square::A7),
            Bitboard::from_square(Square::H2),
        );
        assert_debug_snapshot!(parent.to_board(), @"
        . . . . . . . .
        P . . . . . . .
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        . . . . . . . p
        . . . . . . . .
        ");
        assert_debug_snapshot!(
            parent
                .child_pawn_structures_no_piece_changes()
                .into_iter()
                .map(|ps| ps.to_board())
            .collect::<Vec<Board>>(), @"[]"
        );
    }

    #[test]
    fn child_pawn_structures_with_piece_captures_generates_moves_for_both_colors() {
        let parent = PawnStructure::new(
            Bitboard::from_square(Square::E4),
            Bitboard::from_square(Square::E5),
        );
        assert_debug_snapshot!(parent.to_board(), @"
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        . . . . p . . .
        . . . . P . . .
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        ");
        assert_debug_snapshot!(
            parent
                .child_pawn_structures_with_piece_captures()
                .into_iter()
                .map(|ps| ps.to_board())
                .collect::<Vec<Board>>(), @"
        [
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . p P . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            ,
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . P p . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            ,
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . P p . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            ,
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . p P . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            ,
        ]
        "
        );
    }

    #[test]
    fn child_pawn_structures_with_piece_captures_excludes_capturing_pawns() {
        let parent = PawnStructure::new(
            Bitboard::from_square(Square::E4),
            Bitboard::from_square(Square::D5) | Bitboard::from_square(Square::F5),
        );
        assert_debug_snapshot!(parent.to_board(), @"
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        . . . p . p . .
        . . . . P . . .
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        ");
        assert_debug_snapshot!(
            parent
                .child_pawn_structures_with_piece_captures()
                .into_iter()
                .map(|ps| ps.to_board())
                .collect::<Vec<Board>>(), @"
        [
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . p . . . .
            . . . . P . p .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            ,
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . p . .
            . . p . P . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            ,
        ]
        "
        );
    }
}
