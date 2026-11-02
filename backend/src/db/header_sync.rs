//! Reject header batches whose IMAP reads overlap a completed cache removal.
//! The per-tenant generation is captured before network I/O. Writes check it
//! under the same transaction lock used by removals.

use super::pool::{UserDb, UserTransaction};
use sqlx::Row;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct HeaderSync {
    generation: u64,
}

impl HeaderSync {
    pub(crate) async fn capture(db: &UserDb) -> Result<Self, String> {
        let mut connection = db.acquire().await?;
        let row = sqlx::query(
            "SELECT value FROM user_meta WHERE user_hash=$1 AND key='header_sync_generation'",
        )
        .bind(db.user_hash())
        .fetch_optional(&mut *connection)
        .await
        .map_err(|e| e.to_string())?;
        let generation = row
            .map(|row| row.try_get::<String, _>(0).map_err(|e| e.to_string()))
            .transpose()?
            .map(|value| value.parse::<u64>().map_err(|e| e.to_string()))
            .transpose()?
            .unwrap_or(0);
        Ok(Self { generation })
    }

    pub(crate) async fn begin_write(self, db: &UserDb) -> Result<UserTransaction, String> {
        let transaction = db.begin_write().await?;
        if Self::capture(&transaction).await?.generation != self.generation {
            return Err("Header sync superseded by cache removal; retry on next sync".into());
        }
        Ok(transaction)
    }
}

/// Must run in the same write transaction as a removal or UIDVALIDITY reset.
pub(crate) async fn invalidate(db: &UserDb) -> Result<(), String> {
    let generation = HeaderSync::capture(db)
        .await?
        .generation
        .checked_add(1)
        .ok_or("Header sync generation overflow")?;
    let mut connection = db.acquire().await?;
    sqlx::query(
        "INSERT INTO user_meta (user_hash,key,value) VALUES ($1,'header_sync_generation',$2)
        ON CONFLICT(user_hash,key) DO UPDATE SET value=excluded.value",
    )
    .bind(db.user_hash())
    .bind(generation.to_string())
    .execute(&mut *connection)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{folders, messages, pool::open_test_db};

    #[tokio::test]
    async fn cache_removal_rejects_stale_batches_but_rollback_does_not() {
        let db = open_test_db().await;
        folders::insert_folder_if_new(&db, "INBOX", None, "")
            .await
            .unwrap();
        let stale = HeaderSync::capture(&db).await.unwrap();
        messages::remove_after_mutation(&db, "INBOX", 1)
            .await
            .unwrap();
        assert!(stale.begin_write(&db).await.is_err());
        let fresh = HeaderSync::capture(&db).await.unwrap();
        let transaction = db.begin_write().await.unwrap();
        invalidate(&transaction).await.unwrap();
        drop(transaction);
        fresh
            .begin_write(&db)
            .await
            .unwrap()
            .commit()
            .await
            .unwrap();
    }
}
