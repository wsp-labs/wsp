// SPDX-License-Identifier: AGPL-3.0-only
//! `fetch(url, { redirect: "manual" })`: a 3xx answer is the answer, its `location` unfollowed. Each expected value is what
//! node 22 printed for the same server (a 302 to /final, then 200 "final").

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use wsp_engine::fetch::{platform_fetch, platform_fetch_manual, FetchRequest};

async fn answer(mut s: TcpStream) {
    let mut got = Vec::new();
    let mut buf = [0u8; 4096];
    while !got.windows(4).any(|w| w == b"\r\n\r\n") {
        let n = s.read(&mut buf).await.unwrap_or(0);
        if n == 0 {
            return;
        }
        got.extend_from_slice(&buf[..n]);
    }
    let head = String::from_utf8_lossy(&got).to_string();
    let wire = if head.starts_with("GET /r ") {
        "HTTP/1.1 302 Found\r\nlocation: /final\r\nset-cookie: a=b\r\ncontent-length: 5\r\nconnection: close\r\n\r\nmoved".to_string()
    } else {
        "HTTP/1.1 200 OK\r\ncontent-length: 5\r\nconnection: close\r\n\r\nfinal".to_string()
    };
    let _ = s.write_all(wire.as_bytes()).await;
}

async fn serve() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let Ok((s, _)) = l.accept().await else { return };
            tokio::spawn(answer(s));
        }
    });
    port
}

fn get(url: &str) -> FetchRequest {
    FetchRequest { method: "GET".into(), url: url.into(), headers: vec![], body: None, abort: None }
}

#[tokio::test]
async fn a_manual_fetch_answers_the_redirect_itself() {
    let port = serve().await;
    let res = platform_fetch_manual().fetch(get(&format!("http://127.0.0.1:{port}/r"))).await.unwrap();
    assert_eq!(res.status, 302);
    assert_eq!(res.header("location"), Some("/final"));
    assert_eq!(res.text(), "moved");
}

#[tokio::test]
async fn a_manual_fetch_of_a_plain_answer_is_the_answer() {
    let port = serve().await;
    let res = platform_fetch_manual().fetch(get(&format!("http://127.0.0.1:{port}/final"))).await.unwrap();
    assert_eq!((res.status, res.text().as_str()), (200, "final"));
}

#[tokio::test]
async fn the_platform_fetch_still_follows_the_redirect() {
    let port = serve().await;
    let res = platform_fetch().fetch(get(&format!("http://127.0.0.1:{port}/r"))).await.unwrap();
    assert_eq!((res.status, res.text().as_str()), (200, "final"));
}
