// SPDX-License-Identifier: AGPL-3.0-only
// Ports packages/catalog/test/mcp-launch-codex.test.ts, with the helper of packages/catalog/test/turn-files.ts.
use std::collections::HashSet;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use indexmap::{IndexMap, IndexSet};
use serde_json::{json, Value};
use wsp_catalog::mcp::{McpTransport, CODEX_TOML};
use wsp_catalog::mcp_launch::{codex_launch_config, launch_misses, LaunchConfigs, TurnFile};
use wsp_catalog::signin_rows::server_values_of;

const TOKEN: &str = "tok_TESTONLY";

fn own(text: &str) -> TurnFile {
    TurnFile { path: "/root/.claude.json".to_string(), text: text.to_string() }
}

fn path_dirs() -> Vec<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).filter(|d| !d.as_os_str().is_empty()).collect()
}

fn codex_binary() -> Option<PathBuf> {
    path_dirs().into_iter().map(|d| d.join("codex")).find(|p| p.exists())
}

struct Added {
    text: String,
    stored: IndexMap<String, String>,
    values: IndexMap<String, String>,
}

/// The file `wsp servers add --agent codex --header Authorization=...` leaves on another computer, what it hands the
/// vault, and the values the vault hands a launch back.
fn added(url: &str) -> Added {
    let transport = McpTransport::Http { url: url.to_string(), headers: IndexMap::from([("Authorization".to_string(), format!("Bearer {TOKEN}"))]) };
    let placed = CODEX_TOML.place(None, "acme", &transport).unwrap();
    let named = CODEX_TOML.refer(&placed, None, &[], &IndexSet::new()).unwrap();
    let mut stored: IndexMap<String, String> = IndexMap::new();
    for server in &named.servers {
        stored.extend(server.values.clone());
    }
    let values = server_values_of(&stored);
    Added { text: named.text, stored, values }
}

mod a_codex_server_added_with_a_bearer_token_on_a_computer_whose_launch_hands_its_values {
    use super::*;

    #[test]
    fn names_a_variable_the_launch_fills_and_none_codex_would_read_from_its_own_environ() {
        let Added { text, stored, values } = added("https://acme.example/mcp");
        assert!(!text.contains("bearer_token_env_var"), "{text}");
        assert!(!text.contains(TOKEN), "{text}");
        assert_eq!(stored, IndexMap::from([("WSP_MCP_ACME_AUTHORIZATION".to_string(), TOKEN.to_string())]));
        let server = CODEX_TOML.read(&text, "/root").remove(0);
        let held: IndexSet<String> = values.keys().cloned().collect();
        assert_eq!(launch_misses("codex", CODEX_TOML, &server, &held), Vec::<String>::new());
        assert_eq!(
            codex_launch_config(&LaunchConfigs { user: Some(own(&text)), projects: None, folder: "/root".to_string(), key: None }, &values),
            IndexMap::from([("mcp_servers.acme.http_headers.Authorization".to_string(), format!("Bearer {TOKEN}"))])
        );
    }

    #[test]
    fn is_read_back_from_the_vault_under_the_name_codex_reads_beside_the_token_every_ot() {
        let record = |pairs: &[(&str, &str)]| -> IndexMap<String, String> { pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect() };
        assert_eq!(
            server_values_of(&record(&[("WSP_MCP_ACME_AUTHORIZATION", TOKEN), ("X_AUTHORIZATION", "x")])),
            record(&[
                ("WSP_MCP_ACME_AUTHORIZATION", TOKEN),
                ("WSP_MCP_ACME_AUTHORIZATION_BEARER", &format!("Bearer {TOKEN}")),
                ("X_AUTHORIZATION", "x"),
            ])
        );
        assert_eq!(
            server_values_of(&record(&[("WSP_MCP_ACME_AUTHORIZATION", TOKEN), ("WSP_MCP_ACME_AUTHORIZATION_BEARER", "kept")])),
            record(&[("WSP_MCP_ACME_AUTHORIZATION", TOKEN), ("WSP_MCP_ACME_AUTHORIZATION_BEARER", "kept")])
        );
    }

    type Seen = Arc<Mutex<Vec<Option<String>>>>;

    fn read_body(reader: &mut BufReader<TcpStream>, length: Option<usize>, chunked: bool) -> Vec<u8> {
        let mut body = Vec::new();
        if chunked {
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 {
                    break;
                }
                let size = usize::from_str_radix(line.trim().split(';').next().unwrap_or("0"), 16).unwrap_or(0);
                let mut chunk = vec![0u8; size];
                if reader.read_exact(&mut chunk).is_err() {
                    break;
                }
                body.extend(chunk);
                let mut crlf = String::new();
                let _ = reader.read_line(&mut crlf);
                if size == 0 {
                    break;
                }
            }
        } else if let Some(n) = length {
            body = vec![0u8; n];
            let _ = reader.read_exact(&mut body);
        }
        body
    }

    fn respond(stream: &mut TcpStream, status: &str, body: Option<&str>) {
        let head = match body {
            Some(b) => format!("HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{b}", b.len()),
            None => format!("HTTP/1.1 {status}\r\ncontent-length: 0\r\n\r\n"),
        };
        let _ = stream.write_all(head.as_bytes());
    }

    fn handle(stream: TcpStream, seen: Seen) {
        let mut writer = stream.try_clone().unwrap();
        let mut reader = BufReader::new(stream);
        loop {
            let mut request = String::new();
            if reader.read_line(&mut request).unwrap_or(0) == 0 {
                return;
            }
            let method = request.split_whitespace().next().unwrap_or("").to_string();
            let (mut length, mut chunked, mut authorization) = (None, false, None);
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 {
                    return;
                }
                let line = line.trim_end().to_string();
                if line.is_empty() {
                    break;
                }
                if let Some((name, value)) = line.split_once(':') {
                    let value = value.trim().to_string();
                    match name.to_ascii_lowercase().as_str() {
                        "content-length" => length = value.parse().ok(),
                        "transfer-encoding" => chunked = value.to_ascii_lowercase().contains("chunked"),
                        "authorization" => authorization = Some(value),
                        _ => {}
                    }
                }
            }
            let body = read_body(&mut reader, length, chunked);
            seen.lock().unwrap().push(authorization);
            let msg: Value = if body.is_empty() { json!({}) } else { serde_json::from_slice(&body).unwrap_or(json!({})) };
            let Some(id) = msg.get("id").filter(|_| method == "POST") else {
                respond(&mut writer, if method == "POST" { "202 Accepted" } else { "405 Method Not Allowed" }, None);
                continue;
            };
            let result = match msg.get("method").and_then(Value::as_str) {
                Some("initialize") => json!({
                    "protocolVersion": msg["params"]["protocolVersion"],
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": "acme", "version": "1" },
                }),
                Some("tools/list") => json!({ "tools": [{ "name": "ping", "inputSchema": { "type": "object" } }] }),
                _ => json!({}),
            };
            respond(&mut writer, "200 OK", Some(&json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string()));
        }
    }

    struct Stopper(Option<Child>);

    impl Drop for Stopper {
        // SIGTERM, which the npm wrapper hands its native codex and waits out; a SIGKILL leaves that one writing in the home.
        fn drop(&mut self) {
            if let Some(child) = self.0.as_mut() {
                if child.try_wait().ok().flatten().is_none() {
                    let _ = Command::new("kill").args(["-TERM", &child.id().to_string()]).status();
                    let _ = child.wait();
                }
            }
        }
    }

    #[test]
    fn starts_under_the_real_codex_cli_with_the_token_sent_as_a_bearer_header_and_nothi() {
        let seen: Seen = Arc::new(Mutex::new(Vec::new()));
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let served = seen.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let seen = served.clone();
                std::thread::spawn(move || handle(stream, seen));
            }
        });
        let Added { text, values, .. } = added(&format!("http://127.0.0.1:{port}/mcp"));
        let Some(codex) = codex_binary() else {
            eprintln!("skipped: no codex on PATH");
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("project");
        std::fs::create_dir(dir.path().join(".codex")).unwrap();
        std::fs::create_dir(&folder).unwrap();
        std::fs::write(dir.path().join(".codex").join("config.toml"), &text).unwrap();
        let config = codex_launch_config(&LaunchConfigs { user: Some(own(&text)), projects: None, folder: folder.to_string_lossy().to_string(), key: None }, &values);

        let path = std::env::join_paths(path_dirs()).unwrap();
        let mut child = Command::new(&codex)
            .arg("app-server")
            .current_dir(&folder)
            .env_clear()
            .env("PATH", path)
            .env("HOME", dir.path())
            .env("CODEX_HOME", dir.path().join(".codex"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let _stopper = Stopper(Some(child));
        let (tx, rx) = mpsc::channel::<Value>();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Ok(message) = serde_json::from_str::<Value>(&line) {
                    let _ = tx.send(message);
                }
            }
        });
        let mut send = |message: Value| {
            let mut full = json!({ "jsonrpc": "2.0" });
            full.as_object_mut().unwrap().extend(message.as_object().unwrap().clone());
            writeln!(stdin, "{full}").unwrap();
            stdin.flush().unwrap();
        };
        send(json!({ "id": 1, "method": "initialize", "params": { "clientInfo": { "name": "wsp-test", "version": "0" }, "capabilities": { "experimentalApi": true } } }));
        let deadline = Instant::now() + Duration::from_secs(30);
        let (status, error) = loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let m = rx.recv_timeout(left).expect("codex never said whether acme started");
            if m["id"] == json!(1) {
                send(json!({ "method": "initialized" }));
                send(json!({ "id": 2, "method": "thread/start", "params": { "cwd": folder.to_string_lossy(), "config": config } }));
            } else if m["id"] == json!(2) && m.get("error").is_some() {
                panic!("{}", m["error"]);
            } else if m["method"] == json!("mcpServer/startupStatus/updated") && m["params"]["name"] == json!("acme") && m["params"]["status"] != json!("starting") {
                break (m["params"]["status"].as_str().unwrap().to_string(), m["params"]["error"].as_str().map(str::to_string));
            }
        };
        assert_eq!((status.as_str(), error), ("ready", None));
        let seen = seen.lock().unwrap();
        assert!(!seen.is_empty());
        let distinct: HashSet<Option<String>> = seen.iter().cloned().collect();
        assert_eq!(distinct, HashSet::from([Some(format!("Bearer {TOKEN}"))]));
    }
}
