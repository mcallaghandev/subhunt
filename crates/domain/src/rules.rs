/// Every tunable number in the game, in one place.
///
/// Units: distances in metres, times in seconds, speeds in metres/second,
/// angles in radians. "Noise" is a unitless loudness score.
///
/// Changing these is the quickest way to change how the game feels.
#[derive(Clone, Debug, PartialEq)]
pub struct GameRules {
    /// The play area is a square from `-world_half_extent` to `+world_half_extent`.
    pub world_half_extent: f32,

    // --- Movement ---
    pub max_forward_speed: f32,
    pub max_reverse_speed: f32,
    pub acceleration: f32,
    /// Turn rate at full rudder and full speed.
    pub max_turn_rate: f32,
    /// Fraction of `max_turn_rate` available when stopped (subs steer badly when slow).
    pub min_turn_factor: f32,

    // --- Passive sonar (listening) ---
    /// Noise a submarine makes just by existing (machinery hum).
    pub base_noise: f32,
    /// How far away a noise of 1.0 can be heard.
    pub passive_range_per_noise: f32,
    /// 0..1: how much your own speed deafens your sonar (flow noise). 0.5 means
    /// at full speed you only hear half as far.
    pub flow_noise_deafness: f32,

    // --- Active sonar (pinging) ---
    pub ping_range: f32,
    pub ping_cooldown: f32,
    /// Extra noise made by pinging, which lets everyone nearby hear you.
    pub ping_noise: f32,
    pub ping_noise_duration: f32,

    // --- Torpedoes ---
    pub torpedo_speed: f32,
    pub torpedo_lifetime: f32,
    pub torpedo_cooldown: f32,
    pub torpedo_noise: f32,
    pub torpedo_hit_radius: f32,
    /// How far in front of the sub a torpedo appears.
    pub torpedo_launch_offset: f32,
    pub launch_noise: f32,
    pub launch_noise_duration: f32,
}

// Rust note: `Default` is the standard "sensible starting value" trait.
// `GameRules::default()` gives you these numbers.
impl Default for GameRules {
    fn default() -> Self {
        Self {
            world_half_extent: 2000.0,

            max_forward_speed: 60.0,
            max_reverse_speed: 20.0,
            acceleration: 15.0,
            max_turn_rate: 0.8,
            min_turn_factor: 0.3,

            base_noise: 0.2,
            passive_range_per_noise: 900.0,
            flow_noise_deafness: 0.5,

            ping_range: 1500.0,
            ping_cooldown: 8.0,
            ping_noise: 3.0,
            ping_noise_duration: 2.0,

            torpedo_speed: 140.0,
            torpedo_lifetime: 12.0,
            torpedo_cooldown: 4.0,
            torpedo_noise: 1.0,
            torpedo_hit_radius: 25.0,
            torpedo_launch_offset: 30.0,
            launch_noise: 1.0,
            launch_noise_duration: 1.0,
        }
    }
}
