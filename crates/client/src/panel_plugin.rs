//! Bevy adapter for the instrument panel.
//!
//! Every frame we:
//! 1. pick a layout for the window's shape and work out how big to draw it,
//! 2. turn mouse and touch positions into panel pixels and read the controls,
//! 3. redraw the panel into its pixel buffer and hand that to the GPU as a texture.
//!
//! The texture is shown as a single UI image, scaled up with "nearest"
//! sampling, which keeps every pixel a crisp square instead of blurring it.
//!
//! Why a UI image and not a sprite? Bevy's sprite renderer drew the panel as a
//! single flat colour on an Android phone's Adreno graphics chip, while Bevy's
//! UI renderer (which the earlier, text-based version used) works there.

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

/// Marks the UI node that shows the panel.
#[derive(Component)]
struct PanelImage;

/// Some phones' graphics drivers crash (losing the whole WebGL context) when
/// a texture's rows aren't a multiple of 256 bytes. At 4 bytes per pixel
/// that's 64 pixels, so the texture is padded out to a multiple of 64 wide
/// and the UI image only shows the panel part of it.
const ROW_ALIGNMENT_PIXELS: i32 = 64;
/// Redrawing faster than this gains nothing and costs battery.
const REDRAWS_PER_SECOND: f32 = 30.0;

fn texture_width(layout: &Layout) -> i32 {
    (layout.width + ROW_ALIGNMENT_PIXELS - 1) / ROW_ALIGNMENT_PIXELS * ROW_ALIGNMENT_PIXELS
}

/// The part of the (padded) texture that holds the panel.
fn visible_part(layout: &Layout) -> Rect {
    Rect::new(0.0, 0.0, layout.width as f32, layout.height as f32)
}

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
    let size = Extent3d { width: texture_width(layout) as u32, height: layout.height as u32, depth_or_array_layers: 1 };
    let mut image = Image::new_fill(
        size,
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    // Use the app-wide default sampler (set to "nearest" in main.rs, so pixels
    // stay crisp squares). Don't give this image its own sampler: Bevy creates
    // a fresh GPU sampler every time an image with a custom one changes, and
    // this image changes every frame. That many samplers crashed Chrome on an
    // Android phone (most phone drivers allow only a few thousand).
    image.sampler = ImageSampler::Default;
    image
}

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    commands.spawn(Camera2d);
    let layout = Layout::landscape();
    let image = images.add(blank_image(&layout));
    // A full-window box that centres the panel, with the panel image inside it.
    commands
        .spawn(Node {
            width: percent(100),
            height: percent(100),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_child((
            ImageNode { image: image.clone(), rect: Some(visible_part(&layout)), ..default() },
            Node { width: px(layout.width), height: px(layout.height), ..default() },
            PanelImage,
        ));
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
    mut shown: Query<(&mut ImageNode, &mut Node), With<PanelImage>>,
) {
    let (Ok(window), Ok((mut image_node, mut node))) = (window.single(), shown.single_mut()) else { return };
    let window_size = Vec2::new(window.width(), window.height());
    if window_size.min_element() < 1.0 {
        return; // minimised
    }

    // Switch layouts when the phone is turned round.
    let layout = Layout::for_window(window_size.x, window_size.y);
    if layout.orientation != panel.layout.orientation {
        panel.canvas = Canvas::new(layout.width, layout.height);
        panel.image = images.add(blank_image(&layout));
        image_node.image = panel.image.clone();
        image_node.rect = Some(visible_part(&layout));
        panel.layout = layout;
    }

    // As big as fits. Once there's room for 3× or more, stick to whole numbers
    // so every panel pixel is the same size on screen.
    let panel_size = Vec2::new(panel.layout.width as f32, panel.layout.height as f32);
    let fit = (window_size / panel_size).min_element();
    let scale = if fit >= 3.0 { fit.floor() } else { fit };
    let shown_size = panel_size * scale;
    // Only touch the layout when the size really changes, so Bevy doesn't
    // redo the UI layout every frame.
    if node.width != px(shown_size.x) || node.height != px(shown_size.y) {
        node.width = px(shown_size.x);
        node.height = px(shown_size.y);
    }
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
    mut last_drawn: Local<Option<f32>>,
) {
    // Rust note: `Local<T>` is a value that belongs to this one system and is
    // kept between frames, a bit like a `static` variable in a C function.
    let now = time.elapsed_secs();
    if last_drawn.is_some_and(|last| now - last < 1.0 / REDRAWS_PER_SECOND) {
        return;
    }
    *last_drawn = Some(now);

    // Rust note: `&mut *panel` turns Bevy's `ResMut<Panel>` into a plain
    // `&mut Panel`, so we can borrow its fields separately below.
    let panel = &mut *panel;
    let state =
        PanelState { session: &session.0, throttle: helm.throttle, rudder: helm.rudder, input: panel.input, time: now };
    draw::draw(&mut panel.canvas, &panel.layout, &state);
    if let Some(mut image) = images.get_mut(&panel.image) {
        copy_into_padded_rows(panel.canvas.bytes(), &panel.layout, image.data.get_or_insert_with(Vec::new));
    }
}

/// Copies the panel's pixels into the texture, row by row, leaving the
/// padding at the end of each row alone.
fn copy_into_padded_rows(pixels: &[u8], layout: &Layout, texture: &mut Vec<u8>) {
    let row = layout.width as usize * 4;
    let padded_row = texture_width(layout) as usize * 4;
    texture.resize(padded_row * layout.height as usize, 255);
    // Rust note: `chunks_exact` splits a slice into equal pieces, and `zip`
    // walks two iterators side by side.
    for (source, destination) in pixels.chunks_exact(row).zip(texture.chunks_exact_mut(padded_row)) {
        destination[..row].copy_from_slice(source);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texture_rows_are_a_multiple_of_256_bytes() {
        for layout in [Layout::landscape(), Layout::portrait()] {
            let row_bytes = texture_width(&layout) * 4;
            assert_eq!(row_bytes % 256, 0, "{:?}", layout.orientation);
            assert!(texture_width(&layout) >= layout.width);
        }
    }

    #[test]
    fn padding_keeps_each_row_in_place() {
        let layout = Layout::portrait(); // 180 wide, padded to 192
        let pixels: Vec<u8> = (0..layout.width * layout.height * 4).map(|i| (i % 251) as u8).collect();
        let mut texture = Vec::new();
        copy_into_padded_rows(&pixels, &layout, &mut texture);
        let (row, padded) = (180 * 4, 192 * 4);
        assert_eq!(texture.len(), padded * 340);
        assert_eq!(&texture[padded * 7..padded * 7 + row], &pixels[row * 7..row * 8]);
    }
}
