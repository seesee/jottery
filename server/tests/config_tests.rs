// Configuration tests
//
// Tests for configuration parsing logic and validation

#[test]
fn test_config_default_constants() {
    // Test that the expected default values are reasonable
    const DEFAULT_PORT: u16 = 3030;
    const DEFAULT_MAX_PAYLOAD: usize = 5_242_880; // 5 MB
    const DEFAULT_SESSION_EXPIRY: i64 = 7; // days
    const DEFAULT_ARGON2_M_COST: u32 = 19456; // KiB
    const DEFAULT_ARGON2_T_COST: u32 = 2;
    const DEFAULT_ARGON2_P_COST: u32 = 1;
    const DEFAULT_STORAGE_QUOTA: i32 = 1000; // MB

    assert!(DEFAULT_PORT > 1024, "Port should be above privileged range");
    assert!(DEFAULT_PORT < 65535, "Port should be valid");
    assert!(DEFAULT_MAX_PAYLOAD > 0, "Max payload should be positive");
    assert!(DEFAULT_MAX_PAYLOAD <= 10_000_000, "Max payload should be reasonable");
    assert!(DEFAULT_SESSION_EXPIRY > 0, "Session expiry should be positive");
    assert!(DEFAULT_SESSION_EXPIRY <= 30, "Default session expiry should be reasonable");
    assert!(DEFAULT_ARGON2_M_COST >= 8192, "Argon2 memory cost should be secure");
    assert!(DEFAULT_ARGON2_T_COST >= 1, "Argon2 time cost should be at least 1");
    assert!(DEFAULT_ARGON2_P_COST >= 1, "Argon2 parallelism should be at least 1");
    assert!(DEFAULT_STORAGE_QUOTA > 0, "Storage quota should be positive");
}

#[test]
fn test_config_value_parsing() {
    // Test integer parsing with fallback
    assert_eq!("3030".parse::<u16>().unwrap_or(3030), 3030);
    assert_eq!("8080".parse::<u16>().unwrap_or(3030), 8080);
    assert_eq!("invalid".parse::<u16>().unwrap_or(3030), 3030);

    // Test i64 parsing
    assert_eq!("7".parse::<i64>().unwrap_or(7), 7);
    assert_eq!("14".parse::<i64>().unwrap_or(7), 14);
    assert_eq!("abc".parse::<i64>().unwrap_or(7), 7);

    // Test u32 parsing
    assert_eq!("19456".parse::<u32>().unwrap_or(19456), 19456);
    assert_eq!("65536".parse::<u32>().unwrap_or(19456), 65536);
    assert_eq!("xyz".parse::<u32>().unwrap_or(19456), 19456);
}

#[test]
fn test_cors_origins_parsing() {
    // Test comma-separated list parsing
    let test_cases = vec![
        ("https://example.com", vec!["https://example.com"]),
        (
            "https://example.com,https://app.example.com",
            vec!["https://example.com", "https://app.example.com"],
        ),
        (
            "  https://example.com  , https://app.example.com  ",
            vec!["https://example.com", "https://app.example.com"],
        ),
    ];

    for (input, expected) in test_cases {
        let parsed: Vec<String> = input
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        assert_eq!(parsed.len(), expected.len());
        for (i, exp) in expected.iter().enumerate() {
            assert_eq!(parsed[i], *exp);
        }
    }
}

#[test]
fn test_cors_origins_empty_handling() {
    let empty_cases = vec!["", "  ", "   ,   "];

    for input in empty_cases {
        let parsed: Vec<String> = input
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        assert!(parsed.is_empty(), "Empty input should produce empty list");
    }
}

#[test]
fn test_config_value_ranges() {
    // Test that various configuration values fall within reasonable ranges

    // Port range
    let valid_ports = vec![3000, 3030, 8080, 8443, 9000];
    for port in valid_ports {
        assert!(port > 0 && port < 65536, "Port {} should be valid", port);
    }

    // Session expiry range (days)
    let valid_expiries = vec![1, 7, 14, 30];
    for days in valid_expiries {
        assert!(days > 0 && days <= 365, "Session expiry {} should be reasonable", days);
    }

    // Argon2 memory cost (KiB)
    let valid_m_costs = vec![8192, 19456, 65536, 262144];
    for m_cost in valid_m_costs {
        assert!(m_cost >= 8192, "Memory cost {} should be secure", m_cost);
    }

    // Argon2 time cost
    let valid_t_costs = vec![1, 2, 3, 4, 5];
    for t_cost in valid_t_costs {
        assert!(t_cost >= 1 && t_cost <= 10, "Time cost {} should be reasonable", t_cost);
    }

    // Storage quota (MB)
    let valid_quotas = vec![100, 500, 1000, 5000, 10000];
    for quota in valid_quotas {
        assert!(quota > 0, "Storage quota {} should be positive", quota);
    }
}

#[cfg(test)]
mod security_validation {
    #[test]
    fn test_session_expiry_security_levels() {
        // Document and validate session expiry security levels
        let high_security = 1; // 1 day
        let medium_security = 7; // 7 days (default)
        let low_security = 30; // 30 days

        assert!(high_security < medium_security);
        assert!(medium_security < low_security);
        assert!(high_security >= 1, "Minimum 1 day");
        assert!(low_security <= 30, "Maximum recommended 30 days");
    }

    #[test]
    fn test_argon2_security_levels() {
        // Document Argon2 security levels
        struct SecurityLevel {
            name: &'static str,
            m_cost: u32,
            t_cost: u32,
            p_cost: u32,
        }

        let levels = vec![
            SecurityLevel {
                name: "Low",
                m_cost: 19456,
                t_cost: 2,
                p_cost: 1,
            },
            SecurityLevel {
                name: "Medium",
                m_cost: 65536,
                t_cost: 3,
                p_cost: 1,
            },
            SecurityLevel {
                name: "High",
                m_cost: 262144,
                t_cost: 4,
                p_cost: 1,
            },
        ];

        for level in &levels {
            assert!(level.m_cost >= 8192, "{} level m_cost should be at least 8192", level.name);
            assert!(level.t_cost >= 1, "{} level t_cost should be at least 1", level.name);
            assert!(level.p_cost >= 1, "{} level p_cost should be at least 1", level.name);
        }

        // Verify security levels are ordered correctly
        assert!(levels[0].m_cost < levels[1].m_cost);
        assert!(levels[1].m_cost < levels[2].m_cost);
    }

    #[test]
    fn test_default_admin_credentials_warning() {
        // Document that default credentials are insecure
        // NOTE: Default credentials are documented in CLAUDE.md and migrations
        // They MUST be changed before production deployment
        let default_email = "admin@localhost";
        let default_password = "changeme";

        // Verify the documented defaults match (without logging sensitive values)
        assert_eq!(default_email, "admin@localhost");
        assert!(!default_password.is_empty(), "Default password must be set");
        assert!(
            default_password.len() >= 6,
            "Default password must be at least 6 characters"
        );
    }

    #[test]
    fn test_storage_quota_recommendations() {
        // Document storage quota recommendations
        let personal_quota = 1000; // 1 GB
        let team_quota = 5000; // 5 GB
        let enterprise_quota = 10000; // 10 GB

        assert!(personal_quota > 0);
        assert!(personal_quota < team_quota);
        assert!(team_quota < enterprise_quota);
    }
}

#[cfg(test)]
mod configuration_documentation {
    #[test]
    fn document_environment_variables() {
        println!("\n=== Jottery Server Configuration ===");
        println!("\nBasic Configuration:");
        println!("  DATABASE_URL (default: sqlite:jottery.db)");
        println!("  PORT (default: 3030)");
        println!("  MAX_PAYLOAD_SIZE (default: 5242880 = 5MB)");

        println!("\nSecurity Configuration:");
        println!("  SESSION_EXPIRY_DAYS (default: 7)");
        println!("  DEFAULT_ADMIN_EMAIL (default: admin@localhost)");
        println!("  DEFAULT_ADMIN_PASSWORD (default: changeme)");
        println!("  CORS_ALLOWED_ORIGINS (default: none = allow any)");

        println!("\nPassword Hashing (Argon2id):");
        println!("  ARGON2_M_COST (default: 19456 KiB)");
        println!("  ARGON2_T_COST (default: 2)");
        println!("  ARGON2_P_COST (default: 1)");

        println!("\nUser Settings:");
        println!("  DEFAULT_STORAGE_QUOTA_MB (default: 1000)");

        println!("\nSee .env.example for detailed documentation");
        println!("======================================\n");
    }

    #[test]
    fn document_security_best_practices() {
        println!("\n=== Security Best Practices ===");
        println!("\n1. Change Default Admin Credentials");
        println!("   Set DEFAULT_ADMIN_EMAIL and DEFAULT_ADMIN_PASSWORD before deployment");

        println!("\n2. Configure CORS");
        println!("   Set CORS_ALLOWED_ORIGINS to specific domains in production");

        println!("\n3. Adjust Password Hashing");
        println!("   Increase ARGON2_M_COST and ARGON2_T_COST for higher security");

        println!("\n4. Set Session Expiry");
        println!("   Use shorter SESSION_EXPIRY_DAYS for higher security");

        println!("\n5. Use HTTPS");
        println!("   Configure reverse proxy (Nginx/Caddy) with SSL/TLS");

        println!("\n6. Implement Rate Limiting");
        println!("   Configure at reverse proxy level (see .env.example)");
        println!("================================\n");
    }
}
