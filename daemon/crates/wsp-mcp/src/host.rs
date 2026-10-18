// SPDX-License-Identifier: AGPL-3.0-only
//! The host the tools speak to: one socket kept across calls, dropped when it closes so the next call dials again,
//! and the aim read afresh at every dial, as packages/host/src/mcp.ts `dialer` keeps it. A host on the account that
//! holds no token of this computer's yet, or no longer takes the one it holds, admits it on its device key once, and
//! the token it answers is written into the record, as packages/host/src/verbs.ts `dialHost` does.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;

use crate::aim::{self, Aim, HostRecord, Pick};
use crate::client::{self, Admit, Client, Dial, Presents};
use crate::failure::Failure;
use crate::record::{self, fill};
use crate::release::ReleaseHost;
use crate::start;
use crate::{Args, Env};

/// Whether the cloud is on, read off the variable as the protocol's cloudFromEnv reads it: the greeting and the list
/// are the TypeScript server's for that state. Never in a public build, whose record carries no cloud-on words and
/// Boat as its one provider (build.rs), as the public host turns the cloud on nowhere.
pub fn cloud_on(env: &Env) -> bool {
    !cfg!(wsp_public) && env.get(&record::host().env.cloud).is_some_and(|value| value == "1")
}

pub struct Host {
    args: Args,
    /// The environment the server runs in, which the dial, the aim and the cloud flag read.
    process_env: Env,
    /// What a tool reads a value it sends by name from: the whole environment, and for a guest the pair its launch
    /// carries and the turn's token alone, as the TypeScript server's guest tools read them.
    env: Env,
    home: PathBuf,
    /// The folder the server runs in, where a thread named with no workspace finds its repo; none finds none.
    cwd: Option<PathBuf>,
    held: Mutex<Option<Arc<Client>>>,
    /// The socket a line that dials past the release check holds, kept apart so no other tool rides it to a host of
    /// another release.
    held_any_release: Mutex<Option<Arc<Client>>>,
    /// The device token a pairing code bought for the host here, which every later dial presents in place of the file
    /// beside the state, for as long as this lives.
    paired: std::sync::Mutex<Option<String>>,
}

impl Host {
    pub fn cloud(&self) -> bool {
        cloud_on(&self.process_env)
    }

    pub fn new(args: &Args, env: &Env, cwd: Option<PathBuf>) -> Host {
        let readable: Env = if args.guest {
            let names = [&record::host().env.url, &record::host().env.token, crate::tools::turn_token_env()];
            env.iter().filter(|(name, _)| names.contains(name)).map(|(k, v)| (k.clone(), v.clone())).collect()
        } else {
            env.clone()
        };
        Host {
            args: args.clone(),
            process_env: env.clone(),
            env: readable,
            home: aim::wsp_home(env),
            cwd,
            held: Mutex::new(None),
            held_any_release: Mutex::new(None),
            paired: std::sync::Mutex::new(None),
        }
    }

    /// The environment a tool reads a value it sends by name from.
    pub fn env(&self) -> &Env {
        &self.env
    }

    /// The socket the last call opened while it is still open; a fresh dial otherwise. Two calls at once share one
    /// dial, since the second waits on the first's.
    pub async fn client(&self) -> Result<Arc<Client>, Failure> {
        self.held(None, true).await
    }

    /// The same past the release check, for the restart that is the fix the release refusal names, as `wsp restart`
    /// dials with anyRelease.
    pub async fn client_any_release(&self) -> Result<Arc<Client>, Failure> {
        self.held(None, false).await
    }

    /// The host that came back after it stopped under a wait, within `window`: dialled until one answers, each dial
    /// handed what is left as its own deadline and none of them starting a host, as `hostAgain` in
    /// packages/host/src/verbs.ts dials. Past the window the last dial's refusal says what stands now.
    pub async fn back_within(&self, window: Duration) -> Result<Arc<Client>, Failure> {
        self.back(window, true).await
    }

    /// The same past the release check, for the restart's own wait on the host it restarted.
    pub async fn back_within_any_release(&self, window: Duration) -> Result<Arc<Client>, Failure> {
        self.back(window, false).await
    }

    async fn back(&self, window: Duration, judged: bool) -> Result<Arc<Client>, Failure> {
        let until = Instant::now() + window;
        loop {
            let left = until.saturating_duration_since(Instant::now()).max(Duration::from_millis(1));
            match self.held(Some(left), judged).await {
                Ok(client) => return Ok(client),
                Err(refused) if Instant::now() >= until => return Err(refused),
                Err(_) => {}
            }
            let poll = Duration::from_millis(record::host().poll_ms);
            tokio::time::sleep(poll.min(until.saturating_duration_since(Instant::now()))).await;
        }
    }

    pub fn args(&self) -> &Args {
        &self.args
    }

    pub fn cwd(&self) -> Option<&Path> {
        self.cwd.as_deref()
    }

    async fn held(&self, again: Option<Duration>, judged: bool) -> Result<Arc<Client>, Failure> {
        let mut held = if judged { self.held.lock().await } else { self.held_any_release.lock().await };
        if let Some(open) = held.as_ref().filter(|c| !c.is_closed()) {
            return Ok(open.clone());
        }
        // Boxed: a socket is dialled once, and inline the dial would ride in every request's future.
        let dialled = Arc::new(Box::pin(self.dial(again, judged)).await?);
        *held = Some(dialled.clone());
        Ok(dialled)
    }

    /// `again` is a dial after the host stopped: it starts nothing and is bounded by what the wait has left.
    async fn dial(&self, again: Option<Duration>, judged: bool) -> Result<Client, Failure> {
        let pick = Pick { host: self.args.host.as_deref(), env: &self.process_env, home: self.home.clone() };
        let state = &self.args.state;
        let aim = aim::aimed(state, &pick)?;
        let window = again.unwrap_or_else(|| Duration::from_millis(aim::window_ms(&aim)));
        match &aim {
            Aim::Here => {
                if again.is_none() && aim::serving(state).is_none() && !self.args.wsp.is_empty() {
                    start::started(state, &self.args.wsp, &self.process_env, &mut |line| eprintln!("{line}")).await?;
                }
                let held = self.paired.lock().unwrap().clone();
                let (url, token, at) = match held {
                    Some(token) => aim::here_address(state).map(|(url, at)| (url, token, at))?,
                    None => aim::here_door(state)?,
                };
                let release = ReleaseHost::Here { state: state.to_string_lossy().into_owned() };
                let to = Dial { url: &url, at: &at, window, pinned: None, alias: None, release: judged.then_some(&release) };
                Ok(client::dial(&to, Presents::Token(&token)).await.map_err(|r| r.failure)?.0)
            }
            Aim::Url { url, token, host_key } => {
                let Some(token) = token else {
                    return Err(Failure::usage(fill(&record::words().address_not_paired, &[("url", url)])));
                };
                let release = ReleaseHost::At { at: url.clone(), here: on_this_computer(url) };
                let to = Dial {
                    url: &ws_url(url)?,
                    at: url,
                    window,
                    pinned: host_key.as_deref(),
                    alias: None,
                    release: judged.then_some(&release),
                };
                Ok(client::dial(&to, Presents::Token(token)).await.map_err(|r| r.failure)?.0)
            }
            Aim::Alias { alias, record } => self.dial_account(alias, record, window, judged).await,
        }
    }

    /// Spends a pairing code on the host this aims at, over a socket that presents no token, as a browser does
    /// (`redeemPairingCode`). The token it buys is written into an account host's record, and held for the host here,
    /// which keeps no record of a device. The pairing socket goes: the next call dials with the token, release checked.
    /// Nothing is started.
    pub async fn redeem(&self, code: &str, name: &str) -> Result<(), Failure> {
        let pick = Pick { host: self.args.host.as_deref(), env: &self.process_env, home: self.home.clone() };
        let aim = aim::aimed(&self.args.state, &pick)?;
        let window = Duration::from_millis(aim::window_ms(&aim));
        let presents = Presents::Redeem { code, name };
        let (_, paired) = match &aim {
            Aim::Here => {
                let (url, at) = aim::here_address(&self.args.state)?;
                let to = Dial { url: &url, at: &at, window, pinned: None, alias: None, release: None };
                client::dial(&to, presents).await.map_err(|r| r.failure)?
            }
            Aim::Alias { alias, record } => {
                let url = ws_url(&record.url)?;
                let to = Dial { url: &url, at: &record.url, window, pinned: record.host_key.as_deref(), alias: Some(alias), release: None };
                client::dial(&to, presents).await.map_err(|r| r.failure)?
            }
            Aim::Url { url, .. } => return Err(Failure::usage(fill(&record::words().address_not_paired, &[("url", url)]))),
        };
        let Some(paired) = paired else { return Err(Failure::new("pair.redeem answered no device token")) };
        match &aim {
            Aim::Alias { alias, record } => {
                let mut kept = record.clone();
                (kept.device_id, kept.device_token) = (paired.device_id, paired.device_token);
                aim::write_host(&self.home, alias, &kept).map_err(|e| Failure::new(e.to_string()))?;
            }
            _ => self.hold_token(paired.device_token),
        }
        Ok(())
    }

    /// Whether a dial now goes to the host serving this state file here, rather than one on the account or at an address.
    pub fn aims_here(&self) -> bool {
        let pick = Pick { host: self.args.host.as_deref(), env: &self.process_env, home: self.home.clone() };
        matches!(aim::aimed(&self.args.state, &pick), Ok(Aim::Here))
    }

    /// Presents `token` to the host here in place of the file beside the state: one a pairing bought earlier.
    pub fn hold_token(&self, token: String) {
        *self.paired.lock().unwrap() = Some(token);
    }

    /// The token a pairing bought for the host here, where one did.
    pub fn held_token(&self) -> Option<String> {
        self.paired.lock().unwrap().clone()
    }

    /// A host on the account. A record written off the listing holds no token until its first dial, which is
    /// admitted on the key this computer signs with; a token that host took away while the account still names this
    /// computer is one more dial, proving the key. Either way the token it answers is written into the record.
    async fn dial_account(&self, alias: &str, record: &HostRecord, window: Duration, judged: bool) -> Result<Client, Failure> {
        let admitting = || -> Option<Admit> {
            record.host_key.as_ref()?;
            let key = aim::device_key(&self.home)?;
            Some(Admit { name: hostname(), public_key: key.public_key, private_key_pem: key.private_key_pem })
        };
        let admit = if record.device_token.is_empty() { admitting() } else { None };
        let url = ws_url(&record.url)?;
        let release = ReleaseHost::At { at: alias.to_owned(), here: on_this_computer(&record.url) };
        let to = Dial {
            url: &url,
            at: &record.url,
            window,
            pinned: record.host_key.as_deref(),
            alias: Some(alias),
            release: judged.then_some(&release),
        };
        let first = match &admit {
            Some(key) => Presents::Admit(key),
            None => Presents::Token(&record.device_token),
        };
        let (client, paired) = match client::dial(&to, first).await {
            Ok(dialled) => dialled,
            Err(refused) if admit.is_none() && refused.token && refused.failure.kind.as_deref() == Some("auth") => {
                let Some(key) = admitting() else { return Err(refused.failure) };
                client::dial(&to, Presents::Admit(&key)).await.map_err(|r| r.failure)?
            }
            Err(refused) => return Err(refused.failure),
        };
        if let Some(paired) = paired {
            let mut kept = record.clone();
            (kept.device_id, kept.device_token) = (paired.device_id, paired.device_token);
            aim::write_host(&self.home, alias, &kept).map_err(|e| Failure::new(e.to_string()))?;
        }
        Ok(client)
    }
}

/// Whether an address names a host on this computer, as a turn's launch pair does, whose fix is the one the host here
/// takes rather than an update over there.
fn on_this_computer(url: &str) -> bool {
    aim::served_hostname(url).is_some_and(|host| aim::is_loopback(&host))
}

/// What this computer calls itself, as node's os.hostname reads it: the name an admission gives the host's listing.
fn hostname() -> String {
    let mut name = [0u8; 256];
    // SAFETY: the buffer is ours and its length is the one handed in.
    let read = unsafe { libc::gethostname(name.as_mut_ptr().cast(), name.len()) };
    if read != 0 {
        return String::new();
    }
    let end = name.iter().position(|b| *b == 0).unwrap_or(name.len());
    String::from_utf8_lossy(&name[..end]).into_owned()
}

fn ws_url(url: &str) -> Result<String, Failure> {
    aim::ws_url_of(url).ok_or_else(|| Failure::new(fill(&record::words().no_answer, &[("where", url), ("why", "not an address")])))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_cloud_flag_as_the_protocol_does() {
        for (word, want) in record::host().clouds {
            let env: Env = [("WSP_CLOUD".to_owned(), word.clone())].into_iter().collect();
            assert_eq!(cloud_on(&env), want, "WSP_CLOUD={word:?}");
        }
        assert!(!cloud_on(&Env::new()));
    }

    #[test]
    fn a_guest_reads_no_value_by_name_but_its_launch_pair_and_turn() {
        let env: Env =
            [("WSP_HOST_URL", "u"), ("WSP_HOST_TOKEN", "t"), ("WSP_TURN", "turn"), ("TZ", "UTC"), ("WSP_CLOUD", "1"), ("KEY", "k")]
                .map(|(k, v)| (k.to_owned(), v.to_owned()))
                .into_iter()
                .collect();
        let guest = Host::new(&Args { guest: true, ..Args::default() }, &env, None);
        let mut read: Vec<&str> = guest.env().keys().map(String::as_str).collect();
        read.sort_unstable();
        assert_eq!(read, ["WSP_HOST_TOKEN", "WSP_HOST_URL", "WSP_TURN"]);
        // The cloud's state is still the host's to say.
        assert!(guest.cloud());
        assert_eq!(Host::new(&Args::default(), &env, None).env().len(), env.len());
    }
}
