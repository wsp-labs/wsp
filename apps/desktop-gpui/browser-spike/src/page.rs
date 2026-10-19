// SPDX-License-Identifier: AGPL-3.0-only
//! The page the spike serves itself on 127.0.0.1, standing in for a dev server preview. It reports what it sees
//! (keys, its field's value, its scroll rate) to the app through wry's IPC, which the app prints as `page: ...`.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;

const PAGE: &str = r#"<!doctype html>
<html><head><meta charset="utf-8"><title>spike page</title>
<style>
  body { font: 15px system-ui, sans-serif; margin: 0; }
  header { position: sticky; top: 0; background: #2563eb; color: white; padding: 12px 16px; }
  input { font: inherit; padding: 6px 8px; width: 220px; }
  .row { padding: 10px 16px; border-bottom: 1px solid #ddd; }
  .row:nth-child(odd) { background: #f4f4f5; }
</style></head>
<body>
<header><input id="field" placeholder="type here"> Local preview on <span id="origin"></span></header>
<div id="rows"></div>
<script>
  const post = (m) => window.ipc && window.ipc.postMessage(m);
  document.getElementById('origin').textContent = location.host;
  const rows = document.getElementById('rows');
  for (let i = 0; i < 400; i++) {
    const d = document.createElement('div');
    d.className = 'row';
    d.textContent = 'Row ' + i;
    rows.appendChild(d);
  }
  const field = document.getElementById('field');
  document.addEventListener('keydown', (e) => {
    const mods = (e.ctrlKey ? 'ctrl-' : '') + (e.altKey ? 'alt-' : '') + (e.metaKey ? 'cmd-' : '') + (e.shiftKey ? 'shift-' : '');
    post('key ' + mods + e.key);
  });
  field.addEventListener('input', () => post('field ' + field.value));
  window.addEventListener('focus', () => post('focus'));
  window.addEventListener('blur', () => post('blur'));
  // Frames a second in which the scroll moved; the loop stops after a still second, so a page at rest draws nothing.
  let last = window.scrollY, moved = 0, frames = 0, since = 0, running = false;
  function tick(now) {
    frames++;
    if (window.scrollY !== last) { moved++; last = window.scrollY; }
    if (now - since >= 1000) {
      if (moved === 0) { running = false; return; }
      post('scroll-fps ' + Math.round(moved * 1000 / (now - since)) + ' raf-fps ' + Math.round(frames * 1000 / (now - since)) + ' y ' + Math.round(last));
      moved = 0; frames = 0; since = now;
    }
    requestAnimationFrame(tick);
  }
  window.addEventListener('scroll', () => {
    if (running) return;
    running = true; moved = 0; frames = 0; since = performance.now(); last = window.scrollY;
    requestAnimationFrame(tick);
  });
  window.addEventListener('load', () => post('loaded ' + location.href + ' ' + innerWidth + 'x' + innerHeight));
  window.addEventListener('resize', () => post('viewport ' + innerWidth + 'x' + innerHeight));
</script>
</body></html>"#;

/// Serves the page on an ephemeral port of 127.0.0.1 from a thread of its own and returns its address.
pub fn serve() -> std::io::Result<String> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let address = format!("http://{}/", listener.local_addr()?);
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut reader = BufReader::new(&stream);
            let mut line = String::new();
            while reader.read_line(&mut line).is_ok_and(|n| n > 2) {
                line.clear();
            }
            let mut stream = &stream;
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{PAGE}",
                PAGE.len()
            );
        }
    });
    Ok(address)
}
