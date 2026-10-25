// SPDX-License-Identifier: AGPL-3.0-only
//! `wsp daemon ...` runs the daemon exactly as wsp-daemon does, `wsp mcp ...` the tool server, `wsp host` a stub
//! that touches each library the host links, and `wsp tls` the rustls road every wss dial takes.

#[path = "../../../daemon/crates/wsp-daemon-bin/src/flags.rs"]
mod flags;
#[path = "../../../daemon/crates/wsp-daemon-bin/src/score.rs"]
mod score;
#[path = "../../../daemon/crates/wsp-daemon-bin/src/verbs.rs"]
mod verbs;

use std::time::Instant;

use clap::Parser;
use wsp_daemon::Daemon;
use wsp_frames::words;

fn main() {
    let mut args: Vec<String> = std::env::args().collect();
    let sub = if args.len() > 1 { args.remove(1) } else { String::new() };
    let code = match sub.as_str() {
        "daemon" => daemon(args),
        "mcp" => {
            args.insert(1, "mcp".to_owned());
            daemon(args)
        }
        "host" => host(),
        "tls" => tls(),
        _ => {
            eprintln!("usage: wsp daemon|mcp|host|tls ...");
            3
        }
    };
    std::process::exit(code);
}

fn daemon(args: Vec<String>) -> i32 {
    let started = Instant::now();
    let mut flags = match flags::Flags::try_parse_from(args) {
        Ok(f) => f,
        Err(e) => {
            let _ = e.print();
            return 3;
        }
    };
    if let Some(verb) = flags.verb.take() {
        return verbs::run(verb);
    }
    let host = flags.host.clone();
    let options = flags.into_options();
    score::apply();
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().max_blocking_threads(4).thread_stack_size(256 * 1024).build().unwrap();
    runtime.block_on(async move {
        let daemon = match Daemon::bind(options).await {
            Ok(d) => d,
            Err(e) => {
                eprintln!("{}: {e}", words::FAILED_TO_START);
                return 1;
            }
        };
        println!("{}", words::listening_line(&host, daemon.local_addr().port()));
        eprintln!("{}", words::ready_line(started.elapsed().as_millis()));
        match daemon.run().await {
            Ok(()) => 0,
            Err(_) => 1,
        }
    })
}

fn tls() -> i32 {
    let config = rustls::ClientConfig::builder().with_root_certificates(rustls::RootCertStore::empty()).with_no_client_auth();
    println!("rustls client config built, provider suites: {}", config.crypto_provider().cipher_suites.len());
    0
}

fn host() -> i32 {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let db = rusqlite::Connection::open_in_memory().unwrap();
    db.execute_batch("pragma journal_mode=wal; create table collections (collection text, id text, body text, primary key (collection, id));").unwrap();
    db.execute("insert into collections values ('places', 'a', ?1)", [serde_json::json!({"z": 1, "a": 2}).to_string()]).unwrap();
    let body: String = db.query_row("select body from collections where collection = 'places'", [], |r| r.get(0)).unwrap();
    println!("sqlite {} row {body}", rusqlite::version());
    let collator = icu_collator::Collator::try_new(Default::default(), Default::default()).unwrap();
    println!("collate a<B: {:?}", collator.compare("a", "B"));
    let client = reqwest::Client::builder().build();
    println!("reqwest client: {}", client.is_ok());
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let local = tokio::task::LocalSet::new();
    local.block_on(&runtime, async {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::task::spawn_local(async move {
            loop {
                let (stream, _) = listener.accept().await.unwrap();
                let io = hyper_util::rt::TokioIo::new(stream);
                tokio::task::spawn_local(async move {
                    let svc = hyper::service::service_fn(|_req| async {
                        Ok::<_, std::convert::Infallible>(hyper::Response::new(http_body_util::Full::new(hyper::body::Bytes::from_static(b"wsp"))))
                    });
                    let _ = hyper::server::conn::http1::Builder::new().serve_connection(io, svc).await;
                });
            }
        });
        let mut s = tokio::net::TcpStream::connect(addr).await.unwrap();
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        s.write_all(b"GET / HTTP/1.1\r\nhost: x\r\nconnection: close\r\n\r\n").await.unwrap();
        let mut got = String::new();
        s.read_to_string(&mut got).await.unwrap();
        println!("http answered: {}", got.lines().next().unwrap_or(""));
        #[cfg(target_os = "linux")]
        if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
            for line in status.lines().filter(|l| l.starts_with("VmRSS") || l.starts_with("VmHWM")) {
                println!("{line}");
            }
        }
    });
    0
}
