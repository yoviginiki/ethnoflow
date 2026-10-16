//! SQLite metadata (`project.db`): takes, checksums, settings, schema migrations.
//!
//! Every change is its own committed transaction, so the database is always saved;
//! there is no separate "unsaved" state to lose.

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use crate::error::{ProjectError, Result};

pub const DB_FILE: &str = "project.db";

/// Ordered migrations. Index i upgrades the schema from version i to i + 1.
/// Never edit a released entry; append a new one.
const MIGRATIONS: &[&str] = &[
    // v1
    "CREATE TABLE takes (
        id              TEXT PRIMARY KEY,
        number          INTEGER NOT NULL,
        file            TEXT NOT NULL UNIQUE,
        label           TEXT,
        source          TEXT NOT NULL,
        state           TEXT NOT NULL,
        created_at_ms   INTEGER NOT NULL,
        sample_rate     INTEGER,
        channels        INTEGER,
        frames          INTEGER,
        blake3          TEXT,
        original_name   TEXT,
        device          TEXT,
        overrun_samples INTEGER NOT NULL DEFAULT 0,
        clipped_samples INTEGER NOT NULL DEFAULT 0,
        trim_in_frames  INTEGER NOT NULL DEFAULT 0,
        trim_out_frames INTEGER
    );
    CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
    // v2: takes cut out of another take keep a link to their parent
    "ALTER TABLE takes ADD COLUMN derived_from TEXT;",
];

pub fn schema_version() -> u32 {
    MIGRATIONS.len() as u32
}

pub fn open(path: &std::path::Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    // Rollback journal (not WAL) keeps the project a self-contained folder to copy.
    conn.pragma_update(None, "journal_mode", "DELETE")?;
    conn.pragma_update(None, "synchronous", "FULL")?;
    conn.pragma_update(None, "foreign_keys", true)?;
    migrate(&conn)?;
    Ok(conn)
}

/// Bring the schema to the current version. Returns the version found before migrating.
pub fn migrate(conn: &Connection) -> Result<u32> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            applied_at_ms INTEGER NOT NULL
        );",
    )?;
    let current: u32 = conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
        [],
        |r| r.get(0),
    )?;
    if current > schema_version() {
        return Err(ProjectError::NewerSchema {
            what: "project.db",
            found: current,
            supported: schema_version(),
        });
    }
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(sql)?;
        tx.execute(
            "INSERT INTO schema_migrations (version, applied_at_ms) VALUES (?1, ?2)",
            params![i as u32 + 1, crate::now_ms()],
        )?;
        tx.commit()?;
    }
    Ok(current)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TakeSource {
    Microphone,
    ImportedFile,
    /// Cut out of another take (`derived_from`); stored under `assets/derived/`.
    Derived,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TakeState {
    /// File is being written (or the app died while writing it).
    Recording,
    /// Finished normally; checksum recorded.
    Complete,
    /// Found unfinished after a crash and repaired; checksum recorded.
    Recovered,
}

fn source_str(s: TakeSource) -> &'static str {
    match s {
        TakeSource::Microphone => "microphone",
        TakeSource::ImportedFile => "imported_file",
        TakeSource::Derived => "derived",
    }
}

fn state_str(s: TakeState) -> &'static str {
    match s {
        TakeState::Recording => "recording",
        TakeState::Complete => "complete",
        TakeState::Recovered => "recovered",
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Take {
    pub id: String,
    /// 1-based sequence number within the project, for default display names.
    pub number: u32,
    /// Path relative to the project root, with forward slashes.
    pub file: String,
    pub label: Option<String>,
    pub source: TakeSource,
    pub state: TakeState,
    pub created_at_ms: i64,
    pub sample_rate: Option<u32>,
    pub channels: Option<u16>,
    pub frames: Option<u64>,
    pub blake3: Option<String>,
    pub original_name: Option<String>,
    pub device: Option<String>,
    pub overrun_samples: u64,
    pub clipped_samples: u64,
    pub trim_in_frames: u64,
    pub trim_out_frames: Option<u64>,
    /// Parent take id for `TakeSource::Derived`.
    pub derived_from: Option<String>,
}

const TAKE_COLS: &str = "id, number, file, label, source, state, created_at_ms, sample_rate, \
    channels, frames, blake3, original_name, device, overrun_samples, clipped_samples, \
    trim_in_frames, trim_out_frames, derived_from";

fn row_to_take(r: &Row) -> rusqlite::Result<Take> {
    let source: String = r.get(4)?;
    let state: String = r.get(5)?;
    Ok(Take {
        id: r.get(0)?,
        number: r.get(1)?,
        file: r.get(2)?,
        label: r.get(3)?,
        source: match source.as_str() {
            "imported_file" => TakeSource::ImportedFile,
            "derived" => TakeSource::Derived,
            _ => TakeSource::Microphone,
        },
        state: match state.as_str() {
            "complete" => TakeState::Complete,
            "recovered" => TakeState::Recovered,
            _ => TakeState::Recording,
        },
        created_at_ms: r.get(6)?,
        sample_rate: r.get(7)?,
        channels: r.get(8)?,
        frames: r.get(9)?,
        blake3: r.get(10)?,
        original_name: r.get(11)?,
        device: r.get(12)?,
        overrun_samples: r.get(13)?,
        clipped_samples: r.get(14)?,
        trim_in_frames: r.get(15)?,
        trim_out_frames: r.get(16)?,
        derived_from: r.get(17)?,
    })
}

pub struct NewTake<'a> {
    pub id: &'a str,
    pub file: &'a str,
    pub source: TakeSource,
    pub state: TakeState,
    pub original_name: Option<&'a str>,
    pub device: Option<&'a str>,
    pub derived_from: Option<&'a str>,
}

pub fn insert_take(conn: &Connection, t: &NewTake) -> Result<Take> {
    let tx = conn.unchecked_transaction()?;
    let number: u32 = tx.query_row("SELECT COALESCE(MAX(number), 0) + 1 FROM takes", [], |r| {
        r.get(0)
    })?;
    tx.execute(
        "INSERT INTO takes (id, number, file, source, state, created_at_ms, original_name, device,
            derived_from)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            t.id,
            number,
            t.file,
            source_str(t.source),
            state_str(t.state),
            crate::now_ms(),
            t.original_name,
            t.device,
            t.derived_from
        ],
    )?;
    tx.commit()?;
    get_take(conn, t.id)
}

pub struct AudioFacts {
    pub sample_rate: u32,
    pub channels: u16,
    pub frames: u64,
    pub blake3: String,
}

pub fn set_take_audio(
    conn: &Connection,
    id: &str,
    state: TakeState,
    facts: &AudioFacts,
    overrun_samples: u64,
    clipped_samples: u64,
) -> Result<Take> {
    let n = conn.execute(
        "UPDATE takes SET state = ?2, sample_rate = ?3, channels = ?4, frames = ?5, blake3 = ?6,
            overrun_samples = ?7, clipped_samples = ?8 WHERE id = ?1",
        params![
            id,
            state_str(state),
            facts.sample_rate,
            facts.channels,
            facts.frames,
            facts.blake3,
            overrun_samples,
            clipped_samples
        ],
    )?;
    if n == 0 {
        return Err(ProjectError::TakeNotFound(id.into()));
    }
    get_take(conn, id)
}

pub fn get_take(conn: &Connection, id: &str) -> Result<Take> {
    conn.query_row(
        &format!("SELECT {TAKE_COLS} FROM takes WHERE id = ?1"),
        [id],
        row_to_take,
    )
    .optional()?
    .ok_or_else(|| ProjectError::TakeNotFound(id.into()))
}

pub fn list_takes(conn: &Connection) -> Result<Vec<Take>> {
    let mut stmt = conn.prepare(&format!("SELECT {TAKE_COLS} FROM takes ORDER BY number"))?;
    let rows = stmt.query_map([], row_to_take)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn set_label(conn: &Connection, id: &str, label: Option<&str>) -> Result<Take> {
    if conn.execute(
        "UPDATE takes SET label = ?2 WHERE id = ?1",
        params![id, label],
    )? == 0
    {
        return Err(ProjectError::TakeNotFound(id.into()));
    }
    get_take(conn, id)
}

pub fn set_trim(conn: &Connection, id: &str, trim_in: u64, trim_out: Option<u64>) -> Result<Take> {
    if conn.execute(
        "UPDATE takes SET trim_in_frames = ?2, trim_out_frames = ?3 WHERE id = ?1",
        params![id, trim_in, trim_out],
    )? == 0
    {
        return Err(ProjectError::TakeNotFound(id.into()));
    }
    get_take(conn, id)
}

pub fn delete_take(conn: &Connection, id: &str) -> Result<()> {
    if conn.execute("DELETE FROM takes WHERE id = ?1", [id])? == 0 {
        return Err(ProjectError::TakeNotFound(id.into()));
    }
    Ok(())
}

pub fn get_setting(conn: &Connection, key: &str) -> Result<Option<String>> {
    Ok(conn
        .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| {
            r.get(0)
        })
        .optional()?)
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_empty_database_to_current_version() {
        let conn = Connection::open_in_memory().unwrap();
        assert_eq!(migrate(&conn).unwrap(), 0);
        let v: u32 = conn
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(v, schema_version());
        // Idempotent.
        assert_eq!(migrate(&conn).unwrap(), schema_version());
    }

    /// A database left by the v1 app (only the first migration applied) must upgrade in
    /// place: existing rows survive and the new column reads as NULL.
    #[test]
    fn v1_database_upgrades_to_current() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, applied_at_ms INTEGER NOT NULL);",
        )
        .unwrap();
        conn.execute_batch(MIGRATIONS[0]).unwrap();
        conn.execute("INSERT INTO schema_migrations VALUES (1, 0)", [])
            .unwrap();
        conn.execute(
            "INSERT INTO takes (id, number, file, source, state, created_at_ms)
             VALUES ('old', 1, 'assets/originals/old.wav', 'microphone', 'complete', 0)",
            [],
        )
        .unwrap();
        assert_eq!(migrate(&conn).unwrap(), 1);
        let t = get_take(&conn, "old").unwrap();
        assert_eq!(t.derived_from, None);
        assert_eq!(t.source, TakeSource::Microphone);
        let v: u32 = conn
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(v, schema_version());
    }

    #[test]
    fn newer_database_is_refused() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        conn.execute(
            "INSERT INTO schema_migrations VALUES (?1, 0)",
            [schema_version() + 1],
        )
        .unwrap();
        assert!(matches!(
            migrate(&conn),
            Err(ProjectError::NewerSchema { .. })
        ));
    }

    #[test]
    fn take_rows_roundtrip_and_number_sequentially() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        let a = insert_take(
            &conn,
            &NewTake {
                id: "a",
                file: "assets/originals/a.wav",
                source: TakeSource::Microphone,
                state: TakeState::Recording,
                original_name: None,
                device: Some("Mic"),
                derived_from: None,
            },
        )
        .unwrap();
        assert_eq!(
            (a.number, a.state, a.frames),
            (1, TakeState::Recording, None)
        );
        let b = insert_take(
            &conn,
            &NewTake {
                id: "b",
                file: "assets/originals/b.mp3",
                source: TakeSource::ImportedFile,
                state: TakeState::Complete,
                original_name: Some("re.mp3"),
                device: None,
                derived_from: None,
            },
        )
        .unwrap();
        assert_eq!(b.number, 2);
        let facts = AudioFacts {
            sample_rate: 48_000,
            channels: 2,
            frames: 123,
            blake3: "abc".into(),
        };
        let a = set_take_audio(&conn, "a", TakeState::Complete, &facts, 4, 5).unwrap();
        assert_eq!(a.frames, Some(123));
        assert_eq!(a.overrun_samples, 4);
        let a = set_label(&conn, "a", Some("Първи")).unwrap();
        assert_eq!(a.label.as_deref(), Some("Първи"));
        let a = set_trim(&conn, "a", 10, Some(100)).unwrap();
        assert_eq!((a.trim_in_frames, a.trim_out_frames), (10, Some(100)));
        assert_eq!(list_takes(&conn).unwrap().len(), 2);
        delete_take(&conn, "a").unwrap();
        assert!(matches!(
            get_take(&conn, "a"),
            Err(ProjectError::TakeNotFound(_))
        ));
        assert!(set_label(&conn, "zzz", None).is_err());
    }

    #[test]
    fn settings_upsert() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        assert_eq!(get_setting(&conn, "k").unwrap(), None);
        set_setting(&conn, "k", "1").unwrap();
        set_setting(&conn, "k", "2").unwrap();
        assert_eq!(get_setting(&conn, "k").unwrap().as_deref(), Some("2"));
    }
}
