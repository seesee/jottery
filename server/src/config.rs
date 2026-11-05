use std::env;

/// Limits on the anonymous credential routes. See `rate_limit`.
#[derive(Clone)]
pub struct RateLimitConfig {
    pub enabled: bool,
    /// Proxies in front of this server that append to X-Forwarded-For. Zero
    /// means the header is ignored, because a client can send any value.
    pub trusted_proxy_hops: usize,
    pub auth_per_minute: u32,
    pub register_per_hour: u32,
    pub status_per_minute: u32,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            trusted_proxy_hops: 0,
            // Generous enough that clients sharing one address, behind a proxy
            // this server can't see past, still don't get in each other's way.
            auth_per_minute: 20,
            register_per_hour: 20,
            status_per_minute: 60,
        }
    }
}

fn env_or<T: std::str::FromStr>(name: &str, default: T) -> T {
    env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub port: u16,
    pub max_payload_size: usize,
    pub cors_allowed_origins: Option<Vec<String>>,

    // Security settings
    pub session_expiry_days: i64,
    pub default_admin_email: String,
    /// `None` when unset. There is deliberately no default: a fresh install gets
    /// a generated password instead of a published one (see `bootstrap`).
    pub default_admin_password: Option<String>,

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

    pub rate_limit: RateLimitConfig,

    // Input validation limits
    pub max_device_name_length: usize,
    pub max_inbox_content_size: usize,
    pub max_note_content_size: usize,
    pub max_tag_length: usize,
    pub max_tags_per_note: usize,

    /// Refuse pushes that would take an account past its storage quota. Off unless
    /// asked for: quotas were never enforced before, so an account can already be
    /// over one, and a refused push stops that device's sync.
    pub enforce_storage_quota: bool,
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
                .ok()
                .filter(|password| !password.is_empty()),

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

            rate_limit: {
                let defaults = RateLimitConfig::default();
                RateLimitConfig {
                    enabled: env_or("RATE_LIMIT_ENABLED", defaults.enabled),
                    trusted_proxy_hops: env_or("TRUSTED_PROXY_HOPS", defaults.trusted_proxy_hops),
                    auth_per_minute: env_or("RATE_LIMIT_AUTH_PER_MINUTE", defaults.auth_per_minute),
                    register_per_hour: env_or(
                        "RATE_LIMIT_REGISTER_PER_HOUR",
                        defaults.register_per_hour,
                    ),
                    status_per_minute: env_or(
                        "RATE_LIMIT_STATUS_PER_MINUTE",
                        defaults.status_per_minute,
                    ),
                }
            },

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

            enforce_storage_quota: env_or("ENFORCE_STORAGE_QUOTA", false),
        })
    }
}
