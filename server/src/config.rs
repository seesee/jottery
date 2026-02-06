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
    pub default_storage_quota_mb: i64,
    pub default_max_upload_size_mb: i64,
    pub default_inbox_max_items: i64,
    pub default_inbox_max_size_mb: i64,

    // Password policy
    // Options: "none" (length only), "basic" (2 of 4 classes), "standard" (3 of 4), "strong" (all 4)
    pub password_complexity: String,

    // Security headers
    pub enable_hsts: bool, // Strict-Transport-Security (only enable for HTTPS deployments)

    // Input validation limits
    pub max_device_name_length: usize,
    pub max_inbox_content_size: usize,
    pub max_note_content_size: usize,
    pub max_tag_length: usize,
    pub max_tags_per_note: usize,
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
                .unwrap_or_else(|_| "104857600".to_string())
                .parse()
                .unwrap_or(104_857_600), // 100MB global limit (per-user limits checked in handlers)
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
            default_max_upload_size_mb: env::var("DEFAULT_MAX_UPLOAD_SIZE_MB")
                .unwrap_or_else(|_| "5".to_string())
                .parse()
                .unwrap_or(5),
            default_inbox_max_items: env::var("DEFAULT_INBOX_MAX_ITEMS")
                .unwrap_or_else(|_| "100".to_string())
                .parse()
                .unwrap_or(100),
            default_inbox_max_size_mb: env::var("DEFAULT_INBOX_MAX_SIZE_MB")
                .unwrap_or_else(|_| "10".to_string())
                .parse()
                .unwrap_or(10),

            // Password complexity: none, basic, standard (default), strong
            password_complexity: env::var("PASSWORD_COMPLEXITY")
                .unwrap_or_else(|_| "standard".to_string())
                .to_lowercase(),

            // HSTS: Enable only for HTTPS deployments (default: false for dev compatibility)
            enable_hsts: env::var("ENABLE_HSTS")
                .unwrap_or_else(|_| "false".to_string())
                .to_lowercase()
                == "true",

            // Input validation limits
            max_device_name_length: env::var("MAX_DEVICE_NAME_LENGTH")
                .unwrap_or_else(|_| "255".to_string())
                .parse()
                .unwrap_or(255),
            max_inbox_content_size: env::var("MAX_INBOX_CONTENT_SIZE")
                .unwrap_or_else(|_| "1048576".to_string()) // 1MB
                .parse()
                .unwrap_or(1_048_576),
            max_note_content_size: env::var("MAX_NOTE_CONTENT_SIZE")
                .unwrap_or_else(|_| "10485760".to_string()) // 10MB
                .parse()
                .unwrap_or(10_485_760),
            max_tag_length: env::var("MAX_TAG_LENGTH")
                .unwrap_or_else(|_| "100".to_string())
                .parse()
                .unwrap_or(100),
            max_tags_per_note: env::var("MAX_TAGS_PER_NOTE")
                .unwrap_or_else(|_| "50".to_string())
                .parse()
                .unwrap_or(50),
        })
    }
}
