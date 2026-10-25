// SPDX-License-Identifier: AGPL-3.0-only
//! One row per ecosystem for the files a folder seeds a project with, and the install each runs. Port of
//! packages/catalog/src/seed.ts at a3b870a6e; the spec is spec/seed.md.
use std::sync::LazyLock;

use indexmap::IndexMap;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SeedKind {
    Config,
    Secret,
    Rebuilt,
    Data,
    Never,
    Junk,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SeedStore {
    pub flag: String,
    pub dir: String,
}

/// seed.ts:22.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SeedInstall {
    pub lockfile: String,
    pub run: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store: Option<SeedStore>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub check: Option<String>,
}

/// seed.ts:39.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SeedRow {
    pub id: String,
    pub name: String,
    pub patterns: IndexMap<SeedKind, Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub except: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opens: Option<Vec<String>>,
    pub examples: IndexMap<SeedKind, Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installs: Option<Vec<SeedInstall>>,
}

/// seed.ts:59, in the order a path is judged by.
pub static SEED_ROWS: LazyLock<Vec<SeedRow>> = LazyLock::new(|| {
    vec![
        SeedRow {
            id: "logins".into(),
            name: "logins".into(),
            patterns: IndexMap::from([(
                SeedKind::Never,
                strs(&[
                    ".credentials.json",
                    "auth.json",
                    ".git-credentials",
                    ".netrc",
                    "hosts.yml",
                    ".aws/credentials",
                ]),
            )]),
            except: None,
            opens: None,
            examples: IndexMap::from([(
                SeedKind::Never,
                strs(&[
                    ".credentials.json",
                    ".config/gh/hosts.yml",
                    ".aws/credentials",
                ]),
            )]),
            installs: None,
        },
        SeedRow {
            id: "finder".into(),
            name: "Finder metadata".into(),
            patterns: IndexMap::from([(SeedKind::Junk, strs(&[".DS_Store", "Thumbs.db"]))]),
            except: None,
            opens: None,
            examples: IndexMap::from([(
                SeedKind::Junk,
                strs(&[".DS_Store", "components/.DS_Store"]),
            )]),
            installs: None,
        },
        SeedRow {
            id: "next".into(),
            name: "Next".into(),
            patterns: IndexMap::from([
                (SeedKind::Secret, strs(&[".env*.local"])),
                (SeedKind::Rebuilt, strs(&[".next/", "next-env.d.ts"])),
            ]),
            except: None,
            opens: None,
            examples: IndexMap::from([
                (
                    SeedKind::Secret,
                    strs(&[".env.local", ".env.development.local"]),
                ),
                (SeedKind::Rebuilt, strs(&[".next", "next-env.d.ts"])),
            ]),
            installs: None,
        },
        SeedRow {
            id: "node".into(),
            name: "Node".into(),
            patterns: IndexMap::from([
                (SeedKind::Secret, strs(&[".env", ".env.*", ".npmrc"])),
                (
                    SeedKind::Rebuilt,
                    strs(&[
                        "node_modules/",
                        ".pnpm-store/",
                        ".cache/",
                        ".turbo/",
                        "dist/",
                        "build/",
                        "out/",
                        "coverage/",
                        "*.tsbuildinfo",
                    ]),
                ),
            ]),
            except: Some(strs(&[".env.example", ".env.sample"])),
            opens: None,
            examples: IndexMap::from([
                (
                    SeedKind::Secret,
                    strs(&[".env", ".env.production", "apps/web/.npmrc"]),
                ),
                (
                    SeedKind::Rebuilt,
                    strs(&["node_modules", "packages/host/dist", "tsconfig.tsbuildinfo"]),
                ),
            ]),
            installs: Some(vec![
                SeedInstall {
                    lockfile: "package-lock.json".into(),
                    run: "npm ci".into(),
                    store: None,
                    check: Some("npm --version".into()),
                },
                SeedInstall {
                    lockfile: "pnpm-lock.yaml".into(),
                    run: "pnpm install --frozen-lockfile".into(),
                    store: Some(SeedStore {
                        flag: "--store-dir".into(),
                        dir: ".pnpm-store".into(),
                    }),
                    check: Some("pnpm --version".into()),
                },
                SeedInstall {
                    lockfile: "yarn.lock".into(),
                    run: "yarn install --frozen-lockfile".into(),
                    store: None,
                    check: Some("yarn --version".into()),
                },
                SeedInstall {
                    lockfile: "bun.lock".into(),
                    run: "bun install --frozen-lockfile".into(),
                    store: None,
                    check: Some("bun --version".into()),
                },
                SeedInstall {
                    lockfile: "bun.lockb".into(),
                    run: "bun install --frozen-lockfile".into(),
                    store: None,
                    check: Some("bun --version".into()),
                },
            ]),
        },
        SeedRow {
            id: "python".into(),
            name: "Python".into(),
            patterns: IndexMap::from([
                (SeedKind::Secret, strs(&[".envrc", ".pypirc"])),
                (SeedKind::Config, strs(&["local_settings.py"])),
                (
                    SeedKind::Rebuilt,
                    strs(&[
                        ".venv/",
                        "venv/",
                        "__pycache__/",
                        ".pytest_cache/",
                        ".mypy_cache/",
                        ".ruff_cache/",
                        "*.egg-info/",
                    ]),
                ),
                (SeedKind::Data, strs(&["db.sqlite3"])),
            ]),
            except: None,
            opens: None,
            examples: IndexMap::from([
                (SeedKind::Secret, strs(&[".envrc", ".pypirc"])),
                (SeedKind::Config, strs(&["app/local_settings.py"])),
                (
                    SeedKind::Rebuilt,
                    strs(&[".venv", "app/__pycache__", "wsp.egg-info"]),
                ),
                (SeedKind::Data, strs(&["db.sqlite3"])),
            ]),
            installs: Some(vec![
                SeedInstall {
                    lockfile: "uv.lock".into(),
                    run: "uv sync --frozen".into(),
                    store: None,
                    check: None,
                },
                SeedInstall {
                    lockfile: "poetry.lock".into(),
                    run: "poetry install".into(),
                    store: None,
                    check: None,
                },
            ]),
        },
        SeedRow {
            id: "rails".into(),
            name: "Rails".into(),
            patterns: IndexMap::from([
                (
                    SeedKind::Secret,
                    strs(&["config/master.key", "config/credentials/*.key"]),
                ),
                (SeedKind::Rebuilt, strs(&["tmp/", "log/"])),
            ]),
            except: None,
            opens: None,
            examples: IndexMap::from([
                (
                    SeedKind::Secret,
                    strs(&["config/master.key", "config/credentials/production.key"]),
                ),
                (SeedKind::Rebuilt, strs(&["tmp", "log"])),
            ]),
            installs: None,
        },
        SeedRow {
            id: "rust".into(),
            name: "Rust".into(),
            patterns: IndexMap::from([(SeedKind::Rebuilt, strs(&["target/"]))]),
            except: None,
            opens: None,
            examples: IndexMap::from([(SeedKind::Rebuilt, strs(&["target", "daemon/target"]))]),
            installs: Some(vec![SeedInstall {
                lockfile: "Cargo.lock".into(),
                run: "cargo fetch --locked".into(),
                store: None,
                check: None,
            }]),
        },
        SeedRow {
            id: "go".into(),
            name: "Go".into(),
            patterns: IndexMap::new(),
            except: None,
            opens: None,
            examples: IndexMap::new(),
            installs: Some(vec![SeedInstall {
                lockfile: "go.sum".into(),
                run: "go mod download".into(),
                store: None,
                check: None,
            }]),
        },
        SeedRow {
            id: "vercel".into(),
            name: "Vercel".into(),
            patterns: IndexMap::from([(SeedKind::Config, strs(&[".vercel/"]))]),
            except: None,
            opens: None,
            examples: IndexMap::from([(SeedKind::Config, strs(&[".vercel"]))]),
            installs: None,
        },
        SeedRow {
            id: "terraform".into(),
            name: "Terraform".into(),
            patterns: IndexMap::from([
                (SeedKind::Secret, strs(&["*.tfvars", "*.auto.tfvars"])),
                (SeedKind::Rebuilt, strs(&[".terraform/"])),
            ]),
            except: None,
            opens: None,
            examples: IndexMap::from([
                (
                    SeedKind::Secret,
                    strs(&["prod.tfvars", "infra/stage.auto.tfvars"]),
                ),
                (SeedKind::Rebuilt, strs(&[".terraform"])),
            ]),
            installs: None,
        },
        SeedRow {
            id: "android".into(),
            name: "Android".into(),
            patterns: IndexMap::from([
                (SeedKind::Config, strs(&["local.properties"])),
                (SeedKind::Secret, strs(&["*.keystore", "*.jks"])),
                (SeedKind::Rebuilt, strs(&[".gradle/"])),
            ]),
            except: None,
            opens: None,
            examples: IndexMap::from([
                (SeedKind::Config, strs(&["local.properties"])),
                (
                    SeedKind::Secret,
                    strs(&["app/release.keystore", "app/upload.jks"]),
                ),
                (SeedKind::Rebuilt, strs(&[".gradle"])),
            ]),
            installs: None,
        },
        SeedRow {
            id: "certificates".into(),
            name: "certificates".into(),
            patterns: IndexMap::from([
                (SeedKind::Secret, strs(&["*.pem", "*.key", "*.p12"])),
                (SeedKind::Config, strs(&["*.crt"])),
            ]),
            except: None,
            opens: None,
            examples: IndexMap::from([
                (
                    SeedKind::Secret,
                    strs(&["certs/local.pem", "certs/local.key"]),
                ),
                (SeedKind::Config, strs(&["certs/local.crt"])),
            ]),
            installs: None,
        },
        SeedRow {
            id: "agents".into(),
            name: "agents".into(),
            patterns: IndexMap::from([
                (
                    SeedKind::Config,
                    strs(&[".claude/settings.local.json", "CLAUDE.local.md"]),
                ),
                (SeedKind::Rebuilt, strs(&[".claude/.cc-writes/"])),
            ]),
            except: None,
            opens: Some(strs(&[".claude/"])),
            examples: IndexMap::from([
                (
                    SeedKind::Config,
                    strs(&[".claude/settings.local.json", "CLAUDE.local.md"]),
                ),
                (SeedKind::Rebuilt, strs(&[".claude/.cc-writes"])),
            ]),
            installs: None,
        },
        SeedRow {
            id: "databases".into(),
            name: "databases".into(),
            patterns: IndexMap::from([(
                SeedKind::Data,
                strs(&[
                    "*.sqlite",
                    "*.sqlite3",
                    "*.db",
                    "*.duckdb",
                    "pgdata/",
                    "postgres-data/",
                    "mongo-data/",
                    "redis-data/",
                ]),
            )]),
            except: None,
            opens: None,
            examples: IndexMap::from([(
                SeedKind::Data,
                strs(&["dev.db", "var/app.sqlite3", "pgdata"]),
            )]),
            installs: None,
        },
    ]
});

const KINDS: [SeedKind; 6] = [
    SeedKind::Never,
    SeedKind::Junk,
    SeedKind::Secret,
    SeedKind::Config,
    SeedKind::Rebuilt,
    SeedKind::Data,
];

fn strs(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| item.to_string()).collect()
}

/// `*` is any run of characters, every other character literal (a segment never holds a slash).
fn glob_segment(pattern: &str, text: &str) -> bool {
    let pieces: Vec<&str> = pattern.split('*').collect();
    let (first, last) = (pieces[0], pieces[pieces.len() - 1]);
    if pieces.len() == 1 {
        return text == first;
    }
    let Some(mut rest) = text.strip_prefix(first) else {
        return false;
    };
    let Some(stripped) = rest.strip_suffix(last) else {
        return false;
    };
    rest = stripped;
    for middle in &pieces[1..pieces.len() - 1] {
        match rest.find(middle) {
            Some(at) => rest = &rest[at + middle.len()..],
            None => return false,
        }
    }
    true
}

/// seed.ts:182.
fn pattern_matches(pattern: &str, parts: &[&str], dir: bool) -> bool {
    let (want, wants_dir) = match pattern.strip_suffix('/') {
        Some(want) => (want, true),
        None => (pattern, false),
    };
    if wants_dir && !dir {
        return false;
    }
    let tail: Vec<&str> = want.split('/').collect();
    if tail.len() == 1 {
        return parts.iter().any(|part| glob_segment(tail[0], part));
    }
    if tail.len() > parts.len() {
        return false;
    }
    let at = parts.len() - tail.len();
    tail.iter()
        .enumerate()
        .all(|(i, piece)| glob_segment(piece, parts[at + i]))
}

/// The row and kind of one ignored path, kinds in never, junk, secret, config, rebuilt, data order and rows in table
/// order within each (seed.ts:203).
pub fn seed_row_for(path: &str, dir: bool) -> Option<(&'static SeedRow, SeedKind)> {
    let trimmed = path
        .strip_prefix("./")
        .unwrap_or(path)
        .trim_end_matches('/');
    let parts: Vec<&str> = trimmed.split('/').collect();
    for kind in KINDS {
        for row in SEED_ROWS.iter() {
            if row
                .except
                .iter()
                .flatten()
                .any(|pattern| pattern_matches(pattern, &parts, dir))
            {
                continue;
            }
            if row.patterns.get(&kind).is_some_and(|patterns| {
                patterns
                    .iter()
                    .any(|pattern| pattern_matches(pattern, &parts, dir))
            }) {
                return Some((row, kind));
            }
        }
    }
    None
}

/// The installs the root names, one per row, the first listed lockfile there winning (seed.ts:217).
pub fn seed_installs_for(root_names: &[String]) -> Vec<(&'static SeedRow, &'static SeedInstall)> {
    SEED_ROWS
        .iter()
        .filter_map(|row| {
            let installs = row.installs.as_ref()?;
            installs
                .iter()
                .find(|install| root_names.contains(&install.lockfile))
                .map(|install| (row, install))
        })
        .collect()
}
