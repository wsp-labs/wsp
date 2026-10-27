// SPDX-License-Identifier: AGPL-3.0-only
//! The terminal a line runs at: `terminalIO` of `cli/io.ts`. `log` is stdout and `error` stderr, a question is asked
//! only where a person is at the keyboard (stdin and stdout are terminals), and anywhere else it is refused.
use std::io::{BufRead, Write};

use async_trait::async_trait;
use wsp_cli::verbs::client::{CliIo, IoBytes};
use wsp_words::exit::HostError;

/// Where a key is told to be put by a line that cannot ask for it. `KEY_LAYER_WORDS` of env-keys.ts.
const KEY_LAYER_WORDS: &str = "the environment, the .env in this folder, or the .env beside the state file";

fn is_tty(fd: i32) -> bool {
    // SAFETY: isatty only reads the descriptor's kind.
    unsafe { libc::isatty(fd) == 1 }
}

pub struct TerminalIo {
    screen: bool,
    same_screen: bool,
}

impl TerminalIo {
    pub fn new() -> TerminalIo {
        TerminalIo { screen: is_tty(0) && is_tty(1), same_screen: is_tty(1) && is_tty(2) }
    }
}

impl Default for TerminalIo {
    fn default() -> Self {
        Self::new()
    }
}

fn nobody_line(question: &str, variable: Option<&str>) -> String {
    let first = question.split('\n').next().unwrap_or("");
    format!("{first}: no terminal to ask on; set {} in {KEY_LAYER_WORDS}.", variable.unwrap_or("it"))
}

fn read_answer() -> Result<String, HostError> {
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line).map_err(|e| HostError::new(e.to_string()))?;
    Ok(line.trim().to_string())
}

#[async_trait]
impl CliIo for TerminalIo {
    fn log(&self, line: &str) {
        let mut out = std::io::stdout().lock();
        let _ = writeln!(out, "{line}");
        let _ = out.flush();
    }

    fn error(&self, line: &str) {
        let mut err = std::io::stderr().lock();
        let _ = writeln!(err, "{line}");
        let _ = err.flush();
    }

    fn stream(&self, text: &str) {
        let mut err = std::io::stderr().lock();
        let _ = err.write_all(text.as_bytes());
        let _ = err.flush();
    }

    fn bytes(&self) -> Option<IoBytes> {
        Some(IoBytes { input: Box::new(tokio::io::stdin()), output: Box::new(tokio::io::stdout()) })
    }

    async fn ask(&self, question: &str) -> Result<String, HostError> {
        if !self.screen {
            return Err(HostError::new(nobody_line(question, None)));
        }
        let first = question.split('\n').next().unwrap_or("");
        self.stream(&format!("{first} (y/N) "));
        let said = read_answer()?.to_lowercase();
        Ok(if said == "y" || said == "yes" { "yes" } else { "no" }.to_string())
    }

    fn is_tty(&self) -> bool {
        self.screen
    }

    fn same_screen(&self) -> bool {
        self.same_screen
    }

    async fn ask_secret(&self, question: &str, variable: Option<&str>) -> Result<String, HostError> {
        if !self.screen {
            return Err(wsp_words::exit::auth_refusal(&nobody_line(question, variable)));
        }
        let first = question.split('\n').next().unwrap_or("");
        self.stream(&format!("{first}: "));
        read_answer()
    }
}
