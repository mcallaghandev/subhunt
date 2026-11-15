//! Where everything sits on the panel.
//!
//! There are two hand-drawn layouts: a wide one for computers and phones held
//! sideways, and a tall one for phones held upright. All positions are in
//! panel pixels (before scaling up), measured from the top-left corner.
//!
//! The drawing code and the control code both read positions from here, so
//! what you see is always exactly what you can press.

use super::canvas::IRect;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Orientation {
    Landscape,
    Portrait,
}

/// A round instrument: centre and radius of its face.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dial {
    pub x: i32,
    pub y: i32,
    pub r: i32,
}

impl Dial {
    pub fn centre(&self) -> (i32, i32) {
        (self.x, self.y)
    }

    #[cfg(test)] // only the tests need this
    /// The space a small gauge takes up: its bezel plus the label plate below.
    pub fn footprint(&self) -> IRect {
        let outer = self.r + 3;
        IRect::new(self.x - outer, self.y - outer, 2 * outer + 1, 2 * outer + 1 + 9)
    }

    #[cfg(test)] // only the tests need this
    /// The space the sonar screen takes up: a thicker bezel and no label.
    pub fn sonar_footprint(&self) -> IRect {
        let outer = self.r + 4;
        IRect::new(self.x - outer, self.y - outer, 2 * outer + 1, 2 * outer + 1)
    }
}

/// Small enough to snap to "stop" or "centred" when you're close.
const DETENT: f32 = 0.06;

#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub orientation: Orientation,
    pub width: i32,
    pub height: i32,
    /// Name plate along the top: submarine number and score.
    pub plate: IRect,
    pub sonar: Dial,
    pub speed: Dial,
    pub heading: Dial,
    pub noise: Dial,
    pub throttle: IRect,
    pub rudder: IRect,
    pub fire: IRect,
    pub ping: IRect,
    /// Warning lamps.
    pub lamps: IRect,
    /// Two-line message display.
    pub message: IRect,
}

impl Layout {
    /// Picks the layout that best fits a window of this shape.
    pub fn for_window(width: f32, height: f32) -> Self {
        if height > width { Self::portrait() } else { Self::landscape() }
    }

    /// 320×180: sonar on the left, instruments and controls on the right.
    pub fn landscape() -> Self {
        Self {
            orientation: Orientation::Landscape,
            width: 320,
            height: 180,
            plate: IRect::new(180, 4, 136, 9),
            sonar: Dial { x: 90, y: 90, r: 76 },
            speed: Dial { x: 201, y: 36, r: 18 },
            heading: Dial { x: 248, y: 36, r: 18 },
            noise: Dial { x: 295, y: 36, r: 18 },
            throttle: IRect::new(180, 70, 36, 106),
            fire: IRect::new(222, 70, 45, 28),
            ping: IRect::new(271, 70, 45, 28),
            lamps: IRect::new(222, 102, 94, 22),
            rudder: IRect::new(222, 128, 94, 22),
            message: IRect::new(222, 154, 94, 22),
        }
    }

    /// 180×340: sonar on top, instruments in a row, controls underneath.
    pub fn portrait() -> Self {
        Self {
            orientation: Orientation::Portrait,
            width: 180,
            height: 340,
            plate: IRect::new(5, 4, 170, 9),
            sonar: Dial { x: 90, y: 96, r: 76 },
            speed: Dial { x: 34, y: 204, r: 18 },
            heading: Dial { x: 90, y: 204, r: 18 },
            noise: Dial { x: 146, y: 204, r: 18 },
            throttle: IRect::new(6, 240, 36, 96),
            fire: IRect::new(48, 240, 61, 28),
            ping: IRect::new(113, 240, 61, 28),
            lamps: IRect::new(48, 272, 126, 20),
            rudder: IRect::new(48, 296, 126, 20),
            message: IRect::new(48, 320, 126, 16),
        }
    }

    /// Top and bottom of the throttle lever's slot (full ahead at the top).
    pub fn throttle_track(&self) -> (i32, i32) {
        (self.throttle.y + 14, self.throttle.bottom() - 6)
    }

    /// Where the throttle handle sits for a setting from -1 to 1.
    pub fn throttle_y(&self, setting: f32) -> i32 {
        let (top, bottom) = self.throttle_track();
        let t = (1.0 - setting.clamp(-1.0, 1.0)) / 2.0;
        top + ((bottom - top) as f32 * t).round() as i32
    }

    /// The throttle setting for a finger or mouse at height `y`.
    pub fn throttle_setting_at(&self, y: f32) -> f32 {
        let (top, bottom) = self.throttle_track();
        let t = (y - top as f32) / (bottom - top) as f32;
        snap((1.0 - 2.0 * t).clamp(-1.0, 1.0))
    }

    /// Left end, right end and height of the rudder lever's slot.
    pub fn rudder_track(&self) -> (i32, i32, i32) {
        (self.rudder.x + 6, self.rudder.right() - 7, self.rudder.y + 15)
    }

    /// Where the rudder handle sits. Port (+1, turn left) is at the left end.
    pub fn rudder_x(&self, setting: f32) -> i32 {
        let (left, right, _) = self.rudder_track();
        let t = (1.0 - setting.clamp(-1.0, 1.0)) / 2.0;
        left + ((right - left) as f32 * t).round() as i32
    }

    pub fn rudder_setting_at(&self, x: f32) -> f32 {
        let (left, right, _) = self.rudder_track();
        let t = (x - left as f32) / (right - left) as f32;
        snap((1.0 - 2.0 * t).clamp(-1.0, 1.0))
    }
}

fn snap(setting: f32) -> f32 {
    if setting.abs() < DETENT { 0.0 } else { setting }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn every_part(layout: &Layout) -> Vec<(&'static str, IRect)> {
        vec![
            ("plate", layout.plate),
            ("sonar", layout.sonar.sonar_footprint()),
            ("speed", layout.speed.footprint()),
            ("heading", layout.heading.footprint()),
            ("noise", layout.noise.footprint()),
            ("throttle", layout.throttle),
            ("fire", layout.fire),
            ("ping", layout.ping),
            ("lamps", layout.lamps),
            ("rudder", layout.rudder),
            ("message", layout.message),
        ]
    }

    #[test]
    fn everything_fits_and_nothing_overlaps() {
        for layout in [Layout::landscape(), Layout::portrait()] {
            let screen = IRect::new(0, 0, layout.width, layout.height);
            let parts = every_part(&layout);
            for (i, (name, rect)) in parts.iter().enumerate() {
                let inside = rect.x >= 0 && rect.y >= 0 && rect.right() <= screen.w && rect.bottom() <= screen.h;
                assert!(inside, "{:?}: {name} sticks out of the panel: {rect:?}", layout.orientation);
                for (other, other_rect) in &parts[i + 1..] {
                    assert!(!rect.overlaps(other_rect), "{:?}: {name} overlaps {other}", layout.orientation);
                }
            }
        }
    }

    #[test]
    fn lever_positions_and_settings_agree() {
        for layout in [Layout::landscape(), Layout::portrait()] {
            for setting in [-1.0, -0.5, 0.0, 0.5, 1.0] {
                let y = layout.throttle_y(setting) as f32;
                assert!((layout.throttle_setting_at(y) - setting).abs() < 0.05);
                let x = layout.rudder_x(setting) as f32;
                assert!((layout.rudder_setting_at(x) - setting).abs() < 0.05);
            }
        }
    }

    #[test]
    fn tall_windows_get_the_tall_layout() {
        assert_eq!(Layout::for_window(390.0, 844.0).orientation, Orientation::Portrait);
        assert_eq!(Layout::for_window(960.0, 540.0).orientation, Orientation::Landscape);
    }
}
