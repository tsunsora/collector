//! Hermes, the WebSocket Twitch's web client uses for live events since it
//! retired PubSub. Messages are JSON envelopes; a `notification` carries the
//! old PubSub payload as a string. Protocol as reverse-engineered in the
//! reference miner's PR 728. The least certain part of the client.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpStream;
use tokio_tungstenite::{
    tungstenite::{client::IntoClientRequest, http::HeaderValue, Message},
    MaybeTlsStream, WebSocketStream,
};

use super::{constants::*, random_id, TwitchError};
use crate::collector::{Pushed, Reason};

pub type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// Open a connection the way Twitch's web client does. Twitch speaks first,
/// with a `welcome`.
pub async fn connect() -> Result<Socket, TwitchError> {
    let network = |e: &dyn std::fmt::Display| TwitchError::Network(format!("hermes: {e}"));
    let mut request = format!("{HERMES_URL}?clientId={CLIENT_ID_WEB}")
        .into_client_request()
        .map_err(|e| network(&e))?;
    request.headers_mut().insert("Origin", HeaderValue::from_static(SITE_URL));
    let (socket, _) = tokio_tungstenite::connect_async(request).await.map_err(|e| network(&e))?;
    Ok(socket)
}

#[derive(Debug, PartialEq)]
pub enum Incoming {
    Welcome { keepalive_sec: u64 },
    /// `Ok(())` or Twitch's error text.
    Authenticated(Result<(), String>),
    Subscribed { subscription_id: String, ok: bool },
    /// `pubsub` is the legacy PubSub message, still JSON-encoded.
    Notification { subscription_id: String, pubsub: String },
    Keepalive,
    /// Twitch wants the client to move to this URL.
    Reconnect { url: String },
    Other(String),
}

pub fn parse(text: &str) -> Option<Incoming> {
    let v: Value = serde_json::from_str(text).ok()?;
    let kind = v["type"].as_str()?;
    Some(match kind {
        "welcome" => {
            let k = &v["welcome"]["keepaliveSec"];
            let keepalive_sec = k.as_u64().or_else(|| k.as_str()?.parse().ok())?;
            Incoming::Welcome { keepalive_sec }
        }
        "authenticateResponse" => {
            let r = &v["authenticateResponse"];
            Incoming::Authenticated(match r["result"].as_str()? {
                "ok" => Ok(()),
                other => Err(r["error"].as_str().unwrap_or(other).to_string()),
            })
        }
        "subscribeResponse" => {
            let r = &v["subscribeResponse"];
            Incoming::Subscribed {
                subscription_id: r["subscription"]["id"].as_str()?.to_string(),
                ok: r["result"].as_str()? == "ok",
            }
        }
        "notification" => {
            let n = &v["notification"];
            Incoming::Notification {
                subscription_id: n["subscription"]["id"].as_str()?.to_string(),
                pubsub: n["pubsub"].as_str()?.to_string(),
            }
        }
        "keepalive" => Incoming::Keepalive,
        "reconnect" => Incoming::Reconnect { url: v["reconnect"]["url"].as_str()?.to_string() },
        other => Incoming::Other(other.to_string()),
    })
}

/// Read the legacy PubSub message inside a notification on `topic`. Returns
/// the Twitch id of the channel it is about and what happened, or `None` for
/// the many messages the app has no use for (view counts, predictions, ...).
pub fn parse_pubsub(topic: &str, pubsub: &str) -> Option<(String, Pushed)> {
    let (family, subject) = topic.rsplit_once('.')?;
    let v: Value = serde_json::from_str(pubsub).ok()?;
    let data = &v["data"];
    // Ids arrive as strings, amounts as numbers; accept either for both.
    let text = |v: &Value| v.as_str().map(str::to_string).or_else(|| v.as_u64().map(|n| n.to_string()));
    let number = |v: &Value| v.as_u64().or_else(|| v.as_str()?.parse().ok());
    let balance_channel = || text(&data["channel_id"]).or_else(|| text(&data["balance"]["channel_id"]));

    match (family, v["type"].as_str()?) {
        (TOPIC_USER_POINTS, "points-earned") => {
            let gain = &data["point_gain"];
            let reason = match gain["reason_code"].as_str()? {
                "WATCH" => Reason::Watch,
                "CLAIM" => Reason::Claim,
                "WATCH_STREAK" => Reason::Streak,
                _ => Reason::Other,
            };
            let pushed = Pushed::Earned {
                balance: number(&data["balance"]["balance"])?,
                points: number(&gain["total_points"])?,
                reason,
            };
            Some((balance_channel()?, pushed))
        }
        (TOPIC_USER_POINTS, "points-spent") => {
            Some((balance_channel()?, Pushed::Spent { balance: number(&data["balance"]["balance"])? }))
        }
        (TOPIC_USER_POINTS, "claim-available") => {
            let claim = &data["claim"];
            Some((text(&claim["channel_id"])?, Pushed::BonusAvailable { claim_id: text(&claim["id"])? }))
        }
        (TOPIC_PLAYBACK, "stream-up") => Some((subject.to_string(), Pushed::StreamUp)),
        (TOPIC_PLAYBACK, "stream-down") => Some((subject.to_string(), Pushed::StreamDown)),
        _ => None,
    }
}

pub fn authenticate_message(access_token: &str) -> String {
    json!({
        "id": random_id(21),
        "type": "authenticate",
        "authenticate": { "token": access_token },
        "timestamp": timestamp(SystemTime::now()),
    })
    .to_string()
}

/// Returns the message and the subscription id Twitch will echo back.
pub fn subscribe_message(topic: &str) -> (String, String) {
    let subscription_id = random_id(21);
    let message = json!({
        "id": random_id(21),
        "type": "subscribe",
        "subscribe": { "id": subscription_id, "type": "pubsub", "pubsub": { "topic": topic } },
        "timestamp": timestamp(SystemTime::now()),
    })
    .to_string();
    (message, subscription_id)
}

/// "2025-10-03T07:36:29.045Z", the format Twitch's web client sends.
pub(crate) fn timestamp(at: SystemTime) -> String {
    let ms = at.duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
    let (secs, millis) = (ms.div_euclid(1000), ms.rem_euclid(1000));
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Days since 1970-01-01 to a civil date (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{millis:03}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// What the probe learns from one connect, authenticate, subscribe round trip.
#[derive(Debug, Default)]
pub struct Handshake {
    pub keepalive_sec: Option<u64>,
    pub authenticated: Option<Result<(), String>>,
    pub subscribed: Option<bool>,
}

/// Connect, authenticate, subscribe to the user's points topic, then leave.
/// Read-only: it only listens.
pub async fn handshake(access_token: &str, user_id: &str) -> Result<Handshake, TwitchError> {
    let network = |e: &dyn std::fmt::Display| TwitchError::Network(format!("hermes: {e}"));
    let mut socket = connect().await?;

    let mut report = Handshake::default();
    let (subscribe, subscription_id) = subscribe_message(&topic_user_points(user_id));
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);

    while report.subscribed.is_none() {
        let next = tokio::time::timeout_at(deadline, socket.next()).await;
        let Ok(Some(Ok(message))) = next else { break };
        let Message::Text(text) = message else { continue };
        match parse(&text) {
            Some(Incoming::Welcome { keepalive_sec }) => {
                report.keepalive_sec = Some(keepalive_sec);
                let auth = authenticate_message(access_token);
                socket.send(Message::text(auth)).await.map_err(|e| network(&e))?;
            }
            Some(Incoming::Authenticated(result)) => {
                let ok = result.is_ok();
                report.authenticated = Some(result);
                if !ok {
                    break;
                }
                socket.send(Message::text(subscribe.clone())).await.map_err(|e| network(&e))?;
            }
            Some(Incoming::Subscribed { subscription_id: id, ok }) if id == subscription_id => {
                report.subscribed = Some(ok);
            }
            _ => {}
        }
    }
    let _ = socket.close(None).await;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_each_envelope() {
        let welcome = r#"{"id":"a","type":"welcome","timestamp":"t",
            "welcome":{"keepaliveSec":10,"recoveryUrl":"wss://x","sessionId":"s"}}"#;
        assert_eq!(parse(welcome), Some(Incoming::Welcome { keepalive_sec: 10 }));

        let auth_ok = r#"{"id":"a","parentId":"p","type":"authenticateResponse","timestamp":"t",
            "authenticateResponse":{"result":"ok"}}"#;
        assert_eq!(parse(auth_ok), Some(Incoming::Authenticated(Ok(()))));

        let auth_bad = r#"{"id":"a","parentId":"p","type":"authenticateResponse","timestamp":"t",
            "authenticateResponse":{"result":"error","error":"invalid token","errorCode":"AUTH001"}}"#;
        assert_eq!(parse(auth_bad), Some(Incoming::Authenticated(Err("invalid token".into()))));

        let sub = r#"{"id":"a","parentId":"p","type":"subscribeResponse","timestamp":"t",
            "subscribeResponse":{"result":"ok","subscription":{"id":"sub1"}}}"#;
        assert_eq!(
            parse(sub),
            Some(Incoming::Subscribed { subscription_id: "sub1".into(), ok: true })
        );

        let note = r#"{"id":"a","type":"notification","timestamp":"t","notification":{
            "subscription":{"id":"sub1"},"type":"pubsub",
            "pubsub":"{\"type\":\"points-earned\",\"data\":{}}"}}"#;
        assert_eq!(
            parse(note),
            Some(Incoming::Notification {
                subscription_id: "sub1".into(),
                pubsub: r#"{"type":"points-earned","data":{}}"#.into(),
            })
        );

        assert_eq!(parse(r#"{"id":"a","type":"keepalive","timestamp":"t"}"#), Some(Incoming::Keepalive));
        assert_eq!(
            parse(r#"{"id":"a","type":"reconnect","timestamp":"t","reconnect":{"url":"wss://y"}}"#),
            Some(Incoming::Reconnect { url: "wss://y".into() })
        );
        assert_eq!(parse("not json"), None);
    }

    #[test]
    fn reads_points_earned_with_its_reason() {
        let topic = "community-points-user-v1.999";
        let earned = |reason: &str, points: u64, balance: u64| {
            format!(
                r#"{{"type":"points-earned","data":{{"timestamp":"t","channel_id":"42",
                "point_gain":{{"user_id":"999","channel_id":"42","total_points":{points},
                "baseline_points":10,"reason_code":"{reason}","multipliers":[]}},
                "balance":{{"user_id":"999","channel_id":"42","balance":{balance}}}}}}}"#
            )
        };
        let read = |reason, points, balance| parse_pubsub(topic, &earned(reason, points, balance));
        assert_eq!(
            read("WATCH", 10, 1010),
            Some(("42".into(), Pushed::Earned { balance: 1010, points: 10, reason: Reason::Watch }))
        );
        assert_eq!(
            read("CLAIM", 50, 1060),
            Some(("42".into(), Pushed::Earned { balance: 1060, points: 50, reason: Reason::Claim }))
        );
        assert_eq!(
            read("WATCH_STREAK", 450, 1510),
            Some(("42".into(), Pushed::Earned { balance: 1510, points: 450, reason: Reason::Streak }))
        );
        assert_eq!(
            read("RAID", 250, 1760),
            Some(("42".into(), Pushed::Earned { balance: 1760, points: 250, reason: Reason::Other }))
        );
    }

    #[test]
    fn reads_spending_bonuses_and_stream_changes() {
        let user = "community-points-user-v1.999";
        // The channel id is only inside the balance here.
        let spent = r#"{"type":"points-spent","data":{"timestamp":"t",
            "balance":{"user_id":"999","channel_id":"42","balance":200}}}"#;
        assert_eq!(parse_pubsub(user, spent), Some(("42".into(), Pushed::Spent { balance: 200 })));

        let claim = r#"{"type":"claim-available","data":{"timestamp":"t",
            "claim":{"id":"claim-1","user_id":"999","channel_id":"42","created_at":"t"}}}"#;
        assert_eq!(
            parse_pubsub(user, claim),
            Some(("42".into(), Pushed::BonusAvailable { claim_id: "claim-1".into() }))
        );

        let playback = "video-playback-by-id.42";
        let up = r#"{"type":"stream-up","server_time":1700000000.5,"play_delay":0}"#;
        let down = r#"{"type":"stream-down","server_time":1700000000.5}"#;
        assert_eq!(parse_pubsub(playback, up), Some(("42".into(), Pushed::StreamUp)));
        assert_eq!(parse_pubsub(playback, down), Some(("42".into(), Pushed::StreamDown)));
    }

    #[test]
    fn ignores_messages_it_has_no_use_for() {
        let playback = "video-playback-by-id.42";
        let views = r#"{"type":"viewcount","server_time":1700000000.5,"viewers":1234}"#;
        assert_eq!(parse_pubsub(playback, views), None);
        // A stream message on the wrong topic, a claim already made, and junk.
        let up = r#"{"type":"stream-up","server_time":1.0}"#;
        assert_eq!(parse_pubsub("community-points-user-v1.999", up), None);
        let claimed = r#"{"type":"claim-claimed","data":{"claim":{"id":"c","channel_id":"42"}}}"#;
        assert_eq!(parse_pubsub("community-points-user-v1.999", claimed), None);
        assert_eq!(parse_pubsub(playback, "not json"), None);
        assert_eq!(parse_pubsub("no-dot", up), None);
    }

    /// Talks to Twitch, with no account: subscribes to a live channel's
    /// public stream topic and waits for one notification, to confirm the
    /// envelope the client expects is the one Twitch sends. Run by hand:
    /// `HERMES_TEST_CHANNEL_ID=<id> cargo test live_playback -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn live_playback_notification_has_the_expected_envelope() {
        let channel = std::env::var("HERMES_TEST_CHANNEL_ID").expect("HERMES_TEST_CHANNEL_ID");
        let topic = topic_playback(&channel);
        let mut socket = connect().await.expect("connect");
        let (subscribe, subscription_id) = subscribe_message(&topic);
        let deadline = tokio::time::Instant::now() + Duration::from_secs(100);
        let mut keepalives = 0;
        loop {
            let next = tokio::time::timeout_at(deadline, socket.next()).await;
            let Ok(Some(Ok(message))) = next else { panic!("no notification before the deadline") };
            let Message::Text(text) = message else { continue };
            match parse(&text).expect("an envelope the client can read") {
                Incoming::Welcome { keepalive_sec } => {
                    println!("welcome, keepalive {keepalive_sec} s");
                    socket.send(Message::text(subscribe.clone())).await.expect("send");
                }
                Incoming::Subscribed { subscription_id: id, ok } => {
                    println!("subscribed without signing in: {ok}");
                    assert_eq!(id, subscription_id);
                    assert!(ok, "Twitch refused the subscription");
                }
                Incoming::Notification { subscription_id: id, pubsub } => {
                    assert_eq!(id, subscription_id);
                    let inner: Value = serde_json::from_str(&pubsub).expect("pubsub is JSON");
                    println!("notification type: {}", inner["type"]);
                    println!("read as: {:?}", parse_pubsub(&topic, &pubsub));
                    assert!(inner["type"].is_string());
                    break;
                }
                Incoming::Keepalive => keepalives += 1,
                other => println!("other: {other:?}"),
            }
        }
        println!("keepalives seen: {keepalives}");
        let _ = socket.close(None).await;
    }

    #[test]
    fn subscribe_message_carries_the_topic_and_its_id() {
        let (message, id) = subscribe_message("community-points-user-v1.42");
        let v: Value = serde_json::from_str(&message).unwrap();
        assert_eq!(v["type"], "subscribe");
        assert_eq!(v["subscribe"]["id"], id.as_str());
        assert_eq!(v["subscribe"]["pubsub"]["topic"], "community-points-user-v1.42");
    }

    #[test]
    fn timestamps_match_the_web_client_format() {
        let at = UNIX_EPOCH + Duration::from_millis(1_759_476_989_045);
        assert_eq!(timestamp(at), "2025-10-03T07:36:29.045Z");
        assert_eq!(timestamp(UNIX_EPOCH), "1970-01-01T00:00:00.000Z");
        // A leap day.
        let leap = UNIX_EPOCH + Duration::from_secs(1_709_164_800);
        assert_eq!(timestamp(leap), "2024-02-29T00:00:00.000Z");
    }
}
