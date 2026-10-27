fn spec(doc: serde_json::Value) -> MachineSpec {
    serde_json::from_value(doc).expect("a machine spec")
}

async fn made(backend: &Arc<StubBackend>) -> Arc<StubMachine> {
    backend.make(spec(json!({ "kind": "sandbox" })), None, None).await.expect("a machine")
}

fn missing_404(e: &EngineError) -> bool {
    matches!(e, EngineError::Refused { kind: Some(wsp_engine::errors::ErrorKind::Missing), status: Some(404), .. })
}

async fn raw_http(url: &str, request: impl FnOnce(&str, &str) -> Vec<u8>) -> Vec<u8> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let rest = url.strip_prefix("http://").expect("an http url");
    let (host, path) = rest.split_once('/').map_or((rest, "/".to_string()), |(h, p)| (h, format!("/{p}")));
    let mut stream = tokio::net::TcpStream::connect(host).await.expect("a connection");
    stream.write_all(&request(host, &path)).await.expect("a request");
    let mut out = Vec::new();
    stream.read_to_end(&mut out).await.expect("a reply");
    out
}

async fn reqwest_free_put(url: &str, body: &[u8]) -> String {
    let body = body.to_vec();
    let reply = raw_http(url, move |host, path| {
        let mut req = format!("PUT {path} HTTP/1.1\r\nhost: {host}\r\ncontent-length: {}\r\n\r\n", body.len()).into_bytes();
        req.extend_from_slice(&body);
        req
    })
    .await;
    String::from_utf8_lossy(&reply).into_owned()
}

async fn http_get(url: &str) -> Vec<u8> {
    let reply = raw_http(url, |host, path| format!("GET {path} HTTP/1.1\r\nhost: {host}\r\n\r\n").into_bytes()).await;
    let at = reply.windows(4).position(|w| w == b"\r\n\r\n").expect("a head") + 4;
    reply[at..].to_vec()
}
