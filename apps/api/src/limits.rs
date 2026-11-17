// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Abuse limits kept in memory. They reset when the process restarts and are
//! never written anywhere, client addresses included.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use axum::http::HeaderMap;
use sha2::{Digest, Sha256};
use tokio::sync::{OwnedMutexGuard, Semaphore};
use uuid::Uuid;

use crate::constants::{
    AUTH_REQUESTS_PER_WINDOW_PER_CLIENT, LOGIN_FAILURES_PER_WINDOW_PER_EMAIL,
    LOGIN_FAILURES_PER_WINDOW_PER_EMAIL_AND_CLIENT, MAX_CONCURRENT_PASSWORD_HASHES,
    RATE_LIMIT_MAX_KEYS, RATE_WINDOW_SECS, UPLOAD_LOCK_WAIT_SECS,
};
use crate::error::{AppError, AppResult};

pub struct Limits {
    /// Sign-up, sign-in, refresh and password changes per client address.
    pub auth_by_client: RateLimiter,
    /// Failed sign-ins per email address, from anywhere.
    pub failures_by_email: RateLimiter,
    /// Failed sign-ins per email address from one client address. Tighter
    /// than the limit per email, so one guesser cannot lock a user out alone.
    pub failures_by_email_and_client: RateLimiter,
    /// Password hashes running at the same time. One takes about 19 MiB.
    pub hashing: Semaphore,
    pub uploads: AccountLocks,
}

impl Limits {
    pub fn new() -> Self {
        let window = Duration::from_secs(RATE_WINDOW_SECS);
        Self {
            auth_by_client: RateLimiter::new(
                window,
                AUTH_REQUESTS_PER_WINDOW_PER_CLIENT,
                WhenFull::LetThrough,
            ),
            failures_by_email: RateLimiter::new(
                window,
                LOGIN_FAILURES_PER_WINDOW_PER_EMAIL,
                WhenFull::Refuse,
            ),
            failures_by_email_and_client: RateLimiter::new(
                window,
                LOGIN_FAILURES_PER_WINDOW_PER_EMAIL_AND_CLIENT,
                WhenFull::Refuse,
            ),
            hashing: Semaphore::new(MAX_CONCURRENT_PASSWORD_HASHES),
            uploads: AccountLocks::default(),
        }
    }

    /// Forgets counts whose window has passed and upload locks nobody holds.
    pub fn prune(&self) {
        for limiter in [
            &self.auth_by_client,
            &self.failures_by_email,
            &self.failures_by_email_and_client,
        ] {
            limiter.prune(Instant::now());
        }
        self.uploads.prune();
    }
}

/// Clears expired counts once per window, so the maps only hold keys that
/// are still counting.
pub fn spawn_pruning(limits: Arc<Limits>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(RATE_WINDOW_SECS));
        loop {
            interval.tick().await;
            limits.prune();
        }
    });
}

/// What a limiter does with a new key once it tracks `RATE_LIMIT_MAX_KEYS`.
#[derive(Clone, Copy)]
pub enum WhenFull {
    /// Lets it through uncounted, so a flood of addresses cannot lock
    /// everyone else out.
    LetThrough,
    /// Refuses it, so a flood cannot switch the limit off for an account.
    Refuse,
}

/// Counts requests per key in fixed windows. Keys are kept as hashes, so a
/// long key costs no more memory than a short one.
pub struct RateLimiter {
    window: Duration,
    max: u32,
    when_full: WhenFull,
    entries: Mutex<HashMap<[u8; 32], (Instant, u32)>>,
}

impl RateLimiter {
    pub fn new(window: Duration, max: u32, when_full: WhenFull) -> Self {
        Self {
            window,
            max,
            when_full,
            entries: Mutex::new(HashMap::new()),
        }
    }

    /// Counts a request for `key` and says whether it stays within the limit.
    pub fn allow(&self, key: &str) -> bool {
        self.count(key, Instant::now()) <= self.max
    }

    /// Whether `key` used up its limit, without counting this call.
    pub fn blocked(&self, key: &str) -> bool {
        let now = Instant::now();
        let entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
        match entries.get(&digest(key)) {
            Some((started, count)) => {
                now.duration_since(*started) < self.window && *count >= self.max
            }
            None => {
                entries.len() >= RATE_LIMIT_MAX_KEYS && matches!(self.when_full, WhenFull::Refuse)
            }
        }
    }

    /// Counts one event for `key`, such as a failed sign-in.
    pub fn record(&self, key: &str) {
        self.count(key, Instant::now());
    }

    /// Counts for `key` and returns the count in the current window.
    fn count(&self, key: &str, now: Instant) -> u32 {
        let key = digest(key);
        let mut entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
        if entries.len() >= RATE_LIMIT_MAX_KEYS && !entries.contains_key(&key) {
            return match self.when_full {
                WhenFull::LetThrough => 0,
                WhenFull::Refuse => u32::MAX,
            };
        }
        let entry = entries.entry(key).or_insert((now, 0));
        if now.duration_since(entry.0) >= self.window {
            *entry = (now, 0);
        }
        entry.1 = entry.1.saturating_add(1);
        entry.1
    }

    fn prune(&self, now: Instant) {
        self.entries
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|_, (started, _)| now.duration_since(*started) < self.window);
    }
}

fn digest(key: &str) -> [u8; 32] {
    Sha256::digest(key.as_bytes()).into()
}

/// One upload at a time per account, so a quota check sees every stored byte.
#[derive(Default)]
pub struct AccountLocks(Mutex<HashMap<Uuid, Arc<tokio::sync::Mutex<()>>>>);

impl AccountLocks {
    fn lock_for(&self, account_id: Uuid) -> Arc<tokio::sync::Mutex<()>> {
        Arc::clone(
            self.0
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .entry(account_id)
                .or_default(),
        )
    }

    /// Waits for the account's other upload to finish, or turns this one away.
    pub async fn lock(&self, account_id: Uuid) -> AppResult<OwnedMutexGuard<()>> {
        self.lock_within(account_id, Duration::from_secs(UPLOAD_LOCK_WAIT_SECS))
            .await
            .ok_or_else(|| AppError::conflict("another upload of this account is still running"))
    }

    /// Waits at most `wait` for the account's other upload to finish.
    async fn lock_within(&self, account_id: Uuid, wait: Duration) -> Option<OwnedMutexGuard<()>> {
        tokio::time::timeout(wait, self.lock_for(account_id).lock_owned())
            .await
            .ok()
    }

    /// The lock if no upload of the account runs right now.
    pub fn try_lock(&self, account_id: Uuid) -> Option<OwnedMutexGuard<()>> {
        self.lock_for(account_id).try_lock_owned().ok()
    }

    /// Drops the locks no upload holds or waits for. Those hold the only
    /// other references.
    fn prune(&self) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|_, lock| Arc::strong_count(lock) > 1);
    }
}

/// The client address a request counts against. A reverse proxy on this
/// machine or in a private network puts the client's address last in
/// `X-Forwarded-For`. Anyone else could make that header up, so their own
/// address counts. IPv6 clients count per /56, the block a home connection
/// usually gets.
pub fn client_key(peer: SocketAddr, headers: &HeaderMap) -> String {
    // A proxy that adds a line of its own instead of extending the one it got
    // leaves a made up line first, so the last line counts.
    let address = headers
        .get_all("x-forwarded-for")
        .iter()
        .next_back()
        .filter(|_| is_local(peer.ip()))
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.rsplit(',').next())
        .and_then(|value| value.trim().parse::<IpAddr>().ok())
        .unwrap_or(peer.ip());
    match address.to_canonical() {
        IpAddr::V6(address) => {
            let [a, b, c, d, ..] = address.segments();
            // The low byte of the fourth group lies beyond the /56.
            format!("{a:x}:{b:x}:{c:x}:{:x}::/56", d & 0xff00)
        }
        address => address.to_string(),
    }
}

/// Loopback or a private network, where a proxy in front of the server sits.
fn is_local(address: IpAddr) -> bool {
    match address.to_canonical() {
        IpAddr::V4(address) => address.is_loopback() || address.is_private(),
        IpAddr::V6(address) => address.is_loopback() || address.is_unique_local(),
    }
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;

    use super::*;

    #[test]
    fn limits_each_key_within_its_window() {
        let limiter = RateLimiter::new(Duration::from_secs(60), 2, WhenFull::LetThrough);
        let start = Instant::now();
        assert_eq!(limiter.count("a", start), 1);
        assert_eq!(limiter.count("a", start), 2);
        assert_eq!(limiter.count("a", start), 3);
        assert_eq!(limiter.count("b", start), 1);
        assert_eq!(limiter.count("a", start + Duration::from_secs(61)), 1);
    }

    #[test]
    fn failures_block_only_once_used_up() {
        let limiter = RateLimiter::new(Duration::from_secs(60), 2, WhenFull::Refuse);
        assert!(!limiter.blocked("player@example.com"));
        limiter.record("player@example.com");
        assert!(!limiter.blocked("player@example.com"));
        limiter.record("player@example.com");
        assert!(limiter.blocked("player@example.com"));
        assert!(!limiter.blocked("other@example.com"));
    }

    #[test]
    fn pruning_forgets_expired_counts() {
        let limiter = RateLimiter::new(Duration::from_secs(60), 2, WhenFull::Refuse);
        let start = Instant::now();
        limiter.count("a", start);
        limiter.prune(start + Duration::from_secs(61));
        assert!(limiter.entries.lock().unwrap().is_empty());
    }

    #[test]
    fn trusts_the_forwarded_address_only_from_a_local_proxy() {
        let public: SocketAddr = "198.51.100.7:4000".parse().unwrap();
        let proxy: SocketAddr = "172.18.0.5:4000".parse().unwrap();
        let mut headers = HeaderMap::new();
        assert_eq!(client_key(proxy, &headers), "172.18.0.5");
        headers.insert(
            "x-forwarded-for",
            HeaderValue::from_static("6.6.6.6, 203.0.113.9"),
        );
        assert_eq!(client_key(public, &headers), "198.51.100.7");
        assert_eq!(client_key(proxy, &headers), "203.0.113.9");
        headers.insert(
            "x-forwarded-for",
            HeaderValue::from_static("2001:db8:1:2ab:3:4:5:6"),
        );
        let mapped_loopback: SocketAddr = "[::ffff:127.0.0.1]:4000".parse().unwrap();
        assert_eq!(client_key(mapped_loopback, &headers), "2001:db8:1:200::/56");

        let mut lines = HeaderMap::new();
        lines.append("x-forwarded-for", HeaderValue::from_static("6.6.6.6"));
        lines.append("x-forwarded-for", HeaderValue::from_static("203.0.113.9"));
        assert_eq!(client_key(proxy, &lines), "203.0.113.9");
    }

    #[test]
    fn knows_the_addresses_a_proxy_sits_at() {
        let local = [
            "127.0.0.1",
            "10.0.0.1",
            "172.16.0.1",
            "172.31.255.255",
            "192.168.1.2",
            "::1",
            "fd00::1",
            "::ffff:10.0.0.1",
        ];
        for address in local {
            assert!(is_local(address.parse().unwrap()), "{address}");
        }
        let public = [
            "172.32.0.1",
            "100.64.0.1",
            "2001:db8::1",
            "::ffff:203.0.113.9",
        ];
        for address in public {
            assert!(!is_local(address.parse().unwrap()), "{address}");
        }
    }

    #[tokio::test]
    async fn uploads_of_one_account_wait_for_each_other() {
        let locks = AccountLocks::default();
        let account = Uuid::new_v4();
        let wait = Duration::from_millis(50);
        let first = locks.lock_within(account, wait).await.unwrap();
        assert!(locks.try_lock(Uuid::new_v4()).is_some());
        assert!(locks.lock_within(account, wait).await.is_none());
        assert!(locks.try_lock(account).is_none());
        locks.prune();
        assert_eq!(locks.0.lock().unwrap().len(), 1);
        drop(first);
        assert!(locks.lock_within(account, wait).await.is_some());
        locks.prune();
        assert!(locks.0.lock().unwrap().is_empty());
    }
}
