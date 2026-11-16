//! Bevy adapter for sound: plays the synthesised effects at the right moments.
//!
//! * One-shot sounds (ping, echo, torpedo, explosion) are entities with an
//!   `AudioPlayer`. `PlaybackSettings::DESPAWN` removes each one when it ends.
//! * The engine is a single looping sound whose volume and speed (pitch)
//!   follow the submarine's speed.
//!
//! Press M to mute or unmute.

use std::sync::Arc;

use bevy::audio::Volume;
use bevy::prelude::*;
use subhunt_application::ConnectionStatus;
use subhunt_domain::{GameRules, OwnShipStatus};

use crate::session::Session;
use crate::sound::cues::{self, Cue};
use crate::sound::synth;

#[derive(Resource)]
struct Sounds {
    ping: Handle<AudioSource>,
    echo: Handle<AudioSource>,
    torpedo: Handle<AudioSource>,
    engine: Handle<AudioSource>,
    explosion: Handle<AudioSource>,
}

/// What the sound system remembers between frames.
#[derive(Resource)]
struct Listening {
    /// Our boat as of the last frame, to spot reload timers jumping up.
    previous_ship: Option<OwnShipStatus>,
    echoes_heard: u64,
    notices_heard: u64,
    /// Echoes still travelling back through the water: (when to play, loudness).
    incoming_echoes: Vec<(f32, f32)>,
    muted: bool,
}

#[derive(Component)]
struct EngineSound;

pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Listening {
            previous_ship: None,
            echoes_heard: 0,
            notices_heard: 0,
            incoming_echoes: Vec::new(),
            muted: false,
        })
        .add_systems(Startup, make_sounds)
        .add_systems(Update, (toggle_mute, play_effects, drive_engine));
    }
}

/// Turns synthesised samples into a sound Bevy can play.
fn to_audio(samples: &[f32]) -> AudioSource {
    AudioSource { bytes: Arc::from(synth::encode_wav(samples)) }
}

fn make_sounds(mut commands: Commands, mut sources: ResMut<Assets<AudioSource>>) {
    let sounds = Sounds {
        ping: sources.add(to_audio(&synth::sonar_ping())),
        echo: sources.add(to_audio(&synth::echo_blip())),
        torpedo: sources.add(to_audio(&synth::torpedo_launch())),
        engine: sources.add(to_audio(&synth::engine_loop())),
        explosion: sources.add(to_audio(&synth::explosion())),
    };
    commands.spawn((
        AudioPlayer(sounds.engine.clone()),
        PlaybackSettings::LOOP.with_volume(Volume::Linear(0.0)),
        EngineSound,
    ));
    commands.insert_resource(sounds);
}

fn play_once(commands: &mut Commands, sound: &Handle<AudioSource>, loudness: f32) {
    commands.spawn((AudioPlayer(sound.clone()), PlaybackSettings::DESPAWN.with_volume(Volume::Linear(loudness))));
}

fn toggle_mute(keys: Res<ButtonInput<KeyCode>>, mut listening: ResMut<Listening>) {
    if keys.just_pressed(KeyCode::KeyM) {
        listening.muted = !listening.muted;
    }
}

fn play_effects(
    mut commands: Commands,
    session: Res<Session>,
    sounds: Res<Sounds>,
    time: Res<Time>,
    mut listening: ResMut<Listening>,
) {
    let session = &session.0;
    let now = time.elapsed_secs();
    let Some(picture) = session.picture() else { return };
    let ship = picture.own_ship.clone();

    let mut cues = listening.previous_ship.as_ref().map_or_else(Vec::new, |before| cues::own_ship_cues(before, &ship));

    // Echoes that arrived since last frame are the newest ones in the list.
    let new_echoes = (session.echo_count() - listening.echoes_heard) as usize;
    let echoes = session.echoes();
    for echo in &echoes[echoes.len().saturating_sub(new_echoes)..] {
        cues.push(cues::echo_cue(ship.position, echo.position, session.display_range()));
    }
    listening.echoes_heard = session.echo_count();

    // Notices that arrived since last frame are the newest, at the front.
    let new_notices = (session.notice_count() - listening.notices_heard) as usize;
    cues.extend(session.notices().take(new_notices).filter_map(|timed| cues::notice_cue(&timed.notice)));
    listening.notices_heard = session.notice_count();
    listening.previous_ship = Some(ship);

    let muted = listening.muted;
    for cue in cues {
        match cue {
            Cue::PingSent if !muted => play_once(&mut commands, &sounds.ping, 0.7),
            Cue::TorpedoLaunched if !muted => play_once(&mut commands, &sounds.torpedo, 0.8),
            Cue::Explosion { loudness } if !muted => play_once(&mut commands, &sounds.explosion, loudness),
            Cue::Echo { delay, loudness } => listening.incoming_echoes.push((now + delay, loudness)),
            _ => {}
        }
    }

    // Play any echoes that have finished travelling back.
    let mut arrived = Vec::new();
    listening.incoming_echoes.retain(|&(due, loudness)| {
        let due_now = due <= now;
        if due_now {
            arrived.push(loudness);
        }
        !due_now
    });
    if !muted {
        for loudness in arrived {
            play_once(&mut commands, &sounds.echo, 0.6 * loudness);
        }
    }
}

/// The engine gets louder and higher pitched as the boat speeds up.
fn drive_engine(
    session: Res<Session>,
    listening: Res<Listening>,
    time: Res<Time>,
    mut engine: Query<&mut AudioSink, With<EngineSound>>,
) {
    // The sink appears a moment after the sound starts playing.
    let Ok(mut sink) = engine.single_mut() else { return };
    let speed = session.0.picture().map_or(0.0, |p| p.own_ship.speed.abs());
    let (mut volume, playback_speed) = cues::engine_mix(speed / GameRules::default().max_forward_speed);
    if listening.muted || session.0.status() != ConnectionStatus::Playing {
        volume = 0.0;
    }
    // Ease towards the target so changes are smooth rather than stepped.
    let ease = (time.delta_secs() * 4.0).min(1.0);
    let current = sink.volume().to_linear();
    sink.set_volume(Volume::Linear(current + (volume - current) * ease));
    sink.set_speed(sink.speed() + (playback_speed - sink.speed()) * ease);
}
