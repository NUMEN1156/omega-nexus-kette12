use async_trait::async_trait;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{error, info};

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS outbox_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    topic TEXT NOT NULL,
    payload BLOB NOT NULL,
    created_at INTEGER NOT NULL,
    attempts INTEGER NOT NULL DEFAULT 0,
    published_at INTEGER,
    last_error TEXT
);
CREATE INDEX IF NOT EXISTS idx_outbox_pending ON outbox_events (published_at, id);
CREATE INDEX IF NOT EXISTS idx_outbox_topic ON outbox_events (topic, published_at, id);
"#;

#[derive(Clone, Debug)]
pub struct OutboxEvent {
    pub id: i64,
    pub topic: String,
    pub payload: Vec<u8>,
    pub created_at: i64,
    pub attempts: i64,
}

#[derive(Clone)]
pub struct OutboxWriter {
    path: PathBuf,
    connection: Arc<Mutex<Connection>>,
}

impl OutboxWriter {
    pub fn open(path: impl AsRef<Path>) -> rusqlite::Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            }
        }
        let connection = Connection::open(&path)?;
        connection.execute_batch(SCHEMA)?;
        Ok(Self {
            path,
            connection: Arc::new(Mutex::new(connection)),
        })
    }

    pub fn in_memory() -> rusqlite::Result<Self> {
        let connection = Connection::open_in_memory()?;
        connection.execute_batch(SCHEMA)?;
        Ok(Self {
            path: PathBuf::from(":memory:"),
            connection: Arc::new(Mutex::new(connection)),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn enqueue(&self, topic: &str, payload: &[u8]) -> rusqlite::Result<i64> {
        let created_at = now_secs();
        let connection = self.connection.lock().expect("outbox mutex poisoned");
        connection.execute(
            "INSERT INTO outbox_events (topic, payload, created_at) VALUES (?1, ?2, ?3)",
            params![topic, payload, created_at],
        )?;
        Ok(connection.last_insert_rowid())
    }

    pub fn pending(&self, limit: usize) -> rusqlite::Result<Vec<OutboxEvent>> {
        let connection = self.connection.lock().expect("outbox mutex poisoned");
        let mut statement = connection.prepare(
            "SELECT id, topic, payload, created_at, attempts
             FROM outbox_events WHERE published_at IS NULL ORDER BY id LIMIT ?1",
        )?;
        let rows = statement.query_map(params![limit as i64], |row| {
            Ok(OutboxEvent {
                id: row.get(0)?,
                topic: row.get(1)?,
                payload: row.get(2)?,
                created_at: row.get(3)?,
                attempts: row.get(4)?,
            })
        })?;
        rows.collect()
    }

    pub fn mark_published(&self, id: i64) -> rusqlite::Result<()> {
        let connection = self.connection.lock().expect("outbox mutex poisoned");
        connection.execute(
            "UPDATE outbox_events SET published_at = ?1, attempts = attempts + 1, last_error = NULL WHERE id = ?2",
            params![now_secs(), id],
        )?;
        Ok(())
    }

    pub fn mark_failed(&self, id: i64, message: &str) -> rusqlite::Result<()> {
        let connection = self.connection.lock().expect("outbox mutex poisoned");
        connection.execute(
            "UPDATE outbox_events SET attempts = attempts + 1, last_error = ?1 WHERE id = ?2",
            params![message, id],
        )?;
        Ok(())
    }

    pub fn pending_count(&self) -> rusqlite::Result<i64> {
        let connection = self.connection.lock().expect("outbox mutex poisoned");
        connection.query_row(
            "SELECT COUNT(*) FROM outbox_events WHERE published_at IS NULL",
            [],
            |row| row.get(0),
        )
    }
}

#[async_trait]
pub trait L1Sink: Send + Sync {
    async fn publish(&self, event: &OutboxEvent) -> Result<(), String>;
}

#[derive(Default)]
pub struct NoopSink;

#[async_trait]
impl L1Sink for NoopSink {
    async fn publish(&self, _event: &OutboxEvent) -> Result<(), String> {
        Ok(())
    }
}

pub struct LogSink;

#[async_trait]
impl L1Sink for LogSink {
    async fn publish(&self, event: &OutboxEvent) -> Result<(), String> {
        info!(event_id = event.id, topic = %event.topic, bytes = event.payload.len(), "outbox event published to log sink");
        Ok(())
    }
}

pub struct OutboxDispatcher<S> {
    writer: OutboxWriter,
    sink: Arc<S>,
    batch_size: usize,
}

impl<S: L1Sink + 'static> OutboxDispatcher<S> {
    pub fn new(writer: OutboxWriter, sink: Arc<S>, batch_size: usize) -> Self {
        Self {
            writer,
            sink,
            batch_size: batch_size.max(1),
        }
    }

    pub async fn drain_once(&self) -> usize {
        let events = match self.writer.pending(self.batch_size) {
            Ok(events) => events,
            Err(error) => {
                error!(%error, "failed to read outbox");
                return 0;
            }
        };
        let mut published = 0;
        for event in events {
            match self.sink.publish(&event).await {
                Ok(()) => {
                    if self.writer.mark_published(event.id).is_ok() {
                        published += 1;
                    }
                }
                Err(error) => {
                    let _ = self.writer.mark_failed(event.id, &error);
                }
            }
        }
        published
    }
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn writes_and_drains_event() {
        let writer = OutboxWriter::in_memory().unwrap();
        let id = writer
            .enqueue("clap.embedding.request", b"payload")
            .unwrap();
        assert_eq!(id, 1);
        assert_eq!(writer.pending_count().unwrap(), 1);

        let dispatcher = OutboxDispatcher::new(writer.clone(), Arc::new(NoopSink), 10);
        assert_eq!(dispatcher.drain_once().await, 1);
        assert_eq!(writer.pending_count().unwrap(), 0);
    }

    #[test]
    fn failed_events_remain_pending() {
        let writer = OutboxWriter::in_memory().unwrap();
        let id = writer.enqueue("topic", b"data").unwrap();
        writer.mark_failed(id, "temporary failure").unwrap();
        assert_eq!(writer.pending_count().unwrap(), 1);
        assert_eq!(writer.pending(1).unwrap()[0].attempts, 1);
    }

    #[allow(dead_code)]
    fn _optional_lookup_is_compile_checked(
        connection: &Connection,
        id: i64,
    ) -> rusqlite::Result<Option<i64>> {
        connection
            .query_row(
                "SELECT published_at FROM outbox_events WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .optional()
    }
}

// Keep the error type imported by downstream implementations without requiring a public alias.
#[allow(dead_code)]
fn _assert_error_is_send_sync(error: rusqlite::Error) -> Box<dyn std::error::Error + Send + Sync> {
    Box::new(error)
}

// Make sure the module remains useful when logging is disabled by the caller.
#[allow(dead_code)]
fn _log_sink_is_constructible() -> LogSink {
    LogSink
}
