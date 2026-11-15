//! Keyboard, gamepad and touch are all folded into one `PlayerInput` that gameplay reads.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

/// The merged input for this frame.
#[derive(Resource, Default)]
pub struct PlayerInput {
    /// Movement, each axis in `[-1, 1]`; +y is up.
    pub mv: Vec2,
    /// Held.
    pub fire: bool,
    /// Pressed this frame.
    pub missile: bool,
    pub bomb: bool,
    pub pause: bool,
    /// "Confirm" on menus: fire, enter, a pad face button or a tap.
    pub start: bool,
}

/// On-screen touch controls, positioned in logical window pixels (origin top-left).
#[derive(Clone, Copy)]
pub struct TouchLayout {
    pub fire: (Vec2, f32),
    pub missile: (Vec2, f32),
    pub bomb: (Vec2, f32),
    pub pause: (Vec2, f32),
}

const STICK_RANGE: f32 = 56.0;

/// How much the web page magnifies the canvas. On phones the page may render the game at a
/// reduced resolution and stretch it (see `index.html`), which makes one window pixel larger
/// than one CSS pixel. Touch controls are sized for thumbs, so they divide this back out.
pub fn page_zoom(window_height: f32) -> f32 {
    #[cfg(target_arch = "wasm32")]
    {
        let page = web_sys::window()
            .and_then(|w| w.inner_height().ok())
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0) as f32;
        if page > 0.0 && window_height > 0.0 {
            return (page / window_height).clamp(1.0, 8.0);
        }
    }
    let _ = window_height;
    1.0
}

/// Stick travel in window pixels.
pub fn stick_range(zoom: f32) -> f32 {
    STICK_RANGE / zoom
}

/// Button centres and radii in window pixels, for a window of `w` x `h` magnified by `zoom`.
pub fn touch_layout(w: f32, h: f32, zoom: f32) -> TouchLayout {
    // Lay out in on-screen (CSS) pixels, then convert back to window pixels.
    let (w, h) = (w * zoom, h * zoom);
    // Scale the controls down a little on small screens.
    let s = (w.min(h) / 420.0).clamp(0.75, 1.3);
    let at = |x: f32, y: f32, r: f32| (Vec2::new(x, y) / zoom, r / zoom);
    TouchLayout {
        fire: at(w - 84.0 * s, h - 84.0 * s, 58.0 * s),
        missile: at(w - 204.0 * s, h - 64.0 * s, 42.0 * s),
        bomb: at(w - 72.0 * s, h - 204.0 * s, 42.0 * s),
        pause: at(w - 36.0, 112.0, 24.0),
    }
}

#[derive(Resource, Default)]
pub struct TouchState {
    /// Set once the first touch is seen; the on-screen controls are shown from then on.
    pub used: bool,
    /// Id of the touch driving the virtual stick, with its origin and current position.
    pub stick: Option<(u64, Vec2, Vec2)>,
    pub fire: bool,
    pub missile: bool,
    pub bomb: bool,
}

fn inside(p: Vec2, c: (Vec2, f32)) -> bool {
    // Slightly generous hit area: thumbs are imprecise.
    p.distance(c.0) <= c.1 * 1.15
}

pub fn gather_input(
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    touches: Res<Touches>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut input: ResMut<PlayerInput>,
    mut ts: ResMut<TouchState>,
) {
    let mut i = PlayerInput::default();

    // --- Keyboard ---
    let key = |codes: &[KeyCode]| codes.iter().any(|c| keys.pressed(*c));
    let key_down = |codes: &[KeyCode]| codes.iter().any(|c| keys.just_pressed(*c));
    if key(&[KeyCode::ArrowLeft, KeyCode::KeyA]) {
        i.mv.x -= 1.0;
    }
    if key(&[KeyCode::ArrowRight, KeyCode::KeyD]) {
        i.mv.x += 1.0;
    }
    if key(&[KeyCode::ArrowUp, KeyCode::KeyW]) {
        i.mv.y += 1.0;
    }
    if key(&[KeyCode::ArrowDown, KeyCode::KeyS]) {
        i.mv.y -= 1.0;
    }
    i.fire |= key(&[KeyCode::Space, KeyCode::KeyJ, KeyCode::KeyZ]);
    i.missile |= key_down(&[
        KeyCode::ShiftLeft,
        KeyCode::ShiftRight,
        KeyCode::KeyX,
        KeyCode::KeyK,
    ]);
    i.bomb |= key_down(&[KeyCode::KeyB, KeyCode::Enter, KeyCode::KeyL]);
    i.pause |= key_down(&[KeyCode::KeyP, KeyCode::Escape]);
    i.start |= key_down(&[KeyCode::Space, KeyCode::Enter]);

    // --- Gamepads (any connected pad) ---
    for pad in &gamepads {
        let axis = |a: GamepadAxis| {
            let v = pad.get(a).unwrap_or(0.0);
            if v.abs() < 0.2 {
                0.0
            } else {
                v
            }
        };
        i.mv.x += axis(GamepadAxis::LeftStickX);
        i.mv.y += axis(GamepadAxis::LeftStickY);
        if pad.pressed(GamepadButton::DPadLeft) {
            i.mv.x -= 1.0;
        }
        if pad.pressed(GamepadButton::DPadRight) {
            i.mv.x += 1.0;
        }
        if pad.pressed(GamepadButton::DPadUp) {
            i.mv.y += 1.0;
        }
        if pad.pressed(GamepadButton::DPadDown) {
            i.mv.y -= 1.0;
        }
        i.fire |= pad.pressed(GamepadButton::South) || pad.pressed(GamepadButton::RightTrigger2);
        i.missile |= pad.just_pressed(GamepadButton::West)
            || pad.just_pressed(GamepadButton::LeftTrigger2)
            || pad.just_pressed(GamepadButton::RightTrigger);
        i.bomb |= pad.just_pressed(GamepadButton::East)
            || pad.just_pressed(GamepadButton::North)
            || pad.just_pressed(GamepadButton::LeftTrigger);
        i.pause |= pad.just_pressed(GamepadButton::Start);
        i.start |= pad.just_pressed(GamepadButton::South) || pad.just_pressed(GamepadButton::Start);
    }

    // --- Touch ---
    if let Ok(win) = window.single() {
        let (w, h) = (win.width(), win.height());
        let zoom = page_zoom(h);
        let range = stick_range(zoom);
        let lay = touch_layout(w, h, zoom);
        ts.fire = false;
        ts.missile = false;
        ts.bomb = false;

        if touches.any_just_pressed() {
            ts.used = true;
        }
        for t in touches.iter_just_pressed() {
            let p = t.position();
            if inside(p, lay.pause) {
                i.pause = true;
            } else if inside(p, lay.missile) {
                i.missile = true;
                ts.missile = true;
            } else if inside(p, lay.bomb) {
                i.bomb = true;
                ts.bomb = true;
            } else {
                i.start = true;
                // A new touch on the left side of the screen grabs the stick.
                if ts.stick.is_none() && p.x < w * 0.5 {
                    ts.stick = Some((t.id(), p, p));
                }
            }
        }
        if let Some((id, origin, _)) = ts.stick {
            match touches.get_pressed(id) {
                Some(t) => {
                    let mut origin = origin;
                    let mut d = t.position() - origin;
                    // The stick base follows the thumb once it is dragged past its range.
                    if d.length() > range * 1.6 {
                        origin = t.position() - d.normalize() * range * 1.6;
                        d = t.position() - origin;
                    }
                    ts.stick = Some((id, origin, t.position()));
                    let v = (d / range).clamp_length_max(1.0);
                    let dead = |x: f32| if x.abs() < 0.18 { 0.0 } else { x };
                    i.mv.x += dead(v.x);
                    i.mv.y -= dead(v.y);
                }
                None => ts.stick = None,
            }
        }
        for t in touches.iter() {
            if Some(t.id()) == ts.stick.map(|s| s.0) {
                continue;
            }
            if inside(t.position(), lay.fire) {
                i.fire = true;
                ts.fire = true;
            }
        }
    }

    i.mv = Vec2::new(i.mv.x.clamp(-1.0, 1.0), i.mv.y.clamp(-1.0, 1.0));
    *input = i;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn touch_buttons_fit_on_small_and_large_screens() {
        for (w, h) in [(360.0, 640.0), (640.0, 360.0), (1280.0, 720.0), (2560.0, 1440.0)] {
            let l = touch_layout(w, h, 1.0);
            for (c, r) in [l.fire, l.missile, l.bomb, l.pause] {
                assert!(c.x - r >= 0.0 && c.x + r <= w, "{w}x{h}: {c} r{r}");
                assert!(c.y - r >= 0.0 && c.y + r <= h, "{w}x{h}: {c} r{r}");
            }
            // Fire, missile and bomb must not overlap each other.
            assert!(l.fire.0.distance(l.missile.0) > l.fire.1 + l.missile.1);
            assert!(l.fire.0.distance(l.bomb.0) > l.fire.1 + l.bomb.1);
            assert!(l.missile.0.distance(l.bomb.0) > l.missile.1 + l.bomb.1);
            // Rendering at half resolution leaves the buttons in the same place on screen.
            let half = touch_layout(w / 2.0, h / 2.0, 2.0);
            assert!((half.bomb.0 * 2.0).distance(l.bomb.0) < 0.01);
            assert!((half.bomb.1 * 2.0 - l.bomb.1).abs() < 0.01);
        }
    }
}
