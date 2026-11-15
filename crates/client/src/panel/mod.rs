//! The instrument panel: everything the player sees and touches.
//!
//! The panel is drawn pixel by pixel into a small image (a *software
//! framebuffer*, like games from the 1980s) that Bevy then scales up without
//! smoothing. That gives genuinely chunky pixels, and it means everything in
//! this module is plain Rust: no GPU, no Bevy systems. The tests below each
//! file draw panels and press buttons without opening a window.
//!
//! * [`canvas`]: the framebuffer and drawing primitives (lines, circles, text)
//! * [`font`]: a 3×5 pixel font
//! * [`layout`]: where each instrument sits, for wide and tall screens
//! * [`controls`]: turns clicks and touches into helm orders
//! * [`draw`]: paints the instruments from the game state
//!
//! The Bevy side lives in `panel_plugin.rs`.

pub mod canvas;
pub mod controls;
pub mod draw;
pub mod font;
pub mod layout;
