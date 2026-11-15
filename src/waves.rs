//! Wave composition and progression, the planet's destruction and restoration, and new games.

use bevy::prelude::*;

use crate::colonists::{spawn_colonists, Colonist};
use crate::consts::*;
use crate::enemies::{spawn_enemy, Enemy, Kind};
use crate::meshes::Art;
use crate::player::{spawn_player, PlayerState};
use crate::state::*;
use crate::terrain;
use crate::world::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaveSpec {
    pub harvesters: u32,
    pub wraiths: u32,
    pub hives: u32,
    pub stalkers: u32,
}

/// What a wave contains. Planet waves are built around Harvesters; void waves are all hunters.
pub fn wave_spec(wave: u32, phase: Phase) -> WaveSpec {
    let w = wave.max(1);
    match phase {
        Phase::Planet => WaveSpec {
            harvesters: (5 + w).min(14),
            wraiths: w.saturating_sub(2).min(4),
            hives: (w / 2).min(4),
            stalkers: 0,
        },
        Phase::Void => WaveSpec {
            harvesters: 3,
            wraiths: (5 + w / 2).min(10),
            hives: (2 + w / 3).min(5),
            stalkers: (1 + w / 4).min(3),
        },
    }
}

/// Enemies still to arrive in the current wave: `(time into the wave, kind)`, sorted by time.
#[derive(Resource, Default)]
pub struct WaveQueue(pub Vec<(f32, Kind)>);

fn fill_queue(queue: &mut WaveQueue, spec: WaveSpec) {
    queue.0.clear();
    // Harvesters arrive in squads a few seconds apart; the rest are sprinkled in between.
    for i in 0..spec.harvesters {
        queue.0.push((
            (i / 4) as f32 * 7.0 + (i % 4) as f32 * 0.25,
            Kind::Harvester,
        ));
    }
    for i in 0..spec.hives {
        queue.0.push((1.5 + i as f32 * 4.0, Kind::Hive));
    }
    for i in 0..spec.wraiths {
        queue.0.push((3.0 + i as f32 * 3.0, Kind::Wraith));
    }
    for i in 0..spec.stalkers {
        queue.0.push((8.0 + i as f32 * 6.0, Kind::Stalker));
    }
    queue.0.sort_by(|a, b| a.0.total_cmp(&b.0));
}

fn spawn_point(rng: &mut Rng, ps: &PlayerState, kind: Kind, phase: Phase) -> Vec2 {
    // Never on top of the player.
    let off = rng.range(520.0, WORLD_W * 0.5) * rng.sign();
    let x = wrap(ps.pos.x + off);
    let y = match (kind, phase) {
        (Kind::Harvester, Phase::Planet) => rng.range(150.0, TOP_Y - 20.0),
        _ => rng.range(-120.0, TOP_Y - 30.0),
    };
    Vec2::new(x, y)
}

#[allow(clippy::too_many_arguments)]
pub fn run_waves(
    time: Res<Time>,
    mut commands: Commands,
    art: Res<Art>,
    ps: Res<PlayerState>,
    mut game: ResMut<Game>,
    mut queue: ResMut<WaveQueue>,
    mut rng: ResMut<Rng>,
    mut banner: ResMut<Banner>,
    mut flash: ResMut<Flash>,
    mut cam: ResMut<GameCam>,
    enemies: Query<&Enemy>,
    colonists: Query<(), With<Colonist>>,
    mut fx: MessageWriter<Fx>,
    mut sfx: MessageWriter<Sfx>,
) {
    let dt = time.delta_secs();
    match game.wave_state {
        WaveState::Intermission(t) => {
            let t = t - dt;
            if t > 0.0 {
                game.wave_state = WaveState::Intermission(t);
                return;
            }
            fill_queue(&mut queue, wave_spec(game.wave, game.phase));
            game.wave_state = WaveState::Active;
            game.wave_time = 0.0;
            game.stalker_timer = 0.0;
        }
        WaveState::Active => {
            game.wave_time += dt;
            let elite = game.phase == Phase::Void;

            while queue.0.first().is_some_and(|(t, _)| *t <= game.wave_time) {
                let (_, kind) = queue.0.remove(0);
                let pos = spawn_point(&mut rng, &ps, kind, game.phase);
                spawn_enemy(&mut commands, &art, &mut rng, kind, elite, pos, true);
                fx.write(Fx::new(pos, kind.color(elite), FxKind::Warp));
            }

            // Dawdling brings out the Stalkers.
            if game.wave_time > STALKER_AFTER {
                game.stalker_timer -= dt;
                let stalkers = enemies.iter().filter(|e| e.kind == Kind::Stalker).count();
                if game.stalker_timer <= 0.0 && stalkers < 3 && ps.alive {
                    game.stalker_timer = STALKER_EVERY;
                    let pos =
                        Vec2::new(wrap(ps.pos.x + rng.sign() * 720.0), rng.range(-80.0, 200.0));
                    spawn_enemy(
                        &mut commands,
                        &art,
                        &mut rng,
                        Kind::Stalker,
                        elite,
                        pos,
                        true,
                    );
                    fx.write(Fx::new(pos, Kind::Stalker.color(elite), FxKind::Warp));
                    sfx.write(Sfx::StalkerWarn);
                }
            }

            // Losing the last colonist destroys the planet.
            if game.phase == Phase::Planet && colonists.is_empty() {
                game.phase = Phase::Void;
                game.void_waves_left = VOID_WAVES;
                banner.show("PLANET LOST", "THE VOID AWAITS - SURVIVE TO REBUILD");
                flash.0 = 1.0;
                cam.shake = 2.5;
                sfx.write(Sfx::PlanetLost);
                for i in 0..90 {
                    let x = wrap(cam.x + (i as f32 / 90.0 - 0.5) * 2200.0);
                    let p = Vec2::new(x, terrain::height(x) - rng.range(0.0, 40.0));
                    fx.write(Fx::new(p, C_MAGENTA, FxKind::PlanetChunk));
                }
                return;
            }

            if queue.0.is_empty() && enemies.is_empty() {
                // Wave cleared.
                let survivors = colonists.iter().count() as u32;
                let bonus = 100 * game.wave * survivors;
                if game.add_score(bonus) {
                    sfx.write(Sfx::ExtraLife);
                }
                game.missiles = (game.missiles + 2).min(MAX_MISSILES);
                game.wave += 1;
                game.wave_state = WaveState::Intermission(3.2);
                sfx.write(Sfx::WaveStart);

                let mut restored = false;
                if game.phase == Phase::Void {
                    game.void_waves_left = game.void_waves_left.saturating_sub(1);
                    if game.void_waves_left == 0 {
                        game.phase = Phase::Planet;
                        spawn_colonists(&mut commands, &art, &mut rng);
                        flash.0 = 0.7;
                        restored = true;
                    }
                }
                if restored {
                    banner.show("PLANET RESTORED", format!("WAVE {}", game.wave));
                } else if game.phase == Phase::Void {
                    banner.show(
                        format!("WAVE {}", game.wave),
                        format!("VOID - {} TO GO", game.void_waves_left),
                    );
                } else {
                    banner.show(
                        format!("WAVE {}", game.wave),
                        format!("COLONIST BONUS {bonus}"),
                    );
                }
            }
        }
    }
}

/// Clears the previous run and sets up a new one.
pub fn new_game(
    mut commands: Commands,
    art: Res<Art>,
    time: Res<Time>,
    mut game: ResMut<Game>,
    mut ps: ResMut<PlayerState>,
    mut queue: ResMut<WaveQueue>,
    mut rng: ResMut<Rng>,
    mut cam: ResMut<GameCam>,
    mut banner: ResMut<Banner>,
    old: Query<Entity, With<GameEntity>>,
    mut sfx: MessageWriter<Sfx>,
) {
    for e in &old {
        commands.entity(e).try_despawn();
    }
    // Mix the time the player pressed start into the seed so runs differ.
    rng.0 ^= (time.elapsed_secs_f64() * 1_000_003.0) as u64 | 1;
    rng.next_u32();

    game.reset();
    queue.0.clear();
    *ps = PlayerState {
        alive: true,
        ..default()
    };
    cam.x = ps.pos.x;
    cam.shake = 0.0;
    spawn_player(&mut commands, &art, ps.pos, 1.0, 2.0);
    spawn_colonists(&mut commands, &art, &mut rng);
    banner.show("WAVE 1", "PROTECT THE COLONISTS");
    sfx.write(Sfx::WaveStart);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waves_escalate_but_stay_bounded() {
        let first = wave_spec(1, Phase::Planet);
        assert_eq!(first.harvesters, 6);
        assert_eq!(first.wraiths, 0);
        assert_eq!(first.stalkers, 0);
        let mut prev = 0;
        for w in 1..200 {
            for phase in [Phase::Planet, Phase::Void] {
                let s = wave_spec(w, phase);
                let total = s.harvesters + s.wraiths + s.hives + s.stalkers;
                assert!(total > 0 && total <= 24, "wave {w} {phase:?}: {total}");
            }
            let s = wave_spec(w, Phase::Planet);
            let total = s.harvesters + s.wraiths + s.hives;
            assert!(total >= prev, "planet waves never get easier");
            prev = total;
        }
    }

    #[test]
    fn void_waves_send_hunters() {
        let s = wave_spec(4, Phase::Void);
        assert!(s.stalkers >= 1);
        assert!(s.wraiths > s.harvesters);
    }

    #[test]
    fn queue_is_sorted_and_complete() {
        let mut q = WaveQueue::default();
        let spec = wave_spec(7, Phase::Planet);
        fill_queue(&mut q, spec);
        let n = (spec.harvesters + spec.wraiths + spec.hives + spec.stalkers) as usize;
        assert_eq!(q.0.len(), n);
        assert!(q.0.windows(2).all(|w| w[0].0 <= w[1].0));
    }
}
