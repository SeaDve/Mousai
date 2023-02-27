use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use serde::{Deserialize, Serialize};

use std::{
    error::Error as StdError, fmt, marker::PhantomData, path::Path, result::Result as StdResult,
    time::Instant,
};

type Result<T> = StdResult<T, DatabaseError>;

pub struct Timer {
    task_name: String,
    start_time: Instant,
}

impl Timer {
    pub fn new(task_name: &str) -> Self {
        Self {
            task_name: task_name.to_string(),
            start_time: Instant::now(),
        }
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        tracing::debug!("{} took {:?}", self.task_name, self.start_time.elapsed());
    }
}

#[derive(Debug)]
pub enum DatabaseError {
    NotFound,
    AlreadyExist,
    Internal(Box<dyn StdError + Send + Sync>),
}

impl fmt::Display for DatabaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => write!(f, "Not found"),
            Self::AlreadyExist => write!(f, "Already exist"),
            Self::Internal(err) => write!(f, "Internal error: {}", err),
        }
    }
}

impl StdError for DatabaseError {}

impl From<rusqlite::Error> for DatabaseError {
    fn from(err: rusqlite::Error) -> Self {
        Self::Internal(err.into())
    }
}

impl From<r2d2::Error> for DatabaseError {
    fn from(err: r2d2::Error) -> Self {
        Self::Internal(err.into())
    }
}

impl From<serde_json::Error> for DatabaseError {
    fn from(value: serde_json::Error) -> Self {
        Self::Internal(value.into())
    }
}

pub struct Table<T> {
    pool: Pool<SqliteConnectionManager>,
    name: String,
    data_type: PhantomData<T>,
}

impl<T> Table<T>
where
    for<'de> T: Serialize + Deserialize<'de> + 'static,
{
    fn create_if_not_exists(pool: Pool<SqliteConnectionManager>, name: &str) -> Result<Self> {
        let _timer = Timer::new("Table::create_if_not_exists");

        let conn = pool.get()?;

        conn.execute(
            &format!(
                "CREATE TABLE IF NOT EXISTS {} (id TEXT NOT NULL PRIMARY KEY, data TEXT NOT NULL)",
                name
            ),
            (),
        )?;

        Ok(Self {
            pool,
            name: name.to_string(),
            data_type: PhantomData,
        })
    }

    pub fn count(&self) -> Result<usize> {
        let _timer = Timer::new("Table::count");

        let conn = self.pool.get()?;

        let mut statement = conn.prepare_cached(&format!("SELECT COUNT(id) FROM {}", self.name))?;
        let count = statement.query_row((), |row| row.get::<_, usize>(0))?;

        Ok(count)
    }

    pub fn insert_one(&self, id: &str, data: &T) -> Result<()> {
        let _timer = Timer::new("Table::insery_one");

        let conn = self.pool.get()?;

        let mut statement = conn.prepare_cached(&format!(
            "INSERT INTO {} (id, data) VALUES (?, ?)",
            self.name
        ))?;

        let raw_data = serde_json::to_string(data)?;

        match statement.execute((id, raw_data)) {
            Ok(changed) => {
                debug_assert_eq!(changed, 1);
                Ok(())
            }
            Err(err) => {
                if let rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error {
                        code: rusqlite::ErrorCode::ConstraintViolation,
                        ..
                    },
                    message,
                ) = err
                {
                    debug_assert_eq!(
                        message,
                        Some(format!("UNIQUE constraint failed: {}.id", self.name))
                    );
                    Err(DatabaseError::AlreadyExist)
                } else {
                    Err(DatabaseError::Internal(err.into()))
                }
            }
        }
    }

    /// Note: This errors out if any of the items already exist or there
    /// are duplicates in the given items.
    pub fn insert_many<'a>(&self, items: impl IntoIterator<Item = (&'a str, &'a T)>) -> Result<()> {
        let _timer = Timer::new("Table::insery_many");

        let mut conn = self.pool.get()?;

        let transaction = conn.transaction()?;

        {
            let mut statement = transaction.prepare_cached(&format!(
                "INSERT INTO {} (id, data) VALUES (?, ?)",
                self.name
            ))?;

            for item in items.into_iter() {
                let raw_data = serde_json::to_string(&item.1)?;

                match statement.execute((item.0, raw_data)) {
                    Ok(changed) => {
                        debug_assert_eq!(changed, 1);
                        continue;
                    }
                    Err(err) => {
                        if let rusqlite::Error::SqliteFailure(
                            rusqlite::ffi::Error {
                                code: rusqlite::ErrorCode::ConstraintViolation,
                                ..
                            },
                            message,
                        ) = err
                        {
                            debug_assert_eq!(
                                message,
                                Some(format!("UNIQUE constraint failed: {}.id", self.name))
                            );
                            return Err(DatabaseError::AlreadyExist);
                        } else {
                            return Err(DatabaseError::Internal(err.into()));
                        }
                    }
                }
            }
        }
        transaction.commit()?;

        Ok(())
    }

    pub fn upsert_one(&self, id: &str, data: &T) -> Result<()> {
        let _timer = Timer::new("Table::upsert_one");

        let conn = self.pool.get()?;

        let mut statement = conn.prepare_cached(&format!(
            "INSERT OR REPLACE INTO {} (id, data) VALUES (?, ?)",
            self.name
        ))?;

        let raw_data = serde_json::to_string(data)?;

        match statement.execute((id, raw_data)) {
            Ok(changed) => {
                debug_assert_eq!(changed, 1);
                Ok(())
            }
            Err(err) => Err(DatabaseError::Internal(err.into())),
        }
    }

    pub fn upsert_many<'a>(&self, items: impl IntoIterator<Item = (&'a str, &'a T)>) -> Result<()> {
        let _timer = Timer::new("Table::upsert_many");

        let mut conn = self.pool.get()?;

        let transaction = conn.transaction()?;

        {
            let mut statement = transaction.prepare_cached(&format!(
                "INSERT OR REPLACE INTO {} (id, data) VALUES (?, ?)",
                self.name
            ))?;

            for item in items.into_iter() {
                let raw_data = serde_json::to_string(&item.1)?;

                match statement.execute((item.0, raw_data)) {
                    Ok(changed) => {
                        debug_assert_eq!(changed, 1);
                        continue;
                    }
                    Err(err) => {
                        return Err(DatabaseError::Internal(err.into()));
                    }
                }
            }
        }
        transaction.commit()?;

        Ok(())
    }

    pub fn select_one(&self, id: &str) -> Result<T> {
        let _timer = Timer::new("Table::select_one");

        let conn = self.pool.get()?;

        let mut statement =
            conn.prepare_cached(&format!("SELECT data FROM {} WHERE ID = ?", self.name))?;
        let raw_data = statement
            .query_row((id,), |row| row.get::<_, String>(0))
            .map_err(|err| {
                if err == rusqlite::Error::QueryReturnedNoRows {
                    DatabaseError::NotFound
                } else {
                    DatabaseError::Internal(err.into())
                }
            })?;
        let data = serde_json::from_str(&raw_data)?;

        Ok(data)
    }

    pub fn select_all(&self) -> Result<Vec<T>> {
        let _timer = Timer::new("Table::select_all");

        let conn = self.pool.get()?;

        let mut statement = conn.prepare_cached(&format!("SELECT data FROM {}", self.name))?;

        let vec = statement
            .query_map([], |row| {
                let raw_data = row.get::<_, String>(0)?;
                let data = serde_json::from_str::<T>(&raw_data).map_err(|err| {
                    rusqlite::Error::FromSqlConversionFailure(
                        1,
                        rusqlite::types::Type::Blob,
                        err.into(),
                    )
                })?;
                Ok(data)
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(vec)
    }

    pub fn update_one(&self, id: &str, data: &T) -> Result<()> {
        let _timer = Timer::new("Table::update_one");

        let mut conn = self.pool.get()?;

        let transaction = conn.transaction()?;

        let changed = {
            let mut statement = transaction
                .prepare_cached(&format!("UPDATE {} SET data = ? WHERE id = ?", self.name))?;

            let raw_data = serde_json::to_string(data)?;
            statement.execute((raw_data, id))?
        };

        if changed == 1 {
            transaction.commit()?;
            Ok(())
        } else {
            transaction.rollback()?;
            Err(DatabaseError::NotFound)
        }
    }

    /// Note: This does not error out if there are duplicates in the given items.
    pub fn update_many<'a>(&self, items: impl IntoIterator<Item = (&'a str, &'a T)>) -> Result<()> {
        let _timer = Timer::new("Table::update_many");

        let mut conn = self.pool.get()?;

        let transaction = conn.transaction()?;

        let mut items_len = 0;
        let mut changed = 0;

        {
            let mut statement = transaction
                .prepare_cached(&format!("UPDATE {} SET data = ? WHERE id = ?", self.name))?;

            for item in items.into_iter() {
                let raw_data = serde_json::to_string(&item.1)?;
                changed += statement.execute((raw_data, item.0))?;
                items_len += 1;
            }
        }

        if changed == items_len {
            transaction.commit()?;
            Ok(())
        } else {
            transaction.rollback()?;
            Err(DatabaseError::NotFound)
        }
    }

    pub fn delete_one(&self, id: &str) -> Result<()> {
        let _timer = Timer::new("Table::delete_one");

        let mut conn = self.pool.get()?;

        let transaction = conn.transaction()?;

        let changed = {
            let mut statement =
                transaction.prepare_cached(&format!("DELETE FROM {} WHERE ID = ?", self.name))?;
            statement.execute((id,))?
        };

        if changed == 1 {
            transaction.commit()?;
            Ok(())
        } else {
            transaction.rollback()?;
            Err(DatabaseError::NotFound)
        }
    }

    pub fn delete_all(&self) -> Result<()> {
        let _timer = Timer::new("Table::delete_all");

        let conn = self.pool.get()?;

        let mut statement = conn.prepare_cached(&format!("DELETE FROM {}", self.name))?;
        statement.execute(())?;

        Ok(())
    }
}

#[derive(Clone)]
pub struct Database {
    pool: Pool<SqliteConnectionManager>,
}

impl Drop for Database {
    fn drop(&mut self) {
        match self.pool.get() {
            Ok(conn) => {
                if let Err(err) = conn.execute("PRAGMA optimize", ()) {
                    tracing::warn!("Failed to optimize on Database drop: {:?}", err);
                }
            }
            Err(err) => tracing::warn!("Failed to get connection on Database drop: {:?}", err),
        }
    }
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let _timer = Timer::new("Database::open");

        let manager = SqliteConnectionManager::file(path);
        let pool = Pool::new(manager)?;

        let conn = pool.get()?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "normal")?;
        conn.pragma_update(None, "temp_store", "memory")?;
        conn.pragma_update(None, "mmap_size", "30000000000")?;

        Ok(Self { pool })
    }

    pub fn open_in_memory() -> Result<Self> {
        let _timer = Timer::new("Database::open_in_memory");

        let manager = SqliteConnectionManager::memory();
        let pool = Pool::new(manager)?;

        Ok(Self { pool })
    }

    pub fn table<T>(&self, name: &str) -> Result<Table<T>>
    where
        for<'de> T: Serialize + Deserialize<'de> + 'static,
    {
        Table::create_if_not_exists(self.pool.clone(), name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use gtk::gio::{self, prelude::*};
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Song {
        title: String,
    }

    impl Song {
        pub fn new(title: &str) -> Self {
            Self {
                title: title.to_string(),
            }
        }
    }

    /// Run tests both in memory and on disk.
    fn run_test<F>(f: F)
    where
        F: FnOnce(Database) + Clone,
    {
        (f.clone())(Database::open_in_memory().unwrap());

        let (tmp, _) = gio::File::new_tmp(None::<&Path>).unwrap();
        f(Database::open(tmp.path().unwrap()).unwrap());
    }

    #[test]
    fn insert() {
        run_test(|db| {
            let songs = db.table::<Song>("songs").unwrap();
            assert_eq!(songs.count().unwrap(), 0);

            songs.insert_one("1", &Song::new("1")).unwrap();
            assert_eq!(songs.count().unwrap(), 1);

            songs
                .insert_many(vec![("2", &Song::new("2")), ("3", &Song::new("3"))])
                .unwrap();
            assert_eq!(songs.count().unwrap(), 3);

            assert_eq!(songs.select_one("1").unwrap().title, "1");
            assert_eq!(songs.select_one("2").unwrap().title, "2");
            assert_eq!(songs.select_one("3").unwrap().title, "3");
        });
    }

    #[test]
    fn insert_dup_key() {
        run_test(|db| {
            let songs = db.table::<Song>("songs").unwrap();
            songs.insert_one("1", &Song::new("1")).unwrap();
            assert_eq!(songs.count().unwrap(), 1);

            assert!(matches!(
                songs.insert_one("1", &Song::new("2")).unwrap_err(),
                DatabaseError::AlreadyExist
            ));
            assert_eq!(songs.count().unwrap(), 1);
            assert_eq!(songs.select_one("1").unwrap().title, "1");

            assert!(matches!(
                songs
                    .insert_many(vec![("1", &Song::new("2")), ("2", &Song::new("2"))])
                    .unwrap_err(),
                DatabaseError::AlreadyExist
            ));
            assert_eq!(songs.count().unwrap(), 1);
            assert_eq!(songs.select_one("1").unwrap().title, "1");
        });
    }

    #[test]
    fn insert_many_dup_key_param() {
        run_test(|db| {
            let songs = db.table::<Song>("songs").unwrap();

            // FIXME Add DuplicateKeyParam error
            assert!(matches!(
                songs
                    .insert_many(vec![("1", &Song::new("2")), ("1", &Song::new("2"))])
                    .unwrap_err(),
                DatabaseError::AlreadyExist
            ));
            assert_eq!(songs.count().unwrap(), 0);
        });
    }

    #[test]
    fn upsert() {
        run_test(|db| {
            let songs = db.table::<Song>("songs").unwrap();
            assert_eq!(songs.count().unwrap(), 0);

            songs.upsert_one("1", &Song::new("1")).unwrap();
            assert_eq!(songs.count().unwrap(), 1);

            songs
                .upsert_many(vec![("2", &Song::new("2")), ("3", &Song::new("3"))])
                .unwrap();
            assert_eq!(songs.count().unwrap(), 3);

            assert_eq!(songs.select_one("1").unwrap().title, "1");
            assert_eq!(songs.select_one("2").unwrap().title, "2");
            assert_eq!(songs.select_one("3").unwrap().title, "3");

            songs.upsert_one("1", &Song::new("4")).unwrap();
            assert_eq!(songs.count().unwrap(), 3);
            assert_eq!(songs.select_one("1").unwrap().title, "4");
        });
    }

    #[test]
    fn upsert_many_dup_key_param() {
        run_test(|db| {
            let songs = db.table::<Song>("songs").unwrap();

            // FIXME Make as DuplicateKeyParam error
            songs
                .upsert_many(vec![("1", &Song::new("1")), ("1", &Song::new("updated"))])
                .unwrap();
            assert_eq!(songs.count().unwrap(), 1);
            assert_eq!(songs.select_one("1").unwrap().title, "updated");
        });
    }

    #[test]
    fn select() {
        run_test(|db| {
            let songs = db.table::<Song>("songs").unwrap();
            songs
                .insert_many(vec![
                    ("1", &Song::new("1")),
                    ("2", &Song::new("2")),
                    ("3", &Song::new("3")),
                ])
                .unwrap();

            assert_eq!(songs.select_one("1").unwrap().title, "1");
            assert_eq!(songs.select_one("2").unwrap().title, "2");
            assert_eq!(songs.select_one("3").unwrap().title, "3");

            let mut iter = songs.select_all().unwrap().into_iter();
            assert_eq!(iter.next().unwrap().title, "1");
            assert_eq!(iter.next().unwrap().title, "2");
            assert_eq!(iter.next().unwrap().title, "3");
            assert!(iter.next().is_none());
        });
    }

    #[test]
    fn select_missing() {
        run_test(|db| {
            let songs = db.table::<Song>("songs").unwrap();

            assert!(matches!(
                songs.select_one("1").unwrap_err(),
                DatabaseError::NotFound
            ));
        });
    }

    #[test]
    fn select_all_empty() {
        run_test(|db| {
            let songs = db.table::<Song>("songs").unwrap();

            assert!(songs.select_all().unwrap().is_empty());
        });
    }

    #[test]
    fn update() {
        run_test(|db| {
            let songs = db.table::<Song>("songs").unwrap();
            songs
                .insert_many(vec![
                    ("1", &Song::new("1")),
                    ("2", &Song::new("2")),
                    ("3", &Song::new("3")),
                ])
                .unwrap();

            songs.update_one("1", &Song::new("updated")).unwrap();
            assert_eq!(songs.select_one("1").unwrap().title, "updated");
            assert_eq!(songs.select_one("2").unwrap().title, "2");
            assert_eq!(songs.select_one("3").unwrap().title, "3");

            songs
                .update_many(vec![
                    ("2", &Song::new("updated")),
                    ("3", &Song::new("updated")),
                ])
                .unwrap();
            assert_eq!(songs.select_one("1").unwrap().title, "updated");
            assert_eq!(songs.select_one("2").unwrap().title, "updated");
            assert_eq!(songs.select_one("3").unwrap().title, "updated");
        });
    }

    #[test]
    fn update_missing() {
        run_test(|db| {
            let songs = db.table::<Song>("songs").unwrap();
            songs
                .insert_many(vec![("1", &Song::new("1")), ("2", &Song::new("2"))])
                .unwrap();

            assert!(matches!(
                songs.update_one("4", &Song::new("updated")).unwrap_err(),
                DatabaseError::NotFound
            ));
            assert_eq!(songs.select_one("1").unwrap().title, "1");
            assert_eq!(songs.select_one("2").unwrap().title, "2");

            assert!(matches!(
                songs
                    .update_many(vec![
                        ("1", &Song::new("updated")),
                        ("4", &Song::new("updated"))
                    ])
                    .unwrap_err(),
                DatabaseError::NotFound
            ));
            assert_eq!(songs.select_one("1").unwrap().title, "1");
            assert_eq!(songs.select_one("2").unwrap().title, "2");
        });
    }

    #[test]
    fn update_many_dup_key_param() {
        run_test(|db| {
            let songs = db.table::<Song>("songs").unwrap();
            songs
                .insert_many(vec![("1", &Song::new("1")), ("2", &Song::new("2"))])
                .unwrap();

            // FIXME Make this as error DuplicateKeyParam
            songs
                .update_many(vec![
                    ("1", &Song::new("updated")),
                    ("1", &Song::new("updated")),
                ])
                .unwrap();
            assert_eq!(songs.count().unwrap(), 2);

            assert_eq!(songs.select_one("1").unwrap().title, "updated");
            assert_eq!(songs.select_one("2").unwrap().title, "2");
        });
    }

    #[test]
    fn delete() {
        run_test(|db| {
            let songs = db.table::<Song>("songs").unwrap();
            songs
                .insert_many(vec![
                    ("1", &Song::new("1")),
                    ("2", &Song::new("2")),
                    ("3", &Song::new("3")),
                ])
                .unwrap();

            songs.delete_one("1").unwrap();
            assert_eq!(songs.count().unwrap(), 2);
            assert_eq!(songs.select_one("2").unwrap().title, "2");
            assert_eq!(songs.select_one("3").unwrap().title, "3");

            songs.delete_all().unwrap();
            assert_eq!(songs.count().unwrap(), 0);
        });
    }

    #[test]
    fn delete_missing() {
        run_test(|db| {
            let songs = db.table::<Song>("songs").unwrap();
            songs
                .insert_many(vec![("1", &Song::new("1")), ("2", &Song::new("2"))])
                .unwrap();

            assert!(matches!(
                songs.delete_one("4").unwrap_err(),
                DatabaseError::NotFound
            ));
            assert_eq!(songs.count().unwrap(), 2);
        });
    }

    #[test]
    fn delete_all_empty() {
        run_test(|db| {
            let songs = db.table::<Song>("songs").unwrap();

            songs.delete_all().unwrap();
            assert_eq!(songs.count().unwrap(), 0);
        });
    }
}
