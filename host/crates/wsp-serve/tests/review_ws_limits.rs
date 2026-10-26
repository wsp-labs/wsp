// SPDX-License-Identifier: AGPL-3.0-only
//! Re-review of wsframe at bd748d77a: two limits ws's server hands every Receiver that wsframe::Receiver does not
//! count. `WebSocketServer` passes `maxFragments` 16384 and `maxBufferedChunks` 262144 to each Receiver
//! (ws 8.21.3 `lib/websocket-server.js:72-73, 433-434`); the Receiver defaults both to 0, which is "no limit"
//! (`lib/receiver.js:61-62`), so a Receiver built without them (as wsframe is) enforces neither. Both are reached on
//! the production frame path: the listener drives this same Receiver whether the upgrade came on the own port or
//! through `attach`. Each test is red at bd748d77a. A frame's bytes stay masked here, as a client sends them.
use wsp_serve::serve::wsframe::{Fault, Inbound, Receiver};

fn masked_byte(opcode: u8, fin: bool, b: u8) -> Vec<u8> {
    vec![(if fin { 0x80 } else { 0 }) | opcode, 0x81, 1, 2, 3, 4, b ^ 1]
}

// MINOR (ws parity; reachable only by an authed socket past the 4,096-byte pre-auth cap, bounded by the 100 MiB
// payload ceiling, no crash): a message in more than 16,384 fragments. ws closes the socket 1008 "Too many message
// fragments" (receiver.js:504-509, `++this._numFragments > this._maxFragments`); wsframe keeps counting none and
// delivers the joined message. The fragment count is independent of how the bytes are chunked, so the listener
// reaches this whether the fragments arrive in one read or many. Fix: count fragments to 16,384 and answer the same
// 1008 and sentence.
#[test]
fn a_message_past_sixteen_thousand_fragments_is_closed_1008() {
    let mut bytes = masked_byte(1, false, b'a');
    for _ in 0..16_383 {
        bytes.extend(masked_byte(0, false, b'a'));
    }
    bytes.extend(masked_byte(0, true, b'a')); // the 16,385th fragment
    let got = Receiver::new().push(&bytes);
    assert_eq!(
        got,
        vec![Err(Fault { code: 1008, sentence: "Too many message fragments".into() })],
        "ws closes 1008 at the 16,385th fragment; wsframe delivered {got:?}"
    );
}

// Control, green at bd748d77a: exactly 16,384 fragments is at the limit and ws (and wsframe) deliver the message.
#[test]
fn control_sixteen_thousand_three_eighty_four_fragments_is_delivered() {
    let mut bytes = masked_byte(1, false, b'a');
    for _ in 0..16_382 {
        bytes.extend(masked_byte(0, false, b'a'));
    }
    bytes.extend(masked_byte(0, true, b'a'));
    let got = Receiver::new().push(&bytes);
    assert_eq!(got, vec![Ok(Inbound::Text("a".repeat(16_384)))]);
}

// MINOR (ws parity; reachable by a client dribbling one frame in many reads, bounded by the 100 MiB ceiling, no
// crash): a single frame whose payload is delivered in more than 262,144 chunks. ws closes 1008 "Too many buffered
// chunks" (receiver.js:100-108, `this._buffers.length >= this._maxBufferedChunks`); wsframe appends every chunk to
// one buffer, counts nothing, and delivers the frame. The listener calls `push` once per socket read, so this needs
// many tiny reads of one unfinished frame. Fix: count the reads buffered toward one frame to 262,144 and answer the
// same 1008 and sentence.
#[test]
fn a_frame_buffered_in_too_many_chunks_is_closed_1008() {
    // A 300,000-byte binary frame: 10-byte header + 4-byte mask, then the body one byte at a time.
    let n: usize = 300_000;
    let mut header = vec![0x82u8, 0x80 | 127];
    header.extend_from_slice(&(n as u64).to_be_bytes());
    header.extend_from_slice(&[1, 2, 3, 4]);
    let mut receiver = Receiver::new();
    let mut events = Vec::new();
    events.extend(receiver.push(&header));
    for i in 0..n {
        events.extend(receiver.push(&[(i as u8) ^ [1u8, 2, 3, 4][i & 3]]));
    }
    assert_eq!(
        events,
        vec![Err(Fault { code: 1008, sentence: "Too many buffered chunks".into() })],
        "ws closes 1008 after 262,144 buffered chunks; wsframe delivered {events:?}"
    );
}
