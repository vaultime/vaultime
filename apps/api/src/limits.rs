// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Abuse limits kept in memory. They reset when the process restarts and are
//! never written anywhere, client addresses included.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use axum::http::HeaderMap;
use tokio::sync::{OwnedMutexGuard, Semaphore};
use uuid::Uuid;

use crate::constants::{
    AUTH_REQUESTS_PER_WINDOW_PER_CLIENT, BETA_APPLICATIONS_PER_WINDOW_PER_CLIENT,
    LOGIN_ATTEMPTS_PER_WINDOW_PER_EMAIL, MAX_CONCURRENT_PASSWORD_HASHES, RATE_LIMIT_MAX_KEYS,
    RATE_WINDOW_SECS,
};

pub struct Limits {
    /// Sign-up, sign-in, refresh and password changes per client address.
    pub auth_by_client: RateLimiter,
    /// Sign-in attempts per email address.
    pub login_by_email: RateLimiter,
    /// Beta applications per client address.
    pub beta_by_client: RateLimiter,
    /// Password hashes running at the same time, each takes about 19 MiB.
    pub hashing: Semaphore,
    pub uploads: AccountLocks,
}

impl Limits {
    pub fn new() -> Self {
        let window = Duration::from_secs(RATE_WINDOW_SECS);
        Self {
            auth_by_client: RateLimiter::new(window, AUTH_REQUESTS_PER_WINDOW_PER_CLIENT),
            login_by_email: RateLimiter::new(window, LOGIN_ATTEMPTS_PER_WINDOW_PER_EMAIL),
            beta_by_client: RateLimiter::new(window, BETA_APPLICATIONS_PER_WINDOW_PER_CLIENT),
            hashing: Semaphore::new(MAX_CONCURRENT_PASSWORD_HASHES),
            uploads: AccountLocks::default(),
        }
    }
}

/// Counts requests per key in fixed windows.
pub struct RateLimiter {
    window: Duration,
    max: u32,
    entries: Mutex<HashMap<String, (Instant, u32)>>,
}

impl RateLimiter {
    pub fn new(window: Duration, max: u32) -> Self {
        Self {
            window,
            max,
            entries: Mutex::new(HashMap::new()),
        }
    }

    /// Counts a request for `key` and says whether it stays within the limit.
    pub fn allow(&self, key: &str) -> bool {
        self.allow_at(key, Instant::now())
    }

    fn allow_at(&self, key: &str, now: Instant) -> bool {
        let mut entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
        if entries.len() >= RATE_LIMIT_MAX_KEYS && !entries.contains_key(key) {
            entries.retain(|_, (started, _)| now.duration_since(*started) < self.window);
            // A flood of new keys must not lock everyone else out, so new keys
            // go uncounted until the old ones expire.
            if entries.len() >= RATE_LIMIT_MAX_KEYS {
                return true;
            }
        }
        let entry = entries.entry(key.to_owned()).or_insert((now, 0));
        if now.duration_since(entry.0) >= self.window {
            *entry = (now, 0);
        }
        entry.1 = entry.1.saturating_add(1);
        entry.1 <= self.max
    }
}

/// One upload at a time per account, so a quota check sees every stored byte.
#[derive(Default)]
pub struct AccountLocks(Mutex<HashMap<Uuid, Arc<tokio::sync::Mutex<()>>>>);

impl AccountLocks {
    pub async fn lock(&self, account_id: Uuid) -> OwnedMutexGuard<()> {
        let lock = Arc::clone(
            self.0
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .entry(account_id)
                .or_default(),
        );
        lock.lock_owned().await
    }
}

/// The client address as Caddy saw it. The API only listens on localhost
/// behind Caddy, which puts the peer address last in `X-Forwarded-For`. IPv6
/// clients count per /64, the block a single connection usually gets.
pub fn client_key(headers: &HeaderMap) -> String {
    let address = headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.rsplit(',').next())
        .and_then(|value| value.trim().parse::<IpAddr>().ok());
    match address {
        Some(IpAddr::V6(address)) => {
            let [a, b, c, d, ..] = address.segments();
            format!("{a:x}:{b:x}:{c:x}:{d:x}::/64")
        }
        Some(address) => address.to_string(),
        None => "unknown".into(),
    }
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;

    use super::*;

    #[test]
    fn limits_each_key_within_its_window() {
        let limiter = RateLimiter::new(Duration::from_secs(60), 2);
        let start = Instant::now();
        assert!(limiter.allow_at("a", start));
        assert!(limiter.allow_at("a", start));
        assert!(!limiter.allow_at("a", start));
        assert!(limiter.allow_at("b", start));
        assert!(limiter.allow_at("a", start + Duration::from_secs(61)));
    }

    #[test]
    fn takes_the_address_caddy_added_last() {
        let mut headers = HeaderMap::new();
        assert_eq!(client_key(&headers), "unknown");
        headers.insert(
            "x-forwarded-for",
            HeaderValue::from_static("6.6.6.6, 203.0.113.9"),
        );
        assert_eq!(client_key(&headers), "203.0.113.9");
        headers.insert(
            "x-forwarded-for",
            HeaderValue::from_static("2001:db8:1:2:3:4:5:6"),
        );
        assert_eq!(client_key(&headers), "2001:db8:1:2::/64");
    }

    #[tokio::test]
    async fn uploads_of_one_account_wait_for_each_other() {
        let locks = AccountLocks::default();
        let account = Uuid::new_v4();
        let first = locks.lock(account).await;
        let other = locks.lock(Uuid::new_v4()).await;
        let waiting = tokio::time::timeout(Duration::from_millis(50), locks.lock(account)).await;
        assert!(waiting.is_err());
        drop((first, other));
        assert!(
            tokio::time::timeout(Duration::from_millis(50), locks.lock(account))
                .await
                .is_ok()
        );
    }
}
