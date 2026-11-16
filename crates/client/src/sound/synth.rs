//! A tiny synthesiser. Every sound is built from a few basic ingredients:
//!
//! * **sine waves** (pure tones: the sonar ping),
//! * **noise** (random samples: rushing water, rumble),
//! * **envelopes** (how loudness changes over time: a sharp start and slow fade),
//! * **filters** (keep the low rumble, lose the hiss),
//! * **resonators** (something that rings when struck: a creaking hull).
//!
//! A sound is a `Vec<f32>` of samples between -1 and 1, played at
//! [`SAMPLE_RATE`] samples per second.

use std::f32::consts::TAU;

/// Samples per second. Low by modern standards, but plenty for rumbles and
/// pings, and it keeps the sounds small.
pub const SAMPLE_RATE: u32 = 22_050;
/// The engine loop's length. Every frequency in it repeats a whole number of
/// times in this time, so the loop joins up without a click.
pub const ENGINE_LOOP_SECONDS: f32 = 2.0;
/// The pitch of our sonar.
const PING_PITCH: f32 = 1180.0;

fn sample_count(seconds: f32) -> usize {
    (seconds * SAMPLE_RATE as f32) as usize
}

fn time_of(index: usize) -> f32 {
    index as f32 / SAMPLE_RATE as f32
}

/// Small repeatable random numbers (xorshift), so each run sounds the same.
struct Random(u64);

impl Random {
    fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    /// A number in `[0, 1)`.
    fn unit(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }

    /// A number in `[-1, 1)`: white noise.
    fn noise(&mut self) -> f32 {
        self.unit() * 2.0 - 1.0
    }

    fn range(&mut self, low: f32, high: f32) -> f32 {
        low + (high - low) * self.unit()
    }
}

/// Scales a sound so its loudest sample is `peak`.
fn normalise(samples: &mut [f32], peak: f32) {
    let loudest = samples.iter().fold(0.0f32, |most, s| most.max(s.abs()));
    if loudest > 0.0 {
        let gain = peak / loudest;
        for sample in samples.iter_mut() {
            *sample *= gain;
        }
    }
}

/// Adds quieter, delayed copies of a sound to itself: a cheap echo or reverb.
fn add_echoes(samples: &mut Vec<f32>, echoes: &[(f32, f32)]) {
    let dry = samples.clone();
    for &(delay, loudness) in echoes {
        let offset = sample_count(delay);
        samples.resize(samples.len().max(dry.len() + offset), 0.0);
        for (i, sample) in dry.iter().enumerate() {
            samples[i + offset] += sample * loudness;
        }
    }
}

/// The classic sonar "ping": a pure tone that starts sharply, fades slowly
/// and rings on in the water.
pub fn sonar_ping() -> Vec<f32> {
    let mut samples: Vec<f32> = (0..sample_count(1.6))
        .map(|i| {
            let t = time_of(i);
            let envelope = if t < 0.004 { t / 0.004 } else { (-(t - 0.004) * 2.8).exp() };
            let tone = (TAU * PING_PITCH * t).sin() + 0.12 * (TAU * 2.0 * PING_PITCH * t).sin();
            tone * envelope
        })
        .collect();
    add_echoes(&mut samples, &[(0.11, 0.35), (0.23, 0.2), (0.37, 0.12)]);
    normalise(&mut samples, 0.8);
    samples
}

/// A ping coming back off something: shorter, softer and slightly lower.
pub fn echo_blip() -> Vec<f32> {
    let mut samples: Vec<f32> = (0..sample_count(0.4))
        .map(|i| {
            let t = time_of(i);
            let envelope = if t < 0.01 { t / 0.01 } else { (-(t - 0.01) * 10.0).exp() };
            (TAU * PING_PITCH * 0.96 * t).sin() * envelope
        })
        .collect();
    normalise(&mut samples, 0.7);
    samples
}

/// A torpedo leaving the tube: a heavy thump, a rush of water, then bubbles.
pub fn torpedo_launch() -> Vec<f32> {
    let mut random = Random::new(7);
    let length = sample_count(1.6);
    let mut samples = vec![0.0; length];

    // The thump: a low tone that dies away fast.
    for (i, sample) in samples.iter_mut().enumerate() {
        let t = time_of(i);
        *sample += 0.9 * (TAU * 55.0 * t).sin() * (-t * 7.0).exp();
    }

    // The rush: noise through a low-pass filter that closes as it fades.
    // A one-pole low-pass filter just moves part of the way towards each new
    // sample, smoothing out the fast (high, hissy) wiggles.
    let mut smoothed = 0.0;
    for (i, sample) in samples.iter_mut().enumerate() {
        let t = time_of(i);
        let openness = 0.25 * (-t * 1.6).exp() + 0.02;
        smoothed += openness * (random.noise() - smoothed);
        let envelope = (t / 0.05).min(1.0) * (-t * 2.2).exp();
        *sample += 1.6 * smoothed * envelope;
    }

    // Bubbles: short rising chirps at random moments.
    for _ in 0..14 {
        let start = sample_count(random.range(0.2, 1.3));
        let pitch = random.range(300.0, 800.0);
        let loudness = random.range(0.05, 0.15);
        for j in 0..sample_count(0.05) {
            let t = time_of(j);
            if let Some(sample) = samples.get_mut(start + j) {
                *sample += loudness * (TAU * pitch * (1.0 + 4.0 * t) * t).sin() * (-t * 60.0).exp();
            }
        }
    }

    normalise(&mut samples, 0.85);
    samples
}

/// One sample of the engine loop at time `t`. Separate so the tests can
/// check that the loop joins up.
fn engine_sample(t: f32, partials: &[(f32, f32, f32)]) -> f32 {
    // A low rumble made of many quiet tones with random starting points...
    let rumble: f32 =
        partials.iter().map(|&(frequency, loudness, phase)| loudness * (TAU * frequency * t + phase).sin()).sum();
    // ...pulsing with the propeller blades, plus a faint electrical hum.
    let blades = 1.0 + 0.35 * (TAU * 8.0 * t).sin();
    rumble * blades + 0.15 * (TAU * 50.0 * t).sin()
}

/// The tones that make up the engine rumble: (frequency, loudness, phase).
/// Every frequency is a multiple of 0.5 Hz, so each repeats exactly within the
/// 2-second loop.
fn engine_partials() -> Vec<(f32, f32, f32)> {
    let mut random = Random::new(3);
    (40..=240).map(|k| (k as f32 * 0.5, 1.0 / (k as f32).powf(0.8), random.range(0.0, TAU))).collect()
}

/// A seamless loop of engine rumble. The game changes its volume and speed to
/// match how fast the submarine is going.
pub fn engine_loop() -> Vec<f32> {
    let partials = engine_partials();
    let mut samples: Vec<f32> =
        (0..sample_count(ENGINE_LOOP_SECONDS)).map(|i| engine_sample(time_of(i), &partials)).collect();
    normalise(&mut samples, 0.7);
    samples
}

/// The hull groaning under pressure. "Stick-slip": the metal sticks, strains,
/// then slips with a tiny jolt, over and over. Each jolt makes the hull ring
/// at its natural frequencies, which drift as the stress changes.
pub fn hull_creak(seed: u64) -> Vec<f32> {
    let mut random = Random::new(seed);
    let length = sample_count(random.range(1.0, 1.8));
    let low_start = random.range(150.0, 210.0);
    let low_end = low_start * random.range(0.55, 0.75);
    let high = random.range(380.0, 520.0);

    // The jolts: clicks at irregular gaps, fastest in the middle of the creak.
    let mut jolts = vec![0.0; length];
    let mut next = 0;
    while next < length {
        let progress = next as f32 / length as f32;
        jolts[next] = random.range(0.5, 1.0);
        let gap = 0.035 - 0.022 * (progress * std::f32::consts::PI).sin();
        next += sample_count(gap * random.range(0.7, 1.3)).max(1);
    }

    // Two resonators ring when jolted. A resonator feeds its own last two
    // outputs back in, which makes it keep vibrating at one frequency.
    let mut samples = vec![0.0; length];
    let mut low = (0.0, 0.0);
    let mut high_ring = (0.0, 0.0);
    for i in 0..length {
        let progress = i as f32 / length as f32;
        let pitch = low_start + (low_end - low_start) * progress;
        let ring = |state: &mut (f32, f32), frequency: f32, input: f32| {
            let r = 0.996;
            let w = TAU * frequency / SAMPLE_RATE as f32;
            let output = input + 2.0 * r * w.cos() * state.0 - r * r * state.1;
            *state = (output, state.0);
            output
        };
        let envelope = (progress * 8.0).min(1.0) * ((1.0 - progress) * 4.0).min(1.0);
        samples[i] = envelope * (ring(&mut low, pitch, jolts[i]) + 0.4 * ring(&mut high_ring, high, jolts[i]));
    }
    normalise(&mut samples, 0.6);
    samples
}

/// Packs samples into a WAV file (16-bit mono) entirely in memory.
///
/// A WAV file is a short header describing the format, followed by the raw
/// samples as 16-bit whole numbers. Rust note: `to_le_bytes` writes numbers
/// "little-endian" (smallest byte first), which is what WAV requires.
pub fn encode_wav(samples: &[f32]) -> Vec<u8> {
    let data_bytes = (samples.len() * 2) as u32;
    let mut wav = Vec::with_capacity(44 + data_bytes as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_bytes).to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes()); // size of this section
    wav.extend_from_slice(&1u16.to_le_bytes()); // 1 = plain PCM samples
    wav.extend_from_slice(&1u16.to_le_bytes()); // mono
    wav.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    wav.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes()); // bytes per second
    wav.extend_from_slice(&2u16.to_le_bytes()); // bytes per sample
    wav.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_bytes.to_le_bytes());
    for sample in samples {
        let whole = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        wav.extend_from_slice(&whole.to_le_bytes());
    }
    wav
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loudness(samples: &[f32]) -> f32 {
        samples.iter().map(|s| s * s).sum::<f32>() / samples.len().max(1) as f32
    }

    #[test]
    fn sounds_are_in_range_and_not_silent() {
        let sounds = [sonar_ping(), echo_blip(), torpedo_launch(), engine_loop(), hull_creak(1), hull_creak(2)];
        for sound in &sounds {
            assert!(!sound.is_empty());
            assert!(sound.iter().all(|s| s.is_finite() && s.abs() <= 1.0));
            assert!(loudness(sound) > 0.0001);
        }
    }

    #[test]
    fn pings_fade_away() {
        let ping = sonar_ping();
        let tenth = sample_count(0.1);
        assert!(loudness(&ping[..tenth]) > 20.0 * loudness(&ping[ping.len() - tenth..]));
    }

    #[test]
    fn engine_loop_joins_up_without_a_click() {
        let partials = engine_partials();
        let start = engine_sample(0.0, &partials);
        let end = engine_sample(ENGINE_LOOP_SECONDS, &partials);
        assert!((start - end).abs() < 0.01, "start {start} vs end {end}");
    }

    #[test]
    fn creaks_vary() {
        assert_ne!(hull_creak(1), hull_creak(2));
    }

    #[test]
    fn wav_header_describes_the_samples() {
        let wav = encode_wav(&[0.0, 1.0, -1.0]);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(u32::from_le_bytes(wav[24..28].try_into().unwrap()), SAMPLE_RATE);
        assert_eq!(u32::from_le_bytes(wav[40..44].try_into().unwrap()), 6); // 3 samples × 2 bytes
        assert_eq!(i16::from_le_bytes([wav[46], wav[47]]), i16::MAX);
        assert_eq!(wav.len(), 44 + 6);
    }
}
