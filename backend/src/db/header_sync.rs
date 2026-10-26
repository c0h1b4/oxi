//! Reject header batches whose IMAP reads overlap a completed cache removal.
//!
//! The generation is per user database, not per UID: a later sync may discover
//! new destination UIDs or reuse UIDs after UIDVALIDITY changes. No tombstones
//! or locks held over network calls are needed.

use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct HeaderSync {
    generation: u64,
}

impl HeaderSync {
    /// Capture before reading IMAP state, and retain for the whole sync pass.
    pub(crate) fn capture(conn: &Connection) -> Result<Self, String> {
        let generation = conn
            .query_row(
                "SELECT value FROM user_meta WHERE key = 'header_sync_generation'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|e| e.to_string())?
            .map(|value| value.parse::<u64>().map_err(|e| e.to_string()))
            .transpose()?
            .unwrap_or(0);
        Ok(Self { generation })
    }

    /// Serialize the check and all header/index/status writes with removals.
    pub(crate) fn begin_write(self, conn: &Connection) -> Result<Transaction<'_>, String> {
        let transaction = Transaction::new_unchecked(conn, TransactionBehavior::Immediate)
            .map_err(|e| e.to_string())?;
        if Self::capture(conn)?.generation != self.generation {
            return Err("Header sync superseded by cache removal; retry on next sync".into());
        }
        Ok(transaction)
    }
}

/// Must run in the same write transaction as the removal or UIDVALIDITY reset.
pub(crate) fn invalidate(conn: &Transaction<'_>) -> Result<(), String> {
    conn.execute(
        "INSERT INTO user_meta (key, value) VALUES ('header_sync_generation', '1')
         ON CONFLICT(key) DO UPDATE SET value = CAST(value AS INTEGER) + 1",
        [],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
