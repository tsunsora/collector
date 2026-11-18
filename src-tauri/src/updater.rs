//! Updates for the portable app. Once a day while it is open, the app reads a
//! small feed file from the latest GitHub release, and if that names a newer
//! version it downloads the new executable, checks it, and swaps itself for
//! it when the app next quits (or at once, on "Restart and update").
//!
//! Two checks stand between a download and it running:
//!
//! - its SHA-256 must match the feed, which catches a broken download;
//! - its signature must verify against the public key built into this app,
//!   which is what proves it came from whoever holds the private key. The
//!   feed and the file both come from the same place, so the hash alone
//!   would prove nothing about who made them.
//!
//! The feed address and the public key are in `src-tauri/update.json`. With
//! either left empty the app says updates are not set up and never looks.
//! See RELEASING.md.

use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::Duration,
};

use base64::Engine as _;
use reqwest::Client;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager};

use crate::{
    collector::now_ms,
    demo,
    model::{UpdateInfo, UpdateState},
    update as push,
};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
const FEED_NAME: &str = "collector-update.json";
/// Looked for once a day while the app is open, and shortly after it starts.
const CHECK_EVERY: Duration = Duration::from_secs(24 * 60 * 60);
const FIRST_CHECK_AFTER: Duration = Duration::from_secs(20);
/// Far above the real size; a download past this is not our executable.
const MAX_BYTES: u64 = 200 * 1024 * 1024;

/// Where updates come from and how they are checked.
#[derive(Clone, Debug, PartialEq)]
pub struct Source {
    pub feed_url: String,
    /// The minisign public key, base64, as `tauri signer generate` prints it.
    pub public_key: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Embedded {
    releases_repo: String,
    public_key: String,
}

/// The source built into this app, or `None` if updates are not set up.
pub fn embedded_source(json: &str) -> Option<Source> {
    let config: Embedded = serde_json::from_str(json).ok()?;
    let (repo, key) = (config.releases_repo.trim(), config.public_key.trim());
    let (owner, name) = repo.split_once('/')?;
    let plain = |part: &str| {
        !part.is_empty() && part.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };
    if key.is_empty() || !plain(owner) || !plain(name) {
        return None;
    }
    Some(Source {
        feed_url: format!("https://github.com/{owner}/{name}/releases/latest/download/{FEED_NAME}"),
        public_key: key.to_string(),
    })
}

fn source() -> Option<Source> {
    // Demo mode can point at a test feed, to exercise the whole path
    // without a real release.
    if demo::enabled() {
        let feed_url = std::env::var("COLLECTOR_UPDATE_FEED").ok()?;
        let public_key = std::env::var("COLLECTOR_UPDATE_KEY").ok()?;
        return Some(Source { feed_url, public_key });
    }
    // A development build must never replace itself with a release.
    if cfg!(debug_assertions) {
        return None;
    }
    embedded_source(include_str!("../update.json"))
}

/// The feed attached to each release, in the shape Lock On's feed has.
#[derive(Clone, Debug, Deserialize)]
struct Feed {
    version: String,
    notes: Option<String>,
    windows: Option<FeedFile>,
}

#[derive(Clone, Debug, Deserialize)]
struct FeedFile {
    /// A file name, found next to the feed.
    file: String,
    sha256: String,
    size: Option<u64>,
    /// Minisign signature of the file, base64, as `tauri signer sign` writes it.
    signature: String,
}

/// Whether `candidate` is a later version than `current`. Versions compare as
/// numbers, part by part; a leading "v" and anything after a "-" are ignored.
pub fn newer(candidate: &str, current: &str) -> bool {
    let parts = |v: &str| -> Vec<u64> {
        let v = v.trim().trim_start_matches(['v', 'V']);
        let v = v.split('-').next().unwrap_or("");
        v.split('.').map(|p| p.parse().unwrap_or(0)).collect()
    };
    let (a, b) = (parts(candidate), parts(current));
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (a.get(i).copied().unwrap_or(0), b.get(i).copied().unwrap_or(0));
        if x != y {
            return x > y;
        }
    }
    false
}

/// The address of a file named in the feed: next to the feed, and only ever
/// a plain `.exe` name, so a feed cannot send the app anywhere else.
pub fn file_url(feed_url: &str, file: &str) -> Option<String> {
    let plain = !file.is_empty()
        && file.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        && !file.contains("..")
        && file.to_ascii_lowercase().ends_with(".exe");
    let (base, _) = feed_url.rsplit_once('/')?;
    plain.then(|| format!("{base}/{file}"))
}

/// Both checks a download must pass before it is trusted.
pub fn verify(bytes: &[u8], sha256_hex: &str, signature: &str, public_key: &str) -> Result<(), String> {
    let digest: String = Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect();
    if !digest.eq_ignore_ascii_case(sha256_hex.trim()) {
        return Err("its SHA-256 does not match the feed".into());
    }
    let text = |b64: &str, what: &str| -> Result<String, String> {
        let raw = base64::engine::general_purpose::STANDARD
            .decode(b64.trim())
            .map_err(|_| format!("the {what} is not valid base64"))?;
        String::from_utf8(raw).map_err(|_| format!("the {what} is not text"))
    };
    let key = minisign_verify::PublicKey::decode(&text(public_key, "public key")?)
        .map_err(|e| format!("the public key could not be read: {e}"))?;
    let signature = minisign_verify::Signature::decode(&text(signature, "signature")?)
        .map_err(|e| format!("the signature could not be read: {e}"))?;
    key.verify(bytes, &signature, true).map_err(|_| "its signature does not match this app's key".to_string())
}

struct Staged {
    path: PathBuf,
    version: String,
    file: FeedFile,
}

pub struct Updater {
    http: Client,
    source: Option<Source>,
    busy: AtomicBool,
    staged: Mutex<Option<Staged>>,
}

impl Updater {
    pub fn new() -> Self {
        Self {
            http: Client::builder()
                .user_agent(format!("Collector/{VERSION}"))
                .timeout(Duration::from_secs(120))
                .build()
                .expect("http client"),
            source: source(),
            busy: AtomicBool::new(false),
            staged: Mutex::new(None),
        }
    }

    pub fn enabled(&self) -> bool {
        self.source.is_some()
    }

    /// What the UI should show before anything has been checked.
    pub fn initial(&self) -> UpdateInfo {
        UpdateInfo::in_state(if self.enabled() { UpdateState::Idle } else { UpdateState::Off })
    }
}

fn show(app: &AppHandle, info: UpdateInfo) {
    push(app, false, |c| c.update = info);
}

fn staging_dir(app: &AppHandle) -> Option<PathBuf> {
    Some(crate::data_dir(app)?.join("update"))
}

/// Remove anything a previous run left half-done.
pub fn clear_staging(app: &AppHandle) {
    if let Some(dir) = staging_dir(app) {
        let _ = std::fs::remove_dir_all(dir);
    }
}

/// Check now and then once a day, for as long as the app runs.
pub async fn keep_checking(app: AppHandle) {
    if !app.state::<Updater>().enabled() {
        return;
    }
    tokio::time::sleep(FIRST_CHECK_AFTER).await;
    loop {
        let download = app.state::<crate::AppState>().collector.lock().unwrap().auto_update;
        check(app.clone(), download).await;
        tokio::time::sleep(CHECK_EVERY).await;
    }
}

/// Read the feed. If it names a newer version, either say so (`download`
/// false) or fetch and check it, ready to install.
pub async fn check(app: AppHandle, download: bool) {
    let updater = app.state::<Updater>();
    let Some(source) = updater.source.clone() else { return };
    if updater.busy.swap(true, Ordering::SeqCst) {
        return;
    }
    let result = run_check(&app, &source, download).await;
    updater.busy.store(false, Ordering::SeqCst);
    if let Err(why) = result {
        crate::log!("update: {why}");
        // Keep the version, if one was found, so the UI can still name it.
        let version = app.state::<crate::AppState>().collector.lock().unwrap().update.version.clone();
        show(&app, UpdateInfo {
            version,
            checked_at: Some(now_ms()),
            message: Some(why),
            ..UpdateInfo::in_state(UpdateState::Failed)
        });
    }
}

async fn run_check(app: &AppHandle, source: &Source, download: bool) -> Result<(), String> {
    let updater = app.state::<Updater>();
    let earlier = app.state::<crate::AppState>().collector.lock().unwrap().update.clone();
    show(app, UpdateInfo { version: earlier.version.clone(), ..UpdateInfo::in_state(UpdateState::Checking) });

    let unreachable = "Couldn’t reach the update server. Check your connection and try again.";
    let response = updater.http.get(&source.feed_url).send().await.map_err(|_| unreachable.to_string())?;
    if !response.status().is_success() {
        return Err(format!("The update feed answered with HTTP {}.", response.status().as_u16()));
    }
    let feed: Feed = response.json().await.map_err(|_| "The update feed could not be read.".to_string())?;
    let checked_at = Some(now_ms());

    let file = match feed.windows.clone() {
        Some(file) if newer(&feed.version, VERSION) => file,
        _ => {
            show(app, UpdateInfo { checked_at, ..UpdateInfo::in_state(UpdateState::Idle) });
            return Ok(());
        }
    };
    let found = UpdateInfo {
        version: Some(feed.version.clone()),
        notes: feed.notes.clone(),
        checked_at,
        ..UpdateInfo::in_state(UpdateState::Available)
    };
    // Already fetched and checked on an earlier pass.
    if updater.staged.lock().unwrap().as_ref().is_some_and(|s| s.version == feed.version) {
        show(app, UpdateInfo { state: UpdateState::Ready, ..found });
        return Ok(());
    }
    if !download {
        show(app, found);
        return Ok(());
    }

    show(app, UpdateInfo { state: UpdateState::Downloading, ..found.clone() });
    let url = file_url(&source.feed_url, &file.file).ok_or("The update feed names a file this app won’t fetch.")?;
    if file.size.is_some_and(|size| size > MAX_BYTES) {
        return Err("The update is larger than this app expects.".into());
    }
    let failed = "The update didn’t download correctly. Try again.";
    let response = updater.http.get(&url).send().await.map_err(|_| failed.to_string())?;
    if !response.status().is_success() {
        return Err(format!("The update download answered with HTTP {}.", response.status().as_u16()));
    }
    let bytes = response.bytes().await.map_err(|_| failed.to_string())?;
    if bytes.len() as u64 > MAX_BYTES || file.size.is_some_and(|size| size != bytes.len() as u64) {
        return Err(failed.into());
    }
    verify(&bytes, &file.sha256, &file.signature, &source.public_key)
        .map_err(|why| format!("The update was not installed: {why}."))?;

    let dir = staging_dir(app).ok_or("There is nowhere to keep the update.")?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("The update could not be saved: {e}."))?;
    let path = dir.join(&file.file);
    std::fs::write(&path, &bytes).map_err(|e| format!("The update could not be saved: {e}."))?;
    crate::log!("update: {} downloaded and verified", feed.version);
    *updater.staged.lock().unwrap() = Some(Staged { path, version: feed.version.clone(), file });
    show(app, UpdateInfo { state: UpdateState::Ready, ..found });
    Ok(())
}

/// Swap the running executable for the staged one, if there is one. The app
/// keeps running the old code until it exits. Called on Quit, and by
/// "Restart and update". Returns whether an update was put in place.
pub fn install_staged(app: &AppHandle) -> Result<bool, String> {
    let updater = app.state::<Updater>();
    let Some(staged) = updater.staged.lock().unwrap().take() else {
        return Ok(false);
    };
    let Some(source) = &updater.source else { return Ok(false) };
    // Check again what is actually on disk: it has sat there since the download.
    let bytes = std::fs::read(&staged.path).map_err(|e| format!("The update could not be read: {e}."))?;
    verify(&bytes, &staged.file.sha256, &staged.file.signature, &source.public_key)
        .map_err(|why| format!("The update was not installed: {why}."))?;
    self_replace::self_replace(&staged.path).map_err(|e| format!("The update could not be put in place: {e}."))?;
    let _ = std::fs::remove_file(&staged.path);
    crate::log!("update: {} installed over {VERSION}", staged.version);
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_as_numbers() {
        assert!(newer("1.2.10", "1.2.9"));
        assert!(newer("v2.0.0", "1.9.9"));
        assert!(newer("0.5.0", "0.4.1"));
        assert!(newer("26.11.2", "26.11.1"));
        assert!(!newer("1.0", "1.0.0"));
        assert!(!newer("1.1.0-beta", "1.1.0"));
        assert!(!newer("1.0.0", "1.0.1"));
        assert!(!newer("nonsense", "0.0.1"));
    }

    #[test]
    fn the_feed_address_comes_from_the_releases_repo_and_needs_a_key() {
        let source = embedded_source(r#"{ "releasesRepo": "someone/collector", "publicKey": "abc" }"#);
        assert_eq!(
            source.unwrap().feed_url,
            "https://github.com/someone/collector/releases/latest/download/collector-update.json"
        );
        // Not set up: no repo, no key, or something that is not a repo name.
        assert_eq!(embedded_source(r#"{ "releasesRepo": "", "publicKey": "" }"#), None);
        assert_eq!(embedded_source(r#"{ "releasesRepo": "someone/repo", "publicKey": " " }"#), None);
        assert_eq!(embedded_source(r#"{ "releasesRepo": "https://evil.example/x", "publicKey": "abc" }"#), None);
        assert_eq!(embedded_source(r#"{ "releasesRepo": "one/two/three", "publicKey": "abc" }"#), None);
    }

    #[test]
    fn a_feed_can_only_name_an_exe_next_to_itself() {
        let feed = "https://github.com/o/r/releases/latest/download/collector-update.json";
        assert_eq!(
            file_url(feed, "Collector-0.5.0-windows.exe").as_deref(),
            Some("https://github.com/o/r/releases/latest/download/Collector-0.5.0-windows.exe")
        );
        assert_eq!(file_url(feed, "https://evil.example/a.exe"), None);
        assert_eq!(file_url(feed, "../a.exe"), None);
        assert_eq!(file_url(feed, "sub/a.exe"), None);
        assert_eq!(file_url(feed, "a.msi"), None);
        assert_eq!(file_url(feed, ""), None);
    }

    /// Run by hand after making or changing the signing key:
    /// `UPDATE_TEST_FILE=<file> cargo test signed_with_the_real_key -- --ignored`
    /// where `<file>.sig` was written by `tauri signer sign`. It shows that the
    /// public key built into the app verifies what the private key signs.
    #[test]
    #[ignore]
    fn a_file_signed_with_the_real_key_verifies_against_the_built_in_one() {
        let path = std::env::var("UPDATE_TEST_FILE").expect("UPDATE_TEST_FILE");
        let bytes = std::fs::read(&path).expect("the file");
        let signature = std::fs::read_to_string(format!("{path}.sig")).expect("its .sig");
        let source = embedded_source(include_str!("../update.json")).expect("updates are set up in update.json");
        let hash = |b: &[u8]| -> String { Sha256::digest(b).iter().map(|x| format!("{x:02x}")).collect() };

        verify(&bytes, &hash(&bytes), &signature, &source.public_key).expect("the signature verifies");

        let mut changed = bytes.clone();
        changed[0] ^= 1;
        assert_eq!(
            verify(&changed, &hash(&changed), &signature, &source.public_key).unwrap_err(),
            "its signature does not match this app's key"
        );
    }

    #[test]
    fn a_download_with_the_wrong_hash_or_an_unreadable_signature_is_refused() {
        let bytes = b"not really an executable";
        let good_hash: String = Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(verify(bytes, "00", "x", "y").unwrap_err(), "its SHA-256 does not match the feed");
        assert!(verify(bytes, &good_hash, "@@not base64@@", "eA==").is_err());
        assert!(verify(bytes, &good_hash, "eA==", "eA==").is_err(), "a key that is not a key");
    }
}
