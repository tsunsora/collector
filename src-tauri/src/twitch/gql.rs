//! Twitch's private GraphQL API, one function per operation we use.

use std::time::Duration;

use reqwest::{header, Client};
use serde_json::{json, Value};

use super::{constants::*, random_id, status_error, TwitchError};

pub struct Gql {
    http: Client,
    access_token: String,
    /// Stable per install; Twitch's clients send one with every request.
    device_id: String,
    session_id: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stream {
    /// Changes with every new broadcast; used to spot a fresh stream.
    pub broadcast_id: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChannelInfo {
    pub id: String,
    /// The name with the channel's own capitalisation.
    pub display_name: String,
    /// Address of the channel's profile picture.
    pub avatar: Option<String>,
    pub stream: Option<Stream>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PointsContext {
    pub balance: u64,
    /// Present when a bonus is waiting to be claimed.
    pub claim_id: Option<String>,
    /// Sum of the account's active multipliers on this channel (0.2 = +20%).
    pub multiplier: f64,
}

/// A channel the signed-in user follows.
#[derive(Debug, Clone, PartialEq)]
pub struct Follow {
    pub login: String,
    /// The name with the channel's own capitalisation.
    pub display_name: String,
    /// Address of the channel's profile picture.
    pub avatar: Option<String>,
}

/// Follows come 100 to a page. This many pages is 5,000 channels.
const FOLLOW_PAGES_MAX: usize = 50;
const FOLLOW_PAGE_GAP: Duration = Duration::from_millis(300);

impl Gql {
    pub fn new(http: Client, access_token: String, device_id: String) -> Self {
        Self { http, access_token, device_id, session_id: random_id(16) }
    }

    async fn post(&self, op: &Operation, variables: Value) -> Result<Value, TwitchError> {
        let body = json!({
            "operationName": op.name,
            "variables": variables,
            "extensions": { "persistedQuery": { "version": 1, "sha256Hash": op.hash } },
        });
        let res = self
            .http
            .post(GQL_URL)
            .header(header::AUTHORIZATION, format!("OAuth {}", self.access_token))
            .header("Client-Id", CLIENT_ID)
            .header("Client-Session-Id", &self.session_id)
            .header("X-Device-Id", &self.device_id)
            .header(header::USER_AGENT, USER_AGENT_WEB)
            .json(&body)
            .send()
            .await?;
        if !res.status().is_success() {
            return Err(status_error(res.status().as_u16(), op.name));
        }
        check_errors(op.name, res.json().await?)
    }

    /// The channel's id, or `None` if no such channel exists.
    pub async fn channel_id(&self, login: &str) -> Result<Option<String>, TwitchError> {
        let v = self.post(&GET_ID_FROM_LOGIN, json!({ "login": login })).await?;
        Ok(v["data"]["user"]["id"].as_str().map(str::to_string))
    }

    /// Name, id and live state. `None` if no such channel exists.
    pub async fn channel_info(&self, login: &str) -> Result<Option<ChannelInfo>, TwitchError> {
        let v = self.post(&STREAM_INFO, json!({ "channel": login })).await?;
        Ok(parse_channel_info(&v))
    }

    /// Balance and any waiting bonus. Read-only: does not claim.
    pub async fn points_context(&self, login: &str) -> Result<Option<PointsContext>, TwitchError> {
        let v = self.raw_points_context(login).await?;
        Ok(parse_points_context(&v))
    }

    pub async fn raw_points_context(&self, login: &str) -> Result<Value, TwitchError> {
        self.post(&CHANNEL_POINTS_CONTEXT, json!({ "channelLogin": login })).await
    }

    pub async fn claim_bonus(&self, channel_id: &str, claim_id: &str) -> Result<(), TwitchError> {
        let input = json!({ "input": { "channelID": channel_id, "claimID": claim_id } });
        self.post(&CLAIM_COMMUNITY_POINTS, input).await.map(|_| ())
    }

    /// Every channel the signed-in user follows, oldest follow first.
    /// Read-only. One request per hundred channels.
    pub async fn followed(&self) -> Result<Vec<Follow>, TwitchError> {
        let mut all = Vec::new();
        let mut cursor = String::new();
        for _ in 0..FOLLOW_PAGES_MAX {
            let variables = json!({ "limit": 100, "order": "ASC", "cursor": cursor });
            let v = self.post(&CHANNEL_FOLLOWS, variables).await?;
            let (page, next) = parse_follows(&v)
                .ok_or_else(|| TwitchError::Changed("ChannelFollows: no follows in response".into()))?;
            all.extend(page);
            let Some(next) = next else { break };
            cursor = next;
            tokio::time::sleep(FOLLOW_PAGE_GAP).await;
        }
        Ok(all)
    }

    /// The signed token pair needed to request a channel's HLS playlist.
    pub async fn playback_token(&self, login: &str) -> Result<(String, String), TwitchError> {
        let variables = json!({
            "login": login, "isLive": true, "isVod": false, "vodID": "",
            "playerType": "site", "platform": "web",
        });
        let v = self.post(&PLAYBACK_ACCESS_TOKEN, variables).await?;
        let token = &v["data"]["streamPlaybackAccessToken"];
        match (token["signature"].as_str(), token["value"].as_str()) {
            (Some(sig), Some(value)) => Ok((sig.to_string(), value.to_string())),
            _ => Err(TwitchError::Changed("PlaybackAccessToken: no token in response".into())),
        }
    }

    /// Where minute-watched events for this channel are posted. Scraped from
    /// the channel page's settings script, as Twitch's own player does.
    pub async fn spade_url(&self, login: &str) -> Result<String, TwitchError> {
        let get = |url: String| self.http.get(url).header(header::USER_AGENT, USER_AGENT_WEB).send();
        let page = get(format!("{SITE_URL}/{login}")).await?.text().await?;
        let settings_url = find_settings_url(&page)
            .ok_or_else(|| TwitchError::Unexpected("channel page: no settings script".into()))?;
        let settings = get(settings_url.to_string()).await?.text().await?;
        find_spade_url(&settings)
            .ok_or_else(|| TwitchError::Unexpected("settings script: no spade_url".into()))
    }
}

/// GraphQL reports failures inside a 200 response.
fn check_errors(op: &str, v: Value) -> Result<Value, TwitchError> {
    let Some(first) = v["errors"].as_array().and_then(|e| e.first()) else {
        return Ok(v);
    };
    let message = first["message"].as_str().unwrap_or("unknown error");
    // Twitch no longer has the stored query under the hash this build sends.
    if message.contains("PersistedQueryNotFound") {
        return Err(TwitchError::Changed(format!("{op}: {message}")));
    }
    Err(TwitchError::Unexpected(format!("{op}: {message}")))
}

fn parse_channel_info(v: &Value) -> Option<ChannelInfo> {
    let user = &v["data"]["user"];
    Some(ChannelInfo {
        id: user["id"].as_str()?.to_string(),
        display_name: user["displayName"].as_str().or(user["login"].as_str())?.to_string(),
        avatar: avatar(&user["profileImageURL"]),
        stream: user["stream"]["id"]
            .as_str()
            .map(|id| Stream { broadcast_id: id.to_string() }),
    })
}

/// Twitch serves profile pictures from one host. An address anywhere else is
/// dropped, so the UI is never told to load an image from an unknown place.
const AVATAR_HOST: &str = "https://static-cdn.jtvnw.net/";

fn avatar(v: &Value) -> Option<String> {
    v.as_str().filter(|url| url.starts_with(AVATAR_HOST)).map(str::to_string)
}

/// One page of follows, and the cursor for the next page if there is one.
fn parse_follows(v: &Value) -> Option<(Vec<Follow>, Option<String>)> {
    let follows = &v["data"]["user"]["follows"];
    let edges = follows["edges"].as_array()?;
    let page = edges
        .iter()
        .filter_map(|edge| {
            let node = &edge["node"];
            let login = node["login"].as_str()?.to_lowercase();
            let display_name = node["displayName"].as_str().unwrap_or(&login).to_string();
            Some(Follow { login, display_name, avatar: avatar(&node["profileImageURL"]) })
        })
        .collect();
    let more = follows["pageInfo"]["hasNextPage"].as_bool().unwrap_or(false);
    let next = edges.last().and_then(|edge| edge["cursor"].as_str()).filter(|_| more);
    Some((page, next.map(str::to_string)))
}

fn parse_points_context(v: &Value) -> Option<PointsContext> {
    let points = &v["data"]["community"]["channel"]["self"]["communityPoints"];
    Some(PointsContext {
        balance: points["balance"].as_u64()?,
        claim_id: points["availableClaim"]["id"].as_str().map(str::to_string),
        multiplier: points["activeMultipliers"]
            .as_array()
            .map(|m| m.iter().filter_map(|x| x["factor"].as_f64()).sum())
            .unwrap_or(0.0),
    })
}

fn find_settings_url(page: &str) -> Option<&str> {
    ["https://static.twitchcdn.net/config/settings", "https://assets.twitch.tv/config/settings"]
        .iter()
        .find_map(|prefix| {
            let start = page.find(prefix)?;
            let end = start + page[start..].find(".js")? + ".js".len();
            Some(&page[start..end])
        })
}

fn find_spade_url(settings: &str) -> Option<String> {
    const KEY: &str = r#""spade_url":""#;
    let start = settings.find(KEY)? + KEY.len();
    let end = start + settings[start..].find('"')?;
    Some(settings[start..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_profile_picture_is_kept_only_from_twitchs_own_image_host() {
        let good = "https://static-cdn.jtvnw.net/jtv_user_pictures/abc-profile_image-150x150.png";
        let v = json!({ "data": { "user": { "id": "1", "login": "a", "displayName": "A", "profileImageURL": good } } });
        assert_eq!(parse_channel_info(&v).unwrap().avatar.as_deref(), Some(good));
        for bad in ["https://evil.example/x.png", "http://static-cdn.jtvnw.net/x.png", "javascript:alert(1)", ""] {
            let v = json!({ "data": { "user": { "id": "1", "login": "a", "displayName": "A", "profileImageURL": bad } } });
            assert_eq!(parse_channel_info(&v).unwrap().avatar, None, "{bad}");
        }
    }

    #[test]
    fn reads_a_page_of_follows_and_where_the_next_starts() {
        let v = json!({ "data": { "user": { "id": "999", "follows": {
            "edges": [
                { "cursor": "c1", "node": { "id": "1", "login": "LIRIK", "displayName": "LIRIK" } },
                { "cursor": "c2", "node": { "id": "2", "login": "fps_shaka", "displayName": "釈迦" } },
                { "cursor": "c3", "node": { "id": "3", "login": "nameless" } },
                { "cursor": "c4", "node": null },
            ],
            "pageInfo": { "hasNextPage": true }
        }}}});
        let (page, next) = parse_follows(&v).unwrap();
        let pairs: Vec<_> = page.iter().map(|f| (f.login.as_str(), f.display_name.as_str())).collect();
        assert_eq!(pairs, vec![("lirik", "LIRIK"), ("fps_shaka", "釈迦"), ("nameless", "nameless")]);
        assert_eq!(next.as_deref(), Some("c4"));
    }

    #[test]
    fn the_last_page_of_follows_has_no_next_and_a_missing_list_is_an_error() {
        let last = json!({ "data": { "user": { "follows": {
            "edges": [{ "cursor": "c9", "node": { "login": "lirik", "displayName": "LIRIK" } }],
            "pageInfo": { "hasNextPage": false }
        }}}});
        assert_eq!(parse_follows(&last).unwrap().1, None);
        let none = json!({ "data": { "user": { "follows": { "edges": [], "pageInfo": { "hasNextPage": false } }}}});
        assert_eq!(parse_follows(&none), Some((Vec::new(), None)));
        assert_eq!(parse_follows(&json!({ "data": { "user": null } })), None);
    }

    #[test]
    fn reads_a_live_channel() {
        let v = json!({ "data": { "user": {
            "id": "26610234", "login": "cohhcarnage", "displayName": "CohhCarnage",
            "stream": { "id": "318000111", "viewersCount": 9000 }
        }}});
        let info = parse_channel_info(&v).unwrap();
        assert_eq!(info.avatar, None, "no picture in this answer");
        assert_eq!(info.display_name, "CohhCarnage");
        assert_eq!(info.stream, Some(Stream { broadcast_id: "318000111".into() }));
    }

    #[test]
    fn reads_an_offline_channel_and_a_missing_one() {
        let offline = json!({ "data": { "user": { "id": "1", "displayName": "A", "stream": null } } });
        assert_eq!(parse_channel_info(&offline).unwrap().stream, None);
        assert_eq!(parse_channel_info(&json!({ "data": { "user": null } })), None);
    }

    #[test]
    fn reads_balance_claim_and_multipliers() {
        let v = json!({ "data": { "community": { "channel": { "self": { "communityPoints": {
            "balance": 48210,
            "availableClaim": { "id": "claim-1" },
            "activeMultipliers": [{ "factor": 0.2 }, { "factor": 0.1 }]
        }}}}}});
        let ctx = parse_points_context(&v).unwrap();
        assert_eq!(ctx.balance, 48210);
        assert_eq!(ctx.claim_id.as_deref(), Some("claim-1"));
        assert!((ctx.multiplier - 0.3).abs() < 1e-9);
    }

    #[test]
    fn no_claim_when_none_is_waiting() {
        let v = json!({ "data": { "community": { "channel": { "self": { "communityPoints": {
            "balance": 10, "availableClaim": null, "activeMultipliers": []
        }}}}}});
        let ctx = parse_points_context(&v).unwrap();
        assert_eq!(ctx.claim_id, None);
        assert_eq!(ctx.multiplier, 0.0);
    }

    #[test]
    fn graphql_errors_surface_with_the_operation_name() {
        let v = json!({ "errors": [{ "message": "service timeout" }] });
        let err = check_errors("ChannelPointsContext", v).unwrap_err();
        assert_eq!(err, TwitchError::Unexpected("ChannelPointsContext: service timeout".into()));
        assert!(check_errors("x", json!({ "data": {} })).is_ok());
    }

    #[test]
    fn a_stored_query_twitch_no_longer_has_means_twitch_has_changed() {
        let v = json!({ "errors": [{ "message": "PersistedQueryNotFound" }] });
        let err = check_errors("ChannelPointsContext", v).unwrap_err();
        assert_eq!(err, TwitchError::Changed("ChannelPointsContext: PersistedQueryNotFound".into()));
        // So does a request refused outright; a server error does not.
        assert!(matches!(crate::twitch::status_error(400, "x"), TwitchError::Changed(_)));
        assert!(matches!(crate::twitch::status_error(404, "x"), TwitchError::Changed(_)));
        assert!(matches!(crate::twitch::status_error(503, "x"), TwitchError::Unexpected(_)));
    }

    #[test]
    fn finds_the_settings_script_and_spade_url() {
        let page = r#"<script src="https://assets.twitch.tv/config/settings.abc123.js"></script>"#;
        assert_eq!(
            find_settings_url(page),
            Some("https://assets.twitch.tv/config/settings.abc123.js")
        );
        let settings = r#"{"a":1,"spade_url":"https://video-edge.example/abc.ts","b":2}"#;
        assert_eq!(find_spade_url(settings).as_deref(), Some("https://video-edge.example/abc.ts"));
        assert_eq!(find_settings_url("<html></html>"), None);
        assert_eq!(find_spade_url("{}"), None);
    }
}
