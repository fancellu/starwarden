//! The cylindrical world: logical positions, wrap-around maths, camera and the shared RNG.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::consts::*;
use crate::player::PlayerState;

/// Logical position on the world cylinder. `x` is always kept in `[0, WORLD_W)`.
#[derive(Component, Clone, Copy, Default)]
pub struct Pos(pub Vec2);

#[derive(Component, Clone, Copy, Default)]
pub struct Vel(pub Vec2);

/// Everything that belongs to a single play-through and is removed when a new game starts.
#[derive(Component)]
pub struct GameEntity;

/// Camera position on the cylinder plus the current screen-shake amount.
#[derive(Resource, Default)]
pub struct GameCam {
    pub x: f32,
    pub shake: f32,
}

/// Size of the visible area in world units (depends on the window's aspect ratio).
#[derive(Resource)]
pub struct View {
    pub w: f32,
    pub h: f32,
}

impl Default for View {
    fn default() -> Self {
        Self {
            w: 1280.0,
            h: 720.0,
        }
    }
}

/// Wraps an absolute x coordinate onto the cylinder.
pub fn wrap(x: f32) -> f32 {
    x.rem_euclid(WORLD_W)
}

/// Shortest signed horizontal distance for a raw difference `to - from`.
pub fn wrap_delta(d: f32) -> f32 {
    (d + WORLD_W * 0.5).rem_euclid(WORLD_W) - WORLD_W * 0.5
}

/// Shortest vector from `from` to `to` on the cylinder.
pub fn delta(from: Vec2, to: Vec2) -> Vec2 {
    Vec2::new(wrap_delta(to.x - from.x), to.y - from.y)
}

/// Frame-rate independent smoothing factor.
pub fn smooth(rate: f32, dt: f32) -> f32 {
    1.0 - (-rate * dt).exp()
}

/// Small deterministic PRNG (xorshift64*), so no extra dependency is needed on wasm.
#[derive(Resource)]
pub struct Rng(pub u64);

impl Default for Rng {
    fn default() -> Self {
        Self(0x9E37_79B9_7F4A_7C15)
    }
}

impl Rng {
    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        (x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 32) as u32
    }
    /// Uniform in `[0, 1)`.
    pub fn f32(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32
    }
    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f32()
    }
    pub fn sign(&mut self) -> f32 {
        if self.next_u32() & 1 == 0 {
            1.0
        } else {
            -1.0
        }
    }
    pub fn index(&mut self, n: usize) -> usize {
        (self.next_u32() as usize) % n.max(1)
    }
    /// Random unit vector.
    pub fn dir(&mut self) -> Vec2 {
        Vec2::from_angle(self.range(0.0, std::f32::consts::TAU))
    }
}

pub fn update_view(window: Query<&Window, With<PrimaryWindow>>, mut view: ResMut<View>) {
    let Ok(win) = window.single() else {
        return;
    };
    let (w, h) = (win.width().max(1.0), win.height().max(1.0));
    let aspect = w / h;
    if aspect >= VIEW_MIN_W / VIEW_MIN_H {
        view.h = VIEW_MIN_H;
        view.w = VIEW_MIN_H * aspect;
    } else {
        view.w = VIEW_MIN_W;
        view.h = VIEW_MIN_W / aspect;
    }
}

/// Moves everything that has a velocity and keeps it on the cylinder. Enemies and their shots
/// move slower during a time warp.
pub fn integrate(
    time: Res<Time>,
    game: Res<crate::state::Game>,
    mut q: Query<(
        &mut Pos,
        &Vel,
        Has<crate::enemies::Enemy>,
        Has<crate::enemies::EShot>,
    )>,
) {
    let dt = time.delta_secs();
    let warp = game.enemy_time_scale();
    for (mut p, v, enemy, shot) in &mut q {
        let k = if enemy || shot { warp } else { 1.0 };
        p.0 += v.0 * dt * k;
        p.0.x = wrap(p.0.x);
    }
}

/// The camera looks ahead of the ship in the direction it is facing.
pub fn camera_follow(
    time: Res<Time>,
    ps: Res<PlayerState>,
    view: Res<View>,
    mut cam: ResMut<GameCam>,
) {
    let dt = time.delta_secs();
    // Leading by velocity as well cancels the smoothing lag at full speed.
    let lead = ps.facing * view.w * 0.2 + ps.vel.x * 0.28;
    let target = ps.pos.x + lead;
    let d = wrap_delta(target - cam.x);
    cam.x = wrap(cam.x + d * smooth(3.2, dt));
    // Never let the ship leave the screen, however fast it is going.
    let off = wrap_delta(ps.pos.x - cam.x);
    let limit = view.w * 0.36;
    if off.abs() > limit {
        cam.x = wrap(cam.x + off - limit * off.signum());
    }
}

/// Converts logical positions to render-space transforms relative to the camera. The camera
/// itself never moves horizontally, which is what makes the wrap seamless.
pub fn sync_transforms(cam: Res<GameCam>, mut q: Query<(&Pos, &mut Transform)>) {
    for (p, mut t) in &mut q {
        t.translation.x = wrap_delta(p.0.x - cam.x);
        t.translation.y = p.0.y;
    }
}

pub fn camera_shake(
    time: Res<Time>,
    mut cam: ResMut<GameCam>,
    mut rng: ResMut<Rng>,
    mut q: Query<&mut Transform, With<Camera2d>>,
) {
    let Ok(mut t) = q.single_mut() else {
        return;
    };
    cam.shake = (cam.shake - time.delta_secs() * 1.8 * (0.4 + cam.shake)).max(0.0);
    let amp = cam.shake.min(1.5) * 14.0;
    t.translation.x = rng.range(-1.0, 1.0) * amp;
    t.translation.y = rng.range(-1.0, 1.0) * amp;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_keeps_x_on_the_cylinder() {
        assert_eq!(wrap(0.0), 0.0);
        assert!((wrap(-1.0) - (WORLD_W - 1.0)).abs() < 1e-3);
        assert!((wrap(WORLD_W + 5.0) - 5.0).abs() < 1e-3);
        assert!((wrap(-3.0 * WORLD_W + 7.0) - 7.0).abs() < 1e-2);
    }

    #[test]
    fn wrap_delta_takes_the_short_way_round() {
        assert_eq!(wrap_delta(10.0), 10.0);
        assert_eq!(wrap_delta(-10.0), -10.0);
        // Going from x=50 to x=W-50 is 100 units to the left, not W-100 to the right.
        assert!((wrap_delta((WORLD_W - 50.0) - 50.0) + 100.0).abs() < 1e-3);
        assert!((wrap_delta(50.0 - (WORLD_W - 50.0)) - 100.0).abs() < 1e-3);
        for d in [-3.0 * WORLD_W, -0.7 * WORLD_W, 0.3 * WORLD_W, 2.6 * WORLD_W] {
            let r = wrap_delta(d);
            assert!((-WORLD_W * 0.5..WORLD_W * 0.5).contains(&r));
        }
    }

    #[test]
    fn rng_stays_in_range() {
        let mut r = Rng::default();
        for _ in 0..10_000 {
            let v = r.f32();
            assert!((0.0..1.0).contains(&v));
            let v = r.range(-3.0, 5.0);
            assert!((-3.0..5.0).contains(&v));
            assert!(r.index(7) < 7);
        }
    }
}
