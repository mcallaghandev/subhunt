//! Sound effects, synthesised in code: no audio files at all.
//!
//! * [`synth`]: generates each sound as a list of samples, and packs samples
//!   into a WAV file in memory for Bevy to play
//! * [`cues`]: decides which sounds to play by comparing what the sonar knew
//!   a moment ago with what it knows now
//!
//! Like the panel, this is plain Rust with tests. The Bevy side, which
//! actually plays things, is `audio_plugin.rs`.

pub mod cues;
pub mod synth;
