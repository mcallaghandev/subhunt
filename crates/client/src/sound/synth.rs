//! A tiny synthesiser. Every sound is built from a few basic ingredients:
//!
//! * **sine waves** (pure tones: the sonar ping),
//! * **noise** (random samples: rushing water, rumble),
//! * **envelopes** (how loudness changes over time: a sharp start and slow fade),
//! * **filters** (keep the low rumble, lose the hiss),
//! * **resonators and band-pass filters** (keeping just one range of pitches).
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

/// A band-pass filter: lets through a band of frequencies around `centre`
/// and quietens everything else. This is the standard "biquad" design from
/// Robert Bristow-Johnson's Audio EQ Cookbook.
struct BandPass {
    coefficients: [f32; 5],
    history: [f32; 4],
}

impl BandPass {
    fn new(centre: f32, q: f32) -> Self {
        let w = TAU * centre / SAMPLE_RATE as f32;
        let alpha = w.sin() / (2.0 * q);
        let a0 = 1.0 + alpha;
        Self {
            coefficients: [alpha / a0, 0.0, -alpha / a0, -2.0 * w.cos() / a0, (1.0 - alpha) / a0],
            history: [0.0; 4],
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let [b0, b1, b2, a1, a2] = self.coefficients;
        let [x1, x2, y1, y2] = self.history;
        let output = b0 * input + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
        self.history = [input, x1, output, y1];
        output
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

/// The low part of the engine at time `t`. Separate so the tests can check
/// that the loop joins up.
fn engine_sample(t: f32, partials: &[(f32, f32, f32)]) -> f32 {
    // A deep rumble made of many quiet tones with random starting points...
    let rumble: f32 =
        partials.iter().map(|&(frequency, loudness, phase)| loudness * (TAU * frequency * t + phase).sin()).sum();
    // ...pulsing with the propeller blades.
    rumble * blade_pulse(t)
}

/// The propeller blades pass 8 times a second, making the sound throb.
fn blade_pulse(t: f32) -> f32 {
    1.0 + 0.35 * (TAU * 8.0 * t).sin()
}

/// The machinery hum: 50 Hz and its harmonics up to about 1.2 kHz.
///
/// Phone speakers can't play anything much below 300 Hz, so on a phone the
/// deep rumble above is silent. Your ear still hears a low "hum" from its
/// harmonics even when the 50 Hz itself is missing (the "missing
/// fundamental" effect), so these carry the engine on small speakers.
fn machinery_hum(t: f32) -> f32 {
    (1..=24).map(|harmonic| (TAU * 50.0 * harmonic as f32 * t).sin() / (harmonic as f32).powf(0.9)).sum::<f32>()
}

/// The tones that make up the engine rumble: (frequency, loudness, phase).
/// Every frequency is a multiple of 0.5 Hz, so each repeats exactly within the
/// 2-second loop.
fn engine_partials() -> Vec<(f32, f32, f32)> {
    let mut random = Random::new(3);
    (40..=240).map(|k| (k as f32 * 0.5, 1.0 / (k as f32).powf(0.8), random.range(0.0, TAU))).collect()
}

/// The swish of the propeller through the water: noise in the band phone
/// speakers can play, throbbing with the blades.
///
/// To loop without a click, the noise is filtered twice round the loop and
/// only the second lap is kept, so the filter has "warmed up" on the very
/// samples that will come before it when the loop repeats.
fn propeller_swish(length: usize) -> Vec<f32> {
    let mut random = Random::new(11);
    let noise: Vec<f32> = (0..length).map(|_| random.noise()).collect();
    let mut low_band = BandPass::new(450.0, 0.8);
    let mut high_band = BandPass::new(1100.0, 1.2);
    let mut swish = vec![0.0; length];
    for lap in 0..2 {
        for (i, sample) in noise.iter().enumerate() {
            let filtered = low_band.process(*sample) + 0.5 * high_band.process(*sample);
            if lap == 1 {
                swish[i] = filtered * blade_pulse(time_of(i));
            }
        }
    }
    swish
}

/// A seamless loop of engine noise: deep rumble for headphones, hum and
/// propeller swish for phone speakers. The game changes its volume and speed
/// to match how fast the submarine is going.
pub fn engine_loop() -> Vec<f32> {
    let partials = engine_partials();
    let length = sample_count(ENGINE_LOOP_SECONDS);
    let mut rumble: Vec<f32> = (0..length).map(|i| engine_sample(time_of(i), &partials)).collect();
    let mut hum: Vec<f32> = (0..length).map(|i| machinery_hum(time_of(i))).collect();
    let mut swish = propeller_swish(length);
    // Mix the three layers at chosen levels.
    normalise(&mut rumble, 0.8);
    normalise(&mut hum, 0.6);
    normalise(&mut swish, 0.9);
    let mut samples: Vec<f32> = (0..length).map(|i| rumble[i] + hum[i] + swish[i]).collect();
    normalise(&mut samples, 0.8);
    samples
}

/// A torpedo exploding: a deep thump, a long rolling boom, and a crackle
/// near the start that a phone speaker can play (it can't do the deep part).
pub fn explosion() -> Vec<f32> {
    let mut random = Random::new(23);
    let length = sample_count(2.8);
    let mut samples = vec![0.0; length];

    // The thump: a low tone that drops in pitch as it dies.
    let mut phase = 0.0;
    for (i, sample) in samples.iter_mut().enumerate() {
        let t = time_of(i);
        phase += TAU * (35.0 + 45.0 * (-t * 6.0).exp()) / SAMPLE_RATE as f32;
        *sample += 1.2 * phase.sin() * (-t * 3.0).exp();
    }

    // The boom: noise through a low-pass filter that closes as it rolls away.
    let mut smoothed = 0.0;
    for (i, sample) in samples.iter_mut().enumerate() {
        let t = time_of(i);
        let openness = 0.12 * (-t * 2.0).exp() + 0.006;
        smoothed += openness * (random.noise() - smoothed);
        let envelope = (t / 0.01).min(1.0) * (-t * 1.4).exp();
        *sample += 6.0 * smoothed * envelope;
    }

    // The crackle: a burst of mid-range noise, plus scattered pops.
    let mut band = BandPass::new(900.0, 0.7);
    for (i, sample) in samples.iter_mut().enumerate() {
        let t = time_of(i);
        let pop = if random.unit() < 0.004 * (-t * 3.0).exp() { random.range(2.0, 5.0) } else { 0.0 };
        *sample += 0.9 * band.process(random.noise() + pop) * (-t * 3.5).exp();
    }

    normalise(&mut samples, 0.95);
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
        let sounds = [sonar_ping(), echo_blip(), torpedo_launch(), engine_loop(), explosion()];
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

    /// Phone speakers can't play much below 300 Hz. Sounds made only of low
    /// frequencies were silent on a phone, so check the deep sounds have a
    /// good share of their energy where a phone can play it.
    #[test]
    fn deep_sounds_are_audible_on_phone_speakers() {
        fn share_above_300_hz(samples: &[f32]) -> f32 {
            // Two passes of a simple high-pass filter (a "leaky" difference).
            let cut = (-TAU * 300.0 / SAMPLE_RATE as f32).exp();
            let high_pass = |input: &[f32]| {
                let (mut previous_in, mut previous_out) = (0.0, 0.0);
                input
                    .iter()
                    .map(|&x| {
                        let y = cut * (previous_out + x - previous_in);
                        (previous_in, previous_out) = (x, y);
                        y
                    })
                    .collect::<Vec<f32>>()
            };
            loudness(&high_pass(&high_pass(samples))) / loudness(samples)
        }
        for (name, sound) in [("engine", engine_loop()), ("explosion", explosion())] {
            let share = share_above_300_hz(&sound);
            assert!(share > 0.15, "{name}: only {:.0}% of its energy is above 300 Hz", share * 100.0);
        }
    }

    #[test]
    fn explosions_hit_hard_then_roll_away() {
        let boom = explosion();
        let tenth = sample_count(0.1);
        assert!(loudness(&boom[..tenth]) > 20.0 * loudness(&boom[boom.len() - tenth..]));
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
