//! Input adapter: on-screen buttons for phones and tablets.
//!
//! The buttons stay hidden until the screen is first touched, so desktop
//! players never see them. We read every finger from Bevy's `Touches`
//! resource and work out which button each one is on ourselves. That makes
//! multi-touch straightforward: one thumb can steer while the other holds
//! the throttle.
//!
//! The layout is a plain table of numbers (`BUTTONS`), used both to draw the
//! buttons and to hit-test touches, so the two can never disagree.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::helm::{Helm, THROTTLE_LEVER_SPEED, read_keyboard};
use crate::hud::KeyboardHelp;

/// Sized so both clusters fit side by side on a 320-pixel-wide phone.
const BUTTON: f32 = 64.0;
const GAP: f32 = 8.0;
const MARGIN: f32 = 12.0;
/// Second row / column offset.
const STEP: f32 = MARGIN + BUTTON + GAP;

const IDLE: Color = Color::srgba(0.15, 0.85, 0.35, 0.12);
const HELD: Color = Color::srgba(0.15, 0.85, 0.35, 0.45);
const OUTLINE: Color = Color::srgba(0.3, 1.0, 0.5, 0.6);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Component)]
pub enum TouchButton {
    Port,
    Starboard,
    Ahead,
    Astern,
    Stop,
    Fire,
    Ping,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Side {
    Left,
    Right,
}

/// Where a button sits: distance in pixels from the bottom corner on its side.
struct ButtonSpec {
    button: TouchButton,
    label: &'static str,
    side: Side,
    from_side: f32,
    from_bottom: f32,
    width: f32,
}

/// Left thumb steers and fires; right thumb drives and pings.
const BUTTONS: [ButtonSpec; 7] = [
    ButtonSpec {
        button: TouchButton::Port,
        label: "< PORT",
        side: Side::Left,
        from_side: MARGIN,
        from_bottom: MARGIN,
        width: BUTTON,
    },
    ButtonSpec {
        button: TouchButton::Starboard,
        label: "STBD >",
        side: Side::Left,
        from_side: STEP,
        from_bottom: MARGIN,
        width: BUTTON,
    },
    ButtonSpec {
        button: TouchButton::Fire,
        label: "FIRE",
        side: Side::Left,
        from_side: MARGIN,
        from_bottom: STEP,
        width: 2.0 * BUTTON + GAP,
    },
    ButtonSpec {
        button: TouchButton::Astern,
        label: "ASTERN",
        side: Side::Right,
        from_side: MARGIN,
        from_bottom: MARGIN,
        width: BUTTON,
    },
    ButtonSpec {
        button: TouchButton::Ahead,
        label: "AHEAD",
        side: Side::Right,
        from_side: MARGIN,
        from_bottom: STEP,
        width: BUTTON,
    },
    ButtonSpec {
        button: TouchButton::Ping,
        label: "PING",
        side: Side::Right,
        from_side: STEP,
        from_bottom: MARGIN,
        width: BUTTON,
    },
    ButtonSpec {
        button: TouchButton::Stop,
        label: "STOP",
        side: Side::Right,
        from_side: STEP,
        from_bottom: STEP,
        width: BUTTON,
    },
];

impl ButtonSpec {
    /// The button's rectangle in screen pixels (origin top-left, like touches).
    fn rect(&self, screen: Vec2) -> Rect {
        let x = match self.side {
            Side::Left => self.from_side,
            Side::Right => screen.x - self.from_side - self.width,
        };
        let y = screen.y - self.from_bottom - BUTTON;
        Rect::new(x, y, x + self.width, y + BUTTON)
    }
}

/// Which button (if any) is under a point on a screen of the given size.
fn button_at(point: Vec2, screen: Vec2) -> Option<TouchButton> {
    BUTTONS.iter().find(|spec| spec.rect(screen).contains(point)).map(|spec| spec.button)
}

/// Whether touch controls are showing (they appear on the first touch).
#[derive(Resource, Default)]
struct TouchControls {
    active: bool,
}

/// The parent node holding all the buttons, so we can show them in one go.
#[derive(Component)]
struct TouchPanel;

pub struct TouchPlugin;

impl Plugin for TouchPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TouchControls>()
            .add_systems(Startup, spawn_buttons)
            // Touch runs after the keyboard so a held touch button can override
            // the keyboard's "no rudder" for this frame.
            .add_systems(Update, (read_touches.after(read_keyboard), show_on_first_touch, highlight_held));
    }
}

fn spawn_buttons(mut commands: Commands) {
    commands
        .spawn((
            Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() },
            Visibility::Hidden,
            TouchPanel,
        ))
        .with_children(|panel| {
            for spec in &BUTTONS {
                let mut node = Node {
                    position_type: PositionType::Absolute,
                    bottom: px(spec.from_bottom),
                    width: px(spec.width),
                    height: px(BUTTON),
                    border: UiRect::all(px(2.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                };
                match spec.side {
                    Side::Left => node.left = px(spec.from_side),
                    Side::Right => node.right = px(spec.from_side),
                }
                panel.spawn((node, BackgroundColor(IDLE), BorderColor::from(OUTLINE), spec.button)).with_child((
                    Text::new(spec.label),
                    TextFont { font_size: FontSize::Px(14.0), ..default() },
                    TextColor(OUTLINE),
                ));
            }
        });
}

fn show_on_first_touch(
    touches: Res<Touches>,
    mut controls: ResMut<TouchControls>,
    mut panel: Query<&mut Visibility, (With<TouchPanel>, Without<KeyboardHelp>)>,
    mut help: Query<&mut Visibility, (With<KeyboardHelp>, Without<TouchPanel>)>,
) {
    if controls.active || !touches.any_just_pressed() {
        return;
    }
    controls.active = true;
    for mut visibility in &mut panel {
        *visibility = Visibility::Visible;
    }
    for mut visibility in &mut help {
        *visibility = Visibility::Hidden; // keyboard hints are no use on a phone
    }
}

/// The buttons currently under at least one finger.
fn held_buttons(touches: &Touches, screen: Vec2) -> Vec<TouchButton> {
    touches.iter().filter_map(|touch| button_at(touch.position(), screen)).collect()
}

fn read_touches(
    touches: Res<Touches>,
    time: Res<Time>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut helm: ResMut<Helm>,
) {
    let Ok(window) = window.single() else { return };
    let screen = Vec2::new(window.width(), window.height());
    let held = held_buttons(&touches, screen);
    let dt = time.delta_secs();

    if held.contains(&TouchButton::Ahead) {
        helm.throttle = (helm.throttle + THROTTLE_LEVER_SPEED * dt).min(1.0);
    }
    if held.contains(&TouchButton::Astern) {
        helm.throttle = (helm.throttle - THROTTLE_LEVER_SPEED * dt).max(-1.0);
    }
    match (held.contains(&TouchButton::Port), held.contains(&TouchButton::Starboard)) {
        (true, false) => helm.rudder = 1.0,
        (false, true) => helm.rudder = -1.0,
        _ => {} // leave whatever the keyboard decided
    }

    // One-shot actions happen when a finger first lands on the button.
    for touch in touches.iter_just_pressed() {
        match button_at(touch.position(), screen) {
            Some(TouchButton::Fire) => helm.fire_requested = true,
            Some(TouchButton::Ping) => helm.ping_requested = true,
            Some(TouchButton::Stop) => helm.throttle = 0.0,
            _ => {}
        }
    }
}

fn highlight_held(
    touches: Res<Touches>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut buttons: Query<(&TouchButton, &mut BackgroundColor)>,
) {
    let Ok(window) = window.single() else { return };
    let held = held_buttons(&touches, Vec2::new(window.width(), window.height()));
    for (button, mut background) in &mut buttons {
        background.0 = if held.contains(button) { HELD } else { IDLE };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PHONE: Vec2 = Vec2::new(390.0, 844.0);

    #[test]
    fn touches_find_the_right_button() {
        // Bottom-left corner: port rudder. Bottom-right corner: astern.
        assert_eq!(button_at(Vec2::new(30.0, 800.0), PHONE), Some(TouchButton::Port));
        assert_eq!(button_at(Vec2::new(370.0, 800.0), PHONE), Some(TouchButton::Astern));
        // FIRE spans both left columns on the second row.
        assert_eq!(button_at(Vec2::new(130.0, 740.0), PHONE), Some(TouchButton::Fire));
        // The middle of the screen (the scope) is not a button.
        assert_eq!(button_at(Vec2::new(195.0, 422.0), PHONE), None);
    }

    #[test]
    fn buttons_never_overlap_on_a_small_phone() {
        let small = Vec2::new(320.0, 568.0);
        for (i, a) in BUTTONS.iter().enumerate() {
            for b in &BUTTONS[i + 1..] {
                let overlap = a.rect(small).intersect(b.rect(small));
                assert!(overlap.is_empty(), "{:?} overlaps {:?}", a.button, b.button);
            }
        }
    }
}
