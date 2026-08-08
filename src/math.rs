use std::ops::{Add, AddAssign, Mul, Sub};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn rotate(self, radians: f32) -> Self {
        let (sin, cos) = radians.sin_cos();
        Self {
            x: self.x * cos - self.y * sin,
            y: self.x * sin + self.y * cos,
        }
    }

    pub fn length(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    pub fn normalized(self) -> Self {
        let length = self.length();
        if length <= f32::EPSILON {
            Self::ZERO
        } else {
            self * (1.0 / length)
        }
    }
}

impl Add for Vec2 {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl AddAssign for Vec2 {
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

impl Sub for Vec2 {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl Mul<f32> for Vec2 {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self::Output {
        Self::new(self.x * rhs, self.y * rhs)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Attachment {
    Right,
    Left,
    Top,
    Bottom,
}

impl Attachment {
    pub fn normal(self) -> Vec2 {
        match self {
            Self::Right => Vec2::new(1.0, 0.0),
            Self::Left => Vec2::new(-1.0, 0.0),
            Self::Top => Vec2::new(0.0, 1.0),
            Self::Bottom => Vec2::new(0.0, -1.0),
        }
    }

    pub fn opposite(self) -> Self {
        match self {
            Self::Right => Self::Left,
            Self::Left => Self::Right,
            Self::Top => Self::Bottom,
            Self::Bottom => Self::Top,
        }
    }
}

pub fn face_anchor(size: Vec2, attachment: Attachment) -> Vec2 {
    let normal = attachment.normal();
    Vec2::new(normal.x * size.x * 0.5, normal.y * size.y * 0.5)
}

pub fn child_center_offset(parent_size: Vec2, child_size: Vec2, attachment: Attachment) -> Vec2 {
    face_anchor(parent_size, attachment) - face_anchor(child_size, attachment.opposite())
}
