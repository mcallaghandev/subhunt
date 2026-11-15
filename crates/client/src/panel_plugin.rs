//! Bevy adapter for the instrument panel.
//!
//! Every frame we:
//! 1. pick a layout for the window's shape and work out how big to draw it,
//! 2. turn mouse and touch positions into panel pixels and read the controls,
//! 3. redraw the panel into its pixel buffer and hand that to the GPU as a texture.
//!
//! The texture is shown on one sprite and scaled up with "nearest" sampling,
//! which keeps every pixel a crisp square instead of blurring it.

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::window::PrimaryWindow;

use crate::helm::{Helm, read_keyboard};
use crate::panel::canvas::Canvas;
use crate::panel::controls::{self, PanelInput, Pointer};
use crate::panel::draw::{self, PanelState};
use crate::panel::layout::Layout;
use crate::session::Session;

#[derive(Resource)]
struct Panel {
    layout: Layout,
    canvas: Canvas,
    image: Handle<Image>,
    /// Screen pixels per panel pixel.
    scale: f32,
    /// Where the panel's top-left corner is in the window.
    offset: Vec2,
    input: PanelInput,
}

#[derive(Component)]
struct PanelSprite;

pub struct PanelPlugin;

impl Plugin for PanelPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup).add_systems(
            Update,
            // `.chain()` runs these in order; reading controls waits for the
            // keyboard so a lever being dragged wins over "no key pressed".
            (fit_to_window, read_controls.after(read_keyboard), draw_panel).chain(),
        );
    }
}

fn blank_image(layout: &Layout) -> Image {
    let size = Extent3d { width: layout.width as u32, height: layout.height as u32, depth_or_array_layers: 1 };
    let mut image = Image::new_fill(
        size,
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.sampler = ImageSampler::nearest(); // big square pixels, no blurring
    image
}

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    commands.spawn(Camera2d);
    let layout = Layout::landscape();
    let image = images.add(blank_image(&layout));
    commands.spawn((Sprite { image: image.clone(), ..default() }, PanelSprite));
    commands.insert_resource(Panel {
        canvas: Canvas::new(layout.width, layout.height),
        layout,
        image,
        scale: 1.0,
        offset: Vec2::ZERO,
        input: PanelInput::default(),
    });
}

fn fit_to_window(
    window: Query<&Window, With<PrimaryWindow>>,
    mut panel: ResMut<Panel>,
    mut images: ResMut<Assets<Image>>,
    mut sprite: Query<&mut Sprite, With<PanelSprite>>,
) {
    let (Ok(window), Ok(mut sprite)) = (window.single(), sprite.single_mut()) else { return };
    let window_size = Vec2::new(window.width(), window.height());
    if window_size.min_element() < 1.0 {
        return; // minimised
    }

    // Switch layouts when the phone is turned round.
    let layout = Layout::for_window(window_size.x, window_size.y);
    if layout.orientation != panel.layout.orientation {
        panel.canvas = Canvas::new(layout.width, layout.height);
        panel.image = images.add(blank_image(&layout));
        sprite.image = panel.image.clone();
        panel.layout = layout;
    }

    // As big as fits. Once there's room for 3× or more, stick to whole numbers
    // so every panel pixel is the same size on screen.
    let panel_size = Vec2::new(panel.layout.width as f32, panel.layout.height as f32);
    let fit = (window_size / panel_size).min_element();
    let scale = if fit >= 3.0 { fit.floor() } else { fit };
    sprite.custom_size = Some(panel_size * scale);
    panel.scale = scale;
    panel.offset = (window_size - panel_size * scale) / 2.0;
}

fn read_controls(
    mouse: Res<ButtonInput<MouseButton>>,
    touches: Res<Touches>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut panel: ResMut<Panel>,
    mut helm: ResMut<Helm>,
) {
    let Ok(window) = window.single() else { return };
    let (offset, scale) = (panel.offset, panel.scale);
    let to_panel = |screen: Vec2, just_pressed: bool| {
        let at = (screen - offset) / scale;
        Pointer { x: at.x, y: at.y, just_pressed }
    };

    // The mouse and every finger on the screen are all just "pointers".
    // A quick click or tap can go down *and* up within one frame, so we count
    // anything that went down this frame even if it's already been released.
    let mut pointers = Vec::new();
    let clicked = mouse.just_pressed(MouseButton::Left);
    if (mouse.pressed(MouseButton::Left) || clicked)
        && let Some(cursor) = window.cursor_position()
    {
        pointers.push(to_panel(cursor, clicked));
    }
    for touch in touches.iter() {
        pointers.push(to_panel(touch.position(), touches.just_pressed(touch.id())));
    }
    for tap in touches.iter_just_released().filter(|touch| touches.just_pressed(touch.id())) {
        pointers.push(to_panel(tap.position(), true));
    }

    let input = controls::read(&panel.layout, &pointers);
    if let Some(throttle) = input.throttle {
        helm.throttle = throttle;
    }
    if let Some(rudder) = input.rudder {
        helm.rudder = rudder;
    }
    helm.fire_requested |= input.fire;
    helm.ping_requested |= input.ping;
    panel.input = input;
}

fn draw_panel(
    mut panel: ResMut<Panel>,
    session: Res<Session>,
    helm: Res<Helm>,
    time: Res<Time>,
    mut images: ResMut<Assets<Image>>,
) {
    // Rust note: `&mut *panel` turns Bevy's `ResMut<Panel>` into a plain
    // `&mut Panel`, so we can borrow its fields separately below.
    let panel = &mut *panel;
    let state = PanelState {
        session: &session.0,
        throttle: helm.throttle,
        rudder: helm.rudder,
        input: panel.input,
        time: time.elapsed_secs(),
    };
    draw::draw(&mut panel.canvas, &panel.layout, &state);
    if let Some(mut image) = images.get_mut(&panel.image) {
        image.data = Some(panel.canvas.bytes().to_vec());
    }
}
