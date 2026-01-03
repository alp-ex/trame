use rusqlite::{params, Connection};
use std::sync::Mutex;

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
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "UPDATE notes SET content = ?1, updated_at = ?2 WHERE user_id = ?3",
            params![content, now, user_id],
        )?;

        drop(conn);
        self.get_or_create_note(user_id)
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

        // Update
        let updated = db.update_note("user1", "Hello world").unwrap();
        assert_eq!(updated.content, "Hello world");

        // Get again returns same note
        let same = db.get_or_create_note("user1").unwrap();
        assert_eq!(same.id, note.id);
        assert_eq!(same.content, "Hello world");
    }
}
