//! Paints the whole panel from the game state, every frame.
//!
//! Each instrument is one function. They all follow the same recipe: draw the
//! body (bezel, face or frame), the fixed markings (ticks, numbers, labels),
//! then the moving part (needle, lever handle, lamp).

use std::f32::consts::{FRAC_PI_2, PI};

use subhunt_application::{ClientSession, ConnectionStatus, Notice};
use subhunt_domain::{Contact, ContactKind, GameRules, Vec2, compass_degrees};

use super::canvas::{Canvas, IRect, Point, Rgb, polar};
use super::controls::PanelInput;
use super::font;
use super::layout::{Dial, Layout};

/// A small, fixed palette keeps the retro look consistent.
mod palette {
    use super::Rgb;

    pub const PANEL: Rgb = [84, 96, 104];
    pub const PANEL_LIGHT: Rgb = [122, 134, 142];
    pub const PANEL_DARK: Rgb = [50, 58, 64];
    pub const RIM: Rgb = [150, 156, 160];
    pub const RIM_DARK: Rgb = [34, 38, 42];
    pub const BEZEL: Rgb = [20, 22, 24];
    pub const FACE: Rgb = [14, 16, 18];
    pub const INK: Rgb = [232, 228, 212];
    pub const INK_DIM: Rgb = [128, 130, 124];
    pub const INK_BLUE: Rgb = [120, 176, 200];
    pub const INK_GREEN: Rgb = [130, 206, 150];
    pub const PLATE: Rgb = [12, 12, 14];
    pub const SCREW: Rgb = [168, 172, 176];
    pub const SCREW_SLOT: Rgb = [70, 74, 78];
    pub const SLOT: Rgb = [16, 18, 20];
    pub const HANDLE: Rgb = [196, 58, 42];
    pub const HANDLE_LIGHT: Rgb = [240, 124, 100];
    pub const RED: Rgb = [216, 50, 38];
    pub const RED_DIM: Rgb = [112, 32, 26];
    pub const ORANGE: Rgb = [226, 118, 42];
    pub const AMBER: Rgb = [242, 182, 52];
    pub const AMBER_DIM: Rgb = [128, 94, 30];
    pub const LAMP_GREEN: Rgb = [84, 222, 112];
    pub const LAMP_OFF: Rgb = [46, 36, 32];
    pub const CAP_RED: Rgb = [200, 44, 34];
    pub const CAP_BLUE: Rgb = [70, 112, 168];
    pub const PHOSPHOR: Rgb = [96, 255, 144];
    pub const PHOSPHOR_MID: Rgb = [44, 172, 88];
    pub const PHOSPHOR_DIM: Rgb = [22, 82, 44];
    pub const SCOPE_FACE: Rgb = [6, 20, 12];
    pub const YELLOW: Rgb = [255, 240, 112];
    pub const WHITE: Rgb = [252, 252, 252];
    pub const DISPLAY: Rgb = [10, 14, 10];
}
use palette::*;

/// How long warning lamps stay lit after something happens.
const LAMP_SECONDS: f32 = 4.0;
/// Gauges sweep 270°, starting bottom-left (like most real gauges).
const SCALE_START: f32 = 1.25 * PI;
const SCALE_SWEEP: f32 = 1.5 * PI;

/// Everything the panel shows, gathered in one place.
pub struct PanelState<'a> {
    pub session: &'a ClientSession,
    /// The local helm settings (the levers move the instant you touch them,
    /// without waiting for the server).
    pub throttle: f32,
    pub rudder: f32,
    pub input: PanelInput,
    /// Seconds since the game started, for animations.
    pub time: f32,
}

pub fn draw(canvas: &mut Canvas, layout: &Layout, state: &PanelState) {
    let session = state.session;
    let ship = session.picture().map(|picture| &picture.own_ship);
    let rules = GameRules::default();

    panel_background(canvas);
    screws(canvas, layout);
    name_plate(canvas, layout.plate, session);
    sonar(canvas, layout.sonar, session, state.time);

    speed_gauge(canvas, layout.speed, ship.map_or(0.0, |s| s.speed.abs()), rules.max_forward_speed);
    heading_gauge(canvas, layout.heading, ship.map(|s| s.heading));
    noise_gauge(canvas, layout.noise, ship.map_or(0.0, |s| s.noise));

    throttle_lever(canvas, layout, state.throttle, state.input.throttle.is_some());
    rudder_lever(canvas, layout, state.rudder, state.input.rudder.is_some());

    let torpedo_ready = ship.map_or(0.0, |s| 1.0 - s.torpedo_cooldown / rules.torpedo_cooldown);
    let ping_ready = ship.map_or(0.0, |s| 1.0 - s.ping_cooldown / rules.ping_cooldown);
    push_button(canvas, layout.fire, "FIRE", CAP_RED, torpedo_ready, state.input.fire_held);
    push_button(canvas, layout.ping, "PING", CAP_BLUE, ping_ready, state.input.ping_held);

    warning_lamps(canvas, layout.lamps, session, state.time);
    message_display(canvas, layout.message, session);
}

// ----- Panel furniture -----

/// Painted steel with a little grain, so it doesn't look like flat plastic.
fn panel_background(canvas: &mut Canvas) {
    for y in 0..canvas.height() {
        for x in 0..canvas.width() {
            let grain = (hash(x, y) % 7) as i16 - 3;
            canvas.set(x, y, PANEL.map(|channel| (channel as i16 + grain).clamp(0, 255) as u8));
        }
    }
    // A bevel round the edge of the panel.
    let edge = IRect::new(0, 0, canvas.width(), canvas.height());
    canvas.line((0, 0), (edge.right() - 1, 0), PANEL_LIGHT);
    canvas.line((0, 0), (0, edge.bottom() - 1), PANEL_LIGHT);
    canvas.line((0, edge.bottom() - 1), (edge.right() - 1, edge.bottom() - 1), PANEL_DARK);
    canvas.line((edge.right() - 1, 0), (edge.right() - 1, edge.bottom() - 1), PANEL_DARK);
}

/// A cheap, repeatable pseudo-random number for each pixel.
fn hash(x: i32, y: i32) -> u32 {
    let mut h = (x as u32).wrapping_mul(374_761_393) ^ (y as u32).wrapping_mul(668_265_263);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    h ^ (h >> 16)
}

fn screw(canvas: &mut Canvas, at: Point) {
    canvas.fill_circle(at, 2, SCREW);
    canvas.set(at.0 + 1, at.1 + 2, PANEL_DARK);
    canvas.line((at.0 - 1, at.1 + 1), (at.0 + 1, at.1 - 1), SCREW_SLOT);
}

fn screws(canvas: &mut Canvas, layout: &Layout) {
    let (w, h) = (canvas.width(), canvas.height());
    for at in [(4, 4), (w - 5, 4), (4, h - 5), (w - 5, h - 5)] {
        screw(canvas, at);
    }
    // Four round the sonar, like the screws holding an instrument in place.
    let reach = (layout.sonar.r + 10) as f32;
    for corner in [0.25, 0.75, 1.25, 1.75] {
        screw(canvas, polar(layout.sonar.centre(), corner * PI, reach));
    }
}

/// A black label plate with white lettering, centred on `centre_x`.
fn label_plate(canvas: &mut Canvas, centre_x: i32, y: i32, text: &str) {
    let width = font::text_width(text) + 4;
    let plate = IRect::new(centre_x - width / 2, y, width, 7);
    canvas.fill_rect(plate, PLATE);
    canvas.text(plate.x + 2, y + 1, text, INK);
}

/// A raised metal frame for the controls.
fn raised_frame(canvas: &mut Canvas, rect: IRect) {
    canvas.fill_rect(rect, PANEL_DARK);
    canvas.line((rect.x, rect.y), (rect.right() - 1, rect.y), PANEL_LIGHT);
    canvas.line((rect.x, rect.y), (rect.x, rect.bottom() - 1), PANEL_LIGHT);
}

fn name_plate(canvas: &mut Canvas, plate: IRect, session: &ClientSession) {
    canvas.fill_rect(plate, PLATE);
    let id = session.player_id().map_or("SUB -".to_string(), |id| id.to_string().to_uppercase());
    canvas.text(plate.x + 3, plate.y + 2, &id, INK);
    let kills = format!("KILLS {:03}", session.picture().map_or(0, |p| p.score));
    canvas.text(plate.right() - 3 - font::text_width(&kills), plate.y + 2, &kills, INK);
}

// ----- Round instruments -----

fn dial_body(canvas: &mut Canvas, dial: Dial, face: Rgb) {
    let centre = dial.centre();
    canvas.fill_circle(centre, dial.r + 3, RIM_DARK);
    canvas.fill_circle(centre, dial.r + 2, RIM);
    canvas.fill_circle(centre, dial.r + 1, BEZEL);
    canvas.fill_circle(centre, dial.r, face);
    // A glint on the chrome rim, top left.
    for step in 0..6 {
        let at = polar(centre, 0.6 * PI + step as f32 * 0.08, (dial.r + 2) as f32);
        canvas.set(at.0, at.1, WHITE);
    }
}

/// The angle on a 270° scale for a fraction (0 = start, 1 = end).
fn scale_angle(fraction: f32) -> f32 {
    SCALE_START - SCALE_SWEEP * fraction.clamp(0.0, 1.0)
}

fn tick(canvas: &mut Canvas, dial: Dial, angle: f32, length: i32, colour: Rgb) {
    let outer = (dial.r - 1) as f32;
    canvas.line(polar(dial.centre(), angle, outer), polar(dial.centre(), angle, outer - length as f32), colour);
}

/// Numbers around the inside of the dial.
fn dial_number(canvas: &mut Canvas, dial: Dial, angle: f32, text: &str, colour: Rgb) {
    let (x, y) = polar(dial.centre(), angle, (dial.r - 8) as f32);
    canvas.text_centred(x + 1, y - 2, text, colour);
}

fn needle(canvas: &mut Canvas, dial: Dial, angle: f32, colour: Rgb) {
    let tip = polar(dial.centre(), angle, (dial.r - 3) as f32);
    let tail = polar(dial.centre(), angle + PI, 3.0);
    canvas.line(tail, tip, colour);
    canvas.fill_circle(dial.centre(), 1, RIM);
}

fn speed_gauge(canvas: &mut Canvas, dial: Dial, speed: f32, top_speed: f32) {
    dial_body(canvas, dial, FACE);
    for step in 0..=6 {
        tick(canvas, dial, scale_angle(step as f32 / 6.0), if step % 2 == 0 { 3 } else { 2 }, INK);
    }
    // Single digits with a "×10" plate, like the "1000 REV" multiplier on old
    // tachometers: two-digit numbers don't fit on a dial this small.
    for (step, label) in ["0", "2", "4", "6"].iter().enumerate() {
        dial_number(canvas, dial, scale_angle(step as f32 / 3.0), label, INK);
    }
    needle(canvas, dial, scale_angle(speed / top_speed), INK);
    label_plate(canvas, dial.x, dial.y + dial.r + 5, "SPEED X10");
}

fn heading_gauge(canvas: &mut Canvas, dial: Dial, heading: Option<f32>) {
    dial_body(canvas, dial, FACE);
    for step in 0..12 {
        let angle = step as f32 * PI / 6.0;
        tick(canvas, dial, angle, if step % 3 == 0 { 3 } else { 2 }, INK_DIM);
    }
    let compass = [(FRAC_PI_2, "N", INK), (0.0, "E", INK_BLUE), (-FRAC_PI_2, "S", INK_BLUE), (PI, "W", INK_BLUE)];
    for (angle, letter, colour) in compass {
        dial_number(canvas, dial, angle, letter, colour);
    }
    if let Some(heading) = heading {
        needle(canvas, dial, heading, INK);
        let tip = polar(dial.centre(), heading, (dial.r - 3) as f32);
        canvas.set(tip.0, tip.1, ORANGE);
    }
    let readout = heading.map_or("---".to_string(), |h| format!("{:03.0}", compass_degrees(h)));
    label_plate(canvas, dial.x, dial.y + dial.r + 5, &readout);
}

fn noise_gauge(canvas: &mut Canvas, dial: Dial, noise: f32) {
    const MAX_NOISE: f32 = 5.0;
    dial_body(canvas, dial, FACE);
    // The danger zone: loud enough to be heard a long way off.
    for step in 0..=24 {
        let angle = scale_angle(0.6 + 0.4 * step as f32 / 24.0);
        for radius in [dial.r - 1, dial.r - 2] {
            let (x, y) = polar(dial.centre(), angle, radius as f32);
            canvas.set(x, y, ORANGE);
        }
    }
    for step in 0..=5 {
        let angle = scale_angle(step as f32 / 5.0);
        tick(canvas, dial, angle, 3, INK);
        dial_number(canvas, dial, angle, &step.to_string(), INK_GREEN);
    }
    needle(canvas, dial, scale_angle(noise / MAX_NOISE), INK);
    label_plate(canvas, dial.x, dial.y + dial.r + 5, "NOISE");
}

// ----- The sonar screen -----

fn sonar(canvas: &mut Canvas, dial: Dial, session: &ClientSession, time: f32) {
    let centre = dial.centre();
    let r = dial.r;
    canvas.fill_circle(centre, r + 4, RIM_DARK);
    canvas.fill_circle(centre, r + 3, RIM);
    canvas.fill_circle(centre, r + 1, BEZEL);
    canvas.fill_circle(centre, r, SCOPE_FACE);
    // Only draw inside the glass.
    let inside = |x: i32, y: i32| (x - centre.0).pow(2) + (y - centre.1).pow(2) < (r - 1).pow(2);

    // Range rings, cross hairs and a compass scale round the edge.
    canvas.dotted_circle(centre, r / 3, PHOSPHOR_DIM);
    canvas.dotted_circle(centre, 2 * r / 3, PHOSPHOR_DIM);
    for step in 0..36 {
        let angle = step as f32 * PI / 18.0;
        let length = if step % 9 == 0 {
            5.0
        } else if step % 3 == 0 {
            3.0
        } else {
            1.0
        };
        let outer = (r - 1) as f32;
        canvas.line(polar(centre, angle, outer), polar(centre, angle, outer - length), PHOSPHOR_MID);
    }
    for (angle, letter) in [(FRAC_PI_2, "N"), (0.0, "E"), (-FRAC_PI_2, "S"), (PI, "W")] {
        let (x, y) = polar(centre, angle, (r - 11) as f32);
        canvas.text_centred(x + 1, y - 2, letter, PHOSPHOR_MID);
    }

    // The rotating sweep with a fading phosphor trail.
    let sweep = -time * 1.2;
    for (i, colour) in [PHOSPHOR, PHOSPHOR_MID, PHOSPHOR_MID, PHOSPHOR_DIM, PHOSPHOR_DIM].iter().enumerate() {
        canvas.line_where(centre, polar(centre, sweep + i as f32 * 0.04, (r - 2) as f32), *colour, inside);
    }

    let Some(picture) = session.picture().filter(|_| session.status() == ConnectionStatus::Playing) else {
        let text = if session.status() == ConnectionStatus::Lost { "NO SIGNAL" } else { "TUNING..." };
        canvas.fill_rect(IRect::new(centre.0 - 20, centre.1 + 12, 41, 9), SCOPE_FACE);
        canvas.text_centred(centre.0 + 1, centre.1 + 14, text, PHOSPHOR);
        return;
    };

    // Metres in the world → pixels on the screen, centred on our own boat.
    let scale = r as f32 / session.display_range();
    let me = picture.own_ship.position;
    let to_screen = |p: Vec2| -> Point {
        (centre.0 + ((p.x - me.x) * scale).round() as i32, centre.1 - ((p.y - me.y) * scale).round() as i32)
    };

    // The edge of the play area.
    let edge = session.world_half_extent();
    let corners = [Vec2::new(-edge, -edge), Vec2::new(edge, -edge), Vec2::new(edge, edge), Vec2::new(-edge, edge)];
    for i in 0..4 {
        canvas.line_where(to_screen(corners[i]), to_screen(corners[(i + 1) % 4]), RED_DIM, inside);
    }

    // Passive sonar: lines towards anything we can hear.
    for contact in &picture.contacts {
        if let Contact::Bearing { bearing, kind, strength } = *contact {
            let colour = match (kind, strength > 0.5) {
                (ContactKind::Submarine, true) => AMBER,
                (ContactKind::Submarine, false) => AMBER_DIM,
                (ContactKind::Torpedo, true) => RED,
                (ContactKind::Torpedo, false) => RED_DIM,
            };
            canvas.line_where(polar(centre, bearing, 5.0), polar(centre, bearing, (r - 2) as f32), colour, inside);
        }
    }

    // Ping echoes: exact positions, fading out.
    for echo in session.echoes() {
        let at = to_screen(echo.position);
        if !inside(at.0, at.1) {
            continue;
        }
        let colour = match (echo.kind, echo.intensity()) {
            (ContactKind::Torpedo, _) => RED,
            (_, i) if i > 0.66 => PHOSPHOR,
            (_, i) if i > 0.33 => PHOSPHOR_MID,
            _ => PHOSPHOR_DIM,
        };
        canvas.circle(at, 2, colour);
        canvas.set(at.0, at.1, colour);
    }

    for torpedo in &picture.own_torpedoes {
        let at = to_screen(torpedo.position);
        if inside(at.0, at.1) {
            canvas.fill_rect(IRect::new(at.0, at.1, 2, 2), YELLOW);
        }
    }

    // Our own boat: a little arrowhead.
    let heading = picture.own_ship.heading;
    let nose = polar(centre, heading, 6.0);
    let port = polar(centre, heading + 2.4, 4.0);
    let starboard = polar(centre, heading - 2.4, 4.0);
    canvas.line(nose, port, WHITE);
    canvas.line(nose, starboard, WHITE);
    canvas.line(port, starboard, WHITE);
}

// ----- Levers -----

fn lever_handle(canvas: &mut Canvas, rect: IRect, held: bool) {
    canvas.fill_rect(rect, if held { HANDLE_LIGHT } else { HANDLE });
    canvas.line((rect.x, rect.y), (rect.right() - 1, rect.y), HANDLE_LIGHT);
    canvas.line((rect.x, rect.bottom()), (rect.right() - 1, rect.bottom()), RIM_DARK);
}

fn throttle_lever(canvas: &mut Canvas, layout: &Layout, setting: f32, held: bool) {
    let area = layout.throttle;
    raised_frame(canvas, area);
    label_plate(canvas, area.x + area.w / 2, area.y + 2, "THROTTLE");

    let (top, bottom) = layout.throttle_track();
    let slot_x = area.right() - 9;
    canvas.fill_rect(IRect::new(slot_x - 1, top - 2, 3, bottom - top + 5), SLOT);
    for (value, label) in [(1.0, "FULL"), (0.5, "HALF"), (0.0, "STOP"), (-0.5, ""), (-1.0, "ASTN")] {
        let y = layout.throttle_y(value);
        canvas.line((area.x + 18, y), (slot_x - 3, y), INK_DIM);
        canvas.text(area.x + 2, y - 2, label, if value == 0.0 { AMBER } else { INK });
    }
    let y = layout.throttle_y(setting);
    lever_handle(canvas, IRect::new(slot_x - 6, y - 2, 13, 5), held);
}

fn rudder_lever(canvas: &mut Canvas, layout: &Layout, setting: f32, held: bool) {
    let area = layout.rudder;
    raised_frame(canvas, area);
    label_plate(canvas, area.x + area.w / 2, area.y + 2, "RUDDER");
    canvas.text(area.x + 3, area.y + 3, "P", RED);
    canvas.text(area.right() - 6, area.y + 3, "S", LAMP_GREEN);

    let (left, right, y) = layout.rudder_track();
    canvas.fill_rect(IRect::new(left - 2, y - 1, right - left + 5, 3), SLOT);
    let middle = layout.rudder_x(0.0);
    canvas.line((middle, y - 4), (middle, y - 3), INK_DIM);
    let x = layout.rudder_x(setting);
    lever_handle(canvas, IRect::new(x - 2, y - 3, 5, 6), held);
}

// ----- Buttons, lamps and the message display -----

/// A round push button with a "ready" lamp and a reload bar.
fn push_button(canvas: &mut Canvas, area: IRect, label: &str, cap: Rgb, ready: f32, pressed: bool) {
    raised_frame(canvas, area);
    let radius = ((area.h - 8) / 2).min(9);
    let centre_x = area.x + 4 + radius;
    let centre_y = area.y + area.h / 2;
    canvas.fill_circle((centre_x, centre_y + 1), radius + 1, RIM_DARK);
    let cap_colour = if pressed { cap.map(|c| c / 2) } else { cap };
    let cap_centre = (centre_x, centre_y + i32::from(pressed));
    canvas.fill_circle(cap_centre, radius, cap_colour);
    if !pressed {
        // A highlight to make it look domed.
        let shine = cap.map(|c| c.saturating_add(70));
        canvas.line((centre_x - radius / 2, centre_y - radius / 2), (centre_x - 1, centre_y - radius + 2), shine);
    }

    let text_x = centre_x + radius + 4;
    canvas.text(text_x, area.y + 5, label, INK);

    let is_ready = ready >= 1.0;
    let lamp = if is_ready { LAMP_GREEN } else { AMBER };
    canvas.fill_circle((area.right() - 5, area.bottom() - 7), 2, lamp);
    let bar = IRect::new(text_x, area.bottom() - 8, area.right() - 10 - text_x, 3);
    canvas.fill_rect(bar, SLOT);
    let filled = (bar.w as f32 * ready.clamp(0.0, 1.0)).round() as i32;
    canvas.fill_rect(IRect::new(bar.x, bar.y, filled, bar.h), lamp);
}

fn warning_lamps(canvas: &mut Canvas, area: IRect, session: &ClientSession, time: f32) {
    let recent = |wanted: fn(&Notice) -> bool| session.notices().any(|n| wanted(&n.notice) && n.age < LAMP_SECONDS);
    let flashing = (time * 6.0) as i32 % 2 == 0;
    let heard = session
        .picture()
        .is_some_and(|picture| picture.contacts.iter().any(|contact| matches!(contact, Contact::Bearing { .. })));
    let lamps = [
        ("PINGED", RED, recent(|n| matches!(n, Notice::Pinged { .. })) && flashing),
        ("HEARD", AMBER, heard),
        ("HIT", LAMP_GREEN, recent(|n| matches!(n, Notice::Kill { .. }))),
        ("SUNK", RED, recent(|n| matches!(n, Notice::Sunk { .. }))),
    ];

    raised_frame(canvas, area);
    let columns = if area.w >= 120 { 4 } else { 2 };
    let rows = lamps.len() as i32 / columns;
    let (cell_w, cell_h) = ((area.w - 2) / columns, (area.h - 2) / rows);
    for (i, (label, colour, lit)) in lamps.iter().enumerate() {
        let i = i as i32;
        let cell = IRect::new(area.x + 1 + (i % columns) * cell_w, area.y + 1 + (i / columns) * cell_h, cell_w, cell_h);
        let glass = cell.inset(1);
        canvas.fill_rect(glass, if *lit { *colour } else { LAMP_OFF });
        let text_colour = if *lit { PLATE } else { INK_DIM };
        canvas.text_centred(glass.x + glass.w / 2, glass.y + (glass.h - font::GLYPH_HEIGHT) / 2, label, text_colour);
    }
}

fn message_display(canvas: &mut Canvas, area: IRect, session: &ClientSession) {
    canvas.fill_rect(area, RIM_DARK);
    let screen = area.inset(1);
    canvas.fill_rect(screen, DISPLAY);
    let columns = ((screen.w - 3) / font::ADVANCE) as usize;
    let fit = |text: &str| text.chars().take(columns).collect::<String>();

    let status = match session.status() {
        ConnectionStatus::AwaitingWelcome => "CONNECTING...".to_string(),
        ConnectionStatus::Lost => "CONNECTION LOST".to_string(),
        ConnectionStatus::Playing => session.banner().unwrap_or("ONLINE").to_uppercase(),
    };
    canvas.text(screen.x + 2, screen.y + 2, &fit(&status), AMBER_DIM);
    if let Some(latest) = session.notices().next() {
        canvas.text(screen.x + 2, screen.y + 9, &fit(&short_notice(latest.notice)), AMBER);
    }
}

/// Notices, short enough for the little display.
fn short_notice(notice: Notice) -> String {
    let text = match notice {
        Notice::Sunk { by } => format!("SUNK BY {by}"),
        Notice::Kill { victim } => format!("HIT! {victim} DOWN"),
        Notice::Pinged { bearing } => format!("PINGED FROM {:03.0}", compass_degrees(bearing)),
        Notice::PlayerJoined(id) => format!("{id} JOINED"),
        Notice::PlayerLeft(id) => format!("{id} LEFT"),
    };
    text.to_uppercase()
}

#[cfg(test)]
mod tests {
    use subhunt_application::{ClientInbound, ServerMessage};
    use subhunt_domain::{OwnShipStatus, PlayerId, SensorPicture};

    use super::*;

    fn playing_session() -> ClientSession {
        let mut session = ClientSession::new();
        session.handle(ClientInbound::Message(ServerMessage::Welcome {
            player_id: PlayerId(3),
            world_half_extent: 1200.0,
            ping_range: 1500.0,
        }));
        session.handle(ClientInbound::Message(ServerMessage::Picture(SensorPicture {
            tick: 1,
            own_ship: OwnShipStatus {
                position: Vec2::new(100.0, 50.0),
                heading: 0.5,
                speed: 30.0,
                throttle: 0.5,
                noise: 0.7,
                torpedo_cooldown: 2.0,
                ping_cooldown: 0.0,
            },
            own_torpedoes: vec![],
            contacts: vec![
                Contact::Bearing { bearing: 1.0, kind: ContactKind::Submarine, strength: 0.8 },
                Contact::Fix { position: Vec2::new(300.0, 300.0), kind: ContactKind::Submarine },
            ],
            score: 2,
        })));
        session.handle(ClientInbound::Message(ServerMessage::Notice(Notice::Pinged { bearing: 1.0 })));
        session
    }

    #[test]
    fn panels_draw_in_both_orientations() {
        let session = playing_session();
        for layout in [Layout::landscape(), Layout::portrait()] {
            let mut canvas = Canvas::new(layout.width, layout.height);
            let state =
                PanelState { session: &session, throttle: 0.5, rudder: -1.0, input: Default::default(), time: 1.0 };
            draw(&mut canvas, &layout, &state);
            // The middle of the sonar screen shows our own boat in white.
            let centre = layout.sonar.centre();
            let around_centre = (-4..=4).flat_map(|dx| (-4..=4).map(move |dy| (dx, dy)));
            assert!(around_centre.into_iter().any(|(dx, dy)| canvas.get(centre.0 + dx, centre.1 + dy) == Some(WHITE)));
        }
    }

    #[test]
    fn notices_fit_the_message_display() {
        let narrowest = Layout::portrait().message.w.min(Layout::landscape().message.w);
        let columns = ((narrowest - 5) / font::ADVANCE) as usize;
        for notice in [
            Notice::Sunk { by: PlayerId(12) },
            Notice::Kill { victim: PlayerId(12) },
            Notice::Pinged { bearing: 1.0 },
            Notice::PlayerJoined(PlayerId(12)),
        ] {
            let text = short_notice(notice);
            assert!(text.len() <= columns, "{text:?} is too long");
        }
    }
}
