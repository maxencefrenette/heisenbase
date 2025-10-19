#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transform {
    Identity,
    FlipHorizontal,
    FlipVertical,
    Rotate90,
    Rotate180,
    Rotate270,
    MirrorMain,
    MirrorAnti,
}

pub const ALL_TRANSFORMS: &[Transform] = &[
    Transform::Identity,
    Transform::FlipHorizontal,
    Transform::FlipVertical,
    Transform::Rotate90,
    Transform::Rotate270,
    Transform::Rotate180,
    Transform::MirrorMain,
    Transform::MirrorAnti,
];

pub const ROTATION_ONLY: &[Transform] = &[
    Transform::Identity,
    Transform::Rotate90,
    Transform::Rotate180,
    Transform::Rotate270,
];

pub const HORIZONTAL_ONLY: &[Transform] = &[Transform::Identity, Transform::FlipHorizontal];

pub const IDENTITY_ONLY: &[Transform] = &[Transform::Identity];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransformSet {
    Full,
    Rotations,
    Horizontal,
    Identity,
}

impl TransformSet {
    pub fn transforms(self) -> &'static [Transform] {
        match self {
            TransformSet::Full => ALL_TRANSFORMS,
            TransformSet::Rotations => ROTATION_ONLY,
            TransformSet::Horizontal => HORIZONTAL_ONLY,
            TransformSet::Identity => IDENTITY_ONLY,
        }
    }
}
