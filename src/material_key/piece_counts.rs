use crate::material_key::HbPieceRole;
use std::ops::{Index, IndexMut};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PieceCounts {
    counts: [u8; HbPieceRole::ALL.len()],
}

impl PieceCounts {
    pub fn empty() -> Self {
        Self {
            counts: [0u8; HbPieceRole::ALL.len()],
        }
    }

    pub fn from_array(counts: [u8; HbPieceRole::ALL.len()]) -> Self {
        Self { counts }
    }

    pub fn total(&self) -> u8 {
        self.counts.iter().sum()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, u8> {
        self.counts.iter()
    }

    pub fn swap_bishops(&mut self) {
        self.counts.swap(
            HbPieceRole::LightBishop as usize,
            HbPieceRole::DarkBishop as usize,
        );
    }
}

impl Index<HbPieceRole> for PieceCounts {
    type Output = u8;

    fn index(&self, role: HbPieceRole) -> &Self::Output {
        &self.counts[role as usize]
    }
}

impl IndexMut<HbPieceRole> for PieceCounts {
    fn index_mut(&mut self, role: HbPieceRole) -> &mut Self::Output {
        &mut self.counts[role as usize]
    }
}
