use rusqlite::{params, Connection};
use std::sync::Mutex;

use crate::chunker::chunk_and_hash;

pub struct Database {
    conn: Mutex<Connection>,
}

#[derive(Debug, Clone)]
pub struct User {
    pub id: String,
    pub email: String,
    pub password_hash: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct Note {
    pub id: String,
    pub user_id: String,
    pub content: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct Session {
    pub token: String,
    pub user_id: String,
    pub expires_at: String,
}

#[derive(Debug, Clone)]
pub struct Chunk {
    pub id: String,
    pub note_id: String,
    pub sequence: i32,
    pub chunk_type: String,
    pub heading_level: Option<i32>,
    pub content: String,
    pub content_hash: String,
    pub start_offset: i32,
    pub end_offset: i32,
    pub created_at: String,
    pub updated_at: String,
}

impl Database {
    pub fn open(path: &str) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn migrate(&self) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();

        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS users (
                id TEXT PRIMARY KEY,
                email TEXT UNIQUE NOT NULL,
                password_hash TEXT NOT NULL,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS sessions (
                token TEXT PRIMARY KEY,
                user_id TEXT NOT NULL REFERENCES users(id),
                expires_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS notes (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL REFERENCES users(id),
                content TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_sessions_user ON sessions(user_id);
            CREATE INDEX IF NOT EXISTS idx_notes_user ON notes(user_id);

            CREATE TABLE IF NOT EXISTS chunks (
                id TEXT PRIMARY KEY,
                note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
                sequence INTEGER NOT NULL,
                chunk_type TEXT NOT NULL,
                heading_level INTEGER,
                content TEXT NOT NULL,
                content_hash TEXT NOT NULL,
                start_offset INTEGER NOT NULL,
                end_offset INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_chunks_note ON chunks(note_id);
            ",
        )?;

        Ok(())
    }

    // Users
    pub fn create_user(
        &self,
        id: &str,
        email: &str,
        password_hash: &str,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO users (id, email, password_hash, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![id, email, password_hash, now],
        )?;

        Ok(())
    }

    pub fn get_user_by_email(&self, email: &str) -> Result<Option<User>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn
            .prepare("SELECT id, email, password_hash, created_at FROM users WHERE email = ?1")?;
        let mut rows = stmt.query(params![email])?;

        if let Some(row) = rows.next()? {
            Ok(Some(User {
                id: row.get(0)?,
                email: row.get(1)?,
                password_hash: row.get(2)?,
                created_at: row.get(3)?,
            }))
        } else {
            Ok(None)
        }
    }

    // Sessions
    pub fn create_session(
        &self,
        token: &str,
        user_id: &str,
        expires_at: &str,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "INSERT INTO sessions (token, user_id, expires_at) VALUES (?1, ?2, ?3)",
            params![token, user_id, expires_at],
        )?;

        Ok(())
    }

    pub fn get_session(&self, token: &str) -> Result<Option<Session>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();

        let mut stmt =
            conn.prepare("SELECT token, user_id, expires_at FROM sessions WHERE token = ?1")?;
        let mut rows = stmt.query(params![token])?;

        if let Some(row) = rows.next()? {
            Ok(Some(Session {
                token: row.get(0)?,
                user_id: row.get(1)?,
                expires_at: row.get(2)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn delete_session(&self, token: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM sessions WHERE token = ?1", params![token])?;
        Ok(())
    }

    // Notes
    pub fn get_or_create_note(&self, user_id: &str) -> Result<Note, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();

        // Try to get existing note
        let mut stmt = conn.prepare(
            "SELECT id, user_id, content, created_at, updated_at FROM notes WHERE user_id = ?1 LIMIT 1"
        )?;
        let mut rows = stmt.query(params![user_id])?;

        if let Some(row) = rows.next()? {
            return Ok(Note {
                id: row.get(0)?,
                user_id: row.get(1)?,
                content: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            });
        }

        drop(rows);
        drop(stmt);

        // Create new note
        let id = ulid::Ulid::new().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO notes (id, user_id, content, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, user_id, "", now, now],
        )?;

        Ok(Note {
            id,
            user_id: user_id.to_string(),
            content: String::new(),
            created_at: now.clone(),
            updated_at: now,
        })
    }

    pub fn update_note(&self, user_id: &str, content: &str) -> Result<Note, rusqlite::Error> {
        // Ensure note exists
        let note = self.get_or_create_note(user_id)?;

        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();

        // Simple update - no auto-versioning, last write wins
        conn.execute(
            "UPDATE notes SET content = ?1, updated_at = ?2 WHERE user_id = ?3",
            params![content, now, user_id],
        )?;

        drop(conn);

        // Update chunks
        self.replace_chunks(&note.id, content)?;

        self.get_or_create_note(user_id)
    }

    // Chunks
    pub fn replace_chunks(&self, note_id: &str, content: &str) -> Result<Vec<Chunk>, rusqlite::Error> {
        let new_chunks = chunk_and_hash(content);
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();

        // Get existing chunks with their hashes
        let mut existing_hashes: std::collections::HashMap<String, Chunk> = std::collections::HashMap::new();
        {
            let mut stmt = conn.prepare(
                "SELECT id, note_id, sequence, chunk_type, heading_level, content, content_hash, start_offset, end_offset, created_at, updated_at
                 FROM chunks WHERE note_id = ?1"
            )?;
            let mut rows = stmt.query(params![note_id])?;
            while let Some(row) = rows.next()? {
                let chunk = Chunk {
                    id: row.get(0)?,
                    note_id: row.get(1)?,
                    sequence: row.get(2)?,
                    chunk_type: row.get(3)?,
                    heading_level: row.get(4)?,
                    content: row.get(5)?,
                    content_hash: row.get(6)?,
                    start_offset: row.get(7)?,
                    end_offset: row.get(8)?,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                };
                existing_hashes.insert(chunk.content_hash.clone(), chunk);
            }
        }

        // Delete all existing chunks for this note
        conn.execute("DELETE FROM chunks WHERE note_id = ?1", params![note_id])?;

        // Insert new chunks, reusing timestamps for unchanged content
        let mut result = Vec::new();
        for (seq, chunk_with_hash) in new_chunks.iter().enumerate() {
            let id = ulid::Ulid::new().to_string();
            let chunk = &chunk_with_hash.chunk;

            // Check if content existed before (by hash)
            let (created_at, updated_at) = if let Some(existing) = existing_hashes.get(&chunk_with_hash.content_hash) {
                // Content unchanged - preserve original timestamps
                (existing.created_at.clone(), existing.updated_at.clone())
            } else {
                // New or modified content
                (now.clone(), now.clone())
            };

            conn.execute(
                "INSERT INTO chunks (id, note_id, sequence, chunk_type, heading_level, content, content_hash, start_offset, end_offset, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    id,
                    note_id,
                    seq as i32,
                    chunk.chunk_type.as_str(),
                    chunk.heading_level.map(|l| l as i32),
                    chunk.content,
                    chunk_with_hash.content_hash,
                    chunk.start_offset as i32,
                    chunk.end_offset as i32,
                    created_at,
                    updated_at,
                ],
            )?;

            result.push(Chunk {
                id,
                note_id: note_id.to_string(),
                sequence: seq as i32,
                chunk_type: chunk.chunk_type.as_str().to_string(),
                heading_level: chunk.heading_level.map(|l| l as i32),
                content: chunk.content.clone(),
                content_hash: chunk_with_hash.content_hash.clone(),
                start_offset: chunk.start_offset as i32,
                end_offset: chunk.end_offset as i32,
                created_at,
                updated_at,
            });
        }

        Ok(result)
    }

    pub fn get_chunks(&self, note_id: &str) -> Result<Vec<Chunk>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, note_id, sequence, chunk_type, heading_level, content, content_hash, start_offset, end_offset, created_at, updated_at
             FROM chunks WHERE note_id = ?1 ORDER BY sequence"
        )?;
        let mut rows = stmt.query(params![note_id])?;
        let mut chunks = Vec::new();
        while let Some(row) = rows.next()? {
            chunks.push(Chunk {
                id: row.get(0)?,
                note_id: row.get(1)?,
                sequence: row.get(2)?,
                chunk_type: row.get(3)?,
                heading_level: row.get(4)?,
                content: row.get(5)?,
                content_hash: row.get(6)?,
                start_offset: row.get(7)?,
                end_offset: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            });
        }
        Ok(chunks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_crud() {
        let db = Database::open(":memory:").unwrap();
        db.migrate().unwrap();

        // Create user
        db.create_user("user1", "test@example.com", "hash123")
            .unwrap();

        // Get user
        let user = db.get_user_by_email("test@example.com").unwrap().unwrap();
        assert_eq!(user.id, "user1");
        assert_eq!(user.email, "test@example.com");

        // User not found
        let not_found = db.get_user_by_email("other@example.com").unwrap();
        assert!(not_found.is_none());
    }

    #[test]
    fn test_session_crud() {
        let db = Database::open(":memory:").unwrap();
        db.migrate().unwrap();

        db.create_user("user1", "test@example.com", "hash").unwrap();
        db.create_session("token123", "user1", "2030-01-01T00:00:00Z")
            .unwrap();

        let session = db.get_session("token123").unwrap().unwrap();
        assert_eq!(session.user_id, "user1");

        db.delete_session("token123").unwrap();
        assert!(db.get_session("token123").unwrap().is_none());
    }

    #[test]
    fn test_note_crud() {
        let db = Database::open(":memory:").unwrap();
        db.migrate().unwrap();

        db.create_user("user1", "test@example.com", "hash").unwrap();

        // Get or create
        let note = db.get_or_create_note("user1").unwrap();
        assert_eq!(note.user_id, "user1");
        assert_eq!(note.content, "");

        // Update note
        let updated = db.update_note("user1", "Hello world").unwrap();
        assert_eq!(updated.content, "Hello world");

        // Get again returns same note
        let same = db.get_or_create_note("user1").unwrap();
        assert_eq!(same.id, note.id);
        assert_eq!(same.content, "Hello world");
    }
}
