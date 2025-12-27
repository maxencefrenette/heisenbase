#[cfg(test)]
use shakmaty::Board;
use shakmaty::{Bitboard, ByColor, File, Rank};

/// Represents the pawn structure of a position, i.e. the pawns on the board.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PawnStructure(ByColor<Bitboard>);

impl PawnStructure {
    #[cfg(test)]
    pub fn new(white_pawns: Bitboard, black_pawns: Bitboard) -> Self {
        Self(ByColor {
            white: white_pawns,
            black: black_pawns,
        })
    }

    #[cfg(test)]
    pub fn occupied(&self) -> Bitboard {
        self.0.white | self.0.black
    }

    pub fn flip_sides(&self) -> Self {
        Self(ByColor {
            white: self.0.black.flip_vertical(),
            black: self.0.white.flip_vertical(),
        })
    }

    #[cfg(test)]
    pub fn to_board(&self) -> Board {
        use shakmaty::{ByColor, ByRole, Role};

        let mut by_role = ByRole::default();
        by_role[Role::Pawn] = self.occupied();
        let by_color = ByColor {
            white: self.0.white,
            black: self.0.black,
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
                & ps.0.white
                & !ps.0.black.shift(-8);
            let can_be_moved_two_squares = Bitboard::from_rank(Rank::Second)
                & ps.0.white
                & !ps.0.black.shift(-8)
                & !ps.0.black.shift(-16);
            let can_capture_left = (Bitboard::FULL
                .without(Bitboard::BACKRANKS)
                .without(Bitboard::from_rank(Rank::Seventh)))
            .without(Bitboard::from_file(File::A))
                & ps.0.white
                & ps.0.black.shift(-7);
            let can_capture_right = (Bitboard::FULL
                .without(Bitboard::BACKRANKS)
                .without(Bitboard::from_rank(Rank::Seventh)))
            .without(Bitboard::from_file(File::H))
                & ps.0.white
                & ps.0.black.shift(-9);

            can_be_moved_one_square
                .into_iter()
                .map(|square| {
                    let mut child = ps.clone();
                    child.0.white.discard(square);
                    child.0.white.add(square.offset(8).unwrap());
                    child
                })
                .chain(can_be_moved_two_squares.into_iter().map(|square| {
                    let mut child = ps.clone();
                    child.0.white.discard(square);
                    child.0.white.add(square.offset(16).unwrap());
                    child
                }))
                .chain(can_capture_right.into_iter().map(|square| {
                    let mut child = ps.clone();
                    let target = square.offset(9).unwrap();
                    child.0.white.discard(square);
                    child.0.white.add(target);
                    child.0.black.discard(target);
                    child
                }))
                .chain(can_capture_left.into_iter().map(|square| {
                    let mut child = ps.clone();
                    let target = square.offset(7).unwrap();
                    child.0.white.discard(square);
                    child.0.white.add(target);
                    child.0.black.discard(target);
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
                & ps.0.white
                & !ps.0.black.shift(-7);
            let can_capture_right = (Bitboard::FULL
                .without(Bitboard::BACKRANKS)
                .without(Bitboard::from_rank(Rank::Seventh)))
            .without(Bitboard::from_file(File::H))
                & ps.0.white
                & !ps.0.black.shift(-9);

            can_capture_right
                .into_iter()
                .map(|square| {
                    let mut child = ps.clone();
                    child.0.white.discard(square);
                    child.0.white.add(square.offset(9).unwrap());
                    child
                })
                .chain(can_capture_left.into_iter().map(|square| {
                    let mut child = ps.clone();
                    child.0.white.discard(square);
                    child.0.white.add(square.offset(7).unwrap());
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
        fn one_sided(ps: &PawnStructure) -> impl Iterator<Item = PawnStructure> {
            let can_promote_forward = Bitboard::from_rank(Rank::Seventh) & ps.0.white;

            can_promote_forward.into_iter().map(|square| {
                let mut child = ps.clone();
                child.0.white.discard(square);
                child
            })
        }

        one_sided(&self)
            .chain(one_sided(&self.flip_sides()).map(|ps| ps.flip_sides()))
            .collect()
    }

    /// Returns the pawn structures that can be reached from this pawn structure by capturing a piece which
    /// results in a promotion.
    #[allow(dead_code)]
    pub fn child_pawn_structures_with_piece_captures_and_promotions(&self) -> Vec<PawnStructure> {
        fn one_sided(ps: &PawnStructure) -> impl Iterator<Item = PawnStructure> {
            let can_capture_left = Bitboard::from_rank(Rank::Seventh)
                .without(Bitboard::from_file(File::A))
                & ps.0.white;
            let can_capture_right = Bitboard::from_rank(Rank::Seventh)
                .without(Bitboard::from_file(File::H))
                & ps.0.white;

            can_capture_right
                .into_iter()
                .map(|square| {
                    let mut child = ps.clone();
                    let target = square.offset(9).unwrap();
                    child.0.white.discard(square);
                    child.0.black.discard(target);
                    child
                })
                .chain(can_capture_left.into_iter().map(|square| {
                    let mut child = ps.clone();
                    let target = square.offset(7).unwrap();
                    child.0.white.discard(square);
                    child.0.black.discard(target);
                    child
                }))
        }

        one_sided(&self)
            .chain(one_sided(&self.flip_sides()).map(|ps| ps.flip_sides()))
            .collect()
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
    fn child_pawn_structures_includes_pawn_captures() {
        let parent = PawnStructure::new(
            Bitboard::from_square(Square::E4),
            Bitboard::from_square(Square::D5),
        );
        assert_debug_snapshot!(parent.to_board(), @"
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        . . . p . . . .
        . . . . P . . .
        . . . . . . . .
        . . . . . . . .
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
            . . . p P . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            ,
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . P . . . .
            . . . . . . . .
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
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . p . . .
            . . . . . . . .
            . . . . . . . .
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

    #[test]
    fn child_pawn_structures_with_promotions_removes_promoting_pawn() {
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
                .child_pawn_structures_with_promotions()
                .into_iter()
                .map(|ps| ps.to_board())
                .collect::<Vec<Board>>(), @"
        [
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . p
            . . . . . . . .
            ,
            . . . . . . . .
            P . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            ,
        ]
        "
        );
    }

    #[test]
    fn child_pawn_structures_with_piece_captures_and_promotions_removes_captured_pawn() {
        let parent = PawnStructure::new(
            Bitboard::from_square(Square::B7),
            Bitboard::from_square(Square::G2),
        );
        assert_debug_snapshot!(parent.to_board(), @"
        . . . . . . . .
        . P . . . . . .
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        . . . . . . p .
        . . . . . . . .
        ");
        assert_debug_snapshot!(
            parent
                .child_pawn_structures_with_piece_captures_and_promotions()
                .into_iter()
                .map(|ps| ps.to_board())
                .collect::<Vec<Board>>(), @"
        [
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . p .
            . . . . . . . .
            ,
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . p .
            . . . . . . . .
            ,
            . . . . . . . .
            . P . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            ,
            . . . . . . . .
            . P . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            ,
        ]
        "
        );
    }
}
