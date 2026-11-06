use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, params};

/// Colors a label can have; the UI maps each to a fill and text color.
pub const COLORS: [&str; 9] = [
    "gray", "red", "orange", "yellow", "green", "teal", "blue", "purple", "pink",
];

/// How long a labeled session may be missing from complete scans before its labels go.
pub const PRUNE_GRACE: Duration = Duration::from_secs(7 * 24 * 60 * 60);

const MAX_NAME_CHARS: usize = 60;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label {
    pub id: i64,
    pub name: String,
    pub color: String,
    /// Sessions with this label.
    pub count: u64,
}

/// User-defined labels, keyed by `Session::key()` (`agent:id`), which stays the same when an
/// agent archives or moves a session.
pub struct LabelStore {
    connection: Mutex<Connection>,
}

fn now_secs(now: SystemTime) -> i64 {
    now.duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() as i64)
}

fn clean_name(name: &str) -> Result<String, String> {
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() {
        return Err("A label needs a name.".to_string());
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(format!(
            "Label names can be at most {MAX_NAME_CHARS} characters."
        ));
    }
    Ok(name)
}

fn check_color(color: &str) -> Result<(), String> {
    if COLORS.contains(&color) {
        Ok(())
    } else {
        Err(format!("{color} isn't one of the label colors."))
    }
}

fn agent_of(key: &str) -> &str {
    key.split_once(':').map_or(key, |(agent, _)| agent)
}

fn sql_error(error: rusqlite::Error) -> String {
    match error {
        rusqlite::Error::SqliteFailure(failure, _)
            if failure.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            "There's already a label with that name.".to_string()
        }
        other => other.to_string(),
    }
}

impl LabelStore {
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|error| format!("Couldn't create {}: {error}", dir.display()))?;
        }
        Self::with_connection(Connection::open(path).map_err(|error| error.to_string())?)
    }

    pub fn in_memory() -> Result<Self, String> {
        Self::with_connection(Connection::open_in_memory().map_err(|error| error.to_string())?)
    }

    fn with_connection(connection: Connection) -> Result<Self, String> {
        connection
            .execute_batch(
                "pragma foreign_keys = on;
                 create table if not exists labels (
                   id integer primary key,
                   name text not null unique collate nocase,
                   color text not null
                 );
                 create table if not exists session_labels (
                   session_key text not null,
                   label_id integer not null references labels(id) on delete cascade,
                   primary key (session_key, label_id)
                 );
                 create table if not exists session_seen (
                   session_key text primary key,
                   last_seen integer not null
                 );",
            )
            .map_err(|error| error.to_string())?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub fn labels(&self) -> Result<Vec<Label>, String> {
        let connection = self.connection.lock().unwrap();
        let mut statement = connection
            .prepare(
                "select l.id, l.name, l.color, count(s.session_key) from labels l
                 left join session_labels s on s.label_id = l.id
                 group by l.id order by l.name collate nocase",
            )
            .map_err(sql_error)?;
        statement
            .query_map([], |row| {
                Ok(Label {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    color: row.get(2)?,
                    count: row.get::<_, i64>(3)? as u64,
                })
            })
            .map_err(sql_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(sql_error)
    }

    pub fn create(&self, name: &str, color: &str) -> Result<Label, String> {
        let name = clean_name(name)?;
        check_color(color)?;
        let connection = self.connection.lock().unwrap();
        connection
            .execute(
                "insert into labels (name, color) values (?1, ?2)",
                params![name, color],
            )
            .map_err(sql_error)?;
        Ok(Label {
            id: connection.last_insert_rowid(),
            name,
            color: color.to_string(),
            count: 0,
        })
    }

    pub fn update(&self, id: i64, name: &str, color: &str) -> Result<(), String> {
        let name = clean_name(name)?;
        check_color(color)?;
        let changed = self
            .connection
            .lock()
            .unwrap()
            .execute(
                "update labels set name = ?1, color = ?2 where id = ?3",
                params![name, color, id],
            )
            .map_err(sql_error)?;
        if changed == 0 {
            return Err("That label no longer exists.".to_string());
        }
        Ok(())
    }

    pub fn delete(&self, id: i64) -> Result<(), String> {
        self.connection
            .lock()
            .unwrap()
            .execute("delete from labels where id = ?1", [id])
            .map(|_| ())
            .map_err(sql_error)
    }

    /// Adds and removes labels on sessions in one transaction.
    pub fn apply(
        &self,
        keys: &[String],
        add: &[i64],
        remove: &[i64],
        now: SystemTime,
    ) -> Result<(), String> {
        let mut connection = self.connection.lock().unwrap();
        let transaction = connection.transaction().map_err(sql_error)?;
        for key in keys {
            for id in add {
                transaction
                    .execute(
                        "insert or ignore into session_labels (session_key, label_id) values (?1, ?2)",
                        params![key, id],
                    )
                    .map_err(sql_error)?;
            }
            for id in remove {
                transaction
                    .execute(
                        "delete from session_labels where session_key = ?1 and label_id = ?2",
                        params![key, id],
                    )
                    .map_err(sql_error)?;
            }
            transaction
                .execute(
                    "insert into session_seen (session_key, last_seen) values (?1, ?2)
                     on conflict(session_key) do update set last_seen = excluded.last_seen",
                    params![key, now_secs(now)],
                )
                .map_err(sql_error)?;
        }
        transaction.commit().map_err(sql_error)
    }

    /// Label ids per session key, for sessions that have any.
    pub fn assignments(&self) -> Result<HashMap<String, Vec<i64>>, String> {
        let connection = self.connection.lock().unwrap();
        let mut statement = connection
            .prepare("select session_key, label_id from session_labels order by label_id")
            .map_err(sql_error)?;
        let mut by_session: HashMap<String, Vec<i64>> = HashMap::new();
        for row in statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })
            .map_err(sql_error)?
        {
            let (key, id) = row.map_err(sql_error)?;
            by_session.entry(key).or_default().push(id);
        }
        Ok(by_session)
    }

    /// Drops the labels of sessions Marduk just deleted.
    pub fn forget(&self, keys: &[String]) -> Result<(), String> {
        let mut connection = self.connection.lock().unwrap();
        let transaction = connection.transaction().map_err(sql_error)?;
        for key in keys {
            transaction
                .execute("delete from session_labels where session_key = ?1", [key])
                .map_err(sql_error)?;
            transaction
                .execute("delete from session_seen where session_key = ?1", [key])
                .map_err(sql_error)?;
        }
        transaction.commit().map_err(sql_error)
    }

    /// Records which labeled sessions the scan saw, and drops labels of sessions that have
    /// been missing for longer than `grace`. Only agents whose scan was complete are judged,
    /// since an unreadable part of an agent's storage could still hold the session.
    pub fn reconcile(
        &self,
        seen: &HashSet<String>,
        complete_agents: &HashSet<&str>,
        now: SystemTime,
        grace: Duration,
    ) -> Result<usize, String> {
        let now = now_secs(now);
        let cutoff = now - grace.as_secs() as i64;
        let mut connection = self.connection.lock().unwrap();
        let transaction = connection.transaction().map_err(sql_error)?;
        let labeled: Vec<(String, Option<i64>)> = {
            let mut statement = transaction
                .prepare(
                    "select distinct l.session_key, s.last_seen from session_labels l
                     left join session_seen s on s.session_key = l.session_key",
                )
                .map_err(sql_error)?;
            statement
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                .map_err(sql_error)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(sql_error)?
        };
        let mut pruned = 0;
        for (key, last_seen) in labeled {
            if seen.contains(&key) {
                transaction
                    .execute(
                        "insert into session_seen (session_key, last_seen) values (?1, ?2)
                         on conflict(session_key) do update set last_seen = excluded.last_seen",
                        params![key, now],
                    )
                    .map_err(sql_error)?;
            } else if complete_agents.contains(agent_of(&key)) {
                match last_seen {
                    Some(last_seen) if last_seen < cutoff => {
                        transaction
                            .execute("delete from session_labels where session_key = ?1", [&key])
                            .map_err(sql_error)?;
                        transaction
                            .execute("delete from session_seen where session_key = ?1", [&key])
                            .map_err(sql_error)?;
                        pruned += 1;
                    }
                    Some(_) => {}
                    None => {
                        transaction
                            .execute(
                                "insert into session_seen (session_key, last_seen) values (?1, ?2)",
                                params![key, now],
                            )
                            .map_err(sql_error)?;
                    }
                }
            }
        }
        transaction.commit().map_err(sql_error)?;
        Ok(pruned)
    }

    pub fn label_name(&self, id: i64) -> Option<String> {
        self.connection
            .lock()
            .unwrap()
            .query_row("select name from labels where id = ?1", [id], |row| {
                row.get(0)
            })
            .optional()
            .ok()
            .flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: Duration = Duration::from_secs(24 * 60 * 60);

    fn store() -> LabelStore {
        LabelStore::in_memory().unwrap()
    }

    fn keys(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| item.to_string()).collect()
    }

    #[test]
    fn creates_lists_and_counts_labels_case_insensitively_unique() {
        let store = store();
        let work = store.create("  Work  ", "blue").unwrap();
        store.create("keep", "green").unwrap();
        assert_eq!(work.name, "Work");
        assert!(
            store
                .create("WORK", "red")
                .unwrap_err()
                .contains("already a label")
        );
        assert!(
            store
                .create("x", "neon")
                .unwrap_err()
                .contains("isn't one of")
        );
        assert!(store.create("   ", "red").is_err());

        store
            .apply(
                &keys(&["codex:a", "codex:b"]),
                &[work.id],
                &[],
                SystemTime::now(),
            )
            .unwrap();

        let labels = store.labels().unwrap();
        assert_eq!(
            labels.iter().map(|l| l.name.as_str()).collect::<Vec<_>>(),
            vec!["keep", "Work"]
        );
        assert_eq!(labels.iter().find(|l| l.id == work.id).unwrap().count, 2);
    }

    #[test]
    fn applies_and_removes_labels_and_deleting_a_label_removes_assignments() {
        let store = store();
        let work = store.create("Work", "blue").unwrap();
        let keep = store.create("Keep", "green").unwrap();
        let now = SystemTime::now();
        store
            .apply(&keys(&["codex:a"]), &[work.id, keep.id], &[], now)
            .unwrap();
        store
            .apply(&keys(&["codex:a"]), &[work.id], &[keep.id], now)
            .unwrap();
        assert_eq!(store.assignments().unwrap()["codex:a"], vec![work.id]);

        store.delete(work.id).unwrap();

        assert!(store.assignments().unwrap().is_empty());
    }

    #[test]
    fn renames_and_recolors() {
        let store = store();
        let work = store.create("Work", "blue").unwrap();
        store.update(work.id, "Job", "red").unwrap();
        assert_eq!(store.labels().unwrap()[0].name, "Job");
        assert_eq!(store.labels().unwrap()[0].color, "red");
        assert!(store.update(999, "x", "red").is_err());
    }

    #[test]
    fn forget_drops_a_deleted_sessions_labels() {
        let store = store();
        let work = store.create("Work", "blue").unwrap();
        store
            .apply(
                &keys(&["codex:a", "codex:b"]),
                &[work.id],
                &[],
                SystemTime::now(),
            )
            .unwrap();
        store.forget(&keys(&["codex:a"])).unwrap();
        assert_eq!(
            store.assignments().unwrap().keys().collect::<Vec<_>>(),
            vec!["codex:b"]
        );
    }

    #[test]
    fn prunes_only_after_the_grace_period_and_only_for_complete_agents() {
        let store = store();
        let work = store.create("Work", "blue").unwrap();
        let start = SystemTime::now();
        store
            .apply(
                &keys(&["codex:gone", "codex:here", "pi:unknown"]),
                &[work.id],
                &[],
                start,
            )
            .unwrap();
        let seen = HashSet::from(["codex:here".to_string()]);
        let complete = HashSet::from(["codex"]);

        assert_eq!(
            store
                .reconcile(&seen, &complete, start + DAY, PRUNE_GRACE)
                .unwrap(),
            0
        );
        assert_eq!(
            store
                .reconcile(&seen, &complete, start + 8 * DAY, PRUNE_GRACE)
                .unwrap(),
            1
        );

        let remaining: HashSet<String> = store.assignments().unwrap().into_keys().collect();
        assert_eq!(
            remaining,
            HashSet::from(["codex:here".into(), "pi:unknown".into()])
        );
    }

    #[test]
    fn a_session_seen_again_restarts_its_grace_period() {
        let store = store();
        let work = store.create("Work", "blue").unwrap();
        let start = SystemTime::now();
        store
            .apply(&keys(&["codex:a"]), &[work.id], &[], start)
            .unwrap();
        let complete = HashSet::from(["codex"]);
        store
            .reconcile(
                &HashSet::from(["codex:a".to_string()]),
                &complete,
                start + 6 * DAY,
                PRUNE_GRACE,
            )
            .unwrap();

        assert_eq!(
            store
                .reconcile(&HashSet::new(), &complete, start + 10 * DAY, PRUNE_GRACE)
                .unwrap(),
            0
        );
        assert_eq!(
            store
                .reconcile(&HashSet::new(), &complete, start + 14 * DAY, PRUNE_GRACE)
                .unwrap(),
            1
        );
    }

    #[test]
    fn persists_across_reopening() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("labels.db");
        let work = LabelStore::open(&path)
            .unwrap()
            .create("Work", "blue")
            .unwrap();
        let reopened = LabelStore::open(&path).unwrap();
        assert_eq!(reopened.label_name(work.id).as_deref(), Some("Work"));
    }
}
