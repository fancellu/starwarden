//! Game states, the per-run scoreboard and the messages systems use to talk to each other.

use bevy::prelude::*;

use crate::consts::*;

#[derive(States, Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum GameState {
    #[default]
    Title,
    Playing,
    Paused,
    GameOver,
}

/// Ordering of the per-frame work.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Step {
    Input,
    Logic,
    Move,
    Collide,
    Resolve,
    Present,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    /// The planet is intact and there are colonists to protect.
    Planet,
    /// Every colonist was lost: the planet is gone and the enemy sends its elites.
    Void,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum WaveState {
    /// Counting down to the next wave.
    Intermission(f32),
    Active,
}

#[derive(Resource)]
pub struct Game {
    pub score: u32,
    pub high: u32,
    pub lives: u32,
    pub bombs: u32,
    pub missiles: u32,
    pub wave: u32,
    pub phase: Phase,
    pub void_waves_left: u32,
    pub next_bonus: u32,
    pub wave_state: WaveState,
    pub wave_time: f32,
    pub stalker_timer: f32,
    /// Seconds since the game-over screen appeared (used to ignore held buttons).
    pub over_time: f32,
}

impl Default for Game {
    fn default() -> Self {
        Self {
            score: 0,
            high: 0,
            lives: START_LIVES,
            bombs: START_BOMBS,
            missiles: START_MISSILES,
            wave: 1,
            phase: Phase::Planet,
            void_waves_left: 0,
            next_bonus: BONUS_EVERY,
            wave_state: WaveState::Intermission(2.5),
            wave_time: 0.0,
            stalker_timer: 0.0,
            over_time: 0.0,
        }
    }
}

impl Game {
    /// Starts a fresh run, keeping the high score.
    pub fn reset(&mut self) {
        *self = Self {
            high: self.high,
            ..Self::default()
        };
    }

    /// Adds points; returns true when the score crossed a bonus threshold.
    pub fn add_score(&mut self, pts: u32) -> bool {
        self.score += pts;
        self.high = self.high.max(self.score);
        if self.score >= self.next_bonus {
            self.next_bonus += BONUS_EVERY;
            self.lives += 1;
            self.bombs += 1;
            true
        } else {
            false
        }
    }
}

/// Large centre-screen announcement that fades out.
#[derive(Resource, Default)]
pub struct Banner {
    pub text: String,
    pub sub: String,
    pub t: f32,
}

impl Banner {
    pub fn show(&mut self, text: impl Into<String>, sub: impl Into<String>) {
        self.text = text.into();
        self.sub = sub.into();
        self.t = 2.8;
    }
}

#[derive(Message, Clone, Copy)]
pub struct Damage {
    pub entity: Entity,
    pub amount: i32,
}

#[derive(Message, Clone, Copy)]
pub struct PlayerHit;

#[derive(Message, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sfx {
    Laser,
    Missile,
    Hit,
    ExplodeSmall,
    ExplodeBig,
    Alarm,
    Rescue,
    Landed,
    ColonistLost,
    Mutate,
    Bomb,
    StalkerWarn,
    PlayerDie,
    WaveStart,
    Blip,
    ExtraLife,
    EnemyShot,
    PlanetLost,
}

#[derive(Clone, Copy)]
pub enum FxKind {
    /// Enemy destroyed.
    Explosion,
    /// Player ship destroyed.
    BigExplosion,
    /// Shot impact.
    Spark,
    /// Enemy warping in.
    Warp,
    /// Ring of sparkles when a colonist is caught or set down.
    Rescue,
    /// Engine exhaust; the vector is the exhaust velocity.
    Trail(Vec2),
    /// Missile smoke.
    Smoke,
    /// Chunks of the planet breaking apart.
    PlanetChunk,
    /// Floating score text.
    Score(u32),
}

#[derive(Message, Clone, Copy)]
pub struct Fx {
    pub pos: Vec2,
    pub color: LinearRgba,
    pub kind: FxKind,
}

impl Fx {
    pub fn new(pos: Vec2, color: LinearRgba, kind: FxKind) -> Self {
        Self { pos, color, kind }
    }
}

/// Full-screen white flash (0..1), used by the smart bomb and the planet's destruction.
#[derive(Resource, Default)]
pub struct Flash(pub f32);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bonus_is_awarded_once_per_threshold() {
        let mut g = Game::default();
        assert!(!g.add_score(BONUS_EVERY - 1));
        assert!(g.add_score(1));
        assert_eq!(g.lives, START_LIVES + 1);
        assert_eq!(g.bombs, START_BOMBS + 1);
        assert!(!g.add_score(10));
        assert!(g.add_score(BONUS_EVERY));
        assert_eq!(g.lives, START_LIVES + 2);
    }

    #[test]
    fn reset_keeps_the_high_score() {
        let mut g = Game::default();
        g.add_score(1234);
        g.lives = 0;
        g.reset();
        assert_eq!(g.score, 0);
        assert_eq!(g.high, 1234);
        assert_eq!(g.lives, START_LIVES);
    }
}
