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

/// The default difficulty label, used by a game that has only one.
///
/// Breakout's existing records predate the field entirely and
/// deserialize to this, so its history stays one comparable table
/// rather than splitting into a labelled and an unlabelled half.
pub const DEFAULT_DIFFICULTY: &str = "normal";

/// One scoring run.
///
/// ⚠️ `PartialEq` but NOT `Eq`, since `seconds` is a float and a float
/// has no total equality. Nothing compares entries for identity; they
/// are ranked by score through [`ScoreFile::beats`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub score: u32,
    /// RFC 3339 UTC, e.g. `2026-08-29T02:31:00Z`. A string rather than a
    /// timestamp type so the file stays readable and the crate stays
    /// dependency-light; the marquee only ever displays it.
    pub at: String,
    /// Which difficulty this run was played on.
    ///
    /// A free string rather than an enum: core has no business knowing
    /// that Pong calls its tiers easy/normal/hard while some later game
    /// counts levels. The marquee groups by whatever it finds.
    ///
    /// Scores from different difficulties are NOT comparable — an easy
    /// run and a hard run are different games — so [`best_for`] is the
    /// honest query and [`best`] is only meaningful for a single-tier
    /// game.
    ///
    /// [`best_for`]: ScoreFile::best_for
    /// [`best`]: ScoreFile::best
    #[serde(default = "default_difficulty")]
    pub difficulty: String,
    /// How long the run took, in seconds, for a game where that is a
    /// result rather than a side effect.
    ///
    /// ⚠️ OPTIONAL, AND THAT IS THE WHOLE DESIGN. The marquee is generic
    /// on purpose — `ScoreRecord.qml` says it "never learns how a score
    /// was made", which is what lets a fourth game appear on the bar
    /// without the widget being edited. A mandatory time field would
    /// make every game carry a number only one of them means.
    ///
    /// So: Omaprix writes it, Pixel Break and Volley write nothing, the
    /// key is absent from their files, and the marquee shows a time only
    /// where it finds one. Existing records deserialize to `None`.
    ///
    /// ⚠️ ONLY A COMPLETED RACE HAS ONE. A run that ends at a missed cut
    /// or an empty clock still banks a SCORE, with no time — Brian's
    /// call, and the honest one: "best time" has to mean a race someone
    /// actually finished.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seconds: Option<f32>,
}

fn default_difficulty() -> String {
    DEFAULT_DIFFICULTY.to_string()
}

/// Which direction wins.
///
/// Nothing outside the game itself can know this: Breakout's score is
/// points and bigger is better, but a game scored on time, on strokes,
/// or on goals conceded ranks the other way. Before this existed the
/// answer was hardcoded — `sort desc` + `truncate` would have thrown a
/// lower-is-better game's BEST runs off the table at write time.
///
/// The game declares it; core and the marquee read it. That keeps
/// ranking generic — adding a game never means editing the marquee.
fn default_higher_is_better() -> bool {
    true
}

/// One game's score file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScoreFile {
    pub schema_version: u32,
    /// Matches the filename stem, mirroring how Omarchy names its own state
    /// records. `omarcade-breakout` -> `omarcade-breakout.json`.
    pub id: String,
    /// Human label for the marquee: "Breakout".
    pub name: String,
    /// Whether a bigger number is a better result. See
    /// [`default_higher_is_better`] for why this is declared rather than
    /// assumed. Absent in v1 records, which were all points-scored.
    #[serde(default = "default_higher_is_better")]
    pub higher_is_better: bool,
    /// Best first, capped at [`KEEP`] — where "best" follows
    /// [`higher_is_better`](Self::higher_is_better), not the raw number.
    pub entries: Vec<Entry>,
    pub updated_at: String,
}

impl ScoreFile {
    /// A record with no scores yet, ranked higher-is-better.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            id: id.into(),
            name: name.into(),
            higher_is_better: true,
            entries: Vec::new(),
            updated_at: now_rfc3339(),
        }
    }

    /// Declare that a smaller number is the better result — strokes,
    /// seconds, goals conceded.
    ///
    /// Call this on the record BEFORE the first [`record`](Self::record):
    /// it decides which entries survive the cap, so flipping it after a
    /// table has been trimmed cannot recover what was already dropped.
    pub fn lower_is_better(mut self) -> Self {
        self.higher_is_better = false;
        self
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

    /// Load, adopting an earlier id's file if this game has been renamed.
    ///
    /// A game's id names its score file, so renaming a game orphans every
    /// score anyone ever set. Volley is the first to hit this for real —
    /// it shipped as Pong and its records include runs the author cared
    /// about — but renaming is a property of the CABINET rather than a
    /// rule of any one game, so the migration lives here beside the file
    /// format it rewrites. Game four gets it free.
    ///
    /// The new file wins whenever it exists. That is what makes this
    /// idempotent: after the first launch there is a `<id>.json`, so the
    /// legacy branch is never taken again, and a player who already has
    /// real scores under the new name cannot have them replaced by a
    /// stale file left behind on disk.
    ///
    /// The legacy file is REMOVED once its contents are safely published
    /// under the new name. Leaving it would be a slow leak of exactly the
    /// kind that has bitten this project before: a stale artefact from an
    /// old name that the installer never cleans and the marquee may still
    /// scan, showing the same game twice under two titles.
    ///
    /// Failure is swallowed, matching how games treat [`save`](Self::save):
    /// a scoreboard that cannot be migrated is not a reason to refuse to
    /// start. The worst case is an empty table, which is also exactly what
    /// a fresh install looks like.
    pub fn load_or_migrate(
        id: impl Into<String>,
        name: impl Into<String>,
        legacy_id: &str,
    ) -> Self {
        let id = id.into();
        let name = name.into();

        let new_exists = path_for(&id).is_some_and(|p| p.exists());
        let legacy_path = path_for(legacy_id);
        let legacy_raw = legacy_path
            .as_ref()
            .filter(|_| !new_exists)
            .and_then(|p| fs::read_to_string(p).ok());

        match migration_plan(new_exists, legacy_raw.as_deref()) {
            MigrationPlan::LoadNormally => Self::load_or_new(id, name),
            MigrationPlan::Carry(carried) => {
                let mut carried = *carried;
                // The code owns identity, exactly as load_or_new already
                // asserts for the display name — the stored id is the OLD
                // one by definition, and saving it unchanged would write
                // straight back to the file we are migrating away from.
                carried.id = id;
                carried.name = name;

                // Publish under the new name BEFORE removing the old one,
                // so an interruption between the two leaves the scores
                // readable somewhere rather than nowhere.
                if carried.save().is_ok() {
                    if let Some(p) = legacy_path {
                        let _ = fs::remove_file(p);
                    }
                }

                carried
            }
        }
    }

    /// Load, then apply this game's ranking direction.
    ///
    /// The direction is a fact about the GAME, not about the file: a
    /// stale record written before the game declared itself would
    /// otherwise keep ranking the old way forever. The code is the
    /// authority, the same way it already is for the display name.
    pub fn load_or_new_ranked(
        id: impl Into<String>,
        name: impl Into<String>,
        higher_is_better: bool,
    ) -> Self {
        let mut f = Self::load_or_new(id, name);
        if f.higher_is_better != higher_is_better {
            f.higher_is_better = higher_is_better;
            // The stored table was ordered by the other rule, so its
            // order — and which entries the cap kept — no longer mean
            // what they claim. Re-sort what survives.
            f.sort_entries();
        }
        f
    }

    /// Is `score` a better result than `other` under this record's rule?
    pub fn beats(&self, score: u32, other: u32) -> bool {
        if self.higher_is_better { score > other } else { score < other }
    }

    /// Order entries best-first under this record's rule.
    ///
    /// `sort_by_key` is stable, so ties keep the older run first — it
    /// got there first.
    fn sort_entries(&mut self) {
        if self.higher_is_better {
            self.entries.sort_by_key(|e| std::cmp::Reverse(e.score));
        } else {
            self.entries.sort_by_key(|e| e.score);
        }
    }

    /// Add a score at the default difficulty.
    ///
    /// Returns whether this is the new best — the caller may want to say so
    /// on the game-over screen.
    pub fn record(&mut self, score: u32) -> bool {
        self.record_at(score, DEFAULT_DIFFICULTY)
    }

    /// Add a score played at `difficulty`, keeping the table sorted and
    /// capped.
    ///
    /// "Best" here means best on that same difficulty — an easy run does
    /// not become the new best simply by outscoring every hard one.
    pub fn record_at(&mut self, score: u32, difficulty: &str) -> bool {
        self.record_run(score, difficulty, None)
    }

    /// Add a score, optionally with how long the run took.
    ///
    /// `seconds` is `None` for a game that has no notion of a run
    /// length, and for a run that ended without finishing — see
    /// [`Entry::seconds`].
    pub fn record_run(&mut self, score: u32, difficulty: &str, seconds: Option<f32>) -> bool {
        let is_best = self
            .best_for(difficulty)
            .is_none_or(|b| self.beats(score, b));

        self.entries.push(Entry {
            score,
            at: now_rfc3339(),
            difficulty: difficulty.to_string(),
            seconds,
        });
        self.sort_entries();
        self.entries.truncate(KEEP);
        self.updated_at = now_rfc3339();

        is_best
    }

    /// The current best across every difficulty.
    ///
    /// Only meaningful for a single-tier game. Anything with a
    /// difficulty selector wants [`best_for`](Self::best_for): mixing
    /// tiers compares runs that were never the same game.
    pub fn best(&self) -> Option<u32> {
        self.entries.first().map(|e| e.score)
    }

    /// The best run on one difficulty.
    ///
    /// Entries are already ordered best-first, so the first match wins
    /// without re-ranking.
    pub fn best_for(&self, difficulty: &str) -> Option<u32> {
        self.entries
            .iter()
            .find(|e| e.difficulty == difficulty)
            .map(|e| e.score)
    }

    /// The fastest recorded time for a difficulty, in seconds.
    ///
    /// ⚠️ FASTEST, NOT "the time of the best-scoring run". Those are
    /// different questions and the table is sorted by SCORE, so the top
    /// entry is not the quickest one. A best time has to be found by
    /// searching, or it silently reports whatever the highest scorer
    /// happened to take.
    ///
    /// Entries with no time — every run that did not finish, and every
    /// game that does not record one — are skipped rather than counted
    /// as zero.
    pub fn best_time_for(&self, difficulty: &str) -> Option<f32> {
        self.entries
            .iter()
            .filter(|e| e.difficulty == difficulty)
            .filter_map(|e| e.seconds)
            .fold(None, |best: Option<f32>, t| {
                Some(match best {
                    Some(b) if b <= t => b,
                    _ => t,
                })
            })
    }

    /// Every difficulty this record holds a score for, best-first
    /// within each, in the order they appear in the table.
    pub fn difficulties(&self) -> Vec<&str> {
        let mut seen: Vec<&str> = Vec::new();
        for e in &self.entries {
            if !seen.contains(&e.difficulty.as_str()) {
                seen.push(&e.difficulty);
            }
        }
        seen
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

/// What [`ScoreFile::load_or_migrate`] should do, decided WITHOUT touching
/// the filesystem.
///
/// Split out for one reason: the decision is the part that can be wrong,
/// and it cannot be tested through the real filesystem. `set_var` on
/// `XDG_STATE_HOME` is unsafe in edition 2024 and would race every other
/// test in this binary — which is why [`scores_dir`]'s own test asserts a
/// shape rather than redirecting the directory. Keeping the rules pure
/// means they can be exercised exhaustively, including the cases that
/// only happen on someone else's machine.
#[derive(Debug)]
enum MigrationPlan {
    /// Nothing to carry; take the ordinary load path.
    LoadNormally,
    /// Adopt this record under the new id.
    Carry(Box<ScoreFile>),
}

/// Decide whether a rename should adopt an older file.
///
/// `legacy_raw` is the old file's contents, or `None` if it is absent or
/// unreadable. It is only ever read when the new file does not exist, so
/// passing `None` alongside `new_exists = true` is the normal steady state
/// rather than a special case.
fn migration_plan(new_exists: bool, legacy_raw: Option<&str>) -> MigrationPlan {
    // The new file always wins. This is what makes migration idempotent
    // after the first launch, and it is also the guard that stops a stale
    // file left on disk from overwriting scores someone has actually set
    // under the new name.
    if new_exists {
        return MigrationPlan::LoadNormally;
    }

    let Some(raw) = legacy_raw else {
        return MigrationPlan::LoadNormally;
    };

    match serde_json::from_str::<ScoreFile>(raw) {
        // A record from a newer release. Leave it alone rather than
        // discarding what this version cannot understand — the same call
        // load_or_new makes when it filters on the version.
        Ok(f) if f.schema_version != SCHEMA_VERSION => MigrationPlan::LoadNormally,
        Ok(f) => MigrationPlan::Carry(Box::new(f)),
        Err(_) => MigrationPlan::LoadNormally,
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

    // ------------------------------------------------------------------
    // Ranking direction. Before this was declared, `sort desc + truncate`
    // threw a lower-is-better game's BEST runs off the table at write
    // time — data loss on disk, not just a display bug.
    // ------------------------------------------------------------------

    #[test]
    fn lower_is_better_ranks_the_smallest_first() {
        let mut f = ScoreFile::new("omarcade-test", "Test").lower_is_better();
        for s in [50, 300, 150, 20, 200] {
            f.record(s);
        }
        let scores: Vec<u32> = f.entries.iter().map(|e| e.score).collect();
        assert_eq!(scores, vec![20, 50, 150, 200, 300]);
        assert_eq!(f.best(), Some(20));
    }

    #[test]
    fn lower_is_better_calls_a_smaller_score_the_new_best() {
        let mut f = ScoreFile::new("omarcade-test", "Test").lower_is_better();
        assert!(f.record(100), "the first run is always a best");
        assert!(f.record(40), "40 beats 100 when lower wins");
        assert!(!f.record(90), "90 does not beat 40");
        assert_eq!(f.best(), Some(40));
    }

    #[test]
    fn the_cap_keeps_the_best_runs_under_either_rule() {
        // The regression this whole field exists for: the cap must drop
        // the WORST entries, and "worst" depends on the direction.
        let mut low = ScoreFile::new("omarcade-test", "Test").lower_is_better();
        for s in 1..=(KEEP as u32 + 25) {
            low.record(s);
        }
        assert_eq!(low.entries.len(), KEEP);
        assert_eq!(low.best(), Some(1), "1 is the best possible run here");
        assert_eq!(
            low.entries.last().unwrap().score,
            KEEP as u32,
            "the cap must drop the LARGEST scores when lower is better"
        );
    }

    #[test]
    fn direction_survives_a_round_trip_and_defaults_to_higher() {
        let f = ScoreFile::new("omarcade-test", "Test").lower_is_better();
        let json = serde_json::to_string(&f).unwrap();
        let back: ScoreFile = serde_json::from_str(&json).unwrap();
        assert!(!back.higher_is_better);

        // A v1 record predates the field entirely.
        let v1 = r#"{"schema_version":1,"id":"x","name":"X",
                     "entries":[],"updated_at":"2026-01-01T00:00:00Z"}"#;
        let parsed: ScoreFile = serde_json::from_str(v1).unwrap();
        assert!(parsed.higher_is_better, "v1 records were all points-scored");
    }

    // ------------------------------------------------------------------
    // Difficulty
    // ------------------------------------------------------------------

    #[test]
    fn a_v1_entry_without_difficulty_still_loads() {
        // Brian's existing Breakout scores look exactly like this. They
        // must survive, or the marquee goes blank on upgrade.
        let v1 = r#"{"schema_version":1,"id":"omarcade-breakout","name":"Breakout",
                     "entries":[{"score":1234,"at":"2026-08-29T02:31:00Z"}],
                     "updated_at":"2026-08-29T02:31:00Z"}"#;
        let f: ScoreFile = serde_json::from_str(v1).unwrap();
        assert_eq!(f.best(), Some(1234));
        assert_eq!(f.entries[0].difficulty, DEFAULT_DIFFICULTY);
        assert!(f.higher_is_better);
    }

    #[test]
    fn best_is_tracked_per_difficulty() {
        let mut f = ScoreFile::new("omarcade-volley", "Volley");
        f.record_at(500, "easy");
        f.record_at(300, "hard");

        assert_eq!(f.best_for("easy"), Some(500));
        assert_eq!(f.best_for("hard"), Some(300));
        assert_eq!(f.best_for("nightmare"), None);
    }

    #[test]
    fn a_big_easy_score_is_not_a_new_best_on_hard() {
        let mut f = ScoreFile::new("omarcade-volley", "Volley");
        f.record_at(300, "hard");
        // Outscores every hard run, but it was a different game.
        assert!(f.record_at(9000, "easy"), "still a best FOR EASY");
        assert_eq!(f.best_for("hard"), Some(300), "hard's best is untouched");
    }

    #[test]
    fn difficulties_are_listed_without_duplicates() {
        let mut f = ScoreFile::new("omarcade-volley", "Volley");
        f.record_at(10, "easy");
        f.record_at(30, "hard");
        f.record_at(20, "easy");
        let mut d = f.difficulties();
        d.sort_unstable();
        assert_eq!(d, vec!["easy", "hard"]);
    }

    #[test]
    fn load_or_new_ranked_lets_the_code_override_a_stale_file() {
        // A record written before the game declared its direction.
        let stale = r#"{"schema_version":1,"id":"x","name":"X",
                        "higher_is_better":true,
                        "entries":[{"score":9,"at":"t","difficulty":"normal"},
                                   {"score":2,"at":"t","difficulty":"normal"}],
                        "updated_at":"t"}"#;
        let mut f: ScoreFile = serde_json::from_str(stale).unwrap();
        assert_eq!(f.best(), Some(9));

        // The game now says lower wins; the table must be re-ranked.
        f.higher_is_better = false;
        f.sort_entries();
        assert_eq!(f.best(), Some(2));
    }

    // ------------------------------------------------------------------
    // Renaming a game. Volley shipped as Pong; its score file is named by
    // the id, so the rename would otherwise orphan every run anyone had
    // set. These exercise the DECISION, which is the part that can be
    // wrong — see MigrationPlan for why it is separated from the I/O.
    // ------------------------------------------------------------------

    /// Brian's real omarcade-pong.json, trimmed to four entries. The two
    /// leading scores are from the session where he confirmed the
    /// difficulty fix; carrying them is the whole point of migrating.
    const LEGACY_PONG: &str = r#"{
      "schema_version": 1,
      "id": "omarcade-pong",
      "name": "Pong",
      "higher_is_better": true,
      "entries": [
        {"score":59,"at":"2026-09-11T12:40:52Z","difficulty":"easy"},
        {"score":44,"at":"2026-09-11T12:14:27Z","difficulty":"easy"},
        {"score":36,"at":"2026-09-01T14:59:08Z","difficulty":"easy"},
        {"score":29,"at":"2026-08-31T00:07:26Z","difficulty":"normal"}
      ],
      "updated_at": "2026-09-11T12:47:35Z"
    }"#;

    #[test]
    fn a_rename_carries_the_old_file_when_the_new_one_is_absent() {
        let plan = migration_plan(false, Some(LEGACY_PONG));
        let MigrationPlan::Carry(f) = plan else {
            panic!("expected the legacy file to be carried, got {plan:?}");
        };
        assert_eq!(f.best_for("easy"), Some(59), "today's run must survive");
        assert_eq!(f.best_for("normal"), Some(29));
        assert_eq!(f.entries.len(), 4);
    }

    #[test]
    fn a_rename_is_idempotent_once_the_new_file_exists() {
        // The second launch, and every launch after it. If this ever
        // returned Carry, a real table could be replaced by a stale one.
        let plan = migration_plan(true, Some(LEGACY_PONG));
        assert!(
            matches!(plan, MigrationPlan::LoadNormally),
            "the new file must win outright, got {plan:?}"
        );
    }

    #[test]
    fn a_fresh_install_has_nothing_to_carry() {
        let plan = migration_plan(false, None);
        assert!(matches!(plan, MigrationPlan::LoadNormally), "{plan:?}");
    }

    #[test]
    fn an_unreadable_legacy_file_is_left_alone() {
        // Not carried, and — because nothing is carried — never deleted
        // either. Discarding a record we failed to parse is worse than
        // leaving it for a later release to understand.
        let plan = migration_plan(false, Some("{ this is not json"));
        assert!(matches!(plan, MigrationPlan::LoadNormally), "{plan:?}");
    }

    #[test]
    fn a_legacy_file_from_a_newer_release_is_not_carried() {
        let future = LEGACY_PONG.replace(
            r#""schema_version": 1"#,
            r#""schema_version": 2"#,
        );
        assert!(future.contains(r#""schema_version": 2"#), "fixture edit landed");
        let plan = migration_plan(false, Some(&future));
        assert!(matches!(plan, MigrationPlan::LoadNormally), "{plan:?}");
    }

    #[test]
    fn the_carried_record_is_rewritten_to_the_new_identity() {
        // The id is what names the file. Carrying it unchanged would save
        // straight back over the file being migrated away from, which
        // would leave the rename looking successful while changing
        // nothing on disk.
        let MigrationPlan::Carry(f) = migration_plan(false, Some(LEGACY_PONG)) else {
            panic!("expected Carry");
        };
        let mut carried = *f;
        assert_eq!(carried.id, "omarcade-pong", "as stored");

        carried.id = "omarcade-volley".to_string();
        carried.name = "Volley".to_string();

        let round = serde_json::to_string(&carried).unwrap();
        let back: ScoreFile = serde_json::from_str(&round).unwrap();
        assert_eq!(back.id, "omarcade-volley");
        assert_eq!(back.name, "Volley");
        assert_eq!(back.best_for("easy"), Some(59), "scores survive the rewrite");
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

    /// ⚠️ THE ONE THAT PROTECTS BRIAN'S REAL SCORES. Every record on
    /// disk predates `seconds`, so the field MUST be optional in the
    /// deserialising sense, not merely defaulted — a required field
    /// would make every existing file fail to parse and silently reset
    /// three games' history to empty.
    #[test]
    fn a_record_written_before_times_existed_still_loads() {
        // Byte-for-byte the shape of the shipped files, including the
        // real top entry from Omaprix.
        let json = r#"{
            "schema_version": 1,
            "id": "omarcade-racer",
            "name": "Omaprix",
            "higher_is_better": true,
            "entries": [
                { "score": 2873649, "at": "2026-09-12T01:18:57Z", "difficulty": "grand-prix" }
            ],
            "updated_at": "2026-09-12T01:18:57Z"
        }"#;
        let f: ScoreFile = serde_json::from_str(json).expect("an old record must still parse");
        assert_eq!(f.best_for("grand-prix"), Some(2873649));
        assert_eq!(
            f.best_time_for("grand-prix"),
            None,
            "a record with no times must report none, not zero",
        );
    }

    /// And a game that records no times writes no key at all, so the
    /// other two games' files do not grow a null they never use.
    #[test]
    fn a_run_without_a_time_writes_no_seconds_key() {
        let mut f = ScoreFile::new("omarcade-pixel-break", "Pixel Break");
        f.record(65_150);
        let json = serde_json::to_string(&f).expect("serialises");
        assert!(
            !json.contains("seconds"),
            "a game with no times should not carry the key: {json}",
        );
    }

    /// ⚠️ THE FASTEST RUN IS NOT THE HIGHEST-SCORING ONE.
    ///
    /// The table is sorted by score, so reading the top entry's time
    /// answers "how long did the best scorer take" — a different
    /// question that happens to agree often enough to hide the bug.
    #[test]
    fn the_best_time_is_the_fastest_not_the_top_scorers() {
        let mut f = ScoreFile::new("omarcade-racer", "Omaprix");
        // The high scorer took a long time; a lower score was quicker.
        f.record_run(3_000_000, "grand-prix", Some(180.0));
        f.record_run(1_000_000, "grand-prix", Some(120.5));

        assert_eq!(f.best_for("grand-prix"), Some(3_000_000));
        assert_eq!(
            f.best_time_for("grand-prix"),
            Some(120.5),
            "the best TIME must be the fastest run, not the top scorer's",
        );
    }

    /// A run that did not finish banks a score and no time.
    #[test]
    fn an_unfinished_run_contributes_no_time() {
        let mut f = ScoreFile::new("omarcade-racer", "Omaprix");
        f.record_run(500_000, "grand-prix", None);
        assert_eq!(f.best_for("grand-prix"), Some(500_000));
        assert_eq!(f.best_time_for("grand-prix"), None);

        // And once a race IS finished, that one counts.
        f.record_run(400_000, "grand-prix", Some(143.2));
        assert_eq!(f.best_time_for("grand-prix"), Some(143.2));
    }
}
