//! A tiny bit of 2D maths.
//!
//! We could use a library such as `glam` (Bevy uses it), but writing our own
//! keeps the domain dependency-free and shows how operator overloading works.

use std::f32::consts::{PI, TAU};
use std::ops::{Add, AddAssign, Mul, Sub};

/// A position or direction in world units (think "metres").
///
/// Angles throughout the game are in radians, measured counter-clockwise from
/// the +x axis (east). That matches Bevy's 2D coordinate system, where +y is up.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Self = Self::new(0.0, 0.0);

    /// `const fn` can be evaluated at compile time, so it can be used in `const`s.
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// A unit-length vector pointing along `angle`.
    pub fn from_angle(angle: f32) -> Self {
        Self::new(angle.cos(), angle.sin())
    }

    pub fn length(self) -> f32 {
        self.x.hypot(self.y)
    }

    pub fn distance(self, other: Self) -> f32 {
        (self - other).length()
    }

    /// The direction this vector points in, in radians.
    pub fn angle(self) -> f32 {
        self.y.atan2(self.x)
    }

    /// Keeps the point inside a square centred on the origin.
    pub fn clamp_to_square(self, half_extent: f32) -> Self {
        Self::new(self.x.clamp(-half_extent, half_extent), self.y.clamp(-half_extent, half_extent))
    }
}

// Rust note: implementing the `Add` trait is what makes `a + b` compile for our
// type. `type Output` says what `a + b` produces.
impl Add for Vec2 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl AddAssign for Vec2 {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl Sub for Vec2 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl Mul<f32> for Vec2 {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self {
        Self::new(self.x * rhs, self.y * rhs)
    }
}

/// Wraps an angle into the range (-PI, PI].
pub fn wrap_angle(angle: f32) -> f32 {
    let wrapped = (angle + PI).rem_euclid(TAU) - PI;
    if wrapped <= -PI { wrapped + TAU } else { wrapped }
}

/// Converts a game angle to a compass bearing in degrees
/// (0 = north, 90 = east), which is what a submarine crew would say.
pub fn compass_degrees(angle: f32) -> f32 {
    (90.0 - angle.to_degrees()).rem_euclid(360.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_angle_stays_in_range() {
        for raw in [-10.0, -PI, 0.0, PI, 3.0 * PI, 10.0] {
            let w = wrap_angle(raw);
            assert!(w > -PI - 1e-5 && w <= PI + 1e-5, "{raw} wrapped to {w}");
        }
    }

    #[test]
    fn compass_bearings_match_conventions() {
        assert!((compass_degrees(0.0) - 90.0).abs() < 1e-4); // east
        assert!((compass_degrees(PI / 2.0) - 0.0).abs() < 1e-4); // north
        assert!((compass_degrees(PI) - 270.0).abs() < 1e-4); // west
    }
}
