# How Starwarden works

A tour of the code for someone who knows some Rust and wants to understand, change or borrow
from the game. It explains the ideas and points at the files; it does not walk through every
line. The whole game is about 4,600 lines in `src/`.

If you are new to Bevy, the first section covers the handful of concepts the code relies on.
Otherwise skip to [The frame](#the-frame).

## Bevy in five minutes

Bevy is an *entity component system* (ECS). Three ideas carry most of the code:

- An **entity** is just an id. A **component** is a plain struct attached to an entity. An
  enemy is an entity with `Enemy`, `Pos`, `Vel`, a mesh and a transform.
- A **system** is an ordinary function. Its parameters say which data it wants, and Bevy
  supplies it each frame. `Query<(&mut Pos, &Vel)>` means "every entity that has both".
- A **resource** is a single global value, such as the score (`Res<Game>`) or this frame's
  input (`Res<PlayerInput>`).

Two more things appear everywhere:

- **Messages** let systems talk without knowing about each other. One system writes
  `Sfx::Laser`; the audio system reads it later in the frame and plays the sound.
- **States** (`GameState`) switch groups of systems on and off: title, playing, paused,
  game over.

All of this is wired together in one place, [`src/main.rs`](../src/main.rs).

## The frame

`main.rs` contains no game logic. It registers resources and messages, then lists every
system in the order it runs. Each frame goes through six steps, defined as the `Step` enum in
[`src/state.rs`](../src/state.rs):

| Step      | What happens                                                            |
| --------- | ----------------------------------------------------------------------- |
| `Input`   | Read keyboard, gamepad and touch; handle start and pause                |
| `Logic`   | Ship controls, weapons fire, wave spawning, enemy AI, colonists         |
| `Move`    | Add velocity to position for everything that moves                      |
| `Collide` | Shots against enemies, enemies and shots against the ship               |
| `Resolve` | Apply damage, kill and respawn the ship, move the camera                |
| `Present` | Particles, screen positions, HUD, sound                                 |

The fixed order matters. Collisions are tested after everything has moved, damage is applied
after all collisions are known, and nothing is drawn until the frame's outcome is settled.
That removes a whole class of "it depends which system ran first" bugs.

Two small functions at the bottom of `main.rs` decide what runs when:

- `playing` is true only in the `Playing` state. The ship's own systems use it.
- `simulating` is also true on the game-over screen, so enemies keep flying behind the
  "GAME OVER" text. When paused, neither is true and the world freezes.

## The wrap-around world

This is the idea most worth understanding, because everything else leans on it. It lives in
[`src/world.rs`](../src/world.rs).

The world is a cylinder 9,600 units around (`WORLD_W`). Fly right for long enough and you are
back where you started. Game objects do not store their position in Bevy's `Transform`. They
store it in a separate `Pos` component, whose x is always kept between 0 and `WORLD_W`.

Three helpers do all the wrap maths:

```rust
pub fn wrap(x: f32) -> f32            // put any x back onto the cylinder
pub fn wrap_delta(d: f32) -> f32      // shortest signed distance, going either way round
pub fn delta(from: Vec2, to: Vec2) -> Vec2   // shortest vector between two points
```

Every "how far is that" and "which way is that" question in the game goes through `delta`.
An enemy at x = 9,590 and a ship at x = 10 are 20 units apart, not 9,580, and `delta` says
so. If you add a feature and something behaves oddly at the seam, a plain subtraction that
should have been `delta` is the first thing to look for.

The trick that makes the seam invisible is that **the camera never moves sideways**. The
resource `GameCam` holds the camera's x on the cylinder, and one system places every object
relative to it:

```rust
pub fn sync_transforms(cam: Res<GameCam>, mut q: Query<(&Pos, &mut Transform)>) {
    for (p, mut t) in &mut q {
        t.translation.x = wrap_delta(p.0.x - cam.x);
        t.translation.y = p.0.y;
    }
}
```

An object just across the seam gets a small offset from the camera like any other neighbour,
so it is drawn in the right place with no special case. The terrain is one long mesh, so it
is drawn three times side by side, and the stars use their own parallax scrolling.

`world.rs` also holds the camera follow (it looks ahead of the ship), the screen shake, and a
tiny random number generator so the game needs no extra crate.

## One input for three devices

[`src/input.rs`](../src/input.rs) has a single system, `gather_input`, which reads the
keyboard, every connected gamepad and the touch screen, and merges them into one resource:

```rust
pub struct PlayerInput {
    pub mv: Vec2,       // movement, each axis from -1 to 1
    pub fire: bool,     // held
    pub missile: bool,  // pressed this frame
    pub bomb: bool,
    pub pause: bool,
    pub start: bool,    // "confirm" on menus
}
```

Nothing else in the game knows which device is in use. The ship reads `PlayerInput`, and so
does the self-playing demo in `dev.rs`, which simply overwrites it each frame.

Touch works as a virtual stick: the first finger down on the left half of the screen becomes
the stick's centre, and dragging away from it steers. The button positions come from
`touch_layout`, which the HUD also calls to draw them, so the drawn buttons and the touchable
areas cannot drift apart.

## The ship and its weapons

[`src/player.rs`](../src/player.rs) holds the flight model: thrust with inertia, a top speed,
and a facing direction that flips when you reverse. It keeps a copy of the ship's position,
velocity and facing in the `PlayerState` resource, so that enemies, the camera and the HUD
can read them without querying the ship entity. That resource stays valid while the ship is
dead and waiting to respawn.

[`src/weapons.rs`](../src/weapons.rs) has the three weapons. Laser bolts and missiles are
entities with `Pos` and `Vel`, moved by the shared `integrate` system. The smart bomb is not
an entity at all: it sends a `Damage` message to every enemy on screen.

Weapons never remove enemies themselves. They write a `Damage { entity, amount }` message,
and `apply_damage` in `enemies.rs` is where enemies are destroyed. Scoring, explosions and
the Hive bursting into Stingers therefore happen in one place, whatever did the damage:
laser, missile, smart bomb or ramming the ship. That is also where each kill feeds the combo
multiplier kept on `Game` (`register_kill`, `multiplier`); `run_waves` runs its timer down and
`player_death` ends it. It also rolls for a shield drop: [`src/pickups.rs`](../src/pickups.rs)
spawns a falling `ShieldPickup` that the ship can catch. When the ship is hit, `player_death`
spends a shield (and grants a moment of invulnerability) before it considers losing a ship.

## Enemies

[`src/enemies.rs`](../src/enemies.rs) is the largest file. All five enemy types share one
`Enemy` component with a `kind` field, and `enemy_ai` is a single system with a `match` on
that kind. This is simpler than one component and one system per enemy type, and it keeps
shared behaviour (warping in, shooting, the hit flash) in one loop.

| Kind        | Behaviour                                                                 |
| ----------- | ------------------------------------------------------------------------- |
| `Harvester` | Roams, picks a colonist, descends, grabs them and rises                   |
| `Wraith`    | Fast and erratic, homes on the ship                                       |
| `Hive`      | Drifts slowly, takes three hits, bursts into Stingers                     |
| `Stinger`   | Small, fast swarm unit                                                    |
| `Stalker`   | Arrives when a wave drags on, faster than the ship, fires aimed shots     |

The Harvester is the only one with a real state machine, the `Ai` enum: `Roam`, `Descend`,
`Rise`. If it reaches the top of the screen while carrying a colonist, the colonist is lost
and the Harvester is replaced by a newly spawned Wraith in the same place.

Every enemy also has an `elite` flag, used in the void phase. Elites are red, faster and
shoot more; the flag selects a second set of meshes and adjusts a few numbers. There is no
separate elite code path.

## Colonists and the rescue loop

[`src/colonists.rs`](../src/colonists.rs) is a small state machine per colonist:

```rust
pub enum CState {
    Walking,
    Abducted(Entity),  // held by this Harvester
    Falling,
    Carried,           // caught by the player, waiting to be set down
}
```

The interesting part is that a Harvester and its colonist are two entities that refer to each
other, and either can disappear at any time. The code handles this by having the colonist
check its captor every frame: if the Harvester in `Abducted(entity)` no longer exists, the
colonist starts `Falling`. Nothing has to notify it. The same approach covers a Harvester
whose target was grabbed by another, and a carried colonist whose ship was destroyed.

Carried colonists are counted at the start of `update_colonists`, so that they can all be set
down in the same frame with a bonus that grows with their number. The count is written to
`PlayerState::carried`, where the ship's controls read it to slow down and the HUD shows it.

A fall is survivable if it started low enough (`fall_from` records where it began). The
tests at the bottom of the file cover each of these cases.

## Waves and the void phase

[`src/waves.rs`](../src/waves.rs) decides what arrives and when. `wave_spec(wave, phase)` is
a pure function from a wave number to a count of each enemy, which makes the difficulty curve
easy to read and to test. Starting a wave fills a `WaveQueue` with `(time, kind)` pairs, and
`run_waves` spawns each one as its time comes, away from the ship.

`run_waves` also owns the big transitions:

- The wave is clear when the queue is empty and no enemies are left.
- If the last colonist is lost, the phase becomes `Phase::Void`: the terrain is hidden, the
  background turns red, and the next three waves are elites.
- Surviving those restores the planet with new colonists.

`new_game` runs when leaving the title or game-over screen. It removes every entity marked
`GameEntity` and spawns a fresh ship and colonists. Tagging everything that belongs to one
play-through with that marker is what makes restarting a one-liner.

## Graphics without asset files

There are no image or model files. [`src/meshes.rs`](../src/meshes.rs) builds every shape at
start-up from triangles, using a small `Shape` builder (`fan`, `poly`, `quad`, `circle`,
`ring`). Colour is stored per vertex, so every mesh in the game shares a single material.
The handles are kept in the `Art` resource.

The glow comes from the camera, set up in `main.rs`. It renders in HDR with bloom, and the
colours in [`src/consts.rs`](../src/consts.rs) are deliberately brighter than 1.0:

```rust
pub const C_CYAN: LinearRgba = rgb(0.1, 1.25, 2.1);
```

Anything above 1.0 spills light into its surroundings. That is the whole neon look: bright
shapes on a dark background, with bloom doing the rest.

The other visual pieces:

- [`src/terrain.rs`](../src/terrain.rs): `height(x)` is a sum of sine waves whose
  frequencies are whole numbers of cycles around the world, so the ground joins up with
  itself at the seam. Colonists and Harvesters call the same function to find the ground.
- [`src/background.rs`](../src/background.rs): a gradient sky, soft nebula sprites and three
  layers of stars that scroll at different speeds. Each star layer is baked into one mesh.
- [`src/fx.rs`](../src/fx.rs): particles and floating score numbers. Other systems request
  an effect by writing an `Fx` message (`Explosion`, `Spark`, `Trail`, `Score(150)`), so
  gameplay code never spawns particles directly.
- [`src/hud.rs`](../src/hud.rs): score text, banners and touch buttons use Bevy UI. The
  scanner and tractor beams are drawn with gizmos, Bevy's immediate-mode line drawing, since
  they change every frame.

### Why particles are not sprites

Particles, laser bolts and enemy shots are not Bevy sprites. Each is an entity with a `Glow`
component (colour, size, soft or solid), and one system, `draw_glows`, rebuilds a single mesh
from all of them every frame using the `GlowQuads` builder in `meshes.rs`.

The reason is performance. Bevy gives every sprite with its own colour or size a separate
material, so hundreds of particles that fade a little each frame meant hundreds of materials
and GPU buffers created and thrown away per frame, and one draw call each. Desktop machines
shrug that off; mid-range phones do not. One mesh means one upload and one draw call however
many particles there are. If you add a new kind of small glowing thing, give it a `Glow`
rather than a `Sprite`.

## Sound without asset files

[`src/audio.rs`](../src/audio.rs) synthesises every sound at start-up. `synth(Sfx)` builds a
buffer of samples from oscillators, noise and envelopes.

Playing a sound is then just a message. Any system writes `Sfx::ExplodeBig`, and it is played
at the end of the frame, at most once per effect per frame so that twenty simultaneous
explosions do not add up to a deafening one. The engine hum is a single looping sound whose
volume follows the thrust.

How the samples reach the speakers differs by platform, in two `backend` modules:

- **Desktop:** `encode_wav` wraps each buffer in a WAV header in memory, and the result is
  handed to Bevy's audio as if it had been loaded from a file.
- **Web:** the buffers are given to the browser's own Web Audio engine, and Bevy's audio is
  not compiled in at all. Bevy's web audio is mixed by the game's code on the page's main
  thread, only a few hundredths of a second ahead, so a slow frame makes the sound stutter.
  That is rare on a desktop and constant on a mid-range phone. The browser's engine runs on
  its own thread, so a slow frame can delay a sound but cannot break one up.

## Desktop and web from one codebase

Almost nothing in the Rust code is platform-specific. The differences are:

- `Msaa::Off` on the web, because WebGL2 cannot combine it with HDR.
- Sound playback, as described above.
- `dev.rs` (the self-playing demo) is compiled only on the desktop.
- `page_zoom` in `input.rs` asks the browser how large the game is drawn.
- `lite_graphics` in `main.rs` asks the page whether to drop the glow effects.

The web side is [`index.html`](../index.html), which Trunk turns into the deployable site.
It contains two pieces of JavaScript worth knowing about:

- Browsers refuse to play sound before the user interacts with the page, so a small shim
  resumes audio on the first key press or tap. Gamepad buttons do not count, so it shows a
  "click or press any key" hint while the sound is waiting.
- The page wraps `requestAnimationFrame`, the browser call the game uses to ask for each
  frame. That lets it time the game's code and, on busy phones, draw only every other screen
  refresh (60 frames a second at 120 Hz, 45 at 90 Hz).
- A frame-rate watchdog lowers the graphics quality on devices that cannot keep up. To
  lower the resolution it shrinks the element holding the canvas and stretches it back with
  a CSS transform, so the game simply sees a smaller window; `page_zoom` exists to keep the
  touch buttons thumb-sized when that happens. To drop the glow it sets
  `window.starwardenLite`, which `apply_graphics_mode` in `main.rs` checks every frame,
  removing or restoring the camera's HDR and bloom.
- Adding `?debug=1` to the address shows frame timings on screen.

## Tests and the demo

`cargo test` runs 22 tests. The simple ones check pure functions: the wrap maths, the terrain
joining at the seam, the wave table, the WAV header. The more useful ones, in `enemies.rs`
and `colonists.rs`, build a tiny Bevy app with no window, spawn a Harvester and a colonist,
step the systems for a few simulated seconds and check what happened. They cover the rules
that are easy to break: abduction turning into a Wraith, catching and setting down a
colonist, fatal and survivable falls.

For anything visual, the demo mode plays the game by itself and saves screenshots:

```sh
mkdir shots
STARWARDEN_DEMO=./shots cargo run
```

## Where to start changing things

- **Tuning** (speeds, lives, fire rates, colours, enemy names): `src/consts.rs`.
- **Difficulty curve**: `wave_spec` in `src/waves.rs`.
- **A new enemy**: add a variant to `Kind` in `src/enemies.rs` and let the compiler list the
  `match` statements to fill in (size, score, colour, behaviour). Add a mesh in
  `src/meshes.rs` and a count in `WaveSpec`.
- **A new sound**: add a variant to `Sfx` in `src/state.rs`, a recipe in `synth` and an entry
  in the `ALL` list in `src/audio.rs`.
- **A new effect**: add a variant to `FxKind` and handle it in `spawn_fx` in `src/fx.rs`.
