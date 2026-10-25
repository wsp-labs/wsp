// SPDX-License-Identifier: AGPL-3.0-only
//! A tape: the frames one connection sent a host, and, where the tape writes them down, the answers it expects.
//! Each line is one JSON object: `{"meta": {...}}` first and at most once, then `{"in": frame}` for a frame the host
//! receives and `{"out": frame}` for one it sends, exactly as the door carries them, and `{"close": {code, reason}}`
//! where the host closes the socket.

use serde_json::Value;
use std::fs;
use std::path::Path;

/// The folder under the repository root that holds recorded tapes.
pub const TAPES_DIR: &str = "parity/tapes";
/// The folder whose markdown files hold tapes as fenced blocks with the language `tape`.
pub const SPEC_DIR: &str = "docs/rust-host-spec";

/// The way into a host a tape went through.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Door {
    /// The host's websocket, where the app, the command line and the tools send their ops.
    Ws,
    /// The tool server on stdio, one JSON-RPC message a line.
    Mcp,
    /// The command line: each frame in is `{"argv": [...]}`, each answer `{"exit", "stdout", "stderr"}`.
    Cli,
}

impl Door {
    pub fn parse(word: &str) -> Option<Door> {
        match word {
            "ws" => Some(Door::Ws),
            "mcp" => Some(Door::Mcp),
            "cli" => Some(Door::Cli),
            _ => None,
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            Door::Ws => "ws",
            Door::Mcp => "mcp",
            Door::Cli => "cli",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Tape {
    /// Its path under the tapes folder less `.tape`, or `spec/<file>/<n>` for the n-th block of a spec file.
    pub id: String,
    pub door: Door,
    pub source: String,
    pub ins: Vec<Value>,
    /// The answers the tape writes down itself, as a spec example does; each is held to the golden as well, unless
    /// `recorded` says they are what the recording's own host answered.
    pub outs: Vec<Value>,
    /// Set by `"outs": "recorded"` in the meta line: the answers came from the host the tape was recorded on, which a
    /// test had set up as no fresh host is, so a replay follows the ids they name and holds nothing to them.
    pub recorded: bool,
}

impl Tape {
    /// Reads a tape's lines; `origin` names where they came from in a refusal, and each refusal names its line.
    pub fn parse(id: &str, text: &str, origin: &str, first_line: usize) -> Result<Tape, String> {
        let mut door = Door::Ws;
        let mut source = String::from(origin);
        let mut ins = Vec::new();
        let mut outs = Vec::new();
        let mut seen_frame = false;
        let mut recorded = false;
        for (n, line) in text.lines().enumerate() {
            let at = format!("{origin}:{}", first_line + n);
            if line.trim().is_empty() {
                continue;
            }
            let value: Value = serde_json::from_str(line).map_err(|e| format!("{at}: not a JSON line: {e}"))?;
            let Some(obj) = value.as_object().filter(|o| o.len() == 1) else {
                return Err(format!("{at}: a tape line is one object with one key, meta, in, out or close"));
            };
            let (key, frame) = obj.iter().next().expect("one key");
            match key.as_str() {
                "meta" if !seen_frame => {
                    if let Some(word) = frame.get("door").and_then(Value::as_str) {
                        door = Door::parse(word).ok_or_else(|| format!("{at}: no door is called {word}; the doors are ws, mcp and cli"))?;
                    }
                    if let Some(s) = frame.get("source").and_then(Value::as_str) {
                        source = s.to_string();
                    }
                    recorded = frame.get("outs").and_then(Value::as_str) == Some("recorded");
                }
                "meta" => return Err(format!("{at}: meta comes before the first frame")),
                "in" => {
                    seen_frame = true;
                    ins.push(frame.clone());
                }
                "out" => {
                    seen_frame = true;
                    outs.push(frame.clone());
                }
                // The host closing the socket, as the spec checker writes it: an answer with no request.
                "close" => {
                    seen_frame = true;
                    outs.push(value.clone());
                }
                other => return Err(format!("{at}: {other} is not a tape key; a line is meta, in, out or close")),
            }
        }
        if ins.is_empty() {
            return Err(format!("{origin}: a tape sends the host at least one frame"));
        }
        Ok(Tape { id: id.to_string(), door, source, ins, outs, recorded })
    }

    /// The ops the tape sends over the websocket, in order, repeats kept.
    pub fn ops(&self) -> Vec<String> {
        if self.door != Door::Ws {
            return Vec::new();
        }
        self.ins.iter().filter_map(|f| f.get("op").and_then(Value::as_str)).map(str::to_string).collect()
    }
}

/// Every fenced block with the language `tape` in one markdown file, in order, as `spec/<stem>/<n>` counting from 1.
pub fn spec_tapes(path: &Path) -> Result<Vec<Tape>, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("spec");
    let origin = path.display().to_string();
    let mut tapes = Vec::new();
    let mut block: Option<(usize, String)> = None;
    for (n, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        match &mut block {
            None if trimmed.starts_with("```") && trimmed.trim_start_matches('`').trim() == "tape" => block = Some((n + 2, String::new())),
            None => {}
            Some(_) if trimmed.starts_with("```") && trimmed.trim_start_matches('`').trim().is_empty() => {
                let (first, body) = block.take().expect("open block");
                let id = format!("spec/{stem}/{}", tapes.len() + 1);
                tapes.push(Tape::parse(&id, &body, &origin, first)?);
            }
            Some((_, body)) => {
                body.push_str(line);
                body.push('\n');
            }
        }
    }
    if block.is_some() {
        return Err(format!("{origin}: a tape block is never closed"));
    }
    Ok(tapes)
}

/// Every tape the repository holds: the recorded ones under `parity/tapes` and the spec's examples, sorted by id.
pub fn load(root: &Path) -> Result<Vec<Tape>, String> {
    let mut tapes = Vec::new();
    let dir = root.join(TAPES_DIR);
    let mut files = Vec::new();
    walk(&dir, &mut files);
    for file in files.iter().filter(|f| f.extension().is_some_and(|e| e == "tape")) {
        let rel = file.strip_prefix(&dir).expect("under the tapes folder");
        let id = rel.with_extension("").to_string_lossy().replace('\\', "/");
        let text = fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
        tapes.push(Tape::parse(&id, &text, &format!("{TAPES_DIR}/{}", rel.display()), 1)?);
    }
    let mut specs = Vec::new();
    walk(&root.join(SPEC_DIR), &mut specs);
    for file in specs.iter().filter(|f| f.extension().is_some_and(|e| e == "md")) {
        tapes.extend(spec_tapes(file)?);
    }
    tapes.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(tapes)
}

fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = entries.flatten().map(|e| e.path()).collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            walk(&path, out);
        } else {
            out.push(path);
        }
    }
}
