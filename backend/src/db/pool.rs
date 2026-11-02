use std::ops::{Deref, DerefMut};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use sqlx::any::{AnyConnectOptions, AnyPoolOptions};
use sqlx::pool::PoolConnection;
use sqlx::{Any, AnyConnection, AnyPool, Executor, Transaction};
use tokio::sync::{Mutex, MutexGuard};

/// One backend per server. SQLite retains the existing per-user file layout.
#[derive(Clone)]
pub struct Database {
    postgres: Option<AnyPool>,
    sqlite: Arc<DashMap<String, UserDb>>,
    data_dir: String,
}

impl std::fmt::Debug for Database {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Database")
            .field("postgres", &self.postgres.is_some())
            .finish()
    }
}

impl Database {
    pub async fn connect(
        data_dir: &str,
        url: Option<&str>,
        max_connections: u32,
    ) -> Result<Self, String> {
        sqlx::any::install_default_drivers();
        if max_connections == 0 {
            return Err("DATABASE_MAX_CONNECTIONS must be greater than zero".into());
        }
        let postgres = if let Some(url) = url {
            if !(url.starts_with("postgres://") || url.starts_with("postgresql://")) {
                return Err("DATABASE_URL must use postgres:// or postgresql://".into());
            }
            let options: AnyConnectOptions = url
                .parse()
                .map_err(|_| "Invalid PostgreSQL configuration")?;
            let pool = AnyPoolOptions::new()
                .max_connections(max_connections)
                .connect_with(options)
                .await
                .map_err(|_| "Could not connect to PostgreSQL")?;
            // Use the PostgreSQL migration implementation for advisory migration locking.
            let migration_pool = sqlx::postgres::PgPoolOptions::new()
                .max_connections(1)
                .connect(url)
                .await
                .map_err(|_| "Could not connect for PostgreSQL migrations")?;
            let result = sqlx::migrate!("./postgres-migrations")
                .run(&migration_pool)
                .await;
            migration_pool.close().await;
            result.map_err(|e| format!("PostgreSQL migration failed: {e}"))?;
            Some(pool)
        } else {
            None
        };
        Ok(Self {
            postgres,
            sqlite: Arc::new(DashMap::new()),
            data_dir: data_dir.into(),
        })
    }

    pub async fn user(&self, user_hash: &str) -> Result<UserDb, String> {
        if let Some(pool) = &self.postgres {
            sqlx::query(
                "INSERT INTO users (user_hash) VALUES ($1) ON CONFLICT (user_hash) DO NOTHING",
            )
            .bind(user_hash)
            .execute(pool)
            .await
            .map_err(db_error)?;
            Ok(UserDb {
                pool: pool.clone(),
                user_hash: user_hash.into(),
                postgres: true,
                transaction: None,
            })
        } else {
            if let Some(db) = self.sqlite.get(user_hash) {
                return Ok(db.clone());
            }
            let db = open_user_db(&self.data_dir, user_hash).await?;
            Ok(self.sqlite.entry(user_hash.into()).or_insert(db).clone())
        }
    }

    pub async fn ready(&self) -> Result<(), String> {
        // PostgreSQL does not replace the persistent index/attachment directory.
        tokio::fs::create_dir_all(&self.data_dir)
            .await
            .map_err(|e| e.to_string())?;
        let probe = Path::new(&self.data_dir).join(format!(".ready-{}", uuid::Uuid::new_v4()));
        tokio::fs::write(&probe, [])
            .await
            .map_err(|e| e.to_string())?;
        tokio::fs::remove_file(&probe)
            .await
            .map_err(|e| e.to_string())?;
        if let Some(pool) = &self.postgres {
            sqlx::query("SELECT 1")
                .execute(pool)
                .await
                .map_err(db_error)?;
        }
        Ok(())
    }

    pub async fn close(&self) {
        if let Some(pool) = &self.postgres {
            pool.close().await;
        }
        let pools: Vec<_> = self.sqlite.iter().map(|entry| entry.pool.clone()).collect();
        for pool in pools {
            pool.close().await;
        }
    }
}

/// A user-scoped repository context. Transaction contexts share one locked connection.
#[derive(Clone)]
pub struct UserDb {
    pool: AnyPool,
    user_hash: String,
    postgres: bool,
    transaction: Option<Arc<Mutex<Option<Transaction<'static, Any>>>>>,
}

impl UserDb {
    pub fn user_hash(&self) -> &str {
        &self.user_hash
    }
    pub fn is_postgres(&self) -> bool {
        self.postgres
    }
    pub fn sql<'a>(&self, sqlite: &'a str, postgres: &'a str) -> &'a str {
        if self.postgres { postgres } else { sqlite }
    }
    pub async fn acquire(&self) -> Result<DbConnection<'_>, String> {
        match &self.transaction {
            Some(transaction) => Ok(DbConnection::Transaction(transaction.lock().await)),
            None => Ok(DbConnection::Pool(
                self.pool.acquire().await.map_err(db_error)?,
            )),
        }
    }
    pub async fn begin_write(&self) -> Result<UserTransaction, String> {
        // Repositories may compose atomic operations inside an existing transaction.
        if self.transaction.is_some() {
            return Ok(UserTransaction {
                db: self.clone(),
                owner: false,
            });
        }
        let mut transaction = if self.postgres {
            self.pool.begin().await
        } else {
            self.pool.begin_with("BEGIN IMMEDIATE").await
        }
        .map_err(db_error)?;
        if self.postgres {
            sqlx::query("SELECT user_hash FROM users WHERE user_hash = $1 FOR UPDATE")
                .bind(&self.user_hash)
                .fetch_one(&mut *transaction)
                .await
                .map_err(db_error)?;
        }
        let mut db = self.clone();
        db.transaction = Some(Arc::new(Mutex::new(Some(transaction))));
        Ok(UserTransaction { db, owner: true })
    }
}

pub enum DbConnection<'a> {
    Pool(PoolConnection<Any>),
    Transaction(MutexGuard<'a, Option<Transaction<'static, Any>>>),
}
impl Deref for DbConnection<'_> {
    type Target = AnyConnection;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Pool(conn) => conn,
            Self::Transaction(tx) => tx.as_ref().expect("active transaction"),
        }
    }
}
impl DerefMut for DbConnection<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            Self::Pool(conn) => conn,
            Self::Transaction(tx) => tx.as_mut().expect("active transaction"),
        }
    }
}

pub struct UserTransaction {
    db: UserDb,
    owner: bool,
}
impl Deref for UserTransaction {
    type Target = UserDb;
    fn deref(&self) -> &Self::Target {
        &self.db
    }
}
impl UserTransaction {
    pub async fn commit(self) -> Result<(), String> {
        if self.owner {
            let tx = self
                .db
                .transaction
                .as_ref()
                .expect("transaction context")
                .lock()
                .await
                .take();
            tx.ok_or("Transaction already completed")?
                .commit()
                .await
                .map_err(db_error)?;
        }
        Ok(())
    }
}
// Dropping the owned SQLx Transaction queues rollback, including cancellation paths.

pub fn now() -> String {
    chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string()
}
pub fn db_error(error: sqlx::Error) -> String {
    // SQL errors never include connection options/passwords. Keep config errors separate.
    format!("Database operation failed: {error}")
}

const MIGRATIONS: &[(i64, &str)] = &[
    (1, include_str!("../../migrations/V001__initial_schema.sql")),
    (
        2,
        include_str!("../../migrations/V002__folders_and_messages.sql"),
    ),
    (3, include_str!("../../migrations/V003__add_date_epoch.sql")),
    (
        4,
        include_str!("../../migrations/V004__folder_messages_updated_at.sql"),
    ),
    (
        5,
        include_str!("../../migrations/V005__drafts_and_attachments.sql"),
    ),
    (
        6,
        include_str!("../../migrations/V006__cache_attachments_and_headers.sql"),
    ),
    (7, include_str!("../../migrations/V007__contacts.sql")),
    (8, include_str!("../../migrations/V008__identities.sql")),
    (
        9,
        include_str!("../../migrations/V009__notification_preferences.sql"),
    ),
    (
        10,
        include_str!("../../migrations/V010__contact_groups.sql"),
    ),
    (
        11,
        include_str!("../../migrations/V011__message_reaction.sql"),
    ),
    (12, include_str!("../../migrations/V012__tags.sql")),
    (
        13,
        include_str!("../../migrations/V013__display_preferences.sql"),
    ),
    (
        14,
        include_str!("../../migrations/V014__compose_format.sql"),
    ),
    (15, include_str!("../../migrations/V015__deep_index.sql")),
    (16, include_str!("../../migrations/V016__calendar.sql")),
    (
        17,
        include_str!("../../migrations/V017__add_email_theme.sql"),
    ),
    (
        18,
        include_str!("../../migrations/V018__animation_mode.sql"),
    ),
    (
        19,
        include_str!("../../migrations/V019__known_addresses.sql"),
    ),
    (
        20,
        include_str!("../../migrations/V020__database_backends.sql"),
    ),
];

async fn migrate_sqlite(pool: &AnyPool) -> Result<(), String> {
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await.map_err(db_error)?;
    let mut version: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&mut *transaction)
        .await
        .map_err(db_error)?;
    let legacy: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='refinery_schema_history'",
    )
    .fetch_one(&mut *transaction)
    .await
    .map_err(db_error)?;
    if legacy != 0 {
        let legacy_version: Option<i64> =
            sqlx::query_scalar("SELECT MAX(version) FROM refinery_schema_history")
                .fetch_one(&mut *transaction)
                .await
                .map_err(db_error)?;
        version = version.max(legacy_version.unwrap_or(0));
    }
    for &(next, sql) in MIGRATIONS {
        if next > version {
            (&mut *transaction)
                .execute(sql)
                .await
                .map_err(|e| format!("SQLite migration V{next:03} failed: {e}"))?;
            sqlx::query(&format!("PRAGMA user_version = {next}"))
                .execute(&mut *transaction)
                .await
                .map_err(db_error)?;
        }
    }
    transaction.commit().await.map_err(db_error)
}

pub async fn open_user_db(data_dir: &str, user_hash: &str) -> Result<UserDb, String> {
    sqlx::any::install_default_drivers();
    let directory = Path::new(data_dir).join(user_hash);
    tokio::fs::create_dir_all(&directory)
        .await
        .map_err(|e| format!("Could not create data directory: {e}"))?;
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(directory.join("db.sqlite"))
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(30));
    use sqlx::ConnectOptions;
    let url = options.to_url_lossy().to_string();
    let pool = AnyPoolOptions::new()
        .max_connections(4)
        .after_connect(|connection, _| {
            Box::pin(async move {
                connection.execute("PRAGMA foreign_keys=ON").await?;
                connection.execute("PRAGMA journal_mode=WAL").await?;
                connection.execute("PRAGMA busy_timeout=30000").await?;
                Ok(())
            })
        })
        .connect(&url)
        .await
        .map_err(db_error)?;
    migrate_sqlite(&pool).await?;
    Ok(UserDb {
        pool,
        user_hash: String::new(),
        postgres: false,
        transaction: None,
    })
}

#[cfg(test)]
pub async fn open_test_db() -> UserDb {
    if let Ok(url) = std::env::var("OXI_TEST_DATABASE_URL") {
        let database = Database::connect("/tmp", Some(&url), 4)
            .await
            .expect("PostgreSQL test database");
        return database
            .user(&format!("test-{}", uuid::Uuid::new_v4()))
            .await
            .expect("test tenant");
    }
    open_memory_test_db().await
}

#[cfg(test)]
async fn open_memory_test_db() -> UserDb {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("test connection");
    sqlx::query("PRAGMA foreign_keys=ON")
        .execute(&pool)
        .await
        .unwrap();
    migrate_sqlite(&pool).await.expect("test migrations");
    UserDb {
        pool,
        user_hash: String::new(),
        postgres: false,
        transaction: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::Row;
    #[tokio::test]
    async fn upgrades_legacy_refinery_and_user_version_databases_without_data_loss() {
        for refinery in [false, true] {
            sqlx::any::install_default_drivers();
            let directory = tempfile::tempdir().unwrap();
            let user_directory = directory.path().join("legacy");
            tokio::fs::create_dir(&user_directory).await.unwrap();
            let url = format!(
                "sqlite://{}?mode=rwc",
                user_directory.join("db.sqlite").display()
            );
            let pool = AnyPoolOptions::new()
                .max_connections(1)
                .connect(&url)
                .await
                .unwrap();
            for &(_, sql) in MIGRATIONS.iter().filter(|(version, _)| *version <= 19) {
                let mut conn = pool.acquire().await.unwrap();
                (&mut *conn).execute(sql).await.unwrap();
            }
            sqlx::query("INSERT INTO contacts (id,email,name) VALUES ('kept','keep@test','Kept')")
                .execute(&pool)
                .await
                .unwrap();
            sqlx::query("INSERT INTO folders (name,highest_modseq) VALUES ('INBOX',-1)")
                .execute(&pool)
                .await
                .unwrap();
            if refinery {
                sqlx::query("CREATE TABLE refinery_schema_history (version INTEGER)")
                    .execute(&pool)
                    .await
                    .unwrap();
                sqlx::query("INSERT INTO refinery_schema_history VALUES (19)")
                    .execute(&pool)
                    .await
                    .unwrap();
            } else {
                sqlx::query("PRAGMA user_version=19")
                    .execute(&pool)
                    .await
                    .unwrap();
            }
            pool.close().await;
            let db = open_user_db(directory.path().to_str().unwrap(), "legacy")
                .await
                .unwrap();
            assert_eq!(
                crate::db::contacts::get_contact(&db, "kept")
                    .await
                    .unwrap()
                    .unwrap()
                    .name,
                "Kept"
            );
            assert_eq!(
                crate::db::folders::get_folder(&db, "INBOX")
                    .await
                    .unwrap()
                    .unwrap()
                    .highest_modseq,
                u64::MAX
            );
            let mut conn = db.acquire().await.unwrap();
            let version: i64 = sqlx::query_scalar("PRAGMA user_version")
                .fetch_one(&mut *conn)
                .await
                .unwrap();
            assert_eq!(version, 20);
            drop(conn);
            drop(db);
            // Re-opening must not repeat ALTERs or lose data.
            open_user_db(directory.path().to_str().unwrap(), "legacy")
                .await
                .unwrap();
        }
    }

    #[tokio::test]
    async fn concurrent_postgres_initialization_is_idempotent_and_readiness_tracks_pool() {
        let Ok(url) = std::env::var("OXI_TEST_DATABASE_URL") else {
            return;
        };
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().to_str().unwrap();
        let (first, second) = tokio::join!(
            Database::connect(path, Some(&url), 2),
            Database::connect(path, Some(&url), 2)
        );
        let first = first.unwrap();
        let second = second.unwrap();
        let user = format!("concurrent-{}", uuid::Uuid::new_v4());
        let (a, b) = tokio::join!(first.user(&user), second.user(&user));
        assert_eq!(a.unwrap().user_hash(), b.unwrap().user_hash());
        first.ready().await.unwrap();
        first.close().await;
        assert!(first.ready().await.is_err());
        second.close().await;
    }

    #[tokio::test]
    async fn sqlite_schema_and_persistence() {
        let directory = tempfile::tempdir().unwrap();
        let db = open_user_db(directory.path().to_str().unwrap(), "alice")
            .await
            .unwrap();
        let mut connection = db.acquire().await.unwrap();
        let row = sqlx::query("SELECT value FROM user_meta WHERE key='schema_version'")
            .fetch_one(&mut *connection)
            .await
            .unwrap();
        assert_eq!(row.get::<String, _>(0), "1");
        assert!(directory.path().join("alice/db.sqlite").exists());
    }
    #[tokio::test]
    async fn async_database_futures_are_send() {
        fn assert_send<T: Send>(_: T) {}
        assert_send(open_test_db());
        assert_send(open_user_db("/tmp/oxi-send-test", "alice"));
        let db = open_test_db().await;
        assert_send(db.acquire());
        assert_send(db.begin_write());
        assert_send(crate::db::messages::upsert_message(
            &db, "INBOX", 1, None, None, None, "", "", "", "[]", "[]", "", "", 0, false, "", None,
        ));
    }
    #[tokio::test]
    async fn invalid_configuration_does_not_fall_back() {
        assert!(
            Database::connect("/tmp", Some("sqlite:unexpected"), 10)
                .await
                .is_err()
        );
        assert!(Database::connect("/tmp", None, 0).await.is_err());
    }
}
