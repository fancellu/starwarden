# Starwarden

A side-scrolling planet-defence shooter written in Rust with [Bevy](https://bevyengine.org) 0.20.
It runs on the desktop and in the browser (WebAssembly), and plays with keyboard, gamepad or
touch.

**[Play it in your browser](https://starwarden-nine.vercel.app/)**

All art, terrain and sound effects are generated in code. There are no asset files.

![The game playing itself](docs/demo.gif)

| | |
| --- | --- |
| ![Title screen](docs/title.png) | ![A Harvester approaching](docs/combat.png) |
| ![A Hive bursting](docs/explosion.png) | ![Elite enemies in the void phase](docs/void.png) |

## How to play

You patrol a small planet that wraps around: keep flying in one direction and you come back to
where you started. The scanner at the top of the screen shows the whole world.

- **Harvesters** (green) descend, grab a colonist and carry them upward.
- Shoot the Harvester and the colonist falls. **Fly into them to catch them**, then fly low to
  set them down. A short fall is survivable; a long one is not.
- You can carry several colonists at once. Each one slows the ship a little, but they are set
  down together and each is worth 500 times the number set down (up to x5).
- A Harvester that reaches the top with a colonist becomes a **Wraith** (magenta): fast,
  erratic and aggressive.
- **Hives** (amber hexagons) take several hits and burst into a swarm of **Stingers**.
- Take too long over a wave and **Stalkers** (blue) come hunting.
- Lose every colonist and the planet is destroyed. You then fight three waves of red **elite**
  enemies in the void; survive them and the planet is rebuilt with new colonists.

Weapons: a rapid-fire laser, a limited stock of homing missiles (two more each wave) and smart
bombs that destroy everything on screen. Every 10,000 points earns an extra bomb, and every
20,000 an extra ship.

Destroyed enemies sometimes drop a **shield** (cyan hexagon; Hives most often). Catch it before
it reaches the ground. Each shield held absorbs one hit that would otherwise cost a ship, and
the ship glows while it has any. You can hold up to 9; catching one more scores 250 instead.

More rarely an enemy drops a **powerup**: a coin with a symbol, so each kind can be told apart
by shape as well as colour. Each lasts 10 seconds, and
several can run at once; catching one already running starts its timer again. Active powerups
and their seconds left are shown under the score. They are lost with the ship.

| Powerup      | Symbol          | Colour  | Effect                                              |
| ------------ | --------------- | ------- | --------------------------------------------------- |
| SPEED        | double chevron  | green   | 35% faster                                          |
| INVINCIBLE   | star            | amber   | 7 seconds of no damage; ram enemies to destroy them |
| WIDE BEAM    | three bars      | magenta | a much taller laser that passes through enemies     |
| SUPER SPRINT | lightning bolt  | blue    | sprint never runs out and is faster still           |
| TIME WARP    | hourglass       | violet  | enemies and their shots move at 40% speed           |
| MAGNET       | horseshoe       | red     | pulls in nearby pickups and colonists (see below)   |

The magnet's reach is shown as a dashed red circle around the ship. Falling shields, powerups
and colonists inside it are pulled straight to the ship, and a Harvester carrying a colonist
inside it loses them to the magnet. Colonists walking on the ground are left alone.

![A shield and the six powerups, each named](docs/powerups.png)

| | |
| --- | --- |
| ![An invincible ship ramming a Harvester](docs/invincible.png) | ![The wide beam hitting a Hive during a time warp](docs/widebeam.png) |

Kill quickly for a **combo**: every 5 kills in a row, each within 4 seconds of the last, raise
the score multiplier for enemies by one, up to x5. Losing your ship or pausing too long between
kills ends it.

Your **best score** and the date you set it are remembered and shown on the title screen. In
the browser they are kept in the site's local storage (clearing the site's data resets them).
The desktop version keeps them in `best.txt` in `%APPDATA%\Starwarden` on Windows,
`~/Library/Application Support/Starwarden` on macOS and `~/.local/share/starwarden` on Linux;
delete the file to reset. The self-playing demo never changes it.

## Controls

| Action     | Keyboard                | Gamepad                | Touch                   |
| ---------- | ----------------------- | ---------------------- | ----------------------- |
| Move       | Arrow keys or WASD      | Left stick or D-pad    | Drag on the left half   |
| Laser      | Space                   | A                      | FIRE button (hold)      |
| Missile    | X                       | X                      | MISSILE button          |
| Smart bomb | B                       | B                      | BOMB button             |
| Sprint     | Shift (hold)            | RB (hold)              | SPRINT button (hold)    |
| Pause      | P                       | Start                  | Pause button, top right |
| Start      | Space or Enter          | A or Start             | Tap anywhere            |

The touch controls appear after the first touch.

Sprint makes the ship 60% faster but runs on a meter: two seconds from full, refilling in
four. If it runs dry it cannot be used again until it is back to 30%. A small bar under the
ship shows the meter whenever it is not full.

## Run on the desktop

```sh
cargo run --release
```

The first build compiles Bevy and takes a few minutes. `cargo run` (without `--release`) also
works and is quicker to rebuild while changing the code.

## Run in the browser

One-time setup:

```sh
rustup target add wasm32-unknown-unknown
cargo install trunk
```

Develop with live reload at <http://localhost:8080>:

```sh
trunk serve
```

To try the touch controls on a phone on the same network:

```sh
trunk serve --address 0.0.0.0
```

and open `http://<your-computer's-ip>:8080` on the phone.

## Deploy to the web

```sh
trunk build --release
```

This writes a self-contained static site to `dist/` (`index.html`, one `.js` file and one
`.wasm` file). Upload the contents of that folder to any static host: GitHub Pages, Netlify,
Cloudflare Pages, itch.io (zip the folder) or your own server. Paths are relative, so it works
from a sub-folder too.

The web build uses WebGL2, which every current browser supports. Sound starts after the first
key press, click or tap, because browsers do not allow audio before that. Gamepad buttons do
not count, so when playing with a controller, click the page or press a key once; a hint at the
bottom of the screen says so while the sound is waiting.

### Graphics quality on phones

Phones often pair a very dense, fast screen with a modest processor and graphics chip, and the
game stutters if it runs flat out. The page therefore watches how the game is doing and turns
things down by itself.

If the game's code fills more than 40% of the time between frames, which happens on 120 Hz
phones, the game draws on every other screen refresh: 60 frames a second on a 120 Hz screen,
45 on a 90 Hz one. A steady rate looks smoother than an uneven 110 and leaves the processor
time to spare.

While the frame rate stays below 45 a second, it tries these steps in turn, keeping each one
that helps and undoing each one that does not:

1. Render at 900 pixels along the short side of the screen, stretched to fit.
2. "Lite" graphics: switch off the HDR and bloom passes that produce the glow.
3. Render at 720 pixels, then 540.

The result is remembered in the browser for the next visit.

To choose for yourself, add any of these to the address:

| Address ending     | Effect                                                    |
| ------------------ | --------------------------------------------------------- |
| `?fps=60`          | Draw on every other refresh (`?fps=full` never caps)      |
| `?res=720`         | Fixed resolution (any number, or `full`)                  |
| `?lite=1`          | Glow effects off (`?lite=0` forces them on)               |
| `?quality=reset`   | Forget the remembered result and start again from full    |
| `?debug=1`         | Show frame rate and timings on screen                     |

`fps`, `res` and `lite` each switch their part of the automatic adjustment off for that visit.
They can be combined, for example `?fps=60&res=540&lite=1&debug=1`. On the desktop, set the `STARWARDEN_LITE` environment
variable to see the lite graphics.

## Deploy to Vercel

The live version at <https://starwarden-nine.vercel.app/> is deployed this way. The repository
is set up so that Vercel builds the game itself:

- `vercel.json` tells Vercel to run `scripts/vercel-build.sh` and publish `dist/`.
- `scripts/vercel-build.sh` installs Rust and a prebuilt Trunk on the build machine, then runs
  `trunk build --release`.

Either import the Git repository at <https://vercel.com/new> (every push then deploys), or
deploy from this folder:

```sh
npx vercel          # preview deployment; asks you to log in and create the project first time
npx vercel --prod   # production deployment
```

No project settings need changing. Every deployment compiles Bevy from scratch, so expect the
build to take several minutes.

Vercel's build machines run Amazon Linux 2023. To test the build script in the same OS with
Docker (4 CPUs and 8 GB, like Vercel's standard build machine):

```sh
tar -c --exclude=./target --exclude=./dist --exclude=./.git . |
  docker run -i --rm --cpus 4 --memory 8g amazonlinux:2023 bash -c \
  'dnf install -y -q gcc tar gzip </dev/null && mkdir /app && cd /app && tar -x && bash scripts/vercel-build.sh'
```

## Project layout

For a guided tour of how the code works, see [docs/TUTORIAL.md](docs/TUTORIAL.md).

| File                | What it does                                                    |
| ------------------- | --------------------------------------------------------------- |
| `src/main.rs`       | App setup, camera and bloom, system ordering                    |
| `src/consts.rs`     | Tuning values, names and colours: the place to tweak the game   |
| `src/world.rs`      | Wrap-around maths, camera follow, screen shake, random numbers  |
| `src/input.rs`      | Keyboard, gamepad and touch merged into one input               |
| `src/player.rs`     | Ship flight model, death and respawn                            |
| `src/weapons.rs`    | Laser, homing missiles, smart bomb                              |
| `src/enemies.rs`    | Enemy behaviour, enemy shots, damage and scoring                |
| `src/colonists.rs`  | Walking, abduction, falling and rescue                          |
| `src/pickups.rs`    | Shields and powerups dropped by enemies: falling and catching   |
| `src/waves.rs`      | Wave contents and progression, the void phase, new games        |
| `src/terrain.rs`    | Planet surface height function and mesh                         |
| `src/background.rs` | Sky, nebula and parallax stars                                  |
| `src/fx.rs`         | Particles and score pop-ups                                     |
| `src/meshes.rs`     | Procedural ship, enemy and colonist shapes                      |
| `src/audio.rs`      | Sound synthesis and playback                                    |
| `src/hud.rs`        | Score display, scanner, banners, touch buttons                  |
| `src/menus.rs`      | Title, pause and game-over screens                              |
| `src/save.rs`       | Best score and date, kept in localStorage or a file             |
| `src/dev.rs`        | Desktop-only self-playing demo that saves screenshots           |

## Development

```sh
cargo test      # unit tests for the game rules, wrap maths, wave tables and audio encoding
cargo clippy
```

Self-playing demo with screenshots (desktop only), useful for checking a change visually:

```sh
STARWARDEN_DEMO=./shots cargo run
```

Optional extras: `STARWARDEN_DEMO_SECS=60` sets the run length, `STARWARDEN_DEMO_EVERY=0.08`
sets the seconds between screenshots (default 2.5), `STARWARDEN_DEMO_VOID=1` forces the void
phase, `STARWARDEN_DEMO_GALLERY=1` lines up every enemy type, and `STARWARDEN_DEMO_CARRY=1`
catches three falling colonists, carries them and sets them down together (use about 22 seconds).
`STARWARDEN_DEMO_POWER=1` drops each powerup in turn above the ship, one every 6 seconds (use
about 42 seconds); together with `STARWARDEN_DEMO_GALLERY=1` it lines up a shield and every
powerup, each named, 7 seconds in. The output folder must already exist.

The images in `docs/` were made this way. The animation is the frames of a run at 0.08 s
intervals joined with ffmpeg:

```sh
ffmpeg -framerate 12.5 -i shots/shot%03d.png -vf "fps=10,scale=600:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=128[p];[b][p]paletteuse=dither=bayer:bayer_scale=5" docs/demo.gif
```

## Known limits

- Sound effects only; there is no music.
