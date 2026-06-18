use speak_up_core::ipc::DictationEntry;

pub struct HistoryStore {
    conn: rusqlite::Connection,
}

impl HistoryStore {
    pub fn new(db_path: &str) -> Result<Self, rusqlite::Error> {
        let conn = rusqlite::Connection::open(db_path)?;
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

    pub fn insert_entry(
        &self,
        timestamp: &str,
        raw_text: &str,
        cleaned_text: &str,
        app_context: Option<&str>,
        profile_used: Option<&str>,
        asr_provider: Option<&str>,
        cleaner_provider: Option<&str>,
        duration_ms: Option<i64>,
    ) -> Result<i64, rusqlite::Error> {
        self.conn.execute(
            "INSERT INTO dictations (timestamp, raw_text, cleaned_text, app_context, profile_used, asr_provider, cleaner_provider, duration_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![timestamp, raw_text, cleaned_text, app_context, profile_used, asr_provider, cleaner_provider, duration_ms],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn query_recent(
        &self,
        limit: usize,
        offset: usize,
        search_term: Option<&str>,
    ) -> (Vec<DictationEntry>, usize) {
        let total: usize = match &search_term {
            Some(term) => {
                let pattern = format!("%{}%", term);
                self.conn
                    .query_row(
                        "SELECT COUNT(*) FROM dictations WHERE raw_text LIKE ?1 OR cleaned_text LIKE ?1",
                        rusqlite::params![pattern],
                        |row| row.get(0),
                    )
                    .unwrap_or(0)
            }
            None => {
                self.conn
                    .query_row("SELECT COUNT(*) FROM dictations", [], |row| row.get(0))
                    .unwrap_or(0)
            }
        };

        match &search_term {
            Some(term) => {
                let pattern = format!("%{}%", term);
                let mut stmt = self
                    .conn
                    .prepare(
                        "SELECT id, timestamp, raw_text, cleaned_text, app_context, profile_used, asr_provider, cleaner_provider, duration_ms
                         FROM dictations
                         WHERE raw_text LIKE ?1 OR cleaned_text LIKE ?1
                         ORDER BY timestamp DESC
                         LIMIT ?2 OFFSET ?3",
                    )
                    .unwrap();
                let rows = stmt
                    .query_map(rusqlite::params![pattern, limit as i64, offset as i64], Self::map_row)
                    .unwrap()
                    .filter_map(|r| r.ok())
                    .collect();
                (rows, total)
            }
            None => {
                let mut stmt = self
                    .conn
                    .prepare(
                        "SELECT id, timestamp, raw_text, cleaned_text, app_context, profile_used, asr_provider, cleaner_provider, duration_ms
                         FROM dictations
                         ORDER BY timestamp DESC
                         LIMIT ?1 OFFSET ?2",
                    )
                    .unwrap();
                let rows = stmt
                    .query_map(rusqlite::params![limit as i64, offset as i64], Self::map_row)
                    .unwrap()
                    .filter_map(|r| r.ok())
                    .collect();
                (rows, total)
            }
        }
    }

    fn map_row(row: &rusqlite::Row) -> rusqlite::Result<DictationEntry> {
        Ok(DictationEntry {
            id: row.get(0)?,
            timestamp: row.get(1)?,
            raw_text: row.get(2)?,
            cleaned_text: row.get(3)?,
            app_context: row.get(4)?,
            profile_used: row.get(5)?,
            asr_provider: row.get(6)?,
            cleaner_provider: row.get(7)?,
            duration_ms: row.get(8)?,
        })
    }

    pub fn get_last_dictation(&self) -> Option<DictationEntry> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, timestamp, raw_text, cleaned_text, app_context, profile_used, asr_provider, cleaner_provider, duration_ms
                 FROM dictations ORDER BY timestamp DESC LIMIT 1",
            )
            .ok()?;
        let result: Vec<DictationEntry> = stmt
            .query_map([], Self::map_row)
            .ok()?
            .filter_map(|r| r.ok())
            .collect();
        result.into_iter().next()
    }

    pub fn prune_older_than(&self, days: u32) -> Result<usize, rusqlite::Error> {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
        let cutoff_str = cutoff.to_rfc3339();
        let deleted = self
            .conn
            .execute("DELETE FROM dictations WHERE timestamp < ?1", rusqlite::params![cutoff_str])?;
        Ok(deleted)
    }

    pub fn clear_all(&self) -> Result<usize, rusqlite::Error> {
        let deleted = self.conn.execute("DELETE FROM dictations", [])?;
        Ok(deleted)
    }
}
