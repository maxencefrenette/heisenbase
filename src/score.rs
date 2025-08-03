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

    /// A checkmate is on the board. The side to move is checkmated.
    pub fn checkmate() -> Self {
        Self {
            min: DtzScore::immediate_loss(),
            max: DtzScore::immediate_loss(),
        }
    }

    /// Forced draw.
    pub fn draw() -> Self {
        Self {
            min: DtzScore::draw(),
            max: DtzScore::draw(),
        }
    }

    pub fn is_certain(&self) -> bool {
        self.min == self.max
    }

    pub fn is_uncertain(&self) -> bool {
        self.min != self.max
    }

    /// Flips the score range.
    ///
    /// This is used to convert a score range from the perspective of the side to move to the
    /// perspective of the other side.
    pub fn flip(&self) -> Self {
        Self {
            min: -self.max,
            max: -self.min,
        }
    }

    pub fn add_half_move(&self) -> Self {
        let mut min = self.min;
        let mut max = self.max;

        if min < DtzScore::draw() {
            min += 1;
        }
        if max > DtzScore::draw() {
            max -= 1;
        }

        Self { min, max }
    }

    /// Returns the bound-wise maximum of the two scores.
    pub fn max(&self, other: &Self) -> Self {
        let min = self.min.max(other.min);
        let max = self.max.max(other.max);

        Self { min, max }
    }
}
