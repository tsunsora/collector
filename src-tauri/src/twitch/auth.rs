//! Sign-in with Twitch's device-code flow, and where the token is kept.
//!
//! The user types a short code at twitch.tv/activate in their own browser; the
//! app never sees a password. The token lives in Windows Credential Manager
//! and must never be written to a file or a log.

use reqwest::{header, Client};
use serde::{Deserialize, Serialize};

use super::{constants::*, status_error, TwitchError};

const KEYRING_SERVICE: &str = "Collector";
const KEYRING_USER: &str = "twitch-token";

#[derive(Debug, Deserialize)]
pub struct DeviceCode {
    pub device_code: String,
    /// What the user types at `verification_uri`.
    pub user_code: String,
    pub verification_uri: String,
    /// Seconds until the code stops working.
    pub expires_in: u64,
    /// Seconds to wait between polls.
    pub interval: u64,
}

/// Deliberately not `Debug`, so it cannot end up in a log by accident.
#[derive(Clone, Serialize, Deserialize)]
pub struct Token {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Identity {
    pub user_id: String,
    pub login: String,
    /// Seconds until the token expires; 0 or absent means it does not.
    #[serde(default)]
    pub expires_in: u64,
}

pub enum Poll {
    /// The user has not approved the code yet.
    Pending,
    Granted(Token),
}

fn tv_request(http: &Client, url: &str) -> reqwest::RequestBuilder {
    http.post(url)
        .header("Client-Id", CLIENT_ID)
        .header(header::USER_AGENT, USER_AGENT_TV)
}

pub async fn request_device_code(http: &Client) -> Result<DeviceCode, TwitchError> {
    let res = tv_request(http, DEVICE_URL)
        .form(&[("client_id", CLIENT_ID), ("scopes", SCOPES)])
        .send()
        .await?;
    if !res.status().is_success() {
        return Err(status_error(res.status().as_u16(), "device code"));
    }
    Ok(res.json().await?)
}

/// Ask once whether the user has approved the code. The caller waits
/// `interval` seconds between calls and gives up after `expires_in`.
pub async fn poll_token(http: &Client, device_code: &str) -> Result<Poll, TwitchError> {
    let res = tv_request(http, TOKEN_URL)
        .form(&[
            ("client_id", CLIENT_ID),
            ("device_code", device_code),
            ("grant_type", DEVICE_GRANT),
        ])
        .send()
        .await?;
    match res.status().as_u16() {
        200 => Ok(Poll::Granted(res.json().await?)),
        // Twitch answers 400 "authorization_pending" until the user approves.
        400 => Ok(Poll::Pending),
        status => Err(status_error(status, "token")),
    }
}

/// Who the token belongs to, or `None` if Twitch no longer accepts it.
pub async fn validate(http: &Client, token: &Token) -> Result<Option<Identity>, TwitchError> {
    let res = http
        .get(VALIDATE_URL)
        .header(header::AUTHORIZATION, format!("OAuth {}", token.access_token))
        .send()
        .await?;
    match res.status().as_u16() {
        200 => Ok(Some(res.json().await?)),
        401 => Ok(None),
        status => Err(status_error(status, "validate")),
    }
}

fn entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER).map_err(|e| e.to_string())
}

pub fn load_token() -> Option<Token> {
    let json = entry().ok()?.get_password().ok()?;
    serde_json::from_str(&json).ok()
}

pub fn save_token(token: &Token) -> Result<(), String> {
    let json = serde_json::to_string(token).map_err(|e| e.to_string())?;
    entry()?.set_password(&json).map_err(|e| e.to_string())
}

pub fn delete_token() {
    if let Ok(entry) = entry() {
        let _ = entry.delete_credential();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_twitch_device_code_response() {
        let json = r#"{"device_code":"abc","expires_in":1800,"interval":5,
            "user_code":"ABCDEFGH","verification_uri":"https://www.twitch.tv/activate?device-code=ABCDEFGH"}"#;
        let code: DeviceCode = serde_json::from_str(json).unwrap();
        assert_eq!(code.user_code, "ABCDEFGH");
        assert_eq!(code.interval, 5);
        assert_eq!(code.expires_in, 1800);
    }

    #[test]
    fn token_tolerates_a_missing_refresh_token() {
        let token: Token =
            serde_json::from_str(r#"{"access_token":"t","scope":[],"token_type":"bearer"}"#).unwrap();
        assert_eq!(token.access_token, "t");
        assert!(token.refresh_token.is_none());
    }
}
