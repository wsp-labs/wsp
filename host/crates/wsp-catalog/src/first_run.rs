// SPDX-License-Identifier: AGPL-3.0-only
//! A command on PATH that installs its agent the first time it runs. Port of packages/catalog/src/first-run.ts at
//! a3b870a6e; the spec is spec/seed.md.

/// One module per installer (first-run.ts:268, 284).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirstRunInstaller {
    Mise,
    LazyRun,
}

/// mise, then lazy-run (first-run.ts:299).
pub const FIRST_RUN_INSTALLERS: [FirstRunInstaller; 2] = [FirstRunInstaller::Mise, FirstRunInstaller::LazyRun];

impl FirstRunInstaller {
    pub fn id(self) -> &'static str {
        todo!()
    }

    /// The tool the script installs, off its text (first-run.ts:270, 286).
    pub fn tool(self, script: &str) -> Option<String> {
        todo!()
    }

    /// A shell test that passes once the tool is installed (first-run.ts:278, 290).
    pub fn installed(self, tool: &str) -> String {
        todo!()
    }
}

/// mise's folder name for a tool spec (first-run.ts:256).
pub fn mise_folder(spec: &str) -> String {
    todo!()
}

/// Runs one sh script, answering its stdout, or nothing where it failed (first-run.ts:302).
#[allow(async_fn_in_trait)]
pub trait RunScript {
    async fn run(&self, script: &str) -> Option<String>;
}

/// Whether each bin's command is a first-run installer with its run to come (first-run.ts:311): one script, and a
/// second only where a wrapper was found; false for every bin where a script could not run.
pub async fn installs_on_first_run(run: &impl RunScript, bins: &[String]) -> Vec<bool> {
    todo!()
}
