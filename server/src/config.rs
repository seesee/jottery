use std::env;

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub port: u16,
    pub max_payload_size: usize,
    pub cors_allowed_origins: Option<Vec<String>>,

    // Security settings
    pub session_expiry_days: i64,
    pub default_admin_email: String,
    pub default_admin_password: String,

    // Password hashing (Argon2id parameters)
    pub argon2_m_cost: u32,  // Memory cost in KiB
    pub argon2_t_cost: u32,  // Time cost (iterations)
    pub argon2_p_cost: u32,  // Parallelism (threads)

    // User settings
    pub default_storage_quota_mb: i32,
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

            // Security settings
            session_expiry_days: env::var("SESSION_EXPIRY_DAYS")
                .unwrap_or_else(|_| "7".to_string())
                .parse()
                .unwrap_or(7),
            default_admin_email: env::var("DEFAULT_ADMIN_EMAIL")
                .unwrap_or_else(|_| "admin@localhost".to_string()),
            default_admin_password: env::var("DEFAULT_ADMIN_PASSWORD")
                .unwrap_or_else(|_| "changeme".to_string()),

            // Password hashing (Argon2id parameters)
            argon2_m_cost: env::var("ARGON2_M_COST")
                .unwrap_or_else(|_| "19456".to_string())
                .parse()
                .unwrap_or(19456), // 19 MiB
            argon2_t_cost: env::var("ARGON2_T_COST")
                .unwrap_or_else(|_| "2".to_string())
                .parse()
                .unwrap_or(2),
            argon2_p_cost: env::var("ARGON2_P_COST")
                .unwrap_or_else(|_| "1".to_string())
                .parse()
                .unwrap_or(1),

            // User settings
            default_storage_quota_mb: env::var("DEFAULT_STORAGE_QUOTA_MB")
                .unwrap_or_else(|_| "1000".to_string())
                .parse()
                .unwrap_or(1000),
        })
    }
}
