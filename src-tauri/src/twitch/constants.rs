//! Every Twitch-specific value in one place. Copied from the reference miner
//! (rdavydov/Twitch-Channel-Points-Miner-v2 `constants.py`, and PR 728 for
//! Hermes) on 2026-11-17. When something stops working, compare against the
//! reference first: the persisted-query hashes are the usual culprit.

/// Twitch's TV app. Its device-code sign-in needs no password and no browser
/// automation.
pub const CLIENT_ID: &str = "ue6666qo983tsx6so1t0vnawi233wa";
/// Twitch's web app. Hermes expects this one in the connection URL.
pub const CLIENT_ID_WEB: &str = "kimne78kx3ncx6brgo4mv6wki5h1ko";

pub const DEVICE_URL: &str = "https://id.twitch.tv/oauth2/device";
pub const TOKEN_URL: &str = "https://id.twitch.tv/oauth2/token";
pub const VALIDATE_URL: &str = "https://id.twitch.tv/oauth2/validate";
pub const GQL_URL: &str = "https://gql.twitch.tv/gql";
pub const SITE_URL: &str = "https://www.twitch.tv";
pub const HERMES_URL: &str = "wss://hermes.twitch.tv/v1";

pub const DEVICE_GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";
pub const SCOPES: &str =
    "channel_read chat:read user_blocks_edit user_blocks_read user_follows_edit user_read";

pub const USER_AGENT_TV: &str = "Mozilla/5.0 (Linux; Android 7.1; Smart Box C1) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/108.0.0.0 Safari/537.36";
pub const USER_AGENT_WEB: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/108.0.0.0 Safari/537.36";

/// A GraphQL operation Twitch has stored server-side, addressed by hash.
pub struct Operation {
    pub name: &'static str,
    pub hash: &'static str,
}

pub const GET_ID_FROM_LOGIN: Operation = Operation {
    name: "GetIDFromLogin",
    hash: "94e82a7b1e3c21e186daa73ee2afc4b8f23bade1fbbff6fe8ac133f50a2f58ca",
};
pub const STREAM_INFO: Operation = Operation {
    name: "VideoPlayerStreamInfoOverlayChannel",
    hash: "198492e0857f6aedead9665c81c5a06d67b25b58034649687124083ff288597d",
};
pub const CHANNEL_POINTS_CONTEXT: Operation = Operation {
    name: "ChannelPointsContext",
    hash: "7fe050e3761eb2cf258d70ee1a21cbd76fa8cf3d7e7b12fc437e7029d446b5e3",
};
pub const CLAIM_COMMUNITY_POINTS: Operation = Operation {
    name: "ClaimCommunityPoints",
    hash: "46aaeebe02c99afdf4fc97c7c0cba964124bf6b0af229395f1f6d1feed05b3d0",
};
pub const CHANNEL_FOLLOWS: Operation = Operation {
    name: "ChannelFollows",
    hash: "eecf815273d3d949e5cf0085cc5084cd8a1b5b7b6f7990cf43cb0beadf546907",
};
pub const PLAYBACK_ACCESS_TOKEN: Operation = Operation {
    name: "PlaybackAccessToken",
    hash: "ed230aa1e33e07eebb8928504583da78a5173989fadfb1ac94be06a04f3cdbe9",
};

pub const TOPIC_USER_POINTS: &str = "community-points-user-v1";
pub const TOPIC_PLAYBACK: &str = "video-playback-by-id";

/// Hermes topic carrying the signed-in user's points events.
pub fn topic_user_points(user_id: &str) -> String {
    format!("{TOPIC_USER_POINTS}.{user_id}")
}

/// Hermes topic carrying a channel's stream up / down events.
pub fn topic_playback(channel_id: &str) -> String {
    format!("{TOPIC_PLAYBACK}.{channel_id}")
}
