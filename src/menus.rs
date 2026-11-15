//! Title, pause and game-over screens and the transitions between game states.

use bevy::prelude::*;

use crate::consts::*;
use crate::hud::font;
use crate::input::PlayerInput;
use crate::state::*;

#[derive(Component)]
pub struct Blink;

#[derive(Component)]
pub struct TitleGlow;

fn overlay() -> Node {
    Node {
        position_type: PositionType::Absolute,
        width: percent(100),
        height: percent(100),
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        row_gap: px(14),
        ..default()
    }
}

fn line(text: impl Into<String>, size: f32, color: LinearRgba) -> impl Bundle {
    (
        Text::new(text),
        font(size),
        TextColor(col(color)),
        TextLayout::justify(Justify::Center),
    )
}

pub fn setup_title(mut commands: Commands, game: Res<Game>) {
    let dim = rgb(0.55, 0.65, 0.8);
    commands.spawn((
        DespawnOnExit(GameState::Title),
        overlay(),
        children![
            (TitleGlow, line(TITLE, 92.0, C_CYAN)),
            line("DEFEND THE COLONY", 22.0, C_MAGENTA),
            (Node {
                height: px(18),
                ..default()
            },),
            line(
                format!(
                    "{NAME_HARVESTER}S ABDUCT COLONISTS - SHOOT THEM AND CATCH WHO FALLS\n\
                     AN ESCAPED {NAME_HARVESTER} BECOMES A {NAME_WRAITH}\n\
                     {NAME_HIVE}S BURST INTO {NAME_STINGER}S - {NAME_STALKER}S HUNT THE SLOW\n\
                     LOSE EVERY COLONIST AND THE PLANET IS GONE"
                ),
                15.0,
                dim,
            ),
            (Node {
                height: px(6),
                ..default()
            },),
            line(
                "KEYS   ARROWS/WASD MOVE - SPACE LASER - SHIFT MISSILE - ENTER BOMB - P PAUSE\n\
                 PAD    STICK MOVE - A LASER - X MISSILE - B BOMB - START PAUSE\n\
                 TOUCH  LEFT THUMB STEERS - BUTTONS ON THE RIGHT",
                14.0,
                dim,
            ),
            (Node {
                height: px(18),
                ..default()
            },),
            (
                Blink,
                line("PRESS SPACE, (A) OR TAP TO START", 24.0, C_WHITE)
            ),
            line(format!("BEST {:07}", game.high), 16.0, C_AMBER),
            (
                Node {
                    position_type: PositionType::Absolute,
                    top: px(40),
                    ..default()
                },
                line(REPO_URL, 13.0, dim),
            ),
        ],
    ));
}

pub fn setup_paused(mut commands: Commands) {
    commands.spawn((
        DespawnOnExit(GameState::Paused),
        overlay(),
        BackgroundColor(Color::linear_rgba(0.0, 0.0, 0.02, 0.55)),
        children![
            line("PAUSED", 64.0, C_CYAN),
            (
                Blink,
                line("PRESS P, START OR TAP TO RESUME", 20.0, C_WHITE)
            ),
        ],
    ));
}

pub fn setup_game_over(mut commands: Commands, game: Res<Game>) {
    let best = game.score > 0 && game.score >= game.high;
    commands.spawn((
        DespawnOnExit(GameState::GameOver),
        overlay(),
        BackgroundColor(Color::linear_rgba(0.02, 0.0, 0.0, 0.45)),
        children![
            line("GAME OVER", 76.0, C_RED),
            line(
                format!("SCORE {:07}   WAVE {}", game.score, game.wave),
                26.0,
                C_WHITE
            ),
            line(if best { "NEW BEST SCORE" } else { "" }, 20.0, C_AMBER),
            (Node {
                height: px(14),
                ..default()
            },),
            (
                Blink,
                line("PRESS SPACE, (A) OR TAP TO PLAY AGAIN", 22.0, C_WHITE)
            ),
        ],
    ));
}

pub fn animate_menus(
    time: Res<Time>,
    mut blink: Query<&mut TextColor, (With<Blink>, Without<TitleGlow>)>,
    mut glow: Query<&mut TextColor, (With<TitleGlow>, Without<Blink>)>,
) {
    let t = time.elapsed_secs();
    for mut c in &mut blink {
        c.0.set_alpha(0.35 + 0.65 * (0.5 + 0.5 * (t * 4.0).sin()));
    }
    for mut c in &mut glow {
        c.0 = col(C_CYAN.mix(&C_MAGENTA, 0.5 + 0.5 * (t * 0.8).sin()));
    }
}

pub fn state_transitions(
    time: Res<Time>,
    input: Res<PlayerInput>,
    state: Res<State<GameState>>,
    mut next: ResMut<NextState<GameState>>,
    mut game: ResMut<Game>,
    mut sfx: MessageWriter<Sfx>,
) {
    match state.get() {
        GameState::Title => {
            if input.start {
                next.set(GameState::Playing);
            }
        }
        GameState::Playing => {
            if input.pause {
                sfx.write(Sfx::Blip);
                next.set(GameState::Paused);
            }
        }
        GameState::Paused => {
            if input.pause || input.start {
                sfx.write(Sfx::Blip);
                next.set(GameState::Playing);
            }
        }
        GameState::GameOver => {
            // A short delay so a held fire button does not skip the screen.
            game.over_time += time.delta_secs();
            if game.over_time > 1.2 && input.start {
                next.set(GameState::Playing);
            }
        }
    }
}
