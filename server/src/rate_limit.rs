//! Throttling for the anonymous credential routes.
//!
//! Two layers, because each covers what the other can't:
//!
//! - Per client address, with `tower_governor`, on every anonymous route. This
//!   stops one source spraying many accounts or flooding registration.
//! - Per email address, on the routes that check a password. This stops guessing
//!   against one account from many sources.
//!
//! Devices aren't affected: sync authenticates with an API key, not a password.

use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{HeaderMap, Request, Response},
    response::IntoResponse,
    Router,
};
use std::{
    collections::HashMap,
    net::{IpAddr, Ipv6Addr, SocketAddr},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tower_governor::{
    governor::GovernorConfigBuilder, key_extractor::KeyExtractor, GovernorError, GovernorLayer,
};

use crate::{config::RateLimitConfig, error::AppError};

/// Failures allowed before an address starts to wait.
const FREE_FAILURES: u32 = 5;

/// The first wait, doubling with each further failure.
const BASE_DELAY: Duration = Duration::from_secs(1);

/// The longest wait. A lock also stops the real owner signing in, so it has to
/// stay short enough that an attacker can't hold an account out for long.
const MAX_DELAY: Duration = Duration::from_secs(15 * 60);

/// A failure is forgotten after this long without another.
const FAILURE_MEMORY: Duration = Duration::from_secs(60 * 60);

/// Tracked addresses. Failures for unknown emails are tracked too, or locking
/// would reveal which accounts exist, so this bounds what a sprayer can cost.
const MAX_TRACKED: usize = 100_000;

struct Failures {
    count: u32,
    last_failure: Instant,
}

impl Failures {
    fn delay(&self) -> Duration {
        if self.count < FREE_FAILURES {
            return Duration::ZERO;
        }
        // Capping the shift first keeps the multiplication from overflowing
        let doublings = (self.count - FREE_FAILURES).min(16);
        (BASE_DELAY * 2u32.pow(doublings)).min(MAX_DELAY)
    }
}

/// Failed password attempts per email address, with exponential backoff.
pub struct LoginThrottle {
    enabled: bool,
    failures: Mutex<HashMap<String, Failures>>,
}

impl Default for LoginThrottle {
    fn default() -> Self {
        Self::new(true)
    }
}

impl LoginThrottle {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            failures: Mutex::default(),
        }
    }

    fn key(email: &str) -> String {
        email.trim().to_lowercase()
    }

    /// Refuse an attempt while this address is waiting. Call before verifying
    /// the password: a correct guess during a wait must look like any other,
    /// and a refused attempt costs no Argon2 run.
    pub fn check(&self, email: &str) -> Result<(), AppError> {
        self.check_at(email, Instant::now())
    }

    pub fn check_at(&self, email: &str, now: Instant) -> Result<(), AppError> {
        if !self.enabled {
            return Ok(());
        }

        let failures = self.failures.lock().expect("login throttle lock");
        let Some(entry) = failures.get(&Self::key(email)) else {
            return Ok(());
        };

        let available_at = entry.last_failure + entry.delay();
        if now < available_at {
            let wait = available_at - now;
            return Err(AppError::RateLimited {
                retry_after_secs: wait.as_secs() + 1,
            });
        }
        Ok(())
    }

    pub fn record_failure(&self, email: &str) {
        self.record_failure_at(email, Instant::now());
    }

    pub fn record_failure_at(&self, email: &str, now: Instant) {
        if !self.enabled {
            return;
        }

        let mut failures = self.failures.lock().expect("login throttle lock");
        if failures.len() >= MAX_TRACKED {
            failures.retain(|_, entry| now.duration_since(entry.last_failure) < FAILURE_MEMORY);
        }

        let key = Self::key(email);
        if let Some(entry) = failures.get_mut(&key) {
            let forgotten = now.duration_since(entry.last_failure) >= FAILURE_MEMORY;
            entry.count = if forgotten { 1 } else { entry.count.saturating_add(1) };
            entry.last_failure = now;
        } else if failures.len() < MAX_TRACKED {
            // Past the bound new addresses go untracked: existing locks are
            // worth more, and the per-address limit still applies to a sprayer.
            failures.insert(key, Failures { count: 1, last_failure: now });
        }
    }

    pub fn record_success(&self, email: &str) {
        if self.enabled {
            self.failures
                .lock()
                .expect("login throttle lock")
                .remove(&Self::key(email));
        }
    }
}

/// The address a request came from.
///
/// With no trusted proxies that is the TCP peer, and `X-Forwarded-For` is
/// ignored, because anyone can send one. With `hops` trusted proxies in front of
/// the server, each appends the address it saw, so the entry `hops` from the
/// right was written by the outermost proxy we trust. Anything further left is
/// whatever the client claimed, which is why the leftmost value is never used
/// unless it is exactly that entry.
pub fn client_ip(headers: &HeaderMap, peer: Option<IpAddr>, hops: usize) -> Option<IpAddr> {
    if hops == 0 {
        return peer;
    }

    let forwarded: Vec<&str> = headers
        .get_all("x-forwarded-for")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .collect();

    forwarded
        .iter()
        .rev()
        .nth(hops - 1)
        .and_then(|entry| parse_address(entry))
        .or(peer)
}

fn parse_address(entry: &str) -> Option<IpAddr> {
    entry
        .parse::<IpAddr>()
        .ok()
        .or_else(|| entry.parse::<SocketAddr>().ok().map(|addr| addr.ip()))
}

/// An IPv6 client usually controls a whole /64, so that is what gets counted.
fn limit_key(address: IpAddr) -> IpAddr {
    match address {
        IpAddr::V4(_) => address,
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => IpAddr::V4(v4),
            None => {
                let [a, b, c, d, ..] = v6.segments();
                IpAddr::V6(Ipv6Addr::new(a, b, c, d, 0, 0, 0, 0))
            }
        },
    }
}

/// Keys the rate limiter on the client's address.
#[derive(Clone)]
pub struct ClientAddress {
    trusted_proxy_hops: usize,
    warned_about_proxy: Arc<AtomicBool>,
}

impl KeyExtractor for ClientAddress {
    type Key = IpAddr;

    fn extract<T>(&self, request: &Request<T>) -> Result<IpAddr, GovernorError> {
        let peer = request
            .extensions()
            .get::<ConnectInfo<SocketAddr>>()
            .map(|info| info.0.ip());

        if self.trusted_proxy_hops == 0
            && request.headers().contains_key("x-forwarded-for")
            && !self.warned_about_proxy.swap(true, Ordering::Relaxed)
        {
            tracing::warn!(
                "Requests carry X-Forwarded-For but TRUSTED_PROXY_HOPS is 0, so every client \
                 behind that proxy shares one rate limit. Set it to the number of proxies you \
                 run in front of this server."
            );
        }

        let address = client_ip(request.headers(), peer, self.trusted_proxy_hops);
        // Only a request that bypassed the listener has no address at all
        Ok(limit_key(address.unwrap_or(IpAddr::V6(Ipv6Addr::UNSPECIFIED))))
    }
}

fn limit_exceeded(error: GovernorError) -> Response<Body> {
    match error {
        GovernorError::TooManyRequests { wait_time, .. } => AppError::RateLimited {
            retry_after_secs: wait_time.max(1),
        }
        .into_response(),
        _ => AppError::InternalServerError.into_response(),
    }
}

/// Allow `burst` requests per client address on these routes, refilled one per
/// `period`.
fn limit_by_address<S>(
    router: Router<S>,
    config: &RateLimitConfig,
    period: Duration,
    burst: u32,
) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    if !config.enabled {
        return router;
    }

    let governor_config = GovernorConfigBuilder::default()
        .key_extractor(ClientAddress {
            trusted_proxy_hops: config.trusted_proxy_hops,
            warned_about_proxy: Arc::default(),
        })
        .period(period)
        .burst_size(burst.max(1))
        .error_handler(limit_exceeded)
        .finish()
        .expect("rate limit period and burst are non-zero");

    let limiter = governor_config.limiter().clone();
    tokio::spawn(async move {
        // Forget addresses that have stopped, so the table can't grow forever
        let mut interval = tokio::time::interval(Duration::from_secs(5 * 60));
        loop {
            interval.tick().await;
            limiter.retain_recent();
        }
    });

    router.layer(GovernorLayer {
        config: Arc::new(governor_config),
    })
}

fn per_minute(count: u32) -> Duration {
    Duration::from_secs(60) / count.max(1)
}

/// Limit the routes that take a credential.
pub fn limit_credentials<S>(router: Router<S>, config: &RateLimitConfig) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    limit_by_address(router, config, per_minute(config.auth_per_minute), config.auth_per_minute)
}

/// Limit account creation, which lands in the admin's pending list.
pub fn limit_registration<S>(router: Router<S>, config: &RateLimitConfig) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    let period = Duration::from_secs(60 * 60) / config.register_per_hour.max(1);
    limit_by_address(router, config, period, config.register_per_hour)
}

/// Limit the approval status check, which clients poll.
pub fn limit_status<S>(router: Router<S>, config: &RateLimitConfig) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    limit_by_address(router, config, per_minute(config.status_per_minute), config.status_per_minute)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn forwarded(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", value.parse().unwrap());
        headers
    }

    fn ip(value: &str) -> IpAddr {
        value.parse().unwrap()
    }

    #[test]
    fn with_no_trusted_proxy_the_header_is_ignored() {
        let headers = forwarded("1.1.1.1");
        assert_eq!(client_ip(&headers, Some(ip("9.9.9.9")), 0), Some(ip("9.9.9.9")));
    }

    #[test]
    fn one_hop_takes_the_entry_the_proxy_wrote() {
        // Whatever the client sent comes first; the proxy appends the real peer
        let headers = forwarded("6.6.6.6, 1.1.1.1");
        assert_eq!(client_ip(&headers, Some(ip("127.0.0.1")), 1), Some(ip("1.1.1.1")));
    }

    #[test]
    fn two_hops_take_the_entry_the_outer_proxy_wrote() {
        let headers = forwarded("6.6.6.6, 1.1.1.1, 2.2.2.2");
        assert_eq!(client_ip(&headers, Some(ip("127.0.0.1")), 2), Some(ip("1.1.1.1")));
    }

    #[test]
    fn a_chain_shorter_than_the_trusted_hops_falls_back_to_the_peer() {
        let headers = forwarded("1.1.1.1");
        assert_eq!(client_ip(&headers, Some(ip("9.9.9.9")), 2), Some(ip("9.9.9.9")));
    }

    #[test]
    fn a_missing_header_falls_back_to_the_peer() {
        assert_eq!(client_ip(&HeaderMap::new(), Some(ip("9.9.9.9")), 1), Some(ip("9.9.9.9")));
    }

    #[test]
    fn an_unreadable_entry_falls_back_to_the_peer() {
        let headers = forwarded("not-an-address");
        assert_eq!(client_ip(&headers, Some(ip("9.9.9.9")), 1), Some(ip("9.9.9.9")));
    }

    #[test]
    fn several_headers_are_read_as_one_chain() {
        let mut headers = forwarded("6.6.6.6");
        headers.append("x-forwarded-for", "1.1.1.1".parse().unwrap());
        assert_eq!(client_ip(&headers, Some(ip("127.0.0.1")), 1), Some(ip("1.1.1.1")));
    }

    #[test]
    fn an_entry_with_a_port_is_understood() {
        let headers = forwarded("1.1.1.1:4711");
        assert_eq!(client_ip(&headers, None, 1), Some(ip("1.1.1.1")));
    }

    #[test]
    fn ipv6_clients_are_counted_by_their_slash_64() {
        assert_eq!(limit_key(ip("2001:db8:1:2:aaaa::1")), ip("2001:db8:1:2::"));
        assert_eq!(limit_key(ip("2001:db8:1:2:bbbb::9")), ip("2001:db8:1:2::"));
        assert_ne!(limit_key(ip("2001:db8:1:3::1")), ip("2001:db8:1:2::"));
    }

    #[test]
    fn mapped_ipv4_is_the_same_client_as_plain_ipv4() {
        assert_eq!(limit_key(ip("::ffff:1.1.1.1")), ip("1.1.1.1"));
    }

    fn fail_times(throttle: &LoginThrottle, email: &str, times: u32, now: Instant) {
        for _ in 0..times {
            throttle.record_failure_at(email, now);
        }
    }

    #[test]
    fn the_first_failures_are_free() {
        let throttle = LoginThrottle::default();
        let now = Instant::now();
        fail_times(&throttle, "a@example.com", FREE_FAILURES - 1, now);
        assert!(throttle.check_at("a@example.com", now).is_ok());
    }

    #[test]
    fn the_wait_doubles_with_each_failure() {
        let throttle = LoginThrottle::default();
        let now = Instant::now();

        for (extra_failures, wait_secs) in [(0, 1), (1, 2), (2, 4), (3, 8)] {
            let email = format!("{extra_failures}@example.com");
            fail_times(&throttle, &email, FREE_FAILURES + extra_failures, now);

            let wait = Duration::from_secs(wait_secs);
            assert!(throttle.check_at(&email, now + wait - Duration::from_millis(1)).is_err());
            assert!(throttle.check_at(&email, now + wait).is_ok());
        }
    }

    #[test]
    fn the_wait_is_capped() {
        let throttle = LoginThrottle::default();
        let now = Instant::now();
        fail_times(&throttle, "a@example.com", 1000, now);

        assert!(throttle.check_at("a@example.com", now + MAX_DELAY - Duration::from_secs(1)).is_err());
        assert!(throttle.check_at("a@example.com", now + MAX_DELAY).is_ok());
    }

    #[test]
    fn a_refusal_says_how_long_to_wait() {
        let throttle = LoginThrottle::default();
        let now = Instant::now();
        fail_times(&throttle, "a@example.com", FREE_FAILURES + 3, now);

        let error = throttle.check_at("a@example.com", now).unwrap_err();
        assert!(matches!(error, AppError::RateLimited { retry_after_secs: 9 }), "{error:?}");
    }

    #[test]
    fn success_clears_the_count() {
        let throttle = LoginThrottle::default();
        let now = Instant::now();
        fail_times(&throttle, "a@example.com", FREE_FAILURES - 1, now);
        throttle.record_success("a@example.com");
        fail_times(&throttle, "a@example.com", FREE_FAILURES - 1, now);
        assert!(throttle.check_at("a@example.com", now).is_ok());
    }

    #[test]
    fn old_failures_are_forgotten() {
        let throttle = LoginThrottle::default();
        let now = Instant::now();
        fail_times(&throttle, "a@example.com", FREE_FAILURES - 1, now);

        let later = now + FAILURE_MEMORY;
        fail_times(&throttle, "a@example.com", 1, later);
        assert!(throttle.check_at("a@example.com", later).is_ok(), "starts counting again");
    }

    #[test]
    fn addresses_are_counted_case_insensitively() {
        let throttle = LoginThrottle::default();
        let now = Instant::now();
        fail_times(&throttle, "A@Example.com", FREE_FAILURES, now);
        assert!(throttle.check_at(" a@example.COM ", now).is_err());
    }

    #[test]
    fn one_address_does_not_affect_another() {
        let throttle = LoginThrottle::default();
        let now = Instant::now();
        fail_times(&throttle, "a@example.com", FREE_FAILURES + 5, now);
        assert!(throttle.check_at("b@example.com", now).is_ok());
    }

    #[test]
    fn a_disabled_throttle_never_refuses() {
        let throttle = LoginThrottle::new(false);
        let now = Instant::now();
        fail_times(&throttle, "a@example.com", 100, now);
        assert!(throttle.check_at("a@example.com", now).is_ok());
    }

    #[test]
    fn spraying_addresses_cannot_grow_the_table_past_its_bound() {
        let throttle = LoginThrottle::default();
        let now = Instant::now();
        fail_times(&throttle, "victim@example.com", FREE_FAILURES, now);

        for i in 0..MAX_TRACKED + 10 {
            throttle.record_failure_at(&format!("{i}@example.com"), now);
        }

        assert!(throttle.failures.lock().unwrap().len() <= MAX_TRACKED);
        assert!(throttle.check_at("victim@example.com", now).is_err(), "existing locks survive");
    }
}
