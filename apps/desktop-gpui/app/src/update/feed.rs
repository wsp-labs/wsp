// SPDX-License-Identifier: AGPL-3.0-only
//! A release's file, fetched as today's app fetches it (`get-bundle.ts`): the page hands over a version and no URL,
//! the answer's and the download's URLs are built off the repo, the sha256 and size GitHub publishes for the asset
//! are read, and the bytes are kept only where they match. A download that goes 30 s without a byte is given up.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use sha2::{Digest, Sha256};

/// `REPO` in `bundles.mjs`.
pub const REPO: &str = "https://github.com/wsp-labs/wsp";
/// `RELEASES` in `bundles.mjs`.
pub const RELEASES: &str = "https://github.com/wsp-labs/wsp/releases";
const RELEASE_API: &str = "https://api.github.com";
/// `RELEASE_API_ENV`: moves both the answer and the download, so a test serves the two from one place.
pub const RELEASE_API_ENV: &str = "WSP_RELEASE_API";
/// `BUNDLE_STALL_MS`.
pub const STALL: Duration = Duration::from_secs(30);
/// `RELEASE_CONNECT_MS` and `RELEASE_ANSWER_MS`.
const CONNECT: Duration = Duration::from_secs(30);
const ANSWER: Duration = Duration::from_secs(30);
/// `RELEASE_TIMEOUT_MS`: every connect the release answer's ask tries, then the answer, its body included.
const ANSWER_WHOLE: Duration = Duration::from_secs(3 * 30 + 30 + 10);
/// `RELEASE_BODY_MAX_BYTES`.
const BODY_MAX: u64 = 1024 * 1024;
const CHUNK: usize = 64 * 1024;

/// `BUNDLE_WORDS` in `get-bundle.ts`.
pub mod words {
    pub const NOT_A_VERSION: &str = "not a release version";
    pub fn not_newer(version: &str) -> String {
        format!("{version} is not a release above this app")
    }
    pub fn no_bundle(platform: &str) -> String {
        format!("no wsp bundle is built for {platform}")
    }
    pub fn no_digest(asset: &str) -> String {
        format!("the release publishes no sha256 and size for {asset}")
    }
    pub fn mismatch(asset: &str) -> String {
        format!("{asset} did not match the release's sha256 and was deleted")
    }
    pub fn too_big(asset: &str) -> String {
        format!("{asset} ran past the size the release publishes and was deleted")
    }
    pub fn unsaved(asset: &str) -> String {
        format!("{asset} could not be saved")
    }
    pub fn changed(asset: &str) -> String {
        format!("{asset} changed after it was checked and was not opened")
    }
    pub const GONE: &str = "the checked update is gone; get it again";
    pub const NOTHING_KEPT: &str = "no verified download to open";
    pub fn answered(status: u16) -> String {
        format!("GitHub answered {status}")
    }
    pub fn stalled(secs: u64) -> String {
        format!("no bytes for {secs} s")
    }
}

/// What each download on the release page is called for one version (`bundleNames` in `bundles.mjs`).
pub struct Names {
    pub mac: String,
    pub mac_zip: String,
    pub app_image: String,
}

pub fn bundle_names(version: &str) -> Names {
    Names { mac: format!("wsp-{version}-mac.dmg"), mac_zip: format!("wsp-{version}-mac.zip"), app_image: format!("wsp-{version}.AppImage") }
}

/// The version out of an ask, only where it is one the release workflow would tag (`RELEASE_TAG`).
pub fn asked_version(raw: &str) -> Option<&str> {
    static TAG: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let tag = TAG.get_or_init(|| regex::Regex::new(r"^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$").expect("the tag rule compiles"));
    tag.is_match(raw).then_some(raw)
}

/// A failed step: the sentence the person reads, and what caused it where the sentence alone would lose it.
#[derive(Debug, Clone, PartialEq)]
pub struct Said {
    pub error: String,
    pub cause: Option<String>,
}

impl Said {
    pub fn new(error: impl Into<String>) -> Said {
        Said { error: error.into(), cause: None }
    }
}

impl From<String> for Said {
    fn from(error: String) -> Said {
        Said::new(error)
    }
}

/// The hex sha256 and byte size the release publishes for one asset.
#[derive(Debug, Clone, PartialEq)]
pub struct Published {
    pub sum: String,
    pub size: u64,
}

/// A download kept: where it is and the sum it matched.
#[derive(Debug, Clone, PartialEq)]
pub struct Kept {
    pub file: PathBuf,
    pub sum: String,
}

/// Where the release answers from and how long a download may go silent.
#[derive(Clone)]
pub struct Feed {
    api: Option<String>,
    user_agent: String,
    pub stall: Duration,
}

impl Feed {
    /// GitHub, or the base `api` names, which only a test or a development build takes (`bundleShell` drops the
    /// variable for a packaged app).
    pub fn new(api: Option<String>, user_agent: String) -> Feed {
        Feed { api: api.filter(|a| !a.is_empty()).map(|a| a.trim_end_matches('/').to_owned()), user_agent, stall: STALL }
    }

    /// The release's own answer for one tag (`releaseTagUrl`).
    pub fn tag_url(&self, tag: &str) -> String {
        format!("{}/repos/wsp-labs/wsp/releases/tags/{tag}", self.api.as_deref().unwrap_or(RELEASE_API))
    }

    /// One asset of that release (`releaseAssetUrl`).
    pub fn asset_url(&self, tag: &str, asset: &str) -> String {
        match &self.api {
            Some(api) => format!("{api}/wsp-labs/wsp/releases/download/{tag}/{asset}"),
            None => format!("{REPO}/releases/download/{tag}/{asset}"),
        }
    }

    fn agent(&self, whole: Option<Duration>) -> ureq::Agent {
        ureq::Agent::config_builder()
            .timeout_global(whole)
            .timeout_connect(Some(CONNECT))
            .timeout_recv_response(Some(ANSWER))
            .http_status_as_error(false)
            .user_agent(&self.user_agent)
            .build()
            .into()
    }

    /// The sha256 and size GitHub publishes for this asset of the release, or nothing unless it lists both.
    pub fn published(&self, tag: &str, asset: &str) -> Result<Option<Published>, Said> {
        let mut res = self
            .agent(Some(ANSWER_WHOLE))
            .get(self.tag_url(tag))
            .header("accept", "application/vnd.github+json")
            .call()
            .map_err(failed_ask)?;
        let status = res.status().as_u16();
        if !(200..300).contains(&status) {
            return Err(Said::new(words::answered(status)));
        }
        let text = res.body_mut().with_config().limit(BODY_MAX).read_to_string().map_err(failed_ask)?;
        let answer: serde_json::Value = serde_json::from_str(&text).map_err(|e| Said::new(e.to_string()))?;
        let row = answer
            .get("assets")
            .and_then(|a| a.as_array())
            .and_then(|rows| rows.iter().find(|r| r.get("name").and_then(|n| n.as_str()) == Some(asset)));
        let Some(row) = row else { return Ok(None) };
        let sum = row
            .get("digest")
            .and_then(|d| d.as_str())
            .and_then(|d| d.strip_prefix("sha256:"))
            .filter(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
        Ok(sum.zip(row.get("size").and_then(|s| s.as_u64())).map(|(sum, size)| Published { sum: sum.to_owned(), size }))
    }

    /// Streams the download beside its final name, hashing as it writes and stopping past the published size; renamed
    /// into place on a match, deleted otherwise.
    pub fn download(&self, url: &str, file: &Path, want: &Published, asset: &str) -> Result<(), Said> {
        let part = part_of(file);
        let got = self.stream(url, &part, want, asset);
        match got {
            Ok(sum) if sum == want.sum => fs::rename(&part, file).map_err(|_| Said::new(words::unsaved(asset))),
            Ok(_) => {
                _ = fs::remove_file(&part);
                Err(Said::new(words::mismatch(asset)))
            }
            Err(said) => {
                _ = fs::remove_file(&part);
                Err(said)
            }
        }
    }

    fn stream(&self, url: &str, part: &Path, want: &Published, asset: &str) -> Result<String, Said> {
        let res = self.agent(None).get(url).call().map_err(failed_ask)?;
        let status = res.status().as_u16();
        if !(200..300).contains(&status) {
            return Err(Said::new(words::answered(status)));
        }
        _ = fs::remove_file(part);
        // Exclusive create refuses a link planted at the name between the removal and the open.
        let mut out = OpenOptions::new().write(true).create_new(true).open(part).map_err(|_| Said::new(words::unsaved(asset)))?;
        // The reads go on a thread of their own, so a connection that stops sending is given up on time; that thread
        // ends at its next read once nothing takes its chunks.
        let (chunks, read) = mpsc::sync_channel::<io::Result<Vec<u8>>>(4);
        let mut body = res.into_body().into_reader();
        std::thread::spawn(move || loop {
            let mut buf = vec![0u8; CHUNK];
            let sent = match body.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    buf.truncate(n);
                    chunks.send(Ok(buf))
                }
                Err(e) => chunks.send(Err(e)),
            };
            if sent.is_err() {
                break;
            }
        });
        let mut hash = Sha256::new();
        let mut bytes = 0u64;
        loop {
            match read.recv_timeout(self.stall) {
                Ok(Ok(chunk)) => {
                    bytes += chunk.len() as u64;
                    if bytes > want.size {
                        return Err(Said::new(words::too_big(asset)));
                    }
                    hash.update(&chunk);
                    out.write_all(&chunk).map_err(|_| Said::new(words::unsaved(asset)))?;
                }
                Ok(Err(e)) => return Err(Said { error: e.to_string(), cause: cause_of(&e) }),
                Err(mpsc::RecvTimeoutError::Timeout) => return Err(Said::new(words::stalled(self.stall.as_secs()))),
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        out.sync_all().map_err(|_| Said::new(words::unsaved(asset)))?;
        Ok(hex(&hash.finalize()))
    }

    /// This release's asset in `dir`, verified against the published sha256: a copy already there that matches is
    /// kept as it is (`getBundle`).
    pub fn get(&self, version: &str, dir: &Path, asset: &str) -> Result<Kept, Said> {
        let tag = format!("v{version}");
        let want = self.published(&tag, asset)?.ok_or_else(|| Said::new(words::no_digest(asset)))?;
        let file = dir.join(asset);
        if sum_of(&file).as_deref() == Some(want.sum.as_str()) {
            return Ok(Kept { file, sum: want.sum });
        }
        self.download(&self.asset_url(&tag, asset), &file, &want, asset)?;
        Ok(Kept { file, sum: want.sum })
    }
}

fn part_of(file: &Path) -> PathBuf {
    let mut name = file.file_name().unwrap_or_default().to_owned();
    name.push(".part");
    file.with_file_name(name)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The hex sha256 of a file, or nothing where it cannot be read.
pub fn sum_of(file: &Path) -> Option<String> {
    let mut f = File::open(file).ok()?;
    let mut hash = Sha256::new();
    io::copy(&mut f, &mut hash).ok()?;
    Some(hex(&hash.finalize()))
}

fn failed_ask(e: ureq::Error) -> Said {
    let cause = match &e {
        ureq::Error::Io(io) => cause_of(io),
        _ => None,
    };
    Said { error: e.to_string(), cause }
}

/// What a failed read was caused by, each link with its kind, where the sentence alone would keep none of it.
fn cause_of(e: &io::Error) -> Option<String> {
    let mut links = vec![format!("{:?}: {e}", e.kind())];
    let mut at = std::error::Error::source(e);
    while let Some(link) = at.filter(|_| links.len() < 5) {
        links.push(link.to_string());
        at = link.source();
    }
    Some(links.join("; "))
}

/// One update step's ending as the app log keeps it, a failure's cause beside the sentence the page shows
/// (`updateLogLine`).
pub fn log_line(step: &str, outcome: &Result<(), Said>) -> String {
    match outcome {
        Ok(()) => format!("update: {step} done"),
        Err(Said { error, cause: None }) => format!("update: {step} failed: {error}"),
        Err(Said { error, cause: Some(cause) }) => format!("update: {step} failed: {error} (cause: {cause})"),
    }
}
