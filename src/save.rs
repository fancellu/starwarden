//! The best score and the date it was set, kept between sessions: in the browser's
//! `localStorage` on the web, and in a small file in the user's app-data folder on the desktop.
//! Saving never stops the game: any error just means the score is not remembered.

use bevy::prelude::*;

use crate::state::{Game, GameState};

/// A best score and when it was set (Unix seconds).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Best {
    pub score: u32,
    pub when: u64,
}

#[derive(Resource)]
pub struct SavedBest {
    /// What is in storage (`None` before the first game is ever scored).
    pub best: Option<Best>,
    /// Seconds since the last write, to limit how often a climbing score is saved.
    since_write: f32,
    /// Off for the developer demo, so self-play never overwrites a real best score.
    enabled: bool,
}

/// Seconds between saves while a new best score keeps climbing.
const SAVE_EVERY: f32 = 2.0;

impl SavedBest {
    /// Reads storage. Call before the app starts so the title screen can show the result.
    pub fn load() -> Self {
        Self {
            best: backend::read().as_deref().and_then(decode),
            since_write: 0.0,
            enabled: std::env::var("STARWARDEN_DEMO").is_err(),
        }
    }

    /// Stores `score` with the current time if it beats what is stored.
    fn store(&mut self, score: u32) {
        if !self.enabled || score == 0 || self.best.is_some_and(|b| b.score >= score) {
            return;
        }
        let best = Best {
            score,
            when: backend::now(),
        };
        backend::write(&encode(&best));
        self.best = Some(best);
        self.since_write = 0.0;
    }
}

pub fn encode(b: &Best) -> String {
    format!("{},{}", b.score, b.when)
}

pub fn decode(s: &str) -> Option<Best> {
    let (score, when) = s.trim().split_once(',')?;
    Some(Best {
        score: score.parse().ok()?,
        when: when.parse().ok()?,
    })
}

/// A date such as `16 NOV 2026` (UTC) from Unix seconds.
pub fn date_text(secs: u64) -> String {
    const MONTHS: [&str; 12] = [
        "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
    ];
    // Days since 1970-01-01 to a civil date (Howard Hinnant's algorithm).
    let z = (secs / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{day:02} {} {year}", MONTHS[(month - 1) as usize])
}

/// Saves a new best score as it is set, at most every `SAVE_EVERY` seconds.
pub fn save_best(time: Res<Time>, game: Res<Game>, mut saved: ResMut<SavedBest>) {
    saved.since_write += time.delta_secs();
    if saved.since_write >= SAVE_EVERY {
        saved.store(game.high);
    }
}

/// Saves straight away when a game ends.
pub fn save_best_now(game: Res<Game>, mut saved: ResMut<SavedBest>) {
    saved.store(game.high);
}

pub fn plugin(app: &mut App) {
    app.add_systems(Update, save_best)
        .add_systems(OnEnter(GameState::GameOver), save_best_now);
}

#[cfg(target_arch = "wasm32")]
mod backend {
    const KEY: &str = "starwarden.best";

    fn storage() -> Option<web_sys::Storage> {
        web_sys::window()?.local_storage().ok()?
    }

    pub fn read() -> Option<String> {
        storage()?.get_item(KEY).ok()?
    }

    pub fn write(s: &str) {
        if let Some(st) = storage() {
            let _ = st.set_item(KEY, s);
        }
    }

    pub fn now() -> u64 {
        (js_sys::Date::now() / 1000.0) as u64
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod backend {
    use std::path::PathBuf;

    /// The per-user data folder for the game, from the usual environment variables.
    fn dir() -> Option<PathBuf> {
        let env = |k| std::env::var_os(k).map(PathBuf::from);
        if cfg!(windows) {
            Some(env("APPDATA")?.join("Starwarden"))
        } else if cfg!(target_os = "macos") {
            Some(env("HOME")?.join("Library/Application Support/Starwarden"))
        } else {
            let base = env("XDG_DATA_HOME").or_else(|| Some(env("HOME")?.join(".local/share")))?;
            Some(base.join("starwarden"))
        }
    }

    pub fn read() -> Option<String> {
        std::fs::read_to_string(dir()?.join("best.txt")).ok()
    }

    pub fn write(s: &str) {
        if let Some(d) = dir() {
            let _ = std::fs::create_dir_all(&d);
            let _ = std::fs::write(d.join("best.txt"), s);
        }
    }

    pub fn now() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn best_survives_a_round_trip_and_junk_is_ignored() {
        let b = Best {
            score: 123_450,
            when: 1_794_787_200,
        };
        assert_eq!(decode(&encode(&b)), Some(b));
        assert_eq!(decode(" 42,7\n"), Some(Best { score: 42, when: 7 }));
        for bad in ["", "abc", "12", "12,", ",5", "-1,5", "1,x"] {
            assert_eq!(decode(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn dates_are_shown_as_day_month_year() {
        assert_eq!(date_text(0), "01 JAN 1970");
        assert_eq!(date_text(951_782_400), "29 FEB 2000");
        assert_eq!(date_text(1_794_787_200 + 86_399), "16 NOV 2026");
    }

    #[test]
    fn only_a_higher_score_is_stored_and_never_in_the_demo() {
        let mut s = SavedBest {
            best: Some(Best {
                score: 500,
                when: 1,
            }),
            since_write: 0.0,
            enabled: false,
        };
        s.store(900);
        assert_eq!(s.best.unwrap().score, 500, "disabled");
        s.enabled = true;
        s.store(400);
        assert_eq!(s.best.unwrap().score, 500, "lower scores are not stored");
    }

    /// Writes and reads a real file, in a temporary folder instead of the user's own.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn desktop_save_is_written_and_read_back() {
        let tmp = std::env::temp_dir().join(format!("starwarden-test-{}", std::process::id()));
        for k in ["APPDATA", "HOME", "XDG_DATA_HOME"] {
            std::env::set_var(k, &tmp);
        }
        let mut s = SavedBest {
            best: None,
            since_write: 0.0,
            enabled: true,
        };
        s.store(0);
        assert!(backend::read().is_none(), "a zero score is not saved");
        s.store(4321);
        let back = decode(&backend::read().unwrap()).unwrap();
        assert_eq!(back.score, 4321);
        assert!(back.when > 1_700_000_000, "stamped with the current time");
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
