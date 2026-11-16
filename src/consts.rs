//! Tuning values, names and colours. Everything that is likely to be tweaked lives here.

use bevy::prelude::*;

pub const TITLE: &str = "STARWARDEN";
/// Shown on the title screen.
pub const REPO_URL: &str = "github.com/fancellu/starwarden";

// Display names (used in the title screen legend).
pub const NAME_HARVESTER: &str = "HARVESTER";
pub const NAME_WRAITH: &str = "WRAITH";
pub const NAME_HIVE: &str = "HIVE";
pub const NAME_STINGER: &str = "STINGER";
pub const NAME_STALKER: &str = "STALKER";

// --- View / world -----------------------------------------------------------------------------
/// The visible area is never smaller than this (in world units).
pub const VIEW_MIN_W: f32 = 960.0;
pub const VIEW_MIN_H: f32 = 720.0;
/// Circumference of the cylindrical world.
pub const WORLD_W: f32 = 9600.0;
/// Highest altitude anything in play can reach (the HUD and scanner sit above this).
pub const TOP_Y: f32 = 262.0;
/// Lowest altitude in the void phase, when there is no terrain.
pub const VOID_FLOOR: f32 = -335.0;
pub const GROUND_BASE: f32 = -282.0;

// --- Player -----------------------------------------------------------------------------------
pub const PLAYER_ACCEL: f32 = 2600.0;
pub const PLAYER_DRAG: f32 = 1.7;
pub const PLAYER_MAX_VX: f32 = 980.0;
pub const PLAYER_VY: f32 = 470.0;
pub const PLAYER_RADIUS: f32 = 12.0;
pub const START_LIVES: u32 = 3;
pub const START_BOMBS: u32 = 3;
pub const START_MISSILES: u32 = 4;
pub const MAX_MISSILES: u32 = 8;
pub const BONUS_EVERY: u32 = 10_000;
/// Seconds after a kill in which the next kill keeps the combo going.
pub const COMBO_WINDOW: f32 = 4.0;
/// Kills in a row needed for each step up of the score multiplier.
pub const COMBO_STEP: u32 = 5;
/// Highest score multiplier.
pub const COMBO_MAX: u32 = 5;
/// Fraction of top speed lost for each colonist carried.
pub const CARRY_SLOW: f32 = 0.08;
/// Slowest the ship gets, however many it carries (fraction of normal speed).
pub const CARRY_MIN_SPEED: f32 = 0.6;
/// Points for each colonist set down, multiplied by how many are set down together.
pub const SET_DOWN_POINTS: u32 = 500;
/// Highest set-down multiplier.
pub const CARRY_BONUS_MAX: u32 = 5;

// --- Weapons ----------------------------------------------------------------------------------
pub const LASER_COOLDOWN: f32 = 0.085;
pub const LASER_SPEED: f32 = 2400.0;
pub const LASER_LIFE: f32 = 0.42;
pub const LASER_MAX: usize = 8;
pub const LASER_HALF_LEN: f32 = 44.0;
pub const MISSILE_SPEED: f32 = 860.0;
pub const MISSILE_TURN: f32 = 5.5;
pub const MISSILE_LIFE: f32 = 2.6;
pub const MISSILE_BLAST: f32 = 85.0;

// --- Colonists --------------------------------------------------------------------------------
pub const COLONISTS: usize = 10;
/// A colonist dropped from higher than this above the ground does not survive the landing.
pub const SAFE_FALL: f32 = 130.0;
pub const FALL_SPEED: f32 = 230.0;

// --- Waves ------------------------------------------------------------------------------------
pub const VOID_WAVES: u32 = 3;
pub const STALKER_AFTER: f32 = 28.0;
pub const STALKER_EVERY: f32 = 9.0;
pub const MAX_PARTICLES: usize = 1400;

// --- Z layers ---------------------------------------------------------------------------------
pub const Z_SKY: f32 = -50.0;
pub const Z_NEBULA: f32 = -45.0;
pub const Z_STARS: f32 = -40.0;
pub const Z_TERRAIN: f32 = -10.0;
pub const Z_COLONIST: f32 = 2.0;
pub const Z_ENEMY: f32 = 5.0;
pub const Z_SHOT: f32 = 8.0;
pub const Z_PLAYER: f32 = 10.0;
pub const Z_PARTICLE: f32 = 12.0;
pub const Z_POPUP: f32 = 15.0;

// --- Colours (linear, values above 1.0 glow through the bloom pass) ---------------------------
pub const fn rgb(r: f32, g: f32, b: f32) -> LinearRgba {
    LinearRgba {
        red: r,
        green: g,
        blue: b,
        alpha: 1.0,
    }
}
pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> LinearRgba {
    LinearRgba {
        red: r,
        green: g,
        blue: b,
        alpha: a,
    }
}

pub const C_WHITE: LinearRgba = rgb(3.0, 3.0, 3.0);
pub const C_CYAN: LinearRgba = rgb(0.1, 1.25, 2.1);
pub const C_CYAN_DIM: LinearRgba = rgb(0.05, 0.45, 0.75);
pub const C_MAGENTA: LinearRgba = rgb(2.2, 0.1, 1.2);
pub const C_GREEN: LinearRgba = rgb(0.4, 2.6, 0.5);
pub const C_GREEN_DIM: LinearRgba = rgb(0.05, 0.6, 0.15);
pub const C_AMBER: LinearRgba = rgb(3.0, 1.5, 0.15);
pub const C_AMBER_DIM: LinearRgba = rgb(0.8, 0.3, 0.02);
pub const C_RED: LinearRgba = rgb(3.2, 0.3, 0.25);
pub const C_RED_DIM: LinearRgba = rgb(0.7, 0.05, 0.05);
pub const C_VIOLET: LinearRgba = rgb(1.3, 0.4, 3.0);
pub const C_VIOLET_DIM: LinearRgba = rgb(0.3, 0.06, 0.8);
pub const C_BLUE: LinearRgba = rgb(0.4, 0.9, 3.2);
pub const C_PEACH: LinearRgba = rgb(2.4, 1.6, 1.1);
pub const C_FLAME: LinearRgba = rgb(3.5, 1.4, 0.3);

pub fn col(c: LinearRgba) -> Color {
    Color::LinearRgba(c)
}
pub fn with_alpha(c: LinearRgba, a: f32) -> LinearRgba {
    LinearRgba { alpha: a, ..c }
}
