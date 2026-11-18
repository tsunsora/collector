//! Reporting watch time, the way Twitch's web player does: get a playback
//! token, fetch the playlist down to its newest segment, touch that segment,
//! then post a `minute-watched` event to the channel's spade address.
//! Steps and formats follow the reference miner (`send_minute_watched_events`).

use reqwest::{header, Client};
use serde_json::json;

use super::{constants::USER_AGENT_WEB, gql::Gql, status_error, TwitchError};

const USHER_URL: &str = "https://usher.ttvnw.net/api/channel/hls";

pub struct Target<'a> {
    pub login: &'a str,
    pub channel_id: &'a str,
    pub broadcast_id: &'a str,
    pub spade_url: &'a str,
}

/// One round of "still watching" for one channel.
pub async fn ping(http: &Client, gql: &Gql, user_id: &str, target: &Target<'_>) -> Result<(), TwitchError> {
    let (signature, token) = gql.playback_token(target.login).await?;

    let master = get_text(
        http.get(format!("{USHER_URL}/{}.m3u8", target.login))
            .query(&[("sig", signature.as_str()), ("token", token.as_str())]),
        "playlist",
    )
    .await?;
    let variant_url = last_url(&master).ok_or_else(|| unexpected("playlist has no stream"))?;
    let variant = get_text(http.get(variant_url), "stream playlist").await?;
    let segment_url = last_url(&variant).ok_or_else(|| unexpected("stream playlist has no segment"))?;

    let head = http.head(segment_url).header(header::USER_AGENT, USER_AGENT_WEB).send().await?;
    if !head.status().is_success() {
        return Err(status_error(head.status().as_u16(), "segment"));
    }

    let payload = minute_watched_payload(target, user_id);
    let posted = http
        .post(target.spade_url)
        .header(header::USER_AGENT, USER_AGENT_WEB)
        .form(&[("data", payload.as_str())])
        .send()
        .await?;
    match posted.status().as_u16() {
        204 => Ok(()),
        status => Err(status_error(status, "minute-watched")),
    }
}

async fn get_text(request: reqwest::RequestBuilder, context: &str) -> Result<String, TwitchError> {
    let res = request.header(header::USER_AGENT, USER_AGENT_WEB).send().await?;
    if !res.status().is_success() {
        return Err(status_error(res.status().as_u16(), context));
    }
    Ok(res.text().await?)
}

fn unexpected(message: &str) -> TwitchError {
    TwitchError::Unexpected(message.to_string())
}

/// The last URL in an HLS playlist: the lowest quality in a master playlist,
/// the newest segment in a stream playlist.
pub fn last_url(playlist: &str) -> Option<&str> {
    playlist.lines().map(str::trim).rev().find(|line| line.starts_with("https://"))
}

/// The event body: compact JSON, base64-encoded, sent as the form field `data`.
pub fn minute_watched_payload(target: &Target<'_>, user_id: &str) -> String {
    // The web client sends the user id as a number.
    let user_id = user_id.parse::<u64>().map(|n| json!(n)).unwrap_or_else(|_| json!(user_id));
    let event = json!([{
        "event": "minute-watched",
        "properties": {
            "channel_id": target.channel_id,
            "broadcast_id": target.broadcast_id,
            "player": "site",
            "user_id": user_id,
            "live": true,
            "channel": target.login,
        },
    }]);
    base64(event.to_string().as_bytes())
}

/// Standard base64 with padding.
fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(TABLE[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_the_standard_vectors() {
        for (input, expected) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64(input.as_bytes()), expected);
        }
    }

    #[test]
    fn the_payload_is_the_event_twitch_expects() {
        let target = Target {
            login: "lirik",
            channel_id: "23161357",
            broadcast_id: "318000111",
            spade_url: "https://spade.twitch.tv/track",
        };
        let json = r#"[{"event":"minute-watched","properties":{"broadcast_id":"318000111","channel":"lirik","channel_id":"23161357","live":true,"player":"site","user_id":42}}]"#;
        assert_eq!(minute_watched_payload(&target, "42"), base64(json.as_bytes()));
    }

    #[test]
    fn finds_the_lowest_quality_and_the_newest_segment() {
        let master = "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=6000000\nhttps://video.example/chunked.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=230000\nhttps://video.example/160p.m3u8";
        assert_eq!(last_url(master), Some("https://video.example/160p.m3u8"));

        let variant = "#EXTM3U\n#EXTINF:2.000,live\nhttps://video.example/1.ts\n#EXTINF:2.000,live\nhttps://video.example/2.ts\n";
        assert_eq!(last_url(variant), Some("https://video.example/2.ts"));

        assert_eq!(last_url("#EXTM3U\n"), None);
    }
}
