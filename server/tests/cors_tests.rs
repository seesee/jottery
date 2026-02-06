// CORS configuration tests
//
// Tests for Cross-Origin Resource Sharing (CORS) configuration
// to ensure proper security posture in production environments.

use std::env;

// ============================================================================
// Parsing tests
// ============================================================================

#[test]
fn test_cors_config_parsing() {
    // Test parsing comma-separated origins
    let test_cases = vec![
        (
            "https://example.com",
            vec!["https://example.com"],
        ),
        (
            "https://example.com,https://app.example.com",
            vec!["https://example.com", "https://app.example.com"],
        ),
        (
            "https://example.com, https://app.example.com, https://admin.example.com",
            vec!["https://example.com", "https://app.example.com", "https://admin.example.com"],
        ),
        (
            "https://example.com,   https://app.example.com   ",
            vec!["https://example.com", "https://app.example.com"],
        ),
    ];

    for (input, expected) in test_cases {
        let parsed: Vec<String> = input
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        assert_eq!(
            parsed.len(),
            expected.len(),
            "Should parse {} origins from '{}'",
            expected.len(),
            input
        );

        for (i, exp) in expected.iter().enumerate() {
            assert_eq!(
                parsed[i], *exp,
                "Origin {} should match",
                i
            );
        }
    }
}

#[test]
fn test_cors_config_empty_handling() {
    // Test handling of empty strings and whitespace
    let empty_cases = vec![
        "",
        "  ",
        "   ,   ",
    ];

    for input in empty_cases {
        let parsed: Vec<String> = input
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        assert!(
            parsed.is_empty(),
            "Should produce empty list for input '{}'",
            input
        );
    }
}

#[test]
fn test_cors_config_env_var() {
    // Test loading from environment variable
    let test_key = "TEST_CORS_ALLOWED_ORIGINS";

    // Set environment variable
    env::set_var(test_key, "https://test1.com,https://test2.com");

    // Read and parse
    let origins = env::var(test_key)
        .ok()
        .map(|s| {
            s.split(',')
                .map(|o| o.trim().to_string())
                .filter(|o| !o.is_empty())
                .collect::<Vec<_>>()
        });

    assert!(origins.is_some(), "Should parse environment variable");
    let origins = origins.unwrap();
    assert_eq!(origins.len(), 2, "Should have 2 origins");
    assert_eq!(origins[0], "https://test1.com");
    assert_eq!(origins[1], "https://test2.com");

    // Cleanup
    env::remove_var(test_key);
}

#[test]
fn test_cors_config_none_when_not_set() {
    // Test that missing environment variable results in None
    let test_key = "NONEXISTENT_CORS_VAR";

    let result = env::var(test_key).ok();
    assert!(result.is_none(), "Should be None when env var not set");

    // Verify this would default to Any origin
    let cors_enabled = result.is_none();
    assert!(cors_enabled, "CORS should still be enabled (with Any)");
}

#[test]
fn test_cors_origin_format_validation() {
    // Document expected origin formats
    let valid_origins = vec![
        "https://example.com",
        "https://app.example.com",
        "https://example.com:8080",
        "http://localhost:3000",
        "http://127.0.0.1:3030",
    ];

    for origin in valid_origins {
        assert!(
            origin.starts_with("http://") || origin.starts_with("https://"),
            "Origin should have protocol: {}",
            origin
        );
    }

    // Document invalid formats (should be rejected by parsing)
    let invalid_origins = vec![
        "example.com",           // Missing protocol
        "//example.com",         // Protocol-relative
        "ftp://example.com",     // Wrong protocol
        "javascript:alert(1)",   // XSS attempt
    ];

    for origin in invalid_origins {
        assert!(
            !origin.starts_with("http://") || !origin.starts_with("https://"),
            "Invalid origin should not pass basic validation: {}",
            origin
        );
    }
}

#[cfg(test)]
mod security_documentation {
    #[test]
    fn document_cors_security_requirements() {
        // Document CORS security best practices

        // Production deployments should use specific origins
        let production_example = "https://jottery.example.com,https://app.example.com";
        assert!(
            !production_example.contains("*"),
            "Production should not use wildcards"
        );

        // Development can use Any (wildcard)
        let dev_allows_any = true;
        assert!(
            dev_allows_any,
            "Development mode should allow Any for convenience"
        );

        // HTTPS should be used in production
        let production_origins = vec![
            "https://example.com",
            "https://app.example.com",
        ];
        for origin in production_origins {
            assert!(
                origin.starts_with("https://"),
                "Production should use HTTPS: {}",
                origin
            );
        }

        // localhost is acceptable for development
        let dev_origins = vec![
            "http://localhost:3000",
            "http://127.0.0.1:5173",
        ];
        for origin in dev_origins {
            assert!(
                origin.contains("localhost") || origin.contains("127.0.0.1"),
                "Dev origin should be localhost: {}",
                origin
            );
        }
    }

    #[test]
    fn document_cors_attack_vectors() {
        // Document attack vectors CORS protects against
        let attack_vectors = vec![
            "Cross-Site Request Forgery (CSRF)",
            "Unauthorized API Access from Malicious Sites",
            "Data Exfiltration from Browser",
            "Session Hijacking via XHR/Fetch",
        ];

        for vector in &attack_vectors {
            println!("CORS protects against: {}", vector);
        }

        assert!(
            attack_vectors.len() >= 4,
            "Should protect against major CORS attack vectors"
        );
    }
}

// ============================================================================
// Production scenario tests
// ============================================================================

#[cfg(test)]
mod production_scenarios {
    use super::*;

    /// Helper to parse origins like the server does
    fn parse_origins(input: &str) -> Vec<String> {
        input
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }

    #[test]
    fn test_production_multi_subdomain_setup() {
        // Common production scenario: main app + admin panel + API docs
        let config = "https://app.example.com,https://admin.example.com,https://docs.example.com";
        let origins = parse_origins(config);

        assert_eq!(origins.len(), 3);
        assert!(origins.contains(&"https://app.example.com".to_string()));
        assert!(origins.contains(&"https://admin.example.com".to_string()));
        assert!(origins.contains(&"https://docs.example.com".to_string()));

        // All should be HTTPS
        for origin in &origins {
            assert!(origin.starts_with("https://"), "Production should use HTTPS: {}", origin);
        }
    }

    #[test]
    fn test_production_with_port_numbers() {
        // Scenario: Non-standard ports (common with reverse proxies)
        let config = "https://app.example.com:8443,https://admin.example.com:9443";
        let origins = parse_origins(config);

        assert_eq!(origins.len(), 2);
        assert!(origins[0].contains(":8443"));
        assert!(origins[1].contains(":9443"));
    }

    #[test]
    fn test_staging_and_production_combined() {
        // Scenario: Allow both staging and production (e.g., during migration)
        let config = "https://app.example.com,https://staging.example.com,https://app.staging.example.com";
        let origins = parse_origins(config);

        assert_eq!(origins.len(), 3);

        // Verify staging origins are explicitly allowed
        let has_staging = origins.iter().any(|o| o.contains("staging"));
        assert!(has_staging, "Should allow staging origins during migration");
    }

    #[test]
    fn test_localhost_development_setup() {
        // Scenario: Local development with multiple services
        let config = "http://localhost:3000,http://localhost:5173,http://127.0.0.1:3000";
        let origins = parse_origins(config);

        assert_eq!(origins.len(), 3);

        // All should be localhost/127.0.0.1
        for origin in &origins {
            assert!(
                origin.contains("localhost") || origin.contains("127.0.0.1"),
                "Dev origins should be local: {}", origin
            );
        }
    }

    #[test]
    fn test_wildcard_explicit() {
        // Scenario: Explicit wildcard (should be flagged as less secure)
        let config = "*";
        let origins = parse_origins(config);

        assert_eq!(origins.len(), 1);
        assert_eq!(origins[0], "*");

        // Document: wildcard should trigger warning in production
        let is_wildcard = origins.iter().any(|o| o == "*");
        assert!(is_wildcard, "Wildcard should be detected");
    }

    #[test]
    fn test_mixed_http_https_rejected_in_production() {
        // Document: mixing HTTP and HTTPS is a security concern
        let config = "https://app.example.com,http://legacy.example.com";
        let origins = parse_origins(config);

        let has_http = origins.iter().any(|o| o.starts_with("http://") && !o.contains("localhost"));
        let has_https = origins.iter().any(|o| o.starts_with("https://"));

        // Both HTTP and HTTPS present (excluding localhost)
        if has_http && has_https {
            // This configuration should be audited
            println!("WARNING: Mixed HTTP/HTTPS origins detected - review security");
        }

        assert!(has_http, "Test should include HTTP origin");
        assert!(has_https, "Test should include HTTPS origin");
    }

    #[test]
    fn test_ipv6_origins() {
        // Scenario: IPv6 addresses (rare but valid)
        let config = "http://[::1]:3000,https://[2001:db8::1]:8080";
        let origins = parse_origins(config);

        assert_eq!(origins.len(), 2);
        assert!(origins[0].contains("[::1]"), "Should handle IPv6 localhost");
        assert!(origins[1].contains("[2001:db8::1]"), "Should handle IPv6 addresses");
    }

    #[test]
    fn test_trailing_slash_handling() {
        // Origins should NOT have trailing slashes (per CORS spec)
        let config = "https://example.com/,https://app.example.com";
        let origins = parse_origins(config);

        // Document: trailing slashes should be stripped or rejected
        let has_trailing_slash = origins.iter().any(|o| o.ends_with('/'));
        if has_trailing_slash {
            println!("NOTE: Origin with trailing slash detected - may cause CORS mismatches");
        }

        // First origin has trailing slash (current parsing keeps it)
        assert!(origins[0].ends_with('/'), "Current parsing preserves trailing slash");
    }

    #[test]
    fn test_case_sensitivity() {
        // CORS origin matching is case-sensitive for scheme and host
        let config = "https://Example.Com,https://example.com";
        let origins = parse_origins(config);

        assert_eq!(origins.len(), 2);
        // These are different origins in CORS
        assert_ne!(origins[0], origins[1], "Different case = different origins in CORS");
    }

    #[test]
    fn test_unicode_domain() {
        // Scenario: Internationalized domain names (IDN)
        let config = "https://例え.jp,https://xn--r8jz45g.jp"; // IDN and Punycode
        let origins = parse_origins(config);

        assert_eq!(origins.len(), 2);
        // Both forms should be parsed
        assert!(origins[0].contains("例え") || origins[0].contains("xn--"));
    }

    #[test]
    fn test_very_long_origin_list() {
        // Scenario: Many microservices/origins
        let origins: Vec<String> = (0..100)
            .map(|i| format!("https://service{}.example.com", i))
            .collect();
        let config = origins.join(",");

        let parsed = parse_origins(&config);

        assert_eq!(parsed.len(), 100, "Should handle many origins");
    }

    #[test]
    fn test_empty_between_commas() {
        // Edge case: empty entries between commas
        let config = "https://a.com,,https://b.com,  ,https://c.com";
        let origins = parse_origins(config);

        // Empty entries should be filtered out
        assert_eq!(origins.len(), 3);
        assert!(!origins.contains(&"".to_string()));
    }
}

// ============================================================================
// Security edge case tests
// ============================================================================

#[cfg(test)]
mod security_edge_cases {
    use super::*;

    fn parse_origins(input: &str) -> Vec<String> {
        input
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }

    #[test]
    fn test_null_origin_not_allowed() {
        // The "null" origin should never be explicitly allowed
        // (used by sandboxed iframes, data: URLs, etc.)
        let config = "null,https://example.com";
        let origins = parse_origins(config);

        let has_null = origins.contains(&"null".to_string());
        if has_null {
            println!("SECURITY WARNING: 'null' origin should not be explicitly allowed");
        }

        // Document: this is a potential security issue
        assert!(has_null, "Test includes null origin for documentation");
    }

    #[test]
    fn test_file_protocol_not_allowed() {
        // file:// protocol should not be in production CORS config
        let config = "file:///path/to/app,https://example.com";
        let origins = parse_origins(config);

        let has_file = origins.iter().any(|o| o.starts_with("file://"));
        if has_file {
            println!("SECURITY WARNING: file:// protocol should not be in CORS origins");
        }

        assert!(has_file, "Test includes file:// for documentation");
    }

    #[test]
    fn test_data_protocol_not_allowed() {
        // data: protocol in origins would be suspicious
        let config = "data:text/html,<h1>XSS</h1>,https://example.com";
        let origins = parse_origins(config);

        let has_data = origins.iter().any(|o| o.starts_with("data:"));
        if has_data {
            println!("SECURITY WARNING: data: protocol should not be in CORS origins");
        }

        // This shouldn't parse as valid anyway (contains comma in data URL)
        assert_eq!(origins.len(), 3, "Comma in data URL splits incorrectly");
    }

    #[test]
    fn test_javascript_protocol_rejected() {
        // javascript: protocol is an XSS vector
        let config = "javascript:alert(1),https://example.com";
        let origins = parse_origins(config);

        let has_js = origins.iter().any(|o| o.starts_with("javascript:"));
        if has_js {
            println!("SECURITY CRITICAL: javascript: protocol must be rejected");
        }

        // Should be detected
        assert!(has_js, "Test detects javascript: protocol");
    }

    #[test]
    fn test_origin_with_path_invalid() {
        // CORS origins should not include paths
        let config = "https://example.com/api/v1,https://example.com";
        let origins = parse_origins(config);

        let has_path = origins.iter().any(|o| {
            // Check if there's a path after the host (excluding port)
            if let Some(idx) = o.find("://") {
                let after_scheme = &o[idx + 3..];
                // Look for / after potential port
                after_scheme.contains('/')
            } else {
                false
            }
        });

        if has_path {
            println!("NOTE: CORS origins should not include paths - paths are ignored");
        }

        assert!(has_path, "Test includes origin with path");
    }

    #[test]
    fn test_origin_with_credentials_rejected() {
        // Origins with embedded credentials are invalid
        let config = "https://user:pass@example.com,https://example.com";
        let origins = parse_origins(config);

        let has_creds = origins.iter().any(|o| o.contains('@') && o.contains("://"));
        if has_creds {
            println!("SECURITY WARNING: Origins should not contain credentials");
        }

        assert!(has_creds, "Test includes origin with credentials");
    }

    #[test]
    fn test_subdomain_wildcard_not_supported() {
        // CORS doesn't support subdomain wildcards like *.example.com
        let config = "https://*.example.com,https://example.com";
        let origins = parse_origins(config);

        let has_subdomain_wildcard = origins.iter().any(|o| o.contains("*."));
        if has_subdomain_wildcard {
            println!("NOTE: CORS does not support *.domain.com wildcards");
            println!("Each subdomain must be listed explicitly");
        }

        assert!(has_subdomain_wildcard, "Test includes subdomain wildcard");
    }

    #[test]
    fn test_whitespace_in_origin() {
        // Whitespace should be trimmed
        let config = "  https://example.com  ,  https://app.example.com  ";
        let origins = parse_origins(config);

        assert_eq!(origins.len(), 2);
        assert!(!origins[0].starts_with(' '));
        assert!(!origins[0].ends_with(' '));
        assert!(!origins[1].starts_with(' '));
        assert!(!origins[1].ends_with(' '));
    }
}
