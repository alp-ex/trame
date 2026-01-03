use std::env;

pub struct Config {
    pub port: u16,
    pub host: String,
    pub database_url: String,
    pub allowed_origin: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            port: env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(3000),
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            database_url: env::var("DATABASE_URL").unwrap_or_else(|_| "trame.db".to_string()),
            allowed_origin: env::var("ALLOWED_ORIGIN").unwrap_or_else(|_| "*".to_string()),
        }
    }
}
