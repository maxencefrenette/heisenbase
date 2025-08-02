use std::ops::{Add, AddAssign, Neg, Sub, SubAssign};

/// A DTZ score.
///
/// This score is from the perspective of the side to move.
///
/// +99 means the side to move wins and has a zeroing move immediately available
/// +1 means the side to move wins and has a zeroing move in 100 halfmoves
/// 0 means the side to move draws
/// -1 means the side to move loses and has a zeroing move immediately available
/// -99 means the side to move loses and has a zeroing move in 100 halfmoves
/// -100 means the side to move is checkmated
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DtzScore(i8);

impl DtzScore {
    pub fn immediate_win() -> Self {
        Self(99)
    }

    pub fn immediate_loss() -> Self {
        Self(-100)
    }

    pub fn draw() -> Self {
        Self(0)
    }

    pub fn is_draw(&self) -> bool {
        self.0 == 0
    }
}

impl Neg for DtzScore {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self(-self.0)
    }
}

impl Add<i8> for DtzScore {
    type Output = Self;

    fn add(self, other: i8) -> Self::Output {
        Self(self.0 + other)
    }
}

impl AddAssign<i8> for DtzScore {
    fn add_assign(&mut self, other: i8) {
        self.0 += other;
    }
}

impl Sub<i8> for DtzScore {
    type Output = Self;

    fn sub(self, other: i8) -> Self::Output {
        Self(self.0 - other)
    }
}

impl SubAssign<i8> for DtzScore {
    fn sub_assign(&mut self, other: i8) {
        self.0 -= other;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DtzScoreRange {
    pub min: DtzScore,
    pub max: DtzScore,
}

impl DtzScoreRange {
    pub fn unknown() -> Self {
        Self {
            min: DtzScore::immediate_loss(),
            max: DtzScore::immediate_win(),
        }
    }

    pub fn checkmate() -> Self {
        Self {
            min: DtzScore::immediate_loss(),
            max: DtzScore::immediate_loss(),
        }
    }

    pub fn draw() -> Self {
        Self {
            min: DtzScore::draw(),
            max: DtzScore::draw(),
        }
    }

    fn parent_move_score(&self) -> Self {
        let mut min = -self.max;
        let mut max = -self.min;

        if !min.is_draw() {
            min += 1;
        }
        if !max.is_draw() {
            max -= 1;
        }

        Self { min, max }
    }

    /// Used as part of a reduce call to find the best score.
    ///
    /// other is one halfmove in the future compared to self.
    pub fn negamax(&self, other: &Self) -> Self {
        let other_flipped = other.parent_move_score();

        let min = self.min.max(other_flipped.min);
        let max = self.max.max(other_flipped.max);

        Self { min, max }
    }
}
