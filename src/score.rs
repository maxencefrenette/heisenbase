use std::ops::{Add, AddAssign, Neg, Sub, SubAssign};

use crate::wdl_score_range::WdlScoreRange;

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

    pub fn is_win(&self) -> bool {
        self.0 > 0
    }

    pub fn is_loss(&self) -> bool {
        self.0 < 0
    }
}

impl DtzScore {
    pub fn add_half_move(&self) -> Self {
        if self.0 > 0 {
            Self(self.0 - 1)
        } else if self.0 < 0 {
            Self(self.0 + 1)
        } else {
            self.clone()
        }
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
    min: DtzScore,
    max: DtzScore,
}

impl DtzScoreRange {
    pub fn unknown() -> Self {
        Self {
            min: DtzScore::immediate_loss(),
            max: DtzScore::immediate_win(),
        }
    }

    pub fn illegal() -> Self {
        Self {
            min: DtzScore::immediate_win(),
            max: DtzScore::immediate_loss(),
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
        self.min == self.max || self.is_illegal()
    }

    pub fn is_uncertain(&self) -> bool {
        self.min != self.max
    }

    pub fn is_illegal(&self) -> bool {
        self.min.is_win() && self.max.is_loss()
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
        if self.is_illegal() {
            return *self;
        }
        Self {
            min: self.min.add_half_move(),
            max: self.max.add_half_move(),
        }
    }

    /// Returns the bound-wise maximum of the two scores.
    pub fn max(&self, other: &Self) -> Self {
        let min = self.min.max(other.min);
        let max = self.max.max(other.max);

        Self { min, max }
    }
}

impl From<DtzScoreRange> for WdlScoreRange {
    fn from(score: DtzScoreRange) -> Self {
        match (score.min.0.signum(), score.max.0.signum()) {
            (1, -1) => WdlScoreRange::IllegalPosition,
            (1, 1) => WdlScoreRange::Win,
            (1, 0) => panic!("DtzScoreRange::into: min > 0 and max == 0"),
            (0, 1) => WdlScoreRange::WinOrDraw,
            (0, 0) => WdlScoreRange::Draw,
            (0, -1) => panic!("DtzScoreRange::into: min == 0 and max < 0"),
            (-1, 1) => WdlScoreRange::Unknown,
            (-1, 0) => WdlScoreRange::DrawOrLoss,
            (-1, -1) => WdlScoreRange::Loss,
            (_, _) => unreachable!(),
        }
    }
}

impl From<WdlScoreRange> for DtzScoreRange {
    fn from(value: WdlScoreRange) -> Self {
        match value {
            WdlScoreRange::Unknown => DtzScoreRange::unknown(),
            WdlScoreRange::WinOrDraw => Self {
                min: DtzScore::draw(),
                max: DtzScore::immediate_win(),
            },
            WdlScoreRange::DrawOrLoss => Self {
                min: DtzScore::immediate_loss(),
                max: DtzScore::draw(),
            },
            WdlScoreRange::Win => Self {
                min: DtzScore::immediate_win(),
                max: DtzScore::immediate_win(),
            },
            WdlScoreRange::Draw => Self {
                min: DtzScore::draw(),
                max: DtzScore::draw(),
            },
            WdlScoreRange::Loss => Self {
                min: DtzScore::immediate_loss(),
                max: DtzScore::immediate_loss(),
            },
            WdlScoreRange::IllegalPosition => DtzScoreRange::illegal(),
        }
    }
}
