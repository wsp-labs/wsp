// SPDX-License-Identifier: AGPL-3.0-only
//! The daemon's own requests to the computer's container engine over its unix socket: one request per connection,
//! and the one held stream of engine events, read a line at a time. Where the socket is comes from the person's own
//! Docker setup first, then the places engines put it.

use std::path::{Path, PathBuf};
use std::time::Duration;

use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

/// How long one request waits on the engine before it is given up.
const ASK_TIMEOUT: Duration = Duration::from_secs(5);
/// The most of an answer's body, or of one event line, held: an inspect is a few KB, an event line under one.
const BODY_CAP: usize = 4 << 20;
const HEAD_CAP: usize = 64 << 10;

/// The most of a Docker config or a context's meta file read: each is a few hundred bytes.
const CONFIG_CAP: u64 = 64 << 10;

/// An engine's socket, and the account its listener must run as where the login named it to a root daemon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Engine {
    pub(crate) path: PathBuf,
    pub(crate) owner: Option<u32>,
}

/// The engine's socket on this computer: `DOCKER_HOST` where it names a unix socket, the current Docker context's,
/// then the system's, Docker Desktop's and Podman's. None where no such socket is there. The two the home names are
/// the login's to point anywhere, so a root daemon dials them only where the login serves them.
pub(crate) fn engine_socket(home: Option<&Path>, docker_host: Option<&str>) -> Option<Engine> {
    let unix = |host: &str| host.strip_prefix("unix://").map(PathBuf::from);
    let plain = |path: PathBuf| Engine { path, owner: None };
    let owner = home.and_then(login_of);
    let mut tried: Vec<Engine> = Vec::new();
    tried.extend(docker_host.and_then(unix).map(plain));
    if let Some(home) = home {
        tried.extend(context_socket(home, owner).map(|path| Engine { path, owner }));
    }
    tried.push(plain(PathBuf::from("/var/run/docker.sock")));
    if let Some(home) = home {
        tried.push(Engine { path: home.join(".docker/run/docker.sock"), owner });
    }
    tried.push(plain(PathBuf::from("/run/podman/podman.sock")));
    tried.into_iter().find(|e| std::fs::metadata(&e.path).is_ok_and(|m| std::os::unix::fs::FileTypeExt::is_socket(&m.file_type())))
}

/// On a daemon that is root, the account the home belongs to, which is what wrote its Docker setup; none on a daemon
/// that runs as the person, who can dial nothing they could not dial themselves.
fn login_of(home: &Path) -> Option<u32> {
    use std::os::unix::fs::MetadataExt;
    nix::unistd::geteuid().is_root().then(|| std::fs::symlink_metadata(home).map(|m| m.uid()).unwrap_or(0))
}

/// The unix socket the current Docker context names. With an owner, as on a root daemon, only a socket that account
/// owns: the login writes the context, and a root daemon dialling the socket it names would otherwise send its
/// requests to any root service's socket.
fn context_socket(home: &Path, owner: Option<u32>) -> Option<PathBuf> {
    use std::os::unix::fs::MetadataExt;
    let sock = PathBuf::from(context_host(home)?.strip_prefix("unix://")?);
    let meta = std::fs::symlink_metadata(&sock).ok()?;
    let theirs = owner.is_none_or(|uid| meta.uid() == uid && std::os::unix::fs::FileTypeExt::is_socket(&meta.file_type()));
    theirs.then_some(sock)
}

/// The host the person's current Docker context names, off `~/.docker/config.json` and the context's meta file, which
/// the Docker CLI files under the sha256 of the context's name. None for the default context. Both are read beneath
/// the home through `beneath::file()`, so a link, a fifo or a file past the cap names no context and nothing waits.
fn context_host(home: &Path) -> Option<String> {
    use std::io::Read;
    let held = nix::fcntl::open(home, crate::beneath::dir_flags(), nix::sys::stat::Mode::empty()).ok()?;
    let read = |rel: &str| {
        let mut bytes = Vec::new();
        crate::beneath::file(&held, rel).ok()??.take(CONFIG_CAP).read_to_end(&mut bytes).ok()?;
        serde_json::from_slice::<serde_json::Value>(&bytes).ok()
    };
    let config = read(".docker/config.json")?;
    let name = config.get("currentContext")?.as_str().filter(|n| !n.is_empty() && *n != "default")?;
    let digest = Sha256::digest(name.as_bytes());
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    let meta = read(&format!(".docker/contexts/meta/{hex}/meta.json"))?;
    meta.pointer("/Endpoints/docker/Host")?.as_str().map(str::to_owned)
}

/// A response's status, whether its body is chunked, and the bytes read past its head.
/// The listener's account is checked once connected where the engine names one, since the login can put a link to
/// another socket in place of the one checked before the dial.
async fn open(sock: &Engine, method: &str, path: &str) -> Result<(UnixStream, u16, bool, Vec<u8>), String> {
    let mut stream = UnixStream::connect(&sock.path).await.map_err(|e| format!("{}: {e}", sock.path.display()))?;
    if let Some(owner) = sock.owner {
        let peer = stream.peer_cred().map_err(|e| format!("{}: {e}", sock.path.display()))?.uid();
        if peer != owner {
            return Err(format!("{} is served by another account than the one its Docker context belongs to", sock.path.display()));
        }
    }
    let request = format!("{method} {path} HTTP/1.1\r\nHost: docker\r\nConnection: close\r\nContent-Length: 0\r\n\r\n");
    stream.write_all(request.as_bytes()).await.map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    let end = loop {
        if let Some(at) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break at;
        }
        if buf.len() > HEAD_CAP {
            return Err("the engine's answer has no end to its head".into());
        }
        let mut chunk = [0u8; 8192];
        let n = stream.read(&mut chunk).await.map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("the engine closed before it answered".into());
        }
        buf.extend_from_slice(&chunk[..n]);
    };
    let head = String::from_utf8_lossy(&buf[..end]).into_owned();
    let mut lines = head.split("\r\n");
    let status = lines
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u16>().ok())
        .ok_or("the engine answered with a head that does not parse")?;
    let chunked = lines.any(|l| {
        let l = l.to_ascii_lowercase();
        l.starts_with("transfer-encoding:") && l.contains("chunked")
    });
    Ok((stream, status, chunked, buf[end + 4..].to_vec()))
}

/// One request: the status and the body as JSON, null where there was none.
pub(crate) async fn ask(sock: &Engine, method: &str, path: &str) -> Result<(u16, serde_json::Value), String> {
    let asked = async {
        let (mut stream, status, chunked, mut rest) = open(sock, method, path).await?;
        let mut chunk = [0u8; 8192];
        loop {
            let n = stream.read(&mut chunk).await.map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            rest.extend_from_slice(&chunk[..n]);
            if rest.len() > BODY_CAP {
                return Err("the engine's answer is past what the daemon reads".to_owned());
            }
        }
        let body = if chunked { Dechunk::default().feed(&rest) } else { rest };
        let value = if body.iter().all(u8::is_ascii_whitespace) {
            serde_json::Value::Null
        } else {
            serde_json::from_slice(&body).unwrap_or_default()
        };
        Ok((status, value))
    };
    tokio::time::timeout(ASK_TIMEOUT, asked).await.map_err(|_| "the engine did not answer in time".to_owned())?
}

/// A chunked body taken apart as its bytes come, whatever the reads split it at.
#[derive(Default)]
struct Dechunk {
    raw: Vec<u8>,
    /// The bytes of the chunk under way still to come, and the CRLF after them.
    left: usize,
    done: bool,
}

impl Dechunk {
    fn feed(&mut self, bytes: &[u8]) -> Vec<u8> {
        self.raw.extend_from_slice(bytes);
        let mut out = Vec::new();
        loop {
            if self.done {
                return out;
            }
            if self.left > 0 {
                let take = self.left.min(self.raw.len());
                // The last two bytes of a chunk's share are its CRLF, which are no body.
                let body = take.min(self.left.saturating_sub(2));
                out.extend_from_slice(&self.raw[..body]);
                self.raw.drain(..take);
                self.left -= take;
                if self.left > 0 {
                    return out;
                }
                continue;
            }
            let Some(at) = self.raw.windows(2).position(|w| w == b"\r\n") else { return out };
            let size =
                usize::from_str_radix(String::from_utf8_lossy(&self.raw[..at]).split(';').next().unwrap_or("").trim(), 16).unwrap_or(0);
            self.raw.drain(..at + 2);
            if size == 0 {
                self.done = true;
            } else {
                self.left = size + 2;
            }
        }
    }
}

/// The engine's event stream, held open: each next line is one event's JSON.
pub(crate) struct Events {
    stream: UnixStream,
    chunks: Option<Dechunk>,
    body: Vec<u8>,
}

impl Events {
    /// Subscribes to what `query` names; a status other than 200 is the engine's refusal.
    pub(crate) async fn open(sock: &Engine, query: &str) -> Result<Events, String> {
        let path = format!("/events?{query}");
        let (stream, status, chunked, rest) = tokio::time::timeout(ASK_TIMEOUT, open(sock, "GET", &path))
            .await
            .map_err(|_| "the engine did not answer in time".to_owned())??;
        if status != 200 {
            return Err(format!("the engine refused its events with {status}"));
        }
        let mut events = Events { stream, chunks: chunked.then(Dechunk::default), body: Vec::new() };
        events.take(&rest);
        Ok(events)
    }

    fn take(&mut self, bytes: &[u8]) {
        match &mut self.chunks {
            Some(chunks) => {
                let body = chunks.feed(bytes);
                self.body.extend_from_slice(&body);
            }
            None => self.body.extend_from_slice(bytes),
        }
    }

    /// The next line, or an error once the engine closes the stream. A line past the cap is dropped whole.
    pub(crate) async fn next_line(&mut self) -> Result<Vec<u8>, String> {
        loop {
            if let Some(at) = self.body.iter().position(|b| *b == b'\n') {
                let line: Vec<u8> = self.body.drain(..=at).collect();
                return Ok(line);
            }
            if self.body.len() > BODY_CAP {
                self.body.clear();
            }
            let mut chunk = [0u8; 8192];
            let n = self.stream.read(&mut chunk).await.map_err(|e| e.to_string())?;
            if n == 0 {
                return Err("the engine ended its events".into());
            }
            self.take(&chunk[..n]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chunked_body_reads_whole_whatever_the_reads_split_it_at() {
        let whole = b"5\r\nhello\r\n7;x=y\r\n, world\r\n0\r\n\r\n";
        for split in 0..whole.len() {
            let mut d = Dechunk::default();
            let mut out = d.feed(&whole[..split]);
            out.extend(d.feed(&whole[split..]));
            assert_eq!(out, b"hello, world", "split at {split}");
        }
    }

    #[test]
    fn the_socket_is_docker_host_first_then_the_current_context_then_the_systems() {
        let home = tempfile::tempdir().unwrap();
        let named = home.path().join("named.sock");
        let context = home.path().join("context.sock");
        let _a = std::os::unix::net::UnixListener::bind(&named).unwrap();
        let _b = std::os::unix::net::UnixListener::bind(&context).unwrap();
        let docker = home.path().join(".docker");
        std::fs::create_dir_all(&docker).unwrap();
        std::fs::write(docker.join("config.json"), r#"{"currentContext":"colima"}"#).unwrap();
        let digest: String = Sha256::digest(b"colima").iter().map(|b| format!("{b:02x}")).collect();
        let meta = docker.join("contexts/meta").join(digest);
        std::fs::create_dir_all(&meta).unwrap();
        std::fs::write(meta.join("meta.json"), format!(r#"{{"Endpoints":{{"docker":{{"Host":"unix://{}"}}}}}}"#, context.display()))
            .unwrap();
        let host = format!("unix://{}", named.display());
        let path = |e: Option<Engine>| e.map(|e| e.path);
        assert_eq!(path(engine_socket(Some(home.path()), Some(&host))), Some(named.clone()));
        assert_eq!(path(engine_socket(Some(home.path()), Some("tcp://10.0.0.1:2375"))), Some(context.clone()));
        // A DOCKER_HOST naming no socket that is there falls through to the context's.
        assert_eq!(path(engine_socket(Some(home.path()), Some("unix:///nowhere/docker.sock"))), Some(context));
    }

    #[test]
    fn a_fifo_or_a_link_where_the_context_is_read_names_no_context_and_never_holds_the_read() {
        let home = tempfile::tempdir().unwrap();
        let docker = home.path().join(".docker");
        std::fs::create_dir_all(&docker).unwrap();
        let fifo = |p: &Path| nix::unistd::mkfifo(p, nix::sys::stat::Mode::from_bits_truncate(0o644)).unwrap();
        fifo(&docker.join("config.json"));
        let at = home.path().to_path_buf();
        assert_eq!(crate::kill::tests::within("a fifo at config.json", move || context_host(&at)), None);
        std::fs::remove_file(docker.join("config.json")).unwrap();
        std::fs::write(docker.join("config.json"), r#"{"currentContext":"colima"}"#).unwrap();
        let digest: String = Sha256::digest(b"colima").iter().map(|b| format!("{b:02x}")).collect();
        let meta = docker.join("contexts/meta").join(digest);
        std::fs::create_dir_all(&meta).unwrap();
        fifo(&meta.join("meta.json"));
        let at = home.path().to_path_buf();
        assert_eq!(crate::kill::tests::within("a fifo at meta.json", move || context_host(&at)), None);
        // A link where the meta file goes names nothing either.
        std::fs::remove_file(meta.join("meta.json")).unwrap();
        std::fs::write(home.path().join("meta.json"), r#"{"Endpoints":{"docker":{"Host":"unix:///x.sock"}}}"#).unwrap();
        std::os::unix::fs::symlink(home.path().join("meta.json"), meta.join("meta.json")).unwrap();
        assert_eq!(context_host(home.path()), None);
        std::fs::remove_file(meta.join("meta.json")).unwrap();
        std::fs::rename(home.path().join("meta.json"), meta.join("meta.json")).unwrap();
        assert_eq!(context_host(home.path()).as_deref(), Some("unix:///x.sock"));
    }

    #[test]
    fn a_root_daemon_dials_a_contexts_socket_only_where_the_login_owns_it() {
        let home = tempfile::tempdir().unwrap();
        let context = home.path().join("context.sock");
        let _listening = std::os::unix::net::UnixListener::bind(&context).unwrap();
        let docker = home.path().join(".docker");
        std::fs::create_dir_all(&docker).unwrap();
        std::fs::write(docker.join("config.json"), r#"{"currentContext":"colima"}"#).unwrap();
        let digest: String = Sha256::digest(b"colima").iter().map(|b| format!("{b:02x}")).collect();
        let meta = docker.join("contexts/meta").join(digest);
        std::fs::create_dir_all(&meta).unwrap();
        std::fs::write(meta.join("meta.json"), format!(r#"{{"Endpoints":{{"docker":{{"Host":"unix://{}"}}}}}}"#, context.display()))
            .unwrap();
        let mine = nix::unistd::geteuid().as_raw();
        assert_eq!(context_socket(home.path(), Some(mine)), Some(context.clone()));
        // A socket some other account owns, a root service's, is never one the login's context points the root daemon at.
        assert_eq!(context_socket(home.path(), Some(mine + 1)), None);
        assert_eq!(context_socket(home.path(), None), Some(context));
    }

    #[tokio::test]
    async fn a_dial_the_login_named_to_a_root_daemon_is_dropped_where_another_account_serves_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("engine.sock");
        let listener = tokio::net::UnixListener::bind(&path).unwrap();
        let served = tokio::spawn(async move {
            for _ in 0..2 {
                let (mut s, _) = listener.accept().await.unwrap();
                let mut b = [0u8; 4096];
                if s.read(&mut b).await.unwrap_or(0) > 0 {
                    let _ = s.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}").await;
                }
            }
        });
        let mine = nix::unistd::geteuid().as_raw();
        let theirs = Engine { path: path.clone(), owner: Some(mine + 1) };
        let err = ask(&theirs, "GET", "/containers/json").await.unwrap_err();
        assert!(err.ends_with("is served by another account than the one its Docker context belongs to"), "{err}");
        assert_eq!(ask(&Engine { path, owner: Some(mine) }, "GET", "/containers/json").await.unwrap().0, 200);
        served.await.unwrap();
    }
}
