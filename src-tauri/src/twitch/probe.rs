//! Read-only check that Twitch still behaves the way the client assumes.
//! Debug builds only: `cargo run -- --probe [channel ...]`.
//!
//! It signs in (device code), looks things up and listens. It never claims a
//! bonus and never reports watch time. It prints no token.

use std::time::{Duration, Instant};

use reqwest::Client;

use super::{auth, gql::Gql, hermes, random_id, TwitchError};

const DEFAULT_CHANNELS: &[&str] =
    &["CohhCarnage", "Northernlion", "Lirik", "itsHafu", "DansGaming", "AdmiralBahroo"];

pub async fn run(channels: Vec<String>) {
    let channels: Vec<String> = if channels.is_empty() {
        DEFAULT_CHANNELS.iter().map(|c| c.to_string()).collect()
    } else {
        channels
    };
    let http = Client::builder().timeout(Duration::from_secs(20)).build().expect("http client");

    println!("== 1. Sign-in");
    let (token, identity) = match sign_in(&http).await {
        Ok(pair) => pair,
        Err(e) => return println!("FAIL  sign-in: {e}"),
    };
    println!(
        "ok    signed in as {} (user id {}), token {}",
        identity.login,
        identity.user_id,
        match identity.expires_in {
            0 => "does not expire".to_string(),
            s => format!("expires in {} h {} min", s / 3600, s % 3600 / 60),
        }
    );
    println!("      refresh token issued: {}", token.refresh_token.is_some());

    println!("\n== 2. Channels (GetIDFromLogin, StreamInfo, ChannelPointsContext)");
    let gql = Gql::new(http.clone(), token.access_token.clone(), random_id(32));
    let mut first_live = None;
    let mut sample_context = None;
    for name in &channels {
        let login = name.to_lowercase();
        let id = gql.channel_id(&login).await;
        let info = gql.channel_info(&login).await;
        let points = gql.raw_points_context(&login).await;
        match (&id, &info) {
            (Ok(Some(id)), Ok(Some(info))) => {
                let live = info.stream.is_some();
                if live && first_live.is_none() {
                    first_live = Some(login.clone());
                }
                print!("ok    {:<16} id {:<10} {:<8}", info.display_name, id, if live { "live" } else { "offline" });
                if info.id != *id {
                    print!(" (ids disagree: {})", info.id);
                }
            }
            (Ok(None), _) | (_, Ok(None)) => print!("FAIL  {name:<16} not found"),
            (Err(e), _) | (_, Err(e)) => print!("FAIL  {name:<16} {e}"),
        }
        match &points {
            Ok(v) => {
                let p = &v["data"]["community"]["channel"]["self"]["communityPoints"];
                match p["balance"].as_u64() {
                    Some(balance) => println!(
                        " balance {balance:>8}  bonus waiting: {}",
                        if p["availableClaim"]["id"].is_string() { "yes (not claimed)" } else { "no" }
                    ),
                    None => println!(" balance: not in response"),
                }
                if sample_context.is_none() && p.is_object() {
                    sample_context = Some(v["data"]["community"]["channel"].clone());
                }
            }
            Err(e) => println!(" points: FAIL {e}"),
        }
    }

    println!("\n== 3. Watch prerequisites (looked up, not used)");
    match &first_live {
        None => println!("skip  none of the channels is live right now"),
        Some(login) => {
            match gql.spade_url(login).await {
                Ok(url) => println!("ok    spade URL for {login}: {}", host_of(&url)),
                Err(e) => println!("FAIL  spade URL for {login}: {e}"),
            }
            match gql.playback_token(login).await {
                Ok(_) => println!("ok    playback token for {login} issued"),
                Err(e) => println!("FAIL  playback token for {login}: {e}"),
            }
        }
    }

    println!("\n== 4. Hermes (connect, authenticate, subscribe to own points topic)");
    match hermes::handshake(&token.access_token, &identity.user_id).await {
        Err(e) => println!("FAIL  {e}"),
        Ok(h) => {
            let flag = |ok: bool| if ok { "ok   " } else { "FAIL " };
            println!("{} welcome (keepalive {:?} s)", flag(h.keepalive_sec.is_some()), h.keepalive_sec);
            match &h.authenticated {
                Some(Ok(())) => println!("ok    authenticated"),
                Some(Err(e)) => println!("FAIL  authenticate: {e}"),
                None => println!("FAIL  no authenticate response"),
            }
            match h.subscribed {
                Some(ok) => println!("{} subscribed", flag(ok)),
                None => println!("FAIL  no subscribe response"),
            }
        }
    }

    println!("\n== 5. Field names Twitch returns for a channel's points (for the streak question)");
    match sample_context {
        Some(channel) => {
            println!("channel.self keys:            {}", keys(&channel["self"]));
            println!("channel.self.communityPoints: {}", keys(&channel["self"]["communityPoints"]));
            println!("channel.communityPointsSettings: {}", keys(&channel["communityPointsSettings"]));
        }
        None => println!("skip  no points context came back"),
    }
}

/// `cargo run -- --probe-follows`: fetch the followed channels with the stored
/// sign-in and say how many came back and whether they look right. Read-only.
/// Prints no names, ids or token, so the output is safe to paste anywhere.
pub async fn follows() {
    let http = Client::builder().timeout(Duration::from_secs(20)).build().expect("http client");
    let Some(token) = auth::load_token() else {
        return println!("FAIL  no stored sign-in; sign in through the app first");
    };
    match auth::validate(&http, &token).await {
        Ok(Some(_)) => println!("ok    stored sign-in is still valid"),
        Ok(None) => return println!("FAIL  Twitch rejected the stored sign-in"),
        Err(e) => return println!("FAIL  sign-in check: {e}"),
    }

    let gql = Gql::new(http, token.access_token.clone(), random_id(32));
    let started = Instant::now();
    let follows = match gql.followed().await {
        Ok(follows) => follows,
        Err(e) => return println!("FAIL  followed channels: {e}"),
    };
    let well_formed = |login: &str| {
        !login.is_empty() && login.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    };
    let mut logins: Vec<&str> = follows.iter().map(|f| f.login.as_str()).collect();
    logins.sort_unstable();
    logins.dedup();
    println!("ok    {} followed channels in {:.1} s", follows.len(), started.elapsed().as_secs_f32());
    println!("      requests made:                {}", follows.len() / 100 + 1);
    println!("      repeated logins:              {}", follows.len() - logins.len());
    println!("      logins not lowercase a-z0-9_: {}", logins.iter().filter(|l| !well_formed(l)).count());
    println!("      logins shorter than 4:        {}", logins.iter().filter(|l| l.len() < 4).count());
    println!(
        "      names in another script:      {}",
        follows.iter().filter(|f| f.display_name.to_lowercase() != f.login).count()
    );
    println!("      empty names:                  {}", follows.iter().filter(|f| f.display_name.is_empty()).count());
}

/// Use the stored token if Twitch still accepts it; otherwise run the device flow.
async fn sign_in(http: &Client) -> Result<(auth::Token, auth::Identity), TwitchError> {
    if let Some(token) = auth::load_token() {
        if let Some(identity) = auth::validate(http, &token).await? {
            println!("ok    stored token is still valid");
            return Ok((token, identity));
        }
        println!("      stored token was rejected; signing in again");
        auth::delete_token();
    }

    let code = auth::request_device_code(http).await?;
    println!("ACTION NEEDED");
    println!("      Open {} in your browser", code.verification_uri);
    println!("      and enter this code if asked: {}", code.user_code);
    println!("      Waiting up to {} minutes...", code.expires_in / 60);

    let started = Instant::now();
    let token = loop {
        tokio::time::sleep(Duration::from_secs(code.interval.max(1))).await;
        if started.elapsed().as_secs() >= code.expires_in {
            return Err(TwitchError::Unexpected("the sign-in code expired before it was approved".into()));
        }
        if let auth::Poll::Granted(token) = auth::poll_token(http, &code.device_code).await? {
            break token;
        }
    };
    match auth::save_token(&token) {
        Ok(()) => println!("ok    token saved to Windows Credential Manager"),
        Err(e) => println!("WARN  could not save the token ({e}); you will be asked again next time"),
    }
    let identity = auth::validate(http, &token)
        .await?
        .ok_or(TwitchError::Unauthorized)?;
    Ok((token, identity))
}

fn host_of(url: &str) -> &str {
    url.split('/').nth(2).unwrap_or("(unparseable)")
}

fn keys(v: &serde_json::Value) -> String {
    match v.as_object() {
        Some(map) => map.keys().cloned().collect::<Vec<_>>().join(", "),
        None => "(absent)".to_string(),
    }
}
