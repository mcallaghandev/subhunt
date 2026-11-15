//! Turns mouse clicks and touches on the panel into helm orders.

use super::layout::Layout;

/// A finger or mouse button that is down, in panel pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pointer {
    pub x: f32,
    pub y: f32,
    /// True only in the frame the finger or button went down.
    pub just_pressed: bool,
}

/// What the player is doing with the panel this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PanelInput {
    /// A new throttle setting, if a pointer is on the throttle lever.
    pub throttle: Option<f32>,
    /// A rudder setting, if a pointer is on the rudder lever. When it's let go
    /// the rudder springs back to centre.
    pub rudder: Option<f32>,
    pub fire: bool,
    pub ping: bool,
    /// Buttons being held down, so they can be drawn pressed in.
    pub fire_held: bool,
    pub ping_held: bool,
}

/// Works out what every pointer is pressing. Several can be down at once
/// (one thumb on the throttle, one on the rudder).
pub fn read(layout: &Layout, pointers: &[Pointer]) -> PanelInput {
    let mut input = PanelInput::default();
    for pointer in pointers {
        let (x, y) = (pointer.x, pointer.y);
        if layout.throttle.contains(x, y) {
            input.throttle = Some(layout.throttle_setting_at(y));
        }
        if layout.rudder.contains(x, y) {
            input.rudder = Some(layout.rudder_setting_at(x));
        }
        if layout.fire.contains(x, y) {
            input.fire_held = true;
            input.fire |= pointer.just_pressed;
        }
        if layout.ping.contains(x, y) {
            input.ping_held = true;
            input.ping |= pointer.just_pressed;
        }
    }
    input
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(x: i32, y: i32) -> Pointer {
        Pointer { x: x as f32, y: y as f32, just_pressed: true }
    }

    #[test]
    fn dragging_the_throttle_sets_it() {
        let layout = Layout::landscape();
        let (top, bottom) = layout.throttle_track();
        let x = layout.throttle.x + 20;
        assert_eq!(read(&layout, &[press(x, top)]).throttle, Some(1.0));
        assert_eq!(read(&layout, &[press(x, bottom)]).throttle, Some(-1.0));
        assert_eq!(read(&layout, &[]).throttle, None);
    }

    #[test]
    fn buttons_fire_once_but_show_as_held() {
        let layout = Layout::portrait();
        let (x, y) = (layout.fire.x + 5, layout.fire.y + 5);
        let first = read(&layout, &[press(x, y)]);
        assert!(first.fire && first.fire_held);
        let still_down = read(&layout, &[Pointer { just_pressed: false, ..press(x, y) }]);
        assert!(!still_down.fire && still_down.fire_held);
    }

    #[test]
    fn two_thumbs_work_together() {
        let layout = Layout::portrait();
        let (left, _, track_y) = layout.rudder_track();
        let throttle = press(layout.throttle.x + 20, layout.throttle_y(0.5));
        let rudder = press(left, track_y);
        let input = read(&layout, &[throttle, rudder]);
        assert!(input.throttle.unwrap() > 0.4);
        assert_eq!(input.rudder, Some(1.0)); // left end = hard to port
    }

    #[test]
    fn pressing_the_sonar_screen_does_nothing() {
        let layout = Layout::landscape();
        assert_eq!(read(&layout, &[press(layout.sonar.x, layout.sonar.y)]), PanelInput::default());
    }
}
