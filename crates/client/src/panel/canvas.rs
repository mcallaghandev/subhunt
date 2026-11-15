//! A tiny software framebuffer: a grid of pixels in a `Vec<u8>`, plus the
//! classic drawing algorithms (Bresenham lines, midpoint circles).
//!
//! Rust note: the pixels are stored as one flat `Vec<u8>`, four bytes (red,
//! green, blue, alpha) per pixel, row after row. Pixel (x, y) starts at index
//! `(y * width + x) * 4`. That's exactly the layout the GPU texture wants, so
//! the whole buffer can be handed to Bevy with no conversion.

use super::font;

/// A colour as red, green, blue (0–255 each).
pub type Rgb = [u8; 3];

/// A pixel position.
pub type Point = (i32, i32);

/// A rectangle in pixels. `x`/`y` is the top-left corner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl IRect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }

    pub fn right(&self) -> i32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> i32 {
        self.y + self.h
    }

    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x as f32 && x < self.right() as f32 && y >= self.y as f32 && y < self.bottom() as f32
    }

    #[cfg(test)] // only the tests need this
    pub fn overlaps(&self, other: &IRect) -> bool {
        self.x < other.right() && other.x < self.right() && self.y < other.bottom() && other.y < self.bottom()
    }

    pub fn inset(&self, by: i32) -> Self {
        Self::new(self.x + by, self.y + by, self.w - 2 * by, self.h - 2 * by)
    }
}

/// The point `length` pixels from `centre` in direction `angle`.
///
/// Angles are the game's: radians, counter-clockwise from east. Screen rows
/// count *downwards*, so we subtract the y part.
pub fn polar(centre: Point, angle: f32, length: f32) -> Point {
    (centre.0 + (angle.cos() * length).round() as i32, centre.1 - (angle.sin() * length).round() as i32)
}

pub struct Canvas {
    width: i32,
    height: i32,
    pixels: Vec<u8>,
}

impl Canvas {
    pub fn new(width: i32, height: i32) -> Self {
        Self { width, height, pixels: vec![255; (width * height * 4) as usize] }
    }

    pub fn width(&self) -> i32 {
        self.width
    }

    pub fn height(&self) -> i32 {
        self.height
    }

    /// The raw RGBA bytes, ready to upload as a texture.
    pub fn bytes(&self) -> &[u8] {
        &self.pixels
    }

    /// Where pixel (x, y) starts in `pixels`, or `None` if it's off the canvas.
    fn index(&self, x: i32, y: i32) -> Option<usize> {
        (x >= 0 && y >= 0 && x < self.width && y < self.height).then(|| ((y * self.width + x) * 4) as usize)
    }

    #[cfg(test)] // only the tests need this
    pub fn get(&self, x: i32, y: i32) -> Option<Rgb> {
        self.index(x, y).map(|i| [self.pixels[i], self.pixels[i + 1], self.pixels[i + 2]])
    }

    /// Sets one pixel. Anything off the canvas is quietly ignored, which keeps
    /// all the drawing code below free of bounds checks.
    pub fn set(&mut self, x: i32, y: i32, colour: Rgb) {
        if let Some(i) = self.index(x, y) {
            self.pixels[i..i + 3].copy_from_slice(&colour);
            self.pixels[i + 3] = 255;
        }
    }

    pub fn fill_rect(&mut self, rect: IRect, colour: Rgb) {
        for y in rect.y..rect.bottom() {
            for x in rect.x..rect.right() {
                self.set(x, y, colour);
            }
        }
    }

    pub fn line(&mut self, from: Point, to: Point, colour: Rgb) {
        self.line_where(from, to, colour, |_, _| true);
    }

    /// Bresenham's line algorithm, drawing only the pixels `keep` approves of
    /// (we use that to stop lines at the edge of the round sonar screen).
    ///
    /// Rust note: `impl Fn(i32, i32) -> bool` accepts any function or closure
    /// with that shape.
    pub fn line_where(&mut self, from: Point, to: Point, colour: Rgb, keep: impl Fn(i32, i32) -> bool) {
        let (x1, y1) = to;
        let (mut x, mut y) = from;
        let dx = (x1 - x).abs();
        let dy = -(y1 - y).abs();
        let step_x = if x < x1 { 1 } else { -1 };
        let step_y = if y < y1 { 1 } else { -1 };
        let mut error = dx + dy;
        loop {
            if keep(x, y) {
                self.set(x, y, colour);
            }
            if x == x1 && y == y1 {
                break;
            }
            let doubled = 2 * error;
            if doubled >= dy {
                error += dy;
                x += step_x;
            }
            if doubled <= dx {
                error += dx;
                y += step_y;
            }
        }
    }

    /// The midpoint circle algorithm: works out one eighth of the circle and
    /// mirrors it into the other seven.
    pub fn circle(&mut self, centre: Point, radius: i32, colour: Rgb) {
        let (cx, cy) = centre;
        let (mut x, mut y, mut error) = (radius, 0, 1 - radius);
        while x >= y {
            for (px, py) in [(x, y), (y, x), (-y, x), (-x, y), (-x, -y), (-y, -x), (y, -x), (x, -y)] {
                self.set(cx + px, cy + py, colour);
            }
            y += 1;
            if error < 0 {
                error += 2 * y + 1;
            } else {
                x -= 1;
                error += 2 * (y - x) + 1;
            }
        }
    }

    /// A circle drawn with every other pixel, for faint range rings.
    pub fn dotted_circle(&mut self, centre: Point, radius: i32, colour: Rgb) {
        let steps = (std::f32::consts::TAU * radius as f32) as i32;
        for i in (0..steps).step_by(2) {
            let (x, y) = polar(centre, i as f32 / steps as f32 * std::f32::consts::TAU, radius as f32);
            self.set(x, y, colour);
        }
    }

    pub fn fill_circle(&mut self, centre: Point, radius: i32, colour: Rgb) {
        let limit = (radius as f32 + 0.5).powi(2);
        for dy in -radius..=radius {
            let half = (limit - (dy * dy) as f32).max(0.0).sqrt() as i32;
            for dx in -half..=half {
                self.set(centre.0 + dx, centre.1 + dy, colour);
            }
        }
    }

    /// Draws text with its top-left corner at (x, y).
    pub fn text(&mut self, x: i32, y: i32, text: &str, colour: Rgb) {
        for (i, character) in text.chars().enumerate() {
            let left = x + i as i32 * font::ADVANCE;
            for (row, bits) in font::glyph(character).iter().enumerate() {
                for column in 0..font::GLYPH_WIDTH {
                    if bits & (1 << (font::GLYPH_WIDTH - 1 - column)) != 0 {
                        self.set(left + column, y + row as i32, colour);
                    }
                }
            }
        }
    }

    /// Draws text centred horizontally on `centre_x`.
    pub fn text_centred(&mut self, centre_x: i32, y: i32, text: &str, colour: Rgb) {
        self.text(centre_x - font::text_width(text) / 2, y, text, colour);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: Rgb = [255, 0, 0];

    #[test]
    fn drawing_off_the_edge_is_harmless() {
        let mut canvas = Canvas::new(4, 4);
        canvas.set(-1, 0, RED);
        canvas.set(4, 4, RED);
        canvas.fill_circle((0, 0), 10, RED);
        assert_eq!(canvas.get(3, 3), Some(RED));
        assert_eq!(canvas.get(4, 0), None);
    }

    #[test]
    fn lines_reach_both_ends() {
        let mut canvas = Canvas::new(10, 10);
        canvas.line((1, 8), (7, 2), RED);
        assert_eq!(canvas.get(1, 8), Some(RED));
        assert_eq!(canvas.get(7, 2), Some(RED));
    }

    #[test]
    fn text_lights_the_right_pixels() {
        let mut canvas = Canvas::new(8, 8);
        canvas.text(0, 0, "1", RED);
        // "1" is 010 / 110 / 010 / 010 / 111.
        assert_eq!(canvas.get(1, 0), Some(RED));
        assert_ne!(canvas.get(0, 0), Some(RED));
        assert_eq!(canvas.get(0, 4), Some(RED));
    }

    #[test]
    fn polar_uses_screen_coordinates() {
        assert_eq!(polar((10, 10), 0.0, 5.0), (15, 10)); // east is right
        assert_eq!(polar((10, 10), std::f32::consts::FRAC_PI_2, 5.0), (10, 5)); // north is up
    }
}
