// SPDX-License-Identifier: AGPL-3.0-only
//! gpui-agent is in the app's binary only with the `agent` feature, which no release script turns on: built without it,
//! `nm` finds no symbol of the gpui_agent crate; built with it, the same reading finds them, so the check can see them.

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

#[test]
fn gpui_agent_symbols_are_in_the_binary_only_with_the_agent_feature() {
    let bin = env!("CARGO_BIN_EXE_desktop-gpui");
    let mut nm = Command::new("nm").arg(bin).stdout(Stdio::piped()).spawn().expect("nm on PATH");
    let (mut symbols, mut agent) = (0, 0);
    for line in BufReader::new(nm.stdout.take().unwrap()).lines() {
        symbols += 1;
        agent += usize::from(line.unwrap().contains("gpui_agent"));
    }
    assert!(nm.wait().unwrap().success(), "nm {bin} failed");
    assert!(symbols > 1000, "nm read {symbols} symbols in {bin}, too few for an unstripped build");
    if cfg!(feature = "agent") {
        assert!(agent > 0, "a build with the agent feature carries gpui_agent's symbols");
    } else {
        assert_eq!(agent, 0, "a build without the agent feature carries {agent} gpui_agent symbols");
    }
}
