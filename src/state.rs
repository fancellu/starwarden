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
    /// Enemies destroyed in a row, each within `COMBO_WINDOW` of the last.
    pub combo: u32,
    /// Seconds left to make the next kill before the combo ends.
    pub combo_timer: f32,
    /// Hits the ship can take without being destroyed.
    pub shields: u32,
    /// Seconds left on each powerup, indexed by `Power as usize`.
    pub powers: [f32; Power::COUNT],
}

/// Timed powerups dropped by destroyed enemies. Several can run at once.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Power {
    Speed,
    Invincible,
    WideBeam,
    SuperSprint,
    TimeWarp,
}

impl Power {
    pub const COUNT: usize = 5;
    pub const ALL: [Power; Power::COUNT] = [
        Power::Speed,
        Power::Invincible,
        Power::WideBeam,
        Power::SuperSprint,
        Power::TimeWarp,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Power::Speed => "SPEED",
            Power::Invincible => "INVINCIBLE",
            Power::WideBeam => "WIDE BEAM",
            Power::SuperSprint => "SUPER SPRINT",
            Power::TimeWarp => "TIME WARP",
        }
    }

    pub fn color(self) -> LinearRgba {
        match self {
            Power::Speed => C_GREEN,
            Power::Invincible => C_AMBER,
            Power::WideBeam => C_MAGENTA,
            Power::SuperSprint => C_BLUE,
            Power::TimeWarp => C_VIOLET,
        }
    }

    pub fn duration(self) -> f32 {
        if self == Power::Invincible {
            POWER_INVINCIBLE_TIME
        } else {
            POWER_TIME
        }
    }
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
            combo: 0,
            combo_timer: 0.0,
            shields: 0,
            powers: [0.0; Power::COUNT],
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
            if self.next_bonus.is_multiple_of(SHIP_BONUS_EVERY) {
                self.lives += 1;
            }
            self.next_bonus += BONUS_EVERY;
            self.bombs += 1;
            true
        } else {
            false
        }
    }

    /// Score multiplier for enemy kills: x2 after `COMBO_STEP` quick kills, up to `COMBO_MAX`.
    pub fn multiplier(&self) -> u32 {
        (1 + self.combo / COMBO_STEP).min(COMBO_MAX)
    }

    /// Counts an enemy kill towards the combo; returns true when the multiplier went up.
    pub fn register_kill(&mut self) -> bool {
        let before = self.multiplier();
        self.combo += 1;
        self.combo_timer = COMBO_WINDOW;
        self.multiplier() > before
    }

    /// Runs the combo timer down; the combo ends when it reaches zero.
    pub fn tick_combo(&mut self, dt: f32) {
        if self.combo == 0 {
            return;
        }
        self.combo_timer -= dt;
        if self.combo_timer <= 0.0 {
            self.break_combo();
        }
    }

    pub fn break_combo(&mut self) {
        self.combo = 0;
        self.combo_timer = 0.0;
    }

    /// Adds a shield; returns false when the ship already holds `SHIELD_MAX`.
    pub fn add_shield(&mut self) -> bool {
        if self.shields >= SHIELD_MAX {
            return false;
        }
        self.shields += 1;
        true
    }

    pub fn power_on(&self, p: Power) -> bool {
        self.powers[p as usize] > 0.0
    }

    /// Starts a powerup, or refills its timer if it is already running.
    pub fn grant_power(&mut self, p: Power) {
        self.powers[p as usize] = p.duration();
    }

    /// Runs the powerup timers down; returns true when one ran out this step.
    pub fn tick_powers(&mut self, dt: f32) -> bool {
        let mut expired = false;
        for t in &mut self.powers {
            if *t > 0.0 {
                *t -= dt;
                if *t <= 0.0 {
                    *t = 0.0;
                    expired = true;
                }
            }
        }
        expired
    }

    pub fn clear_powers(&mut self) {
        self.powers = [0.0; Power::COUNT];
    }

    /// Speed that enemies and their shots run at (slower during a time warp).
    pub fn enemy_time_scale(&self) -> f32 {
        if self.power_on(Power::TimeWarp) {
            TIME_WARP
        } else {
            1.0
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
    ShieldUp,
    ShieldHit,
    Sprint,
    PowerUp,
    PowerDown,
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
    /// Floating word, such as the name of a powerup just caught.
    Label(&'static str),
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
        assert_eq!(g.lives, START_LIVES, "no ship at the first bonus");
        assert_eq!(g.bombs, START_BOMBS + 1);
        assert!(!g.add_score(10));
        assert!(g.add_score(BONUS_EVERY));
        assert_eq!(g.lives, START_LIVES + 1, "a ship every second bonus");
        assert_eq!(g.bombs, START_BOMBS + 2);
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

    #[test]
    fn quick_kills_raise_the_multiplier_up_to_the_cap() {
        let mut g = Game::default();
        assert_eq!(g.multiplier(), 1);
        for _ in 0..COMBO_STEP - 1 {
            assert!(!g.register_kill());
        }
        assert!(g.register_kill());
        assert_eq!(g.multiplier(), 2);
        for _ in 0..100 {
            g.register_kill();
        }
        assert_eq!(g.multiplier(), COMBO_MAX);
    }

    #[test]
    fn combo_ends_when_the_window_passes() {
        let mut g = Game::default();
        for _ in 0..COMBO_STEP {
            g.register_kill();
        }
        g.tick_combo(COMBO_WINDOW * 0.5);
        assert_eq!(g.multiplier(), 2);
        g.register_kill();
        g.tick_combo(COMBO_WINDOW * 0.9);
        assert_eq!(g.multiplier(), 2, "a kill refills the window");
        g.tick_combo(COMBO_WINDOW * 0.2);
        assert_eq!(g.multiplier(), 1);
        assert_eq!(g.combo, 0);
    }

    #[test]
    fn being_hit_or_a_new_game_ends_the_combo() {
        let mut g = Game::default();
        for _ in 0..COMBO_STEP * 2 {
            g.register_kill();
        }
        g.break_combo();
        assert_eq!(g.multiplier(), 1);
        for _ in 0..COMBO_STEP {
            g.register_kill();
        }
        g.reset();
        assert_eq!(g.multiplier(), 1);
    }

    #[test]
    fn shields_stop_at_the_maximum_and_reset_with_a_new_game() {
        let mut g = Game::default();
        for _ in 0..SHIELD_MAX {
            assert!(g.add_shield());
        }
        assert!(!g.add_shield());
        assert_eq!(g.shields, SHIELD_MAX);
        g.reset();
        assert_eq!(g.shields, 0);
    }

    #[test]
    fn powers_run_out_refill_and_stack() {
        let mut g = Game::default();
        assert_eq!(g.enemy_time_scale(), 1.0);
        g.grant_power(Power::TimeWarp);
        g.grant_power(Power::Speed);
        assert!(g.power_on(Power::TimeWarp) && g.power_on(Power::Speed));
        assert_eq!(g.enemy_time_scale(), TIME_WARP);
        assert!(!g.tick_powers(POWER_TIME - 1.0));
        g.grant_power(Power::Speed);
        assert!(g.tick_powers(2.0), "time warp ran out");
        assert!(!g.power_on(Power::TimeWarp));
        assert!(g.power_on(Power::Speed), "refilled, so still running");
        g.clear_powers();
        assert!(!g.power_on(Power::Speed));
        g.grant_power(Power::Invincible);
        g.reset();
        assert!(!g.power_on(Power::Invincible));
    }
}
