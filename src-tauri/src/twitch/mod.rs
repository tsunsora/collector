//! Everything that talks to Twitch. None of this is a public API: it follows
//! what Twitch's own web and TV clients do, as documented by community miners
//! (rdavydov/Twitch-Channel-Points-Miner-v2). Expect it to break when Twitch
//! changes things; `constants.rs` is the first place to look.

// A few lookups are only used by the debug-only probe.
#![allow(dead_code)]

pub mod auth;
pub mod constants;
pub mod engine;
pub mod gql;
pub mod hermes;
#[cfg(debug_assertions)]
pub mod probe;
pub mod watch;

use std::hash::{BuildHasher, Hasher};

/// What went wrong, in the terms the UI's status line needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TwitchError {
    /// Could not reach Twitch at all.
    Network(String),
    /// The token is missing, expired or revoked.
    Unauthorized,
    /// Twitch asked us to slow down.
    RateLimited,
    /// Twitch answered, but not with what we expected. Usually passing:
    /// a server error, a stream that has just ended.
    Unexpected(String),
    /// Twitch answered in a way that says its private API has moved on: a
    /// request it no longer knows, or one it now rejects outright. Trying
    /// again will not help; the app needs an update.
    Changed(String),
}

impl std::fmt::Display for TwitchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Network(e) => write!(f, "network: {e}"),
            Self::Unauthorized => write!(f, "not signed in"),
            Self::RateLimited => write!(f, "rate limited"),
            Self::Unexpected(e) => write!(f, "unexpected response: {e}"),
            Self::Changed(e) => write!(f, "Twitch has changed: {e}"),
        }
    }
}

impl From<reqwest::Error> for TwitchError {
    fn from(e: reqwest::Error) -> Self {
        if e.is_decode() {
            Self::Unexpected(e.without_url().to_string())
        } else {
            // Strip the URL: it can carry a playback token.
            Self::Network(e.without_url().to_string())
        }
    }
}

/// Map an HTTP status that is not a success to an error.
pub fn status_error(status: u16, context: &str) -> TwitchError {
    match status {
        401 => TwitchError::Unauthorized,
        429 => TwitchError::RateLimited,
        // The request itself is refused or gone: not something a retry fixes.
        400 | 404 | 410 => TwitchError::Changed(format!("{context}: HTTP {status}")),
        _ => TwitchError::Unexpected(format!("{context}: HTTP {status}")),
    }
}

/// Random lowercase-alphanumeric string. Uses the randomly keyed std hasher,
/// which is plenty for request and device ids.
pub fn random_id(len: usize) -> String {
    const CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut out = String::with_capacity(len);
    while out.len() < len {
        let mut n = std::collections::hash_map::RandomState::new().build_hasher().finish();
        for _ in 0..12 {
            if out.len() == len {
                break;
            }
            out.push(CHARS[(n % CHARS.len() as u64) as usize] as char);
            n /= CHARS.len() as u64;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statuses_map_to_ui_states() {
        assert_eq!(status_error(401, "x"), TwitchError::Unauthorized);
        assert_eq!(status_error(429, "x"), TwitchError::RateLimited);
        assert!(matches!(status_error(503, "gql"), TwitchError::Unexpected(m) if m == "gql: HTTP 503"));
    }

    #[test]
    fn random_ids_have_the_requested_length_and_differ() {
        let a = random_id(21);
        let b = random_id(21);
        assert_eq!(a.len(), 21);
        assert!(a.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()));
        assert_ne!(a, b);
    }
}
