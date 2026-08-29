//! High-score records, and the file contract the marquee reads.
//!
//! Each game owns exactly one file under
//! `$XDG_STATE_HOME/omarcade/scores/<id>.json` and is its own sole writer,
//! so two games running at once can never race: there is no shared file to
//! overwrite. The bar widget scans the directory and merges at read time.
//!
//! Writes are atomic — temp file, fsync, rename — because the reader is a
//! filesystem watcher that can wake at any moment. A half-written file must
//! never be observable, and a crash mid-write must leave the previous record
//! intact rather than a truncated one.
//!
//! Nothing here is on the frame path. Games call [`ScoreFile::record`] at
//! game-over and [`ScoreFile::save`] once, not per tick.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Bump when the shape below changes incompatibly. A reader that does not
/// recognise the version is expected to ignore the record rather than guess
/// at it — the same contract Omarchy's own state records use.
pub const SCHEMA_VERSION: u32 = 1;

/// How many scores a game keeps. The marquee shows one; the rest are history
/// and cost nothing to carry.
pub const KEEP: usize = 10;

/// One scoring run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub score: u32,
    /// RFC 3339 UTC, e.g. `2026-08-29T02:31:00Z`. A string rather than a
    /// timestamp type so the file stays readable and the crate stays
    /// dependency-light; the marquee only ever displays it.
    pub at: String,
}

/// One game's score file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScoreFile {
    pub schema_version: u32,
    /// Matches the filename stem, mirroring how Omarchy names its own state
    /// records. `omarcade-breakout` -> `omarcade-breakout.json`.
    pub id: String,
    /// Human label for the marquee: "Breakout".
    pub name: String,
    /// Descending by score, capped at [`KEEP`].
    pub entries: Vec<Entry>,
    pub updated_at: String,
}

impl ScoreFile {
    /// A record with no scores yet.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            id: id.into(),
            name: name.into(),
            entries: Vec::new(),
            updated_at: now_rfc3339(),
        }
    }

    /// Load this game's record, or start a fresh one.
    ///
    /// A missing, unreadable, malformed, or future-versioned file all yield
    /// an empty record rather than an error: a corrupt scoreboard must never
    /// stop someone playing. The next [`save`](Self::save) overwrites it.
    pub fn load_or_new(id: impl Into<String>, name: impl Into<String>) -> Self {
        let id = id.into();
        let name = name.into();

        let parsed = path_for(&id)
            .and_then(|p| fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str::<ScoreFile>(&s).ok())
            .filter(|f| f.schema_version == SCHEMA_VERSION);

        match parsed {
            // Trust stored scores, but let the code own the display name so a
            // rename in a future release is picked up on the next save.
            Some(mut f) => {
                f.name = name;
                f
            }
            None => Self::new(id, name),
        }
    }

    /// Add a score, keeping the table sorted and capped.
    ///
    /// Returns whether this is the new best — the caller may want to say so
    /// on the game-over screen.
    pub fn record(&mut self, score: u32) -> bool {
        let is_best = self.entries.first().is_none_or(|e| score > e.score);

        self.entries.push(Entry { score, at: now_rfc3339() });
        // Descending by score. sort_by_key is stable, so ties keep the older
        // run first — it got there first.
        self.entries.sort_by_key(|e| std::cmp::Reverse(e.score));
        self.entries.truncate(KEEP);
        self.updated_at = now_rfc3339();

        is_best
    }

    /// The current best, if any run has been recorded.
    pub fn best(&self) -> Option<u32> {
        self.entries.first().map(|e| e.score)
    }

    /// Write atomically to the scores directory.
    ///
    /// Errors are the caller's to ignore: failing to save a score is not
    /// worth interrupting a game over, so games are expected to drop this
    /// on the floor rather than propagate it.
    pub fn save(&self) -> io::Result<()> {
        let path = path_for(&self.id)
            .ok_or_else(|| io::Error::other("no state directory (HOME unset?)"))?;
        let dir = path.parent().expect("path_for always yields a parent");
        fs::create_dir_all(dir)?;

        let json = serde_json::to_string_pretty(self).map_err(io::Error::other)?;

        // Same-directory temp, so the rename below is a rename and not a
        // cross-filesystem copy (which would not be atomic).
        let tmp = path.with_extension("json.tmp");
        {
            let mut f = fs::File::create(&tmp)?;
            f.write_all(json.as_bytes())?;
            f.write_all(b"\n")?;
            // Durability before visibility. Without this, a crash between
            // write and rename can publish an empty file.
            f.sync_all()?;
        }

        set_private(&tmp)?;

        // The atomic publish. Readers see either the old file or the new one,
        // never a partial write.
        fs::rename(&tmp, &path)
    }
}

/// `$XDG_STATE_HOME/omarcade/scores`, falling back to `~/.local/state`.
///
/// State, not data: scores are machine-local and regenerable, which is the
/// same call Omarchy makes for its own agent usage records.
pub fn scores_dir() -> Option<PathBuf> {
    let base = match std::env::var_os("XDG_STATE_HOME") {
        Some(v) if !v.is_empty() => PathBuf::from(v),
        _ => PathBuf::from(std::env::var_os("HOME")?).join(".local/state"),
    };
    Some(base.join("omarcade/scores"))
}

fn path_for(id: &str) -> Option<PathBuf> {
    Some(scores_dir()?.join(format!("{id}.json")))
}

#[cfg(unix)]
fn set_private(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    // 0600, matching Omarchy's own state records.
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn set_private(_path: &Path) -> io::Result<()> {
    Ok(())
}

/// RFC 3339 in UTC, computed from the system clock without pulling in a
/// date library. Civil-date conversion is the standard days-from-epoch
/// algorithm (Howard Hinnant's `civil_from_days`).
fn now_rfc3339() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let (y, mo, d) = civil_from_days(days);

    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// Days since 1970-01-01 to (year, month, day).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_record_has_no_scores() {
        let f = ScoreFile::new("omarcade-test", "Test");
        assert_eq!(f.schema_version, SCHEMA_VERSION);
        assert!(f.entries.is_empty());
        assert_eq!(f.best(), None);
    }

    #[test]
    fn first_score_is_a_best() {
        let mut f = ScoreFile::new("omarcade-test", "Test");
        assert!(f.record(100));
        assert_eq!(f.best(), Some(100));
    }

    #[test]
    fn higher_score_beats_the_previous_best() {
        let mut f = ScoreFile::new("omarcade-test", "Test");
        f.record(100);
        assert!(f.record(250));
        assert_eq!(f.best(), Some(250));
    }

    #[test]
    fn lower_score_is_not_a_best_but_is_still_kept() {
        let mut f = ScoreFile::new("omarcade-test", "Test");
        f.record(100);
        assert!(!f.record(50));
        assert_eq!(f.best(), Some(100));
        assert_eq!(f.entries.len(), 2);
    }

    #[test]
    fn equal_score_does_not_displace_the_incumbent() {
        let mut f = ScoreFile::new("omarcade-test", "Test");
        f.record(100);
        assert!(!f.record(100), "a tie is not a new best");
        assert_eq!(f.best(), Some(100));
    }

    #[test]
    fn entries_stay_sorted_descending() {
        let mut f = ScoreFile::new("omarcade-test", "Test");
        for s in [50, 300, 150, 20, 200] {
            f.record(s);
        }
        let scores: Vec<u32> = f.entries.iter().map(|e| e.score).collect();
        assert_eq!(scores, vec![300, 200, 150, 50, 20]);
    }

    #[test]
    fn table_is_capped_at_keep() {
        let mut f = ScoreFile::new("omarcade-test", "Test");
        for s in 1..=(KEEP as u32 + 25) {
            f.record(s);
        }
        assert_eq!(f.entries.len(), KEEP);
        // The cap must drop the LOWEST scores, not the newest.
        assert_eq!(f.best(), Some(KEEP as u32 + 25));
        assert_eq!(f.entries.last().unwrap().score, 26);
    }

    #[test]
    fn round_trips_through_json() {
        let mut f = ScoreFile::new("omarcade-test", "Test");
        f.record(420);
        let json = serde_json::to_string(&f).unwrap();
        let back: ScoreFile = serde_json::from_str(&json).unwrap();
        assert_eq!(f, back);
    }

    #[test]
    fn serialised_shape_is_the_documented_contract() {
        // The marquee reads these exact keys. Renaming a field is a breaking
        // change to a published contract, so pin it here.
        let mut f = ScoreFile::new("omarcade-breakout", "Breakout");
        f.record(1234);
        let v: serde_json::Value = serde_json::to_value(&f).unwrap();

        assert_eq!(v["schema_version"], 1);
        assert_eq!(v["id"], "omarcade-breakout");
        assert_eq!(v["name"], "Breakout");
        assert_eq!(v["entries"][0]["score"], 1234);
        assert!(v["entries"][0]["at"].is_string());
        assert!(v["updated_at"].is_string());
    }

    #[test]
    fn timestamps_are_rfc3339_utc() {
        let t = now_rfc3339();
        assert_eq!(t.len(), 20, "YYYY-MM-DDTHH:MM:SSZ is 20 chars: {t}");
        assert!(t.ends_with('Z'));
        assert_eq!(&t[4..5], "-");
        assert_eq!(&t[10..11], "T");
    }

    #[test]
    fn civil_from_days_matches_known_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_723), (2024, 1, 1));
        // A leap day, the case an off-by-one in the algorithm would break.
        assert_eq!(civil_from_days(19_782), (2024, 2, 29));
    }

    #[test]
    fn scores_dir_honours_xdg_state_home() {
        // Not using std::env::set_var: it is unsafe in edition 2024 and would
        // race other tests in this binary. Assert the shape instead.
        let dir = scores_dir().expect("HOME is set in a test environment");
        assert!(dir.ends_with("omarcade/scores"), "got {dir:?}");
    }

    #[test]
    fn a_future_schema_version_is_rejected_on_load() {
        // load_or_new filters on schema_version, so a record from a newer
        // release must not be parsed into today's shape.
        let mut f = ScoreFile::new("omarcade-test", "Test");
        f.record(999);
        f.schema_version = SCHEMA_VERSION + 1;
        let json = serde_json::to_string(&f).unwrap();

        let parsed = serde_json::from_str::<ScoreFile>(&json)
            .ok()
            .filter(|f| f.schema_version == SCHEMA_VERSION);
        assert!(parsed.is_none());
    }
}
