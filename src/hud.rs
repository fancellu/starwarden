//! Score readouts, the scanner minimap, announcements, the screen flash and touch controls.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::colonists::{CState, Colonist};
use crate::consts::*;
use crate::enemies::{Enemy, Kind};
use crate::input::{page_zoom, stick_range, touch_layout, TouchState};
use crate::player::PlayerState;
use crate::state::*;
use crate::terrain;
use crate::world::*;

#[derive(Component)]
pub struct HudRoot;
#[derive(Component)]
pub struct ScoreText;
#[derive(Component)]
pub struct StatusText;
#[derive(Component)]
pub struct BannerText;
#[derive(Component)]
pub struct BannerSub;
#[derive(Component)]
pub struct FlashNode;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum TouchUi {
    StickBase,
    StickKnob,
    Fire,
    Missile,
    Bomb,
    Pause,
}

pub fn font(size: f32) -> TextFont {
    TextFont {
        font_size: FontSize::Px(size),
        ..default()
    }
}

pub fn setup_hud(mut commands: Commands) {
    // Screen flash sits under the rest of the UI.
    commands.spawn((
        FlashNode,
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            ..default()
        },
        BackgroundColor(Color::NONE),
        GlobalZIndex(-1),
    ));

    commands.spawn((
        HudRoot,
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            ..default()
        },
        Visibility::Hidden,
        children![
            (
                ScoreText,
                Text::new(""),
                font(22.0),
                TextColor(col(rgb(0.7, 0.95, 1.0))),
                Node {
                    position_type: PositionType::Absolute,
                    top: px(10),
                    left: px(14),
                    ..default()
                },
            ),
            (
                StatusText,
                Text::new(""),
                font(16.0),
                TextColor(col(rgb(1.0, 0.8, 0.95))),
                TextLayout::justify(Justify::Right),
                Node {
                    position_type: PositionType::Absolute,
                    top: px(10),
                    right: px(14),
                    ..default()
                },
            ),
            (
                Node {
                    position_type: PositionType::Absolute,
                    top: percent(30),
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: px(8),
                    ..default()
                },
                children![
                    (
                        BannerText,
                        Text::new(""),
                        font(54.0),
                        TextColor(Color::WHITE),
                    ),
                    (
                        BannerSub,
                        Text::new(""),
                        font(20.0),
                        TextColor(Color::WHITE),
                    ),
                ],
            ),
        ],
    ));

    // Touch controls: hidden until the first touch is seen.
    let ring = |kind: TouchUi, label: &str, color: LinearRgba| {
        (
            kind,
            Node {
                position_type: PositionType::Absolute,
                border: UiRect::all(px(2)),
                border_radius: BorderRadius::MAX,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BorderColor::all(col(with_alpha(color, 0.55))),
            BackgroundColor(col(with_alpha(color, 0.10))),
            Visibility::Hidden,
            GlobalZIndex(5),
            children![(
                Text::new(label),
                font(13.0),
                TextColor(col(with_alpha(color, 0.8))),
            )],
        )
    };
    commands.spawn(ring(TouchUi::StickBase, "", rgb(0.6, 0.9, 1.0)));
    commands.spawn(ring(TouchUi::StickKnob, "", rgb(0.8, 1.0, 1.0)));
    commands.spawn(ring(TouchUi::Fire, "FIRE", rgb(0.4, 1.0, 1.0)));
    commands.spawn(ring(TouchUi::Missile, "MISSILE", rgb(1.0, 0.7, 0.2)));
    commands.spawn(ring(TouchUi::Bomb, "BOMB", rgb(1.0, 0.3, 0.8)));
    commands.spawn(ring(TouchUi::Pause, "II", rgb(1.0, 1.0, 1.0)));
}

/// UI is laid out in world units, so the HUD scales with the play field on any screen.
pub fn update_ui_scale(
    view: Res<View>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut scale: ResMut<UiScale>,
) {
    if let Ok(win) = window.single() {
        let s = (win.height() / view.h).max(0.1);
        if (scale.0 - s).abs() > 1e-3 {
            scale.0 = s;
        }
    }
}

fn set_text(text: &mut Text, value: String) {
    if text.0 != value {
        text.0 = value;
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn update_hud(
    time: Res<Time>,
    state: Res<State<GameState>>,
    game: Res<Game>,
    ps: Res<PlayerState>,
    mut banner: ResMut<Banner>,
    mut flash: ResMut<Flash>,
    colonists: Query<(), With<Colonist>>,
    mut root: Query<&mut Visibility, With<HudRoot>>,
    mut score: Query<&mut Text, (With<ScoreText>, Without<StatusText>)>,
    mut status: Query<&mut Text, (With<StatusText>, Without<ScoreText>)>,
    mut banner_text: Query<
        (&mut Text, &mut TextColor),
        (
            With<BannerText>,
            Without<ScoreText>,
            Without<StatusText>,
            Without<BannerSub>,
        ),
    >,
    mut banner_sub: Query<
        (&mut Text, &mut TextColor),
        (
            With<BannerSub>,
            Without<ScoreText>,
            Without<StatusText>,
            Without<BannerText>,
        ),
    >,
    mut flash_node: Query<&mut BackgroundColor, With<FlashNode>>,
) {
    let dt = time.delta_secs();
    let show = *state.get() != GameState::Title;
    for mut v in &mut root {
        let want = if show {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *v != want {
            *v = want;
        }
    }

    if let Ok(mut t) = score.single_mut() {
        let combo = match game.multiplier() {
            1 => String::new(),
            m => format!("\nCOMBO x{m}"),
        };
        set_text(
            &mut t,
            format!("SCORE {:07}\nBEST  {:07}{combo}", game.score, game.high),
        );
    }
    if let Ok(mut t) = status.single_mut() {
        let people = if game.phase == Phase::Planet {
            let carrying = match ps.carried {
                0 => String::new(),
                n => format!("   CARRYING {n} (x{})", n.min(CARRY_BONUS_MAX)),
            };
            format!("COLONISTS {}{carrying}", colonists.iter().count())
        } else {
            format!("VOID - {} WAVES TO REBUILD", game.void_waves_left)
        };
        set_text(
            &mut t,
            format!(
                "WAVE {}\nSHIPS {}   BOMBS {}   MISSILES {}\n{}",
                game.wave, game.lives, game.bombs, game.missiles, people
            ),
        );
    }

    // Banner: fades in quickly, lingers, fades out.
    if *state.get() == GameState::Playing {
        banner.t = (banner.t - dt).max(0.0);
    }
    let a = if show {
        (banner.t / 0.6).min(1.0) * ((2.8 - banner.t) / 0.25).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let void = game.phase == Phase::Void;
    let main = if void { C_RED } else { C_CYAN };
    if let Ok((mut t, mut c)) = banner_text.single_mut() {
        set_text(&mut t, banner.text.clone());
        c.0 = col(with_alpha(main, a));
    }
    if let Ok((mut t, mut c)) = banner_sub.single_mut() {
        set_text(&mut t, banner.sub.clone());
        c.0 = col(with_alpha(rgb(1.0, 1.0, 1.0), a * 0.85));
    }

    flash.0 = (flash.0 - dt * 1.6).max(0.0);
    for mut bg in &mut flash_node {
        bg.0 = Color::linear_rgba(1.0, 1.0, 1.0, flash.0 * flash.0 * 0.85);
    }
}

/// Scanner minimap and the Harvesters' tractor beams, drawn with immediate-mode lines.
pub fn draw_overlays(
    mut gizmos: Gizmos,
    time: Res<Time>,
    state: Res<State<GameState>>,
    view: Res<View>,
    cam: Res<GameCam>,
    game: Res<Game>,
    ps: Res<PlayerState>,
    enemies: Query<(&Pos, &Enemy)>,
    colonists: Query<(&Pos, &Colonist)>,
) {
    if *state.get() == GameState::Title {
        return;
    }
    let t = time.elapsed_secs();
    let screen = |p: Vec2| Vec2::new(wrap_delta(p.x - cam.x), p.y);

    // Tractor beams.
    for (cpos, c) in &colonists {
        if let CState::Abducted(h) = c.state {
            if let Ok((hpos, _)) = enemies.get(h) {
                let a = 0.5 + 0.5 * (t * 30.0).sin();
                let color = col(with_alpha(C_GREEN, 0.35 + 0.4 * a));
                let top = screen(hpos.0) + Vec2::new(0.0, -10.0);
                let bottom = screen(cpos.0);
                gizmos.line_2d(
                    top + Vec2::new(-7.0, 0.0),
                    bottom + Vec2::new(-3.0, 0.0),
                    color,
                );
                gizmos.line_2d(
                    top + Vec2::new(7.0, 0.0),
                    bottom + Vec2::new(3.0, 0.0),
                    color,
                );
            }
        }
    }

    // --- Scanner ---
    let sw = (view.w * 0.36).min(480.0);
    let sh = 46.0;
    let center = Vec2::new(0.0, view.h * 0.5 - 12.0 - sh * 0.5);
    let map = |p: Vec2| {
        let x = wrap_delta(p.x - cam.x) / WORLD_W * sw;
        let y = ((p.y - VOID_FLOOR) / (TOP_Y - VOID_FLOOR)).clamp(0.0, 1.0) - 0.5;
        center + Vec2::new(x, y * (sh - 8.0))
    };
    let frame = if game.phase == Phase::Void {
        C_RED_DIM
    } else {
        C_CYAN_DIM
    };
    gizmos.rect_2d(center, Vec2::new(sw, sh), col(frame));
    // The part of the world currently on screen.
    gizmos.rect_2d(
        center,
        Vec2::new(view.w / WORLD_W * sw, sh + 6.0),
        col(with_alpha(C_WHITE, 0.5)),
    );

    if game.phase == Phase::Planet {
        const N: usize = 72;
        let pts = (0..=N).map(|i| {
            let x = cam.x + (i as f32 / N as f32 - 0.5) * WORLD_W * 0.999;
            map(Vec2::new(x, terrain::height(x)))
        });
        gizmos.linestrip_2d(pts, col(with_alpha(C_VIOLET_DIM, 0.9)));
    }

    let dash = |g: &mut Gizmos, p: Vec2, half: f32, c: LinearRgba| {
        g.line_2d(p - Vec2::new(half, 0.0), p + Vec2::new(half, 0.0), col(c));
    };
    for (pos, c) in &colonists {
        // Colonists in danger blink.
        let danger = !matches!(c.state, CState::Walking | CState::Carried);
        if danger && ((t * 8.0) as u32).is_multiple_of(2) {
            dash(&mut gizmos, map(pos.0), 2.5, C_WHITE);
        } else {
            dash(&mut gizmos, map(pos.0), 1.2, C_PEACH);
        }
    }
    for (pos, en) in &enemies {
        let c = en.kind.color(en.elite);
        let half = if en.kind == Kind::Stinger { 1.0 } else { 2.0 };
        dash(&mut gizmos, map(pos.0), half, c);
    }
    if ps.alive {
        let p = map(ps.pos);
        dash(&mut gizmos, p, 3.0, C_WHITE);
        gizmos.line_2d(
            p - Vec2::new(0.0, 2.5),
            p + Vec2::new(0.0, 2.5),
            col(C_WHITE),
        );
    }
}

pub fn update_touch_ui(
    ts: Res<TouchState>,
    scale: Res<UiScale>,
    state: Res<State<GameState>>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut q: Query<(&TouchUi, &mut Node, &mut Visibility, &mut BackgroundColor)>,
) {
    let Ok(win) = window.single() else {
        return;
    };
    let zoom = page_zoom(win.height());
    let range = stick_range(zoom);
    let lay = touch_layout(win.width(), win.height(), zoom);
    let in_game = *state.get() == GameState::Playing;
    // Node sizes are in scaled UI units; touch positions are in window pixels.
    let k = 1.0 / scale.0.max(0.01);
    for (kind, mut node, mut vis, mut bg) in &mut q {
        let (shape, pressed) = match kind {
            TouchUi::Fire => (Some(lay.fire), ts.fire),
            TouchUi::Missile => (Some(lay.missile), ts.missile),
            TouchUi::Bomb => (Some(lay.bomb), ts.bomb),
            TouchUi::Pause => (Some(lay.pause), false),
            TouchUi::StickBase => (ts.stick.map(|(_, o, _)| (o, range)), false),
            TouchUi::StickKnob => (
                ts.stick
                    .map(|(_, o, p)| (o + (p - o).clamp_length_max(range), range * 0.45)),
                true,
            ),
        };
        let want = match shape {
            Some(_) if ts.used && in_game => Visibility::Inherited,
            _ => Visibility::Hidden,
        };
        if *vis != want {
            *vis = want;
        }
        if let Some((c, r)) = shape {
            node.left = px((c.x - r) * k);
            node.top = px((c.y - r) * k);
            node.width = px(r * 2.0 * k);
            node.height = px(r * 2.0 * k);
        }
        let a = if pressed { 0.32 } else { 0.10 };
        if (bg.0.alpha() - a).abs() > 0.01 {
            bg.0.set_alpha(a);
        }
    }
}

pub fn setup_gizmos(mut store: ResMut<GizmoConfigStore>) {
    let (config, _) = store.config_mut::<DefaultGizmoConfigGroup>();
    config.line.width = 2.0;
    // Draw on top of the scene.
    config.depth_bias = -1.0;
}
