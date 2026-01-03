pub mod config;
pub mod db;
pub mod handlers;
pub mod router;

use config::Config;
use db::Database;
use std::sync::Arc;

pub struct AppState {
    pub db: Database,
    pub config: Config,
}

impl AppState {
    pub fn new(config: Config) -> Result<Arc<Self>, rusqlite::Error> {
        let db = Database::open(&config.database_url)?;
        db.migrate()?;
        Ok(Arc::new(Self { db, config }))
    }
}
