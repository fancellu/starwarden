//! Sound effects are synthesised at startup, so the game ships without any audio files. On the
//! desktop they are handed to Bevy as in-memory WAV data; on the web they go straight to the
//! browser's audio engine.

use bevy::prelude::*;
use std::f32::consts::TAU;

use crate::input::PlayerInput;
use crate::player::PlayerState;
use crate::state::{GameState, Sfx};

const RATE: u32 = 22_050;

// --- Synth building blocks ---------------------------------------------------------------------

#[derive(Default)]
struct Osc(f32);

impl Osc {
    /// Advances the phase by one sample at `freq` Hz and returns it in `[0, 1)`.
    fn step(&mut self, freq: f32) -> f32 {
        self.0 = (self.0 + freq / RATE as f32).fract();
        self.0
    }
    fn sine(&mut self, f: f32) -> f32 {
        (self.step(f) * TAU).sin()
    }
    fn saw(&mut self, f: f32) -> f32 {
        self.step(f) * 2.0 - 1.0
    }
    fn square(&mut self, f: f32) -> f32 {
        if self.step(f) < 0.5 {
            1.0
        } else {
            -1.0
        }
    }
    fn tri(&mut self, f: f32) -> f32 {
        (self.step(f) * 4.0 - 2.0).abs() - 1.0
    }
}

struct Noise {
    seed: u32,
    lp: f32,
}

impl Noise {
    fn new(seed: u32) -> Self {
        Self { seed, lp: 0.0 }
    }
    fn white(&mut self) -> f32 {
        self.seed = self
            .seed
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        (self.seed >> 8) as f32 / (1u32 << 23) as f32 - 1.0
    }
    /// White noise through a one-pole low-pass; `k` in `(0, 1]`, smaller is darker.
    fn low(&mut self, k: f32) -> f32 {
        let w = self.white();
        self.lp += (w - self.lp) * k;
        self.lp
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Renders `dur` seconds; the closure receives time in seconds and progress in `[0, 1]`.
fn render(dur: f32, mut f: impl FnMut(f32, f32) -> f32) -> Vec<f32> {
    let n = (dur * RATE as f32) as usize;
    (0..n)
        .map(|i| {
            let u = i as f32 / n as f32;
            // Short fades at both ends avoid clicks.
            let edge = (u * n as f32 / 60.0).min(1.0) * ((1.0 - u) * n as f32 / 200.0).min(1.0);
            f(i as f32 / RATE as f32, u) * edge
        })
        .collect()
}

/// A sequence of short notes.
fn arpeggio(notes: &[f32], note_len: f32, square_mix: f32) -> Vec<f32> {
    let mut a = Osc::default();
    let mut b = Osc::default();
    render(notes.len() as f32 * note_len, |t, _| {
        let i = ((t / note_len) as usize).min(notes.len() - 1);
        let local = (t - i as f32 * note_len) / note_len;
        let env = (1.0 - local).powf(1.5);
        let f = notes[i];
        (a.sine(f) * (1.0 - square_mix) + b.square(f) * square_mix * 0.5) * env * 0.6
    })
}

fn explosion(dur: f32, depth: f32, seed: u32) -> Vec<f32> {
    let mut n = Noise::new(seed);
    let mut sub = Osc::default();
    render(dur, |_, u| {
        let env = (1.0 - u).powf(2.2);
        let rumble = n.low(lerp(0.55, 0.03, u.powf(0.5))) * 1.6;
        let thump = sub.sine(lerp(140.0 * depth, 35.0 * depth, u)) * (1.0 - u).powi(3);
        ((rumble + thump * 0.9) * env).clamp(-1.0, 1.0) * 0.9
    })
}

fn synth(sfx: Sfx) -> Vec<f32> {
    match sfx {
        Sfx::Laser => {
            let (mut a, mut b) = (Osc::default(), Osc::default());
            render(0.17, |_, u| {
                let f = lerp(2100.0, 260.0, u.powf(0.6));
                (a.square(f) * 0.35 + b.saw(f * 1.01) * 0.4) * (1.0 - u).powf(1.4)
            })
        }
        Sfx::Missile => {
            let mut n = Noise::new(7);
            let mut a = Osc::default();
            render(0.55, |_, u| {
                let hiss = n.low(lerp(0.12, 0.6, u)) * 0.9;
                let tone = a.saw(lerp(160.0, 520.0, u)) * 0.25;
                (hiss + tone) * (1.0 - u).powf(0.8) * 0.8
            })
        }
        Sfx::Hit => {
            let mut a = Osc::default();
            let mut n = Noise::new(3);
            render(0.07, |_, u| {
                (a.square(lerp(1100.0, 500.0, u)) * 0.3 + n.white() * 0.3) * (1.0 - u)
            })
        }
        Sfx::ExplodeSmall => explosion(0.45, 1.0, 11),
        Sfx::ExplodeBig => explosion(1.1, 0.7, 23),
        Sfx::Alarm => {
            let mut a = Osc::default();
            render(0.62, |t, u| {
                let f = if ((t * 9.0) as u32).is_multiple_of(2) {
                    960.0
                } else {
                    720.0
                };
                a.square(f) * 0.28 * (1.0 - u * 0.5)
            })
        }
        Sfx::Rescue => arpeggio(&[660.0, 880.0, 1320.0, 1760.0], 0.075, 0.2),
        Sfx::Landed => arpeggio(&[523.0, 784.0, 1047.0], 0.09, 0.0),
        Sfx::ColonistLost => {
            let mut a = Osc::default();
            render(0.55, |t, u| {
                let f = lerp(760.0, 140.0, u) * (1.0 + 0.06 * (t * 38.0).sin());
                a.tri(f) * 0.7 * (1.0 - u).powf(0.7)
            })
        }
        Sfx::Mutate => {
            let (mut car, mut md) = (Osc::default(), Osc::default());
            render(0.75, |_, u| {
                let m = md.sine(lerp(40.0, 190.0, u));
                let f = lerp(110.0, 520.0, u * u) * (1.0 + m * 0.5);
                car.saw(f.max(20.0)) * 0.55 * (u * 6.0).min(1.0) * (1.0 - u).powf(0.5)
            })
        }
        Sfx::Bomb => {
            let mut n = Noise::new(99);
            let (mut sub, mut hi) = (Osc::default(), Osc::default());
            render(1.5, |_, u| {
                let swell = (u * 10.0).min(1.0) * (1.0 - u).powf(1.6);
                let roar = n.low(lerp(0.7, 0.04, u.powf(0.4))) * 1.5;
                let boom = sub.sine(lerp(90.0, 28.0, u)) * 0.9;
                let ring = hi.sine(lerp(300.0, 2400.0, u)) * 0.12 * (1.0 - u);
                ((roar + boom) * swell + ring).clamp(-1.0, 1.0) * 0.95
            })
        }
        Sfx::StalkerWarn => {
            let mut a = Osc::default();
            render(0.5, |t, u| {
                let f = if ((t * 22.0) as u32).is_multiple_of(2) {
                    1250.0
                } else {
                    1650.0
                };
                a.tri(f) * 0.5 * (1.0 - u * 0.6)
            })
        }
        Sfx::PlayerDie => {
            let mut n = Noise::new(5);
            let mut a = Osc::default();
            render(1.4, |_, u| {
                let noise = n.low(lerp(0.6, 0.05, u)) * 1.3;
                let fall = a.saw(lerp(620.0, 38.0, u.powf(0.7))) * 0.45;
                ((noise + fall) * (1.0 - u).powf(1.3)).clamp(-1.0, 1.0) * 0.9
            })
        }
        Sfx::WaveStart => arpeggio(&[330.0, 440.0, 554.0, 660.0, 880.0], 0.11, 0.35),
        Sfx::Blip => {
            let mut a = Osc::default();
            render(0.09, |_, u| {
                a.sine(lerp(900.0, 1300.0, u)) * 0.5 * (1.0 - u)
            })
        }
        Sfx::ExtraLife => arpeggio(&[784.0, 988.0, 1175.0, 1568.0, 1976.0, 2349.0], 0.07, 0.15),
        Sfx::EnemyShot => {
            let mut a = Osc::default();
            render(0.16, |_, u| a.tri(lerp(420.0, 980.0, u)) * 0.45 * (1.0 - u))
        }
        Sfx::PlanetLost => {
            let mut n = Noise::new(41);
            let (mut sub, mut b) = (Osc::default(), Osc::default());
            render(2.6, |t, u| {
                let env = (u * 4.0).min(1.0) * (1.0 - u).powf(1.2);
                let roar = n.low(lerp(0.35, 0.02, u)) * 1.8;
                let boom = sub.sine(lerp(70.0, 24.0, u)) * (1.0 + 0.3 * (t * 11.0).sin());
                let groan = b.saw(lerp(180.0, 50.0, u)) * 0.2;
                ((roar + boom + groan) * env).clamp(-1.0, 1.0) * 0.95
            })
        }
    }
}

/// Seamlessly looping engine rumble.
fn thrust_loop() -> Vec<f32> {
    let mut n = Noise::new(77);
    let mut a = Osc::default();
    let n_samples = RATE as usize;
    let raw: Vec<f32> = (0..n_samples + 2000)
        .map(|_| n.low(0.08) * 2.2 + a.saw(55.0) * 0.12)
        .collect();
    // Cross-fade the tail into the head so the loop point is inaudible.
    (0..n_samples)
        .map(|i| {
            if i < 2000 {
                let k = i as f32 / 2000.0;
                raw[i] * k + raw[n_samples + i] * (1.0 - k)
            } else {
                raw[i]
            }
        })
        .collect()
}

/// Encodes mono samples as a 16-bit PCM WAV file.
#[cfg(not(target_arch = "wasm32"))]
pub fn encode_wav(samples: &[f32]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&RATE.to_le_bytes());
    out.extend_from_slice(&(RATE * 2).to_le_bytes()); // byte rate
    out.extend_from_slice(&2u16.to_le_bytes()); // block align
    out.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        let v = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

// --- Playback ----------------------------------------------------------------------------------

const ALL: [Sfx; 18] = [
    Sfx::Laser,
    Sfx::Missile,
    Sfx::Hit,
    Sfx::ExplodeSmall,
    Sfx::ExplodeBig,
    Sfx::Alarm,
    Sfx::Rescue,
    Sfx::Landed,
    Sfx::ColonistLost,
    Sfx::Mutate,
    Sfx::Bomb,
    Sfx::StalkerWarn,
    Sfx::PlayerDie,
    Sfx::WaveStart,
    Sfx::Blip,
    Sfx::ExtraLife,
    Sfx::EnemyShot,
    Sfx::PlanetLost,
];

fn volume(sfx: Sfx) -> f32 {
    match sfx {
        Sfx::Laser => 0.22,
        Sfx::Hit => 0.3,
        Sfx::EnemyShot => 0.25,
        Sfx::Missile => 0.45,
        Sfx::ExplodeSmall => 0.5,
        Sfx::Alarm | Sfx::StalkerWarn => 0.4,
        Sfx::Blip => 0.35,
        Sfx::Bomb | Sfx::PlayerDie | Sfx::PlanetLost | Sfx::ExplodeBig => 0.8,
        _ => 0.5,
    }
}

/// Desktop playback: each sound becomes an in-memory WAV file that Bevy's audio plays.
#[cfg(not(target_arch = "wasm32"))]
mod backend {
    use bevy::audio::Volume;
    use bevy::prelude::*;

    use super::{encode_wav, synth, thrust_loop, volume, ALL};
    use crate::state::Sfx;

    #[derive(Resource)]
    pub struct Sounds(Vec<(Sfx, Handle<AudioSource>)>);

    #[derive(Component)]
    pub struct ThrustLoop;

    pub fn setup_audio(mut commands: Commands, mut sources: ResMut<Assets<AudioSource>>) {
        let mut add = |samples: Vec<f32>| {
            sources.add(AudioSource {
                bytes: encode_wav(&samples).into(),
            })
        };
        let sounds = ALL.iter().map(|s| (*s, add(synth(*s)))).collect();
        let thrust = add(thrust_loop());
        commands.insert_resource(Sounds(sounds));
        commands.spawn((
            ThrustLoop,
            AudioPlayer::new(thrust),
            PlaybackSettings::LOOP.with_volume(Volume::Linear(0.0)),
        ));
    }

    pub fn play(In(wanted): In<Vec<Sfx>>, mut commands: Commands, sounds: Res<Sounds>) {
        for s in wanted {
            if let Some((_, h)) = sounds.0.iter().find(|(k, _)| *k == s) {
                commands.spawn((
                    AudioPlayer::new(h.clone()),
                    PlaybackSettings::DESPAWN.with_volume(Volume::Linear(volume(s))),
                ));
            }
        }
    }

    pub fn set_thrust(In(level): In<f32>, mut q: Query<&mut AudioSink, With<ThrustLoop>>) {
        for mut sink in &mut q {
            sink.set_volume(Volume::Linear(level));
        }
    }
}

/// Web playback: the sounds are handed to the browser's own audio engine.
///
/// Bevy's audio on the web is mixed by the game's code on the page's main thread, a few
/// hundredths of a second ahead. Whenever a frame takes longer than that, which is common on
/// phones, the sound stutters. The browser's engine mixes on its own thread, so a slow frame
/// can delay the start of a sound but can never break one up.
#[cfg(target_arch = "wasm32")]
mod backend {
    use bevy::prelude::*;
    use std::cell::RefCell;
    use web_sys::{AudioBuffer, AudioContext, AudioContextState, GainNode};

    use super::{synth, thrust_loop, volume, ALL, RATE};
    use crate::state::Sfx;

    struct WebAudio {
        ctx: AudioContext,
        buffers: Vec<(Sfx, AudioBuffer)>,
        thrust: GainNode,
    }

    // Browser objects cannot leave the main thread, so they live here, not in a resource.
    thread_local! {
        static AUDIO: RefCell<Option<WebAudio>> = const { RefCell::new(None) };
    }

    fn build() -> Option<WebAudio> {
        let ctx = AudioContext::new().ok()?;
        let buffer = |samples: Vec<f32>| -> Option<AudioBuffer> {
            let b = ctx
                .create_buffer(1, samples.len() as u32, RATE as f32)
                .ok()?;
            b.copy_to_channel(&samples, 0).ok()?;
            Some(b)
        };
        let mut buffers = Vec::new();
        for s in ALL {
            buffers.push((s, buffer(synth(s))?));
        }
        // The engine rumble loops for ever; only its volume changes.
        let thrust = ctx.create_gain().ok()?;
        thrust.gain().set_value(0.0);
        thrust.connect_with_audio_node(&ctx.destination()).ok()?;
        let source = ctx.create_buffer_source().ok()?;
        source.set_buffer(Some(&buffer(thrust_loop())?));
        source.set_loop(true);
        source.connect_with_audio_node(&thrust).ok()?;
        source.start().ok()?;
        Some(WebAudio {
            ctx,
            buffers,
            thrust,
        })
    }

    pub fn setup_audio() {
        let audio = build();
        if audio.is_none() {
            warn!("Web Audio is not available; the game will be silent");
        }
        AUDIO.with(|a| *a.borrow_mut() = audio);
    }

    pub fn play(In(wanted): In<Vec<Sfx>>) {
        AUDIO.with(|a| {
            let a = a.borrow();
            let Some(audio) = a.as_ref() else {
                return;
            };
            // Until the player's first key press or tap the browser keeps audio suspended, and
            // anything started now would all play at once when it resumes.
            if audio.ctx.state() != AudioContextState::Running {
                return;
            }
            for s in wanted {
                let Some((_, buffer)) = audio.buffers.iter().find(|(k, _)| *k == s) else {
                    continue;
                };
                let (Ok(source), Ok(gain)) =
                    (audio.ctx.create_buffer_source(), audio.ctx.create_gain())
                else {
                    continue;
                };
                source.set_buffer(Some(buffer));
                gain.gain().set_value(volume(s));
                let _ = source.connect_with_audio_node(&gain);
                let _ = gain.connect_with_audio_node(&audio.ctx.destination());
                let _ = source.start();
            }
        });
    }

    pub fn set_thrust(In(level): In<f32>) {
        AUDIO.with(|a| {
            if let Some(audio) = a.borrow().as_ref() {
                audio.thrust.gain().set_value(level);
            }
        });
    }
}

pub use backend::setup_audio;

/// Collects this frame's sound requests: each distinct effect at most once.
fn wanted_sfx(mut reader: MessageReader<Sfx>) -> Vec<Sfx> {
    let mut seen: Vec<Sfx> = Vec::new();
    for s in reader.read() {
        if !seen.contains(s) {
            seen.push(*s);
        }
    }
    seen
}

/// Plays the sounds requested this frame.
pub fn play_sfx_plugin(app: &mut App) {
    app.add_systems(
        Update,
        (
            wanted_sfx.pipe(backend::play),
            thrust_level.pipe(backend::set_thrust),
        )
            .chain()
            .in_set(crate::state::Step::Present),
    );
}

/// The engine rumble follows the throttle.
fn thrust_level(
    time: Res<Time>,
    state: Res<State<GameState>>,
    input: Res<PlayerInput>,
    ps: Res<PlayerState>,
    mut level: Local<f32>,
) -> f32 {
    let playing = *state.get() == GameState::Playing && ps.alive;
    let target = if playing {
        0.05 + input.mv.x.abs() * 0.3
    } else {
        0.0
    };
    *level += (target - *level) * (time.delta_secs() * 8.0).min(1.0);
    *level
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_header_is_well_formed() {
        let wav = encode_wav(&[0.0, 0.5, -0.5, 1.0, -1.0, 2.0]);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..16], b"WAVEfmt ");
        assert_eq!(&wav[36..40], b"data");
        assert_eq!(wav.len(), 44 + 12);
        let riff = u32::from_le_bytes(wav[4..8].try_into().unwrap());
        assert_eq!(riff as usize, wav.len() - 8);
        let data = u32::from_le_bytes(wav[40..44].try_into().unwrap());
        assert_eq!(data, 12);
        let rate = u32::from_le_bytes(wav[24..28].try_into().unwrap());
        assert_eq!(rate, RATE);
        // Samples are clamped rather than wrapped.
        let last = i16::from_le_bytes(wav[54..56].try_into().unwrap());
        assert_eq!(last, 32767);
    }

    #[test]
    fn every_sound_is_audible_and_finite() {
        for s in ALL {
            let samples = synth(s);
            assert!(samples.len() > 500, "{s:?} too short");
            assert!(samples.iter().all(|v| v.is_finite()), "{s:?} has NaN");
            let peak = samples.iter().fold(0.0f32, |m, v| m.max(v.abs()));
            assert!(peak > 0.05, "{s:?} is silent (peak {peak})");
        }
        let t = thrust_loop();
        assert!(t.iter().all(|v| v.is_finite()));
    }
}
