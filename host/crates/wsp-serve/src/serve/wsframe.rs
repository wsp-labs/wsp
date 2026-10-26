// SPDX-License-Identifier: AGPL-3.0-only
//! The websocket as the `ws` library (8.21.3) speaks it, which is what the runtime's door sits on (serve.ts:470-603):
//! the upgrade's checks and refusals (`handleUpgrade`), the frames a client sends read the way `Receiver` reads them
//! (its faults, the codes it closes them with, its 100 MiB ceiling) and the frames the server writes. Spec:
//! spec/serve_wire.md, W11-W16.

use std::collections::VecDeque;

/// `maxPayload`: ws's default, which serve.ts does not change.
pub const MAX_PAYLOAD: usize = 100 * 1024 * 1024;

/// `maxFragments`: ws's server default, the most fragments one message may come in.
pub const MAX_FRAGMENTS: usize = 16 * 1024;

/// `maxBufferedChunks`: ws's server default, the most reads that may wait buffered for the rest of a frame.
pub const MAX_BUFFERED_CHUNKS: usize = 256 * 1024;

/// What `Receiver` hands the socket for one thing it read.
#[derive(Debug, PartialEq)]
pub enum Inbound {
    Text(String),
    Binary(Vec<u8>),
    Ping(Vec<u8>),
    Pong(Vec<u8>),
    /// The peer's close frame; the code is 1005 where the frame held none.
    Close(u16, String),
}

/// A frame wrong at the wire: the code `Receiver` closes with and the sentence its error carries.
#[derive(Debug, PartialEq)]
pub struct Fault {
    pub code: u16,
    pub sentence: String,
}

fn fault(code: u16, message: &str, prefixed: bool) -> Fault {
    let sentence = if prefixed { format!("Invalid WebSocket frame: {message}") } else { message.to_string() };
    Fault { code, sentence }
}

enum State {
    Info,
    Length16,
    Length64,
    Mask,
    Data,
}

/// ws's `Receiver` for a server: the bytes a socket delivers in, the messages out, in order.
pub struct Receiver {
    buf: Vec<u8>,
    /// The sizes of the reads `buf` still holds a part of, as ws's `_buffers` holds them.
    chunks: VecDeque<usize>,
    state: State,
    fin: bool,
    opcode: u8,
    length: usize,
    mask: [u8; 4],
    fragmented: u8,
    fragments: usize,
    total: usize,
    parts: Vec<u8>,
    ended: bool,
}

impl Default for Receiver {
    fn default() -> Receiver {
        Receiver::new()
    }
}

impl Receiver {
    pub fn new() -> Receiver {
        Receiver {
            buf: Vec::new(),
            chunks: VecDeque::new(),
            state: State::Info,
            fin: false,
            opcode: 0,
            length: 0,
            mask: [0; 4],
            fragmented: 0,
            fragments: 0,
            total: 0,
            parts: Vec::new(),
            ended: false,
        }
    }

    /// One chunk off the socket: everything it completes, in order. The list ends at a fault or at the peer's close,
    /// and a receiver that ended reads nothing more.
    pub fn push(&mut self, chunk: &[u8]) -> Vec<Result<Inbound, Fault>> {
        let mut out = Vec::new();
        if self.ended {
            return out;
        }
        if self.chunks.len() >= MAX_BUFFERED_CHUNKS {
            self.ended = true;
            out.push(Err(fault(1008, "Too many buffered chunks", false)));
            return out;
        }
        if !chunk.is_empty() {
            self.chunks.push_back(chunk.len());
        }
        self.buf.extend_from_slice(chunk);
        let mut at = 0;
        while !self.ended {
            match self.step(&mut at) {
                Step::More => break,
                Step::Next => {}
                Step::Out(item) => {
                    if matches!(item, Ok(Inbound::Close(..)) | Err(_)) {
                        self.ended = true;
                    }
                    out.push(item);
                }
            }
        }
        self.buf.drain(..at);
        let mut left = at;
        while let Some(front) = self.chunks.front_mut() {
            if *front > left {
                *front -= left;
                break;
            }
            left -= *front;
            self.chunks.pop_front();
        }
        out
    }

    fn step(&mut self, at: &mut usize) -> Step {
        let have = self.buf.len() - *at;
        match self.state {
            State::Info => {
                if have < 2 {
                    return Step::More;
                }
                let (b0, b1) = (self.buf[*at], self.buf[*at + 1]);
                *at += 2;
                if let Some(fault) = self.header(b0, b1) {
                    return Step::Out(Err(fault));
                }
                match b1 & 0x7f {
                    126 => self.state = State::Length16,
                    127 => self.state = State::Length64,
                    n => {
                        self.length = n as usize;
                        if let Some(fault) = self.have_length() {
                            return Step::Out(Err(fault));
                        }
                    }
                }
                Step::Next
            }
            State::Length16 => {
                if have < 2 {
                    return Step::More;
                }
                self.length = u16::from_be_bytes([self.buf[*at], self.buf[*at + 1]]) as usize;
                *at += 2;
                match self.have_length() {
                    Some(fault) => Step::Out(Err(fault)),
                    None => Step::Next,
                }
            }
            State::Length64 => {
                if have < 8 {
                    return Step::More;
                }
                let bytes: [u8; 8] = self.buf[*at..*at + 8].try_into().unwrap_or([0; 8]);
                *at += 8;
                let high = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                if high > (1 << 21) - 1 {
                    return Step::Out(Err(fault(1009, "Unsupported WebSocket frame: payload length > 2^53 - 1", false)));
                }
                self.length = (u64::from_be_bytes(bytes) as usize).min(usize::MAX / 2);
                match self.have_length() {
                    Some(fault) => Step::Out(Err(fault)),
                    None => Step::Next,
                }
            }
            State::Mask => {
                if have < 4 {
                    return Step::More;
                }
                self.mask.copy_from_slice(&self.buf[*at..*at + 4]);
                *at += 4;
                self.state = State::Data;
                Step::Next
            }
            State::Data => {
                if have < self.length {
                    return Step::More;
                }
                let mut data = self.buf[*at..*at + self.length].to_vec();
                *at += self.length;
                for (i, byte) in data.iter_mut().enumerate() {
                    *byte ^= self.mask[i & 3];
                }
                self.state = State::Info;
                if self.opcode > 0x07 {
                    return self.control(data);
                }
                self.fragments += 1;
                if self.fragments > MAX_FRAGMENTS {
                    return Step::Out(Err(fault(1008, "Too many message fragments", false)));
                }
                self.parts.extend_from_slice(&data);
                self.data_message()
            }
        }
    }

    /// The first two bytes of a frame (`getInfo`).
    fn header(&mut self, b0: u8, b1: u8) -> Option<Fault> {
        if b0 & 0x30 != 0 {
            return Some(fault(1002, "RSV2 and RSV3 must be clear", true));
        }
        if b0 & 0x40 != 0 {
            return Some(fault(1002, "RSV1 must be clear", true));
        }
        self.fin = b0 & 0x80 != 0;
        self.opcode = b0 & 0x0f;
        let length = b1 & 0x7f;
        match self.opcode {
            0 => {
                if self.fragmented == 0 {
                    return Some(fault(1002, "invalid opcode 0", true));
                }
                self.opcode = self.fragmented;
            }
            1 | 2 => {
                if self.fragmented != 0 {
                    return Some(fault(1002, &format!("invalid opcode {}", self.opcode), true));
                }
            }
            8..=10 => {
                if !self.fin {
                    return Some(fault(1002, "FIN must be set", true));
                }
                if length > 0x7d || (self.opcode == 8 && length == 1) {
                    return Some(fault(1002, &format!("invalid payload length {length}"), true));
                }
            }
            other => return Some(fault(1002, &format!("invalid opcode {other}"), true)),
        }
        if !self.fin && self.fragmented == 0 {
            self.fragmented = self.opcode;
        }
        if b1 & 0x80 == 0 {
            return Some(fault(1002, "MASK must be set", true));
        }
        None
    }

    /// The payload length is known (`haveLength`).
    fn have_length(&mut self) -> Option<Fault> {
        if self.length > 0 && self.opcode < 0x08 {
            self.total = self.total.saturating_add(self.length);
            if self.total > MAX_PAYLOAD {
                return Some(fault(1009, "Max payload size exceeded", false));
            }
        }
        self.state = State::Mask;
        None
    }

    fn data_message(&mut self) -> Step {
        if !self.fin {
            return Step::Next;
        }
        self.total = 0;
        self.fragmented = 0;
        self.fragments = 0;
        let bytes = std::mem::take(&mut self.parts);
        if self.opcode == 2 {
            return Step::Out(Ok(Inbound::Binary(bytes)));
        }
        match String::from_utf8(bytes) {
            Ok(text) => Step::Out(Ok(Inbound::Text(text))),
            Err(_) => Step::Out(Err(fault(1007, "invalid UTF-8 sequence", true))),
        }
    }

    fn control(&mut self, data: Vec<u8>) -> Step {
        match self.opcode {
            8 => {
                if data.is_empty() {
                    return Step::Out(Ok(Inbound::Close(1005, String::new())));
                }
                let code = u16::from_be_bytes([data[0], data[1]]);
                if !valid_close_code(code) {
                    return Step::Out(Err(fault(1002, &format!("invalid status code {code}"), true)));
                }
                match String::from_utf8(data[2..].to_vec()) {
                    Ok(reason) => Step::Out(Ok(Inbound::Close(code, reason))),
                    Err(_) => Step::Out(Err(fault(1007, "invalid UTF-8 sequence", true))),
                }
            }
            9 => Step::Out(Ok(Inbound::Ping(data))),
            _ => Step::Out(Ok(Inbound::Pong(data))),
        }
    }
}

enum Step {
    More,
    Next,
    Out(Result<Inbound, Fault>),
}

/// ws's `isValidStatusCode`.
fn valid_close_code(code: u16) -> bool {
    (1000..=1014).contains(&code) && !matches!(code, 1004..=1006) || (3000..=4999).contains(&code)
}

/// One frame the server writes: final, unmasked.
pub fn frame(opcode: u8, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(payload.len() + 10);
    out.push(0x80 | opcode);
    match payload.len() {
        n if n < 126 => out.push(n as u8),
        n if n < 65536 => {
            out.push(126);
            out.extend_from_slice(&(n as u16).to_be_bytes());
        }
        n => {
            out.push(127);
            out.extend_from_slice(&(n as u64).to_be_bytes());
        }
    }
    out.extend_from_slice(payload);
    out
}

pub fn text_frame(text: &str) -> Vec<u8> {
    frame(1, text.as_bytes())
}

pub fn binary_frame(bytes: &[u8]) -> Vec<u8> {
    frame(2, bytes)
}

pub fn ping_frame(bytes: &[u8]) -> Vec<u8> {
    frame(9, bytes)
}

pub fn pong_frame(bytes: &[u8]) -> Vec<u8> {
    frame(10, bytes)
}

/// A close frame; no code writes an empty one.
pub fn close_frame(code: Option<u16>, reason: &str) -> Vec<u8> {
    let mut payload = Vec::new();
    if let Some(code) = code {
        payload.extend_from_slice(&code.to_be_bytes());
        payload.extend_from_slice(reason.as_bytes());
    }
    frame(8, &payload)
}

/// A head that is no HTTP request.
#[derive(Debug)]
pub struct BadHead;

/// The head of an HTTP request: its method, its target and its headers (names as sent, values trimmed).
pub struct Head {
    pub method: String,
    pub target: String,
    pub headers: Vec<(String, String)>,
}

impl Head {
    /// Reads the head at the start of `bytes`: it and the count of bytes it took, up to and including the blank
    /// line. `Ok(None)` where the blank line has not come, an error where what came is not a request.
    pub fn read(bytes: &[u8]) -> Result<Option<(Head, usize)>, BadHead> {
        let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") else { return Ok(None) };
        let text = std::str::from_utf8(&bytes[..end]).map_err(|_| BadHead)?;
        let mut lines = text.split("\r\n");
        let mut first = lines.next().ok_or(BadHead)?.split(' ');
        let (method, target, version) = (first.next().ok_or(BadHead)?, first.next().ok_or(BadHead)?, first.next().ok_or(BadHead)?);
        if method.is_empty() || target.is_empty() || !version.starts_with("HTTP/1.") || first.next().is_some() {
            return Err(BadHead);
        }
        let mut headers = Vec::new();
        for line in lines {
            let (name, value) = line.split_once(':').ok_or(BadHead)?;
            if name.is_empty() || name.contains([' ', '\t']) {
                return Err(BadHead);
            }
            headers.push((name.to_ascii_lowercase(), value.trim_matches([' ', '\t']).to_string()));
        }
        Ok(Some((Head { method: method.to_string(), target: target.to_string(), headers }, end + 4)))
    }
}

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers.iter().find(|(key, _)| key.eq_ignore_ascii_case(name)).map(|(_, value)| value.as_str())
}

/// Whether node's HTTP parser would raise `upgrade` for these headers: an Upgrade header and a Connection that names it.
pub fn is_upgrade(headers: &[(String, String)]) -> bool {
    header(headers, "upgrade").is_some()
        && header(headers, "connection").is_some_and(|value| value.split(',').any(|token| token.trim().eq_ignore_ascii_case("upgrade")))
}

/// A handshake ws refuses: its status and sentence, and the headers it adds to the refusal.
pub struct Refusal {
    pub status: u16,
    pub message: &'static str,
    pub extra: &'static [(&'static str, &'static str)],
}

impl Refusal {
    /// `abortHandshake`: the refusal as ws writes it.
    pub fn bytes(&self) -> Vec<u8> {
        refusal_bytes(self.status, self.message, self.extra)
    }
}

/// `abortHandshake(socket, code, message, headers)`.
pub fn refusal_bytes(status: u16, message: &str, extra: &[(&str, &str)]) -> Vec<u8> {
    let mut out = format!(
        "HTTP/1.1 {status} {}\r\nConnection: close\r\nContent-Type: text/html\r\nContent-Length: {}\r\n",
        reason_phrase(status),
        message.len()
    );
    for (name, value) in extra {
        out.push_str(&format!("{name}: {value}\r\n"));
    }
    out.push_str("\r\n");
    out.push_str(message);
    out.into_bytes()
}

fn reason_phrase(status: u16) -> &'static str {
    match status {
        400 => "Bad Request",
        401 => "Unauthorized",
        405 => "Method Not Allowed",
        _ => "Error",
    }
}

/// What a handshake that passed holds: the key to answer and the subprotocol ws selects.
pub struct Accepted {
    pub key: String,
    pub protocol: Option<String>,
}

/// `handleUpgrade`'s checks in ws's order: the method, the Upgrade header, the key, the version, the subprotocols.
pub fn check_upgrade(method: &str, headers: &[(String, String)]) -> Result<Accepted, Refusal> {
    let refuse = |status, message| Refusal { status, message, extra: &[] };
    if method != "GET" {
        return Err(refuse(405, "Invalid HTTP method"));
    }
    if !header(headers, "upgrade").is_some_and(|value| value.eq_ignore_ascii_case("websocket")) {
        return Err(refuse(400, "Invalid Upgrade header"));
    }
    let key = header(headers, "sec-websocket-key").filter(|key| is_key(key));
    let Some(key) = key else {
        return Err(refuse(400, "Missing or invalid Sec-WebSocket-Key header"));
    };
    let version = header(headers, "sec-websocket-version").and_then(|value| value.trim().parse::<f64>().ok());
    if version != Some(13.0) && version != Some(8.0) {
        return Err(Refusal {
            status: 400,
            message: "Missing or invalid Sec-WebSocket-Version header",
            extra: &[("Sec-WebSocket-Version", "13, 8")],
        });
    }
    let protocol = match header(headers, "sec-websocket-protocol") {
        None => None,
        Some(value) => match protocols(value) {
            Some(first) => first,
            None => return Err(refuse(400, "Invalid Sec-WebSocket-Protocol header")),
        },
    };
    Ok(Accepted { key: key.to_string(), protocol })
}

/// `/^[+/0-9A-Za-z]{22}==$/`.
fn is_key(key: &str) -> bool {
    key.len() == 24 && key.ends_with("==") && key[..22].bytes().all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/')
}

/// ws's `subprotocol.parse`: the first offered one (`Some(None)` for an empty header), none where the header is not a list
/// of distinct tokens.
fn protocols(value: &str) -> Option<Option<String>> {
    let mut seen: Vec<&str> = Vec::new();
    for token in value.split(',') {
        let token = token.trim_matches([' ', '\t']);
        let token_char = |b: u8| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b);
        if token.is_empty() || !token.bytes().all(token_char) || seen.contains(&token) {
            return None;
        }
        seen.push(token);
    }
    Some(seen.first().map(|first| first.to_string()))
}

/// The 101 ws writes.
pub fn switching(accept: &str, protocol: Option<&str>) -> Vec<u8> {
    let mut out =
        format!("HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n");
    if let Some(protocol) = protocol {
        out.push_str(&format!("Sec-WebSocket-Protocol: {protocol}\r\n"));
    }
    out.push_str("\r\n");
    out.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn masked(opcode: u8, fin: bool, payload: &[u8]) -> Vec<u8> {
        let mut out = vec![(if fin { 0x80 } else { 0 }) | opcode, 0x80 | payload.len() as u8, 1, 2, 3, 4];
        out.extend(payload.iter().enumerate().map(|(i, b)| b ^ [1u8, 2, 3, 4][i & 3]));
        out
    }

    #[test]
    fn a_chunk_holding_two_frames_reads_both_in_order() {
        let mut both = masked(1, true, b"a");
        both.extend(masked(9, true, b"b"));
        let got = Receiver::new().push(&both);
        assert_eq!(got, vec![Ok(Inbound::Text("a".into())), Ok(Inbound::Ping(b"b".to_vec()))]);
    }

    #[test]
    fn a_frame_split_over_chunks_reads_once_whole() {
        let bytes = masked(1, true, b"hello");
        let mut receiver = Receiver::new();
        assert!(receiver.push(&bytes[..3]).is_empty());
        assert_eq!(receiver.push(&bytes[3..]), vec![Ok(Inbound::Text("hello".into()))]);
    }

    #[test]
    fn fragments_join_and_a_control_frame_may_sit_between() {
        let mut bytes = masked(1, false, b"he");
        bytes.extend(masked(9, true, b"p"));
        bytes.extend(masked(0, true, b"llo"));
        let got = Receiver::new().push(&bytes);
        assert_eq!(got, vec![Ok(Inbound::Ping(b"p".to_vec())), Ok(Inbound::Text("hello".into()))]);
    }

    #[test]
    fn each_wire_fault_has_ws_code_and_sentence() {
        let unmasked = [0x81, 0x01, b'x'];
        let cases: [(Vec<u8>, u16, &str); 6] = [
            (unmasked.to_vec(), 1002, "Invalid WebSocket frame: MASK must be set"),
            (masked(3, true, b"x"), 1002, "Invalid WebSocket frame: invalid opcode 3"),
            (masked(1, true, &[0xff, 0xfe]), 1007, "Invalid WebSocket frame: invalid UTF-8 sequence"),
            (masked(8, true, &1005u16.to_be_bytes()), 1002, "Invalid WebSocket frame: invalid status code 1005"),
            (masked(0, true, b"x"), 1002, "Invalid WebSocket frame: invalid opcode 0"),
            (masked(9, false, b"x"), 1002, "Invalid WebSocket frame: FIN must be set"),
        ];
        for (bytes, code, sentence) in cases {
            let got = Receiver::new().push(&bytes);
            assert_eq!(got, vec![Err(Fault { code, sentence: sentence.to_string() })], "{sentence}");
        }
    }

    #[test]
    fn a_length_past_the_ceiling_is_refused_before_its_payload_comes() {
        let mut head = vec![0x81, 0x80 | 127];
        head.extend_from_slice(&((MAX_PAYLOAD as u64) + 1).to_be_bytes());
        let got = Receiver::new().push(&head);
        assert_eq!(got, vec![Err(Fault { code: 1009, sentence: "Max payload size exceeded".into() })]);
    }

    #[test]
    fn a_close_ends_the_receiver() {
        let mut bytes = masked(8, true, &[0x03, 0xe8, b'o', b'k']);
        bytes.extend(masked(1, true, b"late"));
        let mut receiver = Receiver::new();
        assert_eq!(receiver.push(&bytes), vec![Ok(Inbound::Close(1000, "ok".into()))]);
        assert!(receiver.push(&masked(1, true, b"later")).is_empty());
    }

    #[test]
    fn the_handshake_checks_run_in_ws_order() {
        let h = |pairs: &[(&str, &str)]| pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect::<Vec<_>>();
        let key = ("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ==");
        let said = |method, headers: &[(&str, &str)]| check_upgrade(method, &h(headers)).err().map(|r| (r.status, r.message));
        assert_eq!(said("POST", &[]), Some((405, "Invalid HTTP method")));
        assert_eq!(said("GET", &[("upgrade", "h2c"), key]), Some((400, "Invalid Upgrade header")));
        assert_eq!(said("GET", &[("upgrade", "websocket")]), Some((400, "Missing or invalid Sec-WebSocket-Key header")));
        assert_eq!(said("GET", &[("upgrade", "WebSocket"), key]), Some((400, "Missing or invalid Sec-WebSocket-Version header")));
        let ok = check_upgrade(
            "GET",
            &h(&[("upgrade", "websocket"), key, ("sec-websocket-version", "13"), ("sec-websocket-protocol", "wsp, other")]),
        );
        assert_eq!(ok.ok().and_then(|a| a.protocol), Some("wsp".to_string()));
    }

    #[test]
    fn a_head_reads_to_its_blank_line_and_leaves_the_rest() {
        let bytes = b"GET /ws?a=1 HTTP/1.1\r\nHost: x\r\nUpgrade: websocket\r\n\r\n\x81\x80";
        let (head, took) = Head::read(bytes).unwrap().unwrap();
        assert_eq!((head.method.as_str(), head.target.as_str(), took), ("GET", "/ws?a=1", bytes.len() - 2));
        assert_eq!(head.headers[1], ("upgrade".to_string(), "websocket".to_string()));
        assert!(Head::read(b"GET /ws HTTP/1.1\r\nHost").unwrap().is_none());
        assert!(Head::read(b"nonsense\r\n\r\n").is_err());
    }
}
