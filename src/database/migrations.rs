use anyhow::Result;
use heed::{
    byteorder::LE,
    types::{Str, U64},
};

use std::time::Instant;

use super::USER_VERSION_KEY;

pub struct Migrations {
    #[allow(clippy::type_complexity)]
    migrations: Vec<Box<dyn Fn(&heed::Env, &mut heed::RwTxn<'_>) -> Result<()>>>,
}

impl Migrations {
    pub fn new() -> Self {
        Self {
            migrations: Vec::new(),
        }
    }

    #[allow(dead_code)]
    pub fn add(
        &mut self,
        migration: impl Fn(&heed::Env, &mut heed::RwTxn<'_>) -> Result<()> + 'static,
    ) {
        self.migrations.push(Box::new(migration));
    }

    pub fn run(&self, env: &heed::Env, wtxn: &mut heed::RwTxn<'_>) -> Result<()> {
        let now = Instant::now();

        let db = env.create_poly_database(wtxn, None)?;
        let current_version = db.get::<Str, U64<LE>>(wtxn, USER_VERSION_KEY)?.unwrap_or(0);

        if self.max_version() == current_version {
            tracing::debug!(current_version, "No migrations to run");
            return Ok(());
        }

        tracing::debug!(current_version, "Running migrations...");

        for (index, migration) in self.migrations.iter().enumerate() {
            let migration_version = index as u64 + 1;

            if migration_version > current_version {
                migration(env, wtxn)?;
                tracing::debug!(migration_version, "Migration done");

                db.put::<Str, U64<LE>>(wtxn, USER_VERSION_KEY, &migration_version)?;
            }
        }

        tracing::debug!(
            new_version = db.get::<Str, U64<LE>>(wtxn, USER_VERSION_KEY)?,
            "Done running migrations in {:?}",
            now.elapsed()
        );

        Ok(())
    }

    fn max_version(&self) -> u64 {
        self.migrations.len() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::database;

    fn current_version(env: &heed::Env, rtxn: &heed::RoTxn<'_>) -> Result<u64> {
        match env.open_poly_database(rtxn, None)? {
            Some(db) => Ok(db.get::<Str, U64<LE>>(rtxn, USER_VERSION_KEY)?.unwrap_or(0)),
            None => Ok(0),
        }
    }

    #[test]
    fn migration() {
        tracing_subscriber::fmt::init();

        let (env, _tempdir) = database::new_test_env();
        let mut wtxn = env.write_txn().unwrap();

        let migrations = Migrations::new();

        assert_eq!(migrations.max_version(), 0);
        assert_eq!(current_version(&env, &wtxn).unwrap(), 0);

        let mut migrations = Migrations::new();

        migrations.add(|_, _| Ok(()));
        migrations.add(|_, _| Ok(()));
        migrations.add(|_, _| Ok(()));

        assert_eq!(migrations.max_version(), 3);
        assert_eq!(current_version(&env, &wtxn).unwrap(), 0);

        migrations.run(&env, &mut wtxn).unwrap();

        assert_eq!(migrations.max_version(), 3);
        assert_eq!(current_version(&env, &wtxn).unwrap(), 3);

        migrations.run(&env, &mut wtxn).unwrap();

        migrations.add(|_, _| Ok(()));
        migrations.run(&env, &mut wtxn).unwrap();

        assert_eq!(migrations.max_version(), 4);
        assert_eq!(current_version(&env, &wtxn).unwrap(), 4);
    }
}
