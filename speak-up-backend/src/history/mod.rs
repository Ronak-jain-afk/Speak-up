use rusqlite::Connection;

pub struct HistoryStore {
    conn: Connection,
}

impl HistoryStore {
    pub fn new(db_path: &str) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(db_path)?;
        let store = Self { conn };
        store.initialize()?;
        Ok(store)
    }

    fn initialize(&self) -> Result<(), rusqlite::Error> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS dictations (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                raw_text TEXT NOT NULL,
                cleaned_text TEXT NOT NULL,
                app_context TEXT,
                profile_used TEXT,
                asr_provider TEXT,
                cleaner_provider TEXT,
                duration_ms INTEGER
            );
            CREATE INDEX IF NOT EXISTS idx_dictations_timestamp
                ON dictations(timestamp DESC);",
        )?;
        Ok(())
    }
}
