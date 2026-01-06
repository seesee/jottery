use std::env;

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub port: u16,
    pub max_payload_size: usize,
    pub cors_allowed_origins: Option<Vec<String>>,
}

impl Config {
    pub fn from_env() -> Result<Self, env::VarError> {
        dotenvy::dotenv().ok();

        // Parse CORS allowed origins from comma-separated list
        let cors_allowed_origins = env::var("CORS_ALLOWED_ORIGINS")
            .ok()
            .map(|origins| {
                origins
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            });

        Ok(Config {
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite:jottery.db".to_string()),
            port: env::var("PORT")
                .unwrap_or_else(|_| "3030".to_string())
                .parse()
                .unwrap_or(3030),
            max_payload_size: env::var("MAX_PAYLOAD_SIZE")
                .unwrap_or_else(|_| "5242880".to_string())
                .parse()
                .unwrap_or(5_242_880), // 5MB default
            cors_allowed_origins,
        })
    }
}
