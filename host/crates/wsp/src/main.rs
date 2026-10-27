// SPDX-License-Identifier: AGPL-3.0-only
fn main() {
    std::process::exit(wsp_host::run(std::env::args().skip(1).collect()));
}
