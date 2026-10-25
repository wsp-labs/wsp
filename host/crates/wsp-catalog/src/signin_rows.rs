// SPDX-License-Identifier: AGPL-3.0-only
//! The sign-in rows and everything derived from them: the shared logins, the paths no image or brokered machine may
//! hold, and the vault's variables. Port of packages/catalog/src/signin.ts:343-594 at a3b870a6e; spec/signin.md.
use std::sync::LazyLock;

use indexmap::{IndexMap, IndexSet};
use serde::Serialize;
use wsp_wire::types::init_job::SignInFinish;

use crate::js_regex::JsRegex;
use crate::mcp_bearer::with_bearer_variables;
use crate::roads::{CLAUDE_CONFIG_REL, CLAUDE_KEY_FILE, GUEST_HOME};
use crate::signin::{
    key_env_of, login_state_paths, mints_token, shared_login_of, Headless, KeyFiles, LoginSignIn, LoginSource, NoSignIn, Question, Settle,
    SharedLogin, SignIn, SignInKind, StatusCheck, StatusReader, TokenSignIn, TokenThere, AWS_STATUS, CLOUDFLARED_STATUS, GEMINI_STATUS,
};

/// The rows by tool, in signin.ts:344's order: gh, gcloud, aws, wrangler, vercel, netlify, fly, supabase, railway,
/// doppler, claude, codex, gemini, opencode, cursor, cloudflared, op, kubectl, pi, hermes.
pub static SIGN_IN_ROWS: LazyLock<IndexMap<String, SignIn>> = LazyLock::new(build_rows);

fn build_rows() -> IndexMap<String, SignIn> {
    let mut rows = IndexMap::new();
    rows.insert(
        "gh".to_string(),
        SignIn::Login(LoginSignIn {
            kind: SignInKind::Device,
            login: "gh auth login --hostname github.com --git-protocol https --web --skip-ssh-key".to_string(),
            sources: vec![LoginSource::File, LoginSource::Keychain],
            fallback: None,
            headless: None,
            questions: Some(vec![
                Question { asks: JsRegex::new("Where do you use GitHub\\?"), settle: Settle::Flag("--hostname".to_string()) },
                Question {
                    asks: JsRegex::new("What is your preferred protocol for Git operations on this host\\?"),
                    settle: Settle::Flag("--git-protocol".to_string()),
                },
                Question {
                    asks: JsRegex::new("How would you like to authenticate GitHub CLI\\?"),
                    settle: Settle::Flag("--web".to_string()),
                },
                Question {
                    asks: JsRegex::new("(?:Generate a new SSH key to add to|Upload your SSH public key to) your GitHub account\\?"),
                    settle: Settle::Flag("--skip-ssh-key".to_string()),
                },
                Question { asks: JsRegex::new("Press \\[?Enter\\]? to (?:open|continue)"), settle: Settle::Answer("\r".to_string()) },
                Question {
                    asks: JsRegex::new("Authenticate Git with your GitHub credentials\\?"),
                    settle: Settle::Answer("\r".to_string()),
                },
            ]),
            finish: SignInFinish::None,
            code: Some(JsRegex::new("\\b[A-Z0-9]{4}-[A-Z0-9]{4}\\b")),
            key_env: None,
            status: Some(StatusCheck { command: "gh auth status".to_string(), typed: None, reader: StatusReader::Gh }),
            note: None,
            keys: None,
            shared: None,
            state_on_machine: vec![".config/gh/hosts.yml".to_string()],
        }),
    );
    rows.insert(
        "gcloud".to_string(),
        SignIn::Login(LoginSignIn {
            kind: SignInKind::Oauth,
            login: "gcloud auth login".to_string(),
            sources: vec![LoginSource::File],
            fallback: None,
            headless: None,
            questions: None,
            finish: SignInFinish::Callback,
            code: None,
            key_env: None,
            status: Some(StatusCheck {
                command: "gcloud auth list --filter=status:ACTIVE --format=value(account)".to_string(),
                typed: None,
                reader: StatusReader::Gcloud,
            }),
            note: None,
            keys: None,
            shared: None,
            state_on_machine: vec![".config/gcloud".to_string()],
        }),
    );
    rows.insert(
        "aws".to_string(),
        SignIn::Login(LoginSignIn {
            kind: SignInKind::Oauth,
            login: "aws configure sso".to_string(),
            sources: vec![LoginSource::File],
            fallback: Some("aws configure sso --use-device-code".to_string()),
            headless: Some(Headless::Device),
            questions: Some(vec![
                Question { asks: JsRegex::new("SSO session name \\(Recommended\\)"), settle: Settle::Person(true) },
                Question { asks: JsRegex::new("SSO start URL"), settle: Settle::Person(true) },
                Question { asks: JsRegex::new("SSO region"), settle: Settle::Person(true) },
            ]),
            finish: SignInFinish::Callback,
            code: None,
            key_env: None,
            status: Some(StatusCheck { command: AWS_STATUS.to_string(), typed: None, reader: StatusReader::Aws }),
            note: Some("the check tries every configured profile; the start URL and region it asks for are yours to type".to_string()),
            keys: None,
            shared: None,
            state_on_machine: vec![".aws".to_string()],
        }),
    );
    rows.insert(
        "wrangler".to_string(),
        SignIn::Login(LoginSignIn {
            kind: SignInKind::Oauth,
            login: "wrangler login".to_string(),
            sources: vec![LoginSource::File],
            fallback: None,
            headless: None,
            questions: None,
            finish: SignInFinish::Callback,
            code: None,
            key_env: None,
            status: Some(StatusCheck { command: "wrangler whoami".to_string(), typed: None, reader: StatusReader::Wrangler }),
            note: Some("needs the callback forward; CLOUDFLARE_API_TOKEN on the machine is the alternative".to_string()),
            keys: None,
            shared: None,
            state_on_machine: vec![".config/.wrangler".to_string()],
        }),
    );
    rows.insert(
        "vercel".to_string(),
        SignIn::Login(LoginSignIn {
            kind: SignInKind::Oauth,
            login: "vercel login".to_string(),
            sources: vec![LoginSource::File],
            fallback: None,
            headless: None,
            questions: None,
            finish: SignInFinish::None,
            code: None,
            key_env: None,
            status: Some(StatusCheck { command: "vercel whoami".to_string(), typed: None, reader: StatusReader::Vercel }),
            note: None,
            keys: None,
            shared: None,
            state_on_machine: vec![".config/com.vercel.cli".to_string(), ".vercel".to_string()],
        }),
    );
    rows.insert(
        "netlify".to_string(),
        SignIn::Login(LoginSignIn {
            kind: SignInKind::Oauth,
            login: "netlify login".to_string(),
            sources: vec![],
            fallback: None,
            headless: None,
            questions: None,
            finish: SignInFinish::None,
            code: None,
            key_env: None,
            status: Some(StatusCheck { command: "netlify status".to_string(), typed: None, reader: StatusReader::Netlify }),
            note: None,
            keys: None,
            shared: None,
            state_on_machine: vec![".netlify/config.json".to_string()],
        }),
    );
    rows.insert(
        "fly".to_string(),
        SignIn::Login(LoginSignIn {
            kind: SignInKind::Oauth,
            login: "fly auth login".to_string(),
            sources: vec![],
            fallback: None,
            headless: None,
            questions: None,
            finish: SignInFinish::None,
            code: None,
            key_env: None,
            status: Some(StatusCheck { command: "fly auth whoami".to_string(), typed: None, reader: StatusReader::Fly }),
            note: None,
            keys: None,
            shared: None,
            state_on_machine: vec![".fly".to_string()],
        }),
    );
    rows.insert(
        "supabase".to_string(),
        SignIn::Login(LoginSignIn {
            kind: SignInKind::Oauth,
            login: "supabase login".to_string(),
            sources: vec![],
            fallback: Some("supabase login --no-browser".to_string()),
            headless: None,
            questions: Some(vec![Question {
                asks: JsRegex::new("Press Enter to open browser and login automatically"),
                settle: Settle::Answer("\r".to_string()),
            }]),
            finish: SignInFinish::None,
            code: None,
            key_env: None,
            status: Some(StatusCheck { command: "supabase projects list".to_string(), typed: None, reader: StatusReader::Supabase }),
            note: None,
            keys: None,
            shared: None,
            state_on_machine: vec![".supabase".to_string()],
        }),
    );
    rows.insert(
        "railway".to_string(),
        SignIn::Login(LoginSignIn {
            kind: SignInKind::Oauth,
            login: "railway login".to_string(),
            sources: vec![],
            fallback: None,
            headless: None,
            questions: None,
            finish: SignInFinish::Callback,
            code: None,
            key_env: None,
            status: Some(StatusCheck { command: "railway whoami".to_string(), typed: None, reader: StatusReader::Railway }),
            note: None,
            keys: None,
            shared: None,
            state_on_machine: vec![".railway".to_string()],
        }),
    );
    rows.insert(
        "doppler".to_string(),
        SignIn::Login(LoginSignIn {
            kind: SignInKind::Oauth,
            login: "doppler login --yes".to_string(),
            sources: vec![],
            fallback: None,
            headless: None,
            questions: Some(vec![Question {
                asks: JsRegex::new("Open the authorization page in your browser\\?"),
                settle: Settle::Flag("--yes".to_string()),
            }]),
            finish: SignInFinish::None,
            code: Some(JsRegex::new("(?<=Your auth code is:\\s*)[a-z]+(?:_[a-z]+){2,}")),
            key_env: None,
            status: Some(StatusCheck { command: "doppler me".to_string(), typed: None, reader: StatusReader::Doppler }),
            note: None,
            keys: None,
            shared: None,
            state_on_machine: vec![".doppler".to_string()],
        }),
    );
    rows.insert(
        "claude".to_string(),
        SignIn::Token(TokenSignIn {
            kind: SignInKind::Token,
            mint: "claude setup-token".to_string(),
            token_env: "CLAUDE_CODE_OAUTH_TOKEN".to_string(),
            token: JsRegex::new("sk-ant-oat01-[A-Za-z0-9_-]{20,}"),
            key_env: Some("ANTHROPIC_API_KEY".to_string()),
            status: Some(StatusCheck { command: "claude auth status".to_string(), typed: None, reader: StatusReader::Claude }),
            note: None,
            there: Some(TokenThere {
                login: "claude auth login --claudeai".to_string(),
                finish: SignInFinish::Code,
                headless: Some(Headless::Code),
            }),
            sources: Vec::new(),
            state_on_machine: Vec::new(),
        }),
    );
    rows.insert(
        "codex".to_string(),
        SignIn::Login(LoginSignIn {
            kind: SignInKind::Oauth,
            login: "codex login".to_string(),
            sources: vec![],
            fallback: Some("codex login --device-auth".to_string()),
            headless: Some(Headless::Device),
            questions: None,
            finish: SignInFinish::Callback,
            code: Some(JsRegex::new("\\b[A-Z0-9]{4,}-[A-Z0-9]{4,}\\b")),
            key_env: Some("OPENAI_API_KEY".to_string()),
            status: Some(StatusCheck { command: "codex login status".to_string(), typed: None, reader: StatusReader::Codex }),
            note: None,
            keys: None,
            shared: Some(SharedLogin {
                dir: "codex".to_string(),
                file: "auth.json".to_string(),
                target: format!("{GUEST_HOME}/.codex/auth.json"),
                home_env: "CODEX_HOME".to_string(),
            }),
            state_on_machine: vec![],
        }),
    );
    rows.insert(
        "gemini".to_string(),
        SignIn::Login(LoginSignIn {
            kind: SignInKind::Oauth,
            login: "gemini --skip-trust".to_string(),
            sources: vec![LoginSource::File],
            fallback: None,
            headless: None,
            questions: Some(vec![
                Question {
                    asks: JsRegex::new("Do you trust the files in this folder\\?"),
                    settle: Settle::Flag("--skip-trust".to_string()),
                },
                Question {
                    asks: JsRegex::new("How would you like to authenticate for this project\\?"),
                    settle: Settle::Answer("\r".to_string()),
                },
            ]),
            finish: SignInFinish::Callback,
            code: None,
            key_env: Some("GEMINI_API_KEY".to_string()),
            status: Some(StatusCheck { command: GEMINI_STATUS.to_string(), typed: None, reader: StatusReader::Gemini }),
            note: None,
            keys: None,
            shared: None,
            state_on_machine: vec![".gemini/oauth_creds.json".to_string()],
        }),
    );
    rows.insert(
        "opencode".to_string(),
        SignIn::Login(LoginSignIn {
            kind: SignInKind::Key,
            login: "opencode auth login".to_string(),
            sources: vec![LoginSource::File],
            fallback: None,
            headless: None,
            questions: Some(vec![Question { asks: JsRegex::new("Select provider"), settle: Settle::Person(true) }]),
            finish: SignInFinish::None,
            code: None,
            key_env: None,
            status: Some(StatusCheck { command: "opencode auth list".to_string(), typed: None, reader: StatusReader::Opencode }),
            note: Some("OpenCode dropped its Anthropic sign-in in 1.3.0; it takes an API key there".to_string()),
            keys: None,
            shared: None,
            state_on_machine: vec![".local/share/opencode/auth.json".to_string()],
        }),
    );
    rows.insert(
        "cursor".to_string(),
        SignIn::Login(LoginSignIn {
            kind: SignInKind::Key,
            login: "cursor-agent login".to_string(),
            sources: vec![],
            fallback: None,
            headless: None,
            questions: None,
            finish: SignInFinish::None,
            code: None,
            key_env: Some("CURSOR_API_KEY".to_string()),
            status: Some(StatusCheck { command: "cursor-agent status".to_string(), typed: None, reader: StatusReader::Cursor }),
            note: None,
            keys: None,
            shared: None,
            state_on_machine: vec![".config/cursor/auth.json".to_string()],
        }),
    );
    rows.insert(
        "cloudflared".to_string(),
        SignIn::Login(LoginSignIn {
            kind: SignInKind::Oauth,
            login: "cloudflared tunnel login".to_string(),
            sources: vec![LoginSource::File],
            fallback: None,
            headless: None,
            questions: None,
            finish: SignInFinish::None,
            code: None,
            key_env: None,
            status: Some(StatusCheck { command: CLOUDFLARED_STATUS.to_string(), typed: None, reader: StatusReader::Cloudflared }),
            note: None,
            keys: None,
            shared: None,
            state_on_machine: vec![".cloudflared".to_string()],
        }),
    );
    rows.insert(
        "op".to_string(),
        SignIn::None(NoSignIn {
            kind: SignInKind::None,
            note: Some("needs the 1Password desktop app; set OP_SERVICE_ACCOUNT_TOKEN on the machine instead".to_string()),
            sources: vec![],
            status: None,
            state_on_machine: None,
        }),
    );
    rows.insert(
        "kubectl".to_string(),
        SignIn::None(NoSignIn {
            kind: SignInKind::None,
            note: Some("kubectl has no sign-in; copy the kubeconfig instead".to_string()),
            sources: vec![LoginSource::File],
            status: Some(StatusCheck {
                command: "kubectl config current-context".to_string(),
                typed: Some("kubectl config current-context 2>/dev/null".to_string()),
                reader: StatusReader::Kubectl,
            }),
            state_on_machine: Some(vec![".kube/config".to_string()]),
        }),
    );
    rows.insert(
        "pi".to_string(),
        SignIn::Login(LoginSignIn {
            kind: SignInKind::Oauth,
            login: "pi".to_string(),
            sources: vec![LoginSource::File],
            fallback: None,
            headless: None,
            questions: Some(vec![Question { asks: JsRegex::new("Trust project folder\\?"), settle: Settle::Person(true) }]),
            finish: SignInFinish::None,
            code: None,
            key_env: None,
            status: Some(StatusCheck { command: "pi --list-models".to_string(), typed: None, reader: StatusReader::Pi }),
            note: Some("type /login inside pi and pick a provider, then /exit; a key on the machine counts".to_string()),
            keys: None,
            shared: None,
            state_on_machine: vec![".pi/agent/auth.json".to_string()],
        }),
    );
    rows.insert(
        "hermes".to_string(),
        SignIn::Login(LoginSignIn {
            kind: SignInKind::Device,
            login: "hermes auth".to_string(),
            sources: vec![LoginSource::File],
            fallback: None,
            headless: None,
            questions: Some(vec![Question { asks: JsRegex::new("What would you like to do\\?"), settle: Settle::Person(true) }]),
            finish: SignInFinish::None,
            code: None,
            key_env: None,
            status: Some(StatusCheck { command: "hermes auth list".to_string(), typed: None, reader: StatusReader::Hermes }),
            note: Some("pick Add a credential in the menu; keys in ~/.hermes/.env count".to_string()),
            keys: Some(KeyFiles {
                paths: vec!["~/.hermes/.env".to_string()],
                note: "the keys in ~/.hermes/.env travel only by copy; no sign-in produces them".to_string(),
            }),
            shared: None,
            state_on_machine: vec![".hermes/auth.json".to_string(), ".hermes/.env".to_string()],
        }),
    );
    rows
}

/// One row by its tool; panics on a tool with no row, which the TS type system rules out.
pub fn sign_in_row(tool: &str) -> &'static SignIn {
    SIGN_IN_ROWS.get(tool).unwrap_or_else(|| panic!("{tool} has no sign-in row"))
}

/// signin.ts:539.
pub static SHARED_LOGINS: LazyLock<Vec<SharedLogin>> =
    LazyLock::new(|| SIGN_IN_ROWS.values().filter_map(|s| shared_login_of(s).cloned()).collect());

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Share {
    pub source: String,
    pub target: String,
}

/// signin.ts:545.
pub fn shares_in(logins: &str) -> Vec<Share> {
    SHARED_LOGINS.iter().map(|s| Share { source: format!("{logins}/{}", shared_file_in(s)), target: s.target.clone() }).collect()
}

/// `<dir>/<file>` (signin.ts:550).
pub fn shared_file_in(shared: &SharedLogin) -> String {
    format!("{}/{}", shared.dir, shared.file)
}

/// signin.ts:557.
pub static NEVER_IN_IMAGE: LazyLock<Vec<String>> = LazyLock::new(|| {
    let mut paths =
        vec![format!("{GUEST_HOME}/{CLAUDE_CONFIG_REL}/.credentials.json"), format!("{GUEST_HOME}/{CLAUDE_CONFIG_REL}/{CLAUDE_KEY_FILE}")];
    paths.extend(SHARED_LOGINS.iter().map(|s| s.target.clone()));
    paths
});

/// Where a machine exports the secrets it is given by name: login shells read profile.d, fish reads its conf.d instead.
pub const SECRETS_FILE_SH: &str = "/etc/profile.d/wsp-secrets.sh";
pub const SECRETS_FILE_FISH: &str = "/etc/fish/conf.d/wsp-secrets.fish";

/// Deduplicated, first place kept (signin.ts:569).
pub static LOGIN_PATHS: LazyLock<Vec<String>> = LazyLock::new(|| {
    let mut paths: IndexSet<String> = NEVER_IN_IMAGE.iter().cloned().collect();
    paths.extend(SIGN_IN_ROWS.values().flat_map(login_state_paths));
    paths.extend([SECRETS_FILE_SH.to_string(), SECRETS_FILE_FISH.to_string()]);
    paths.into_iter().collect()
});

/// signin.ts:575.
pub static NEVER_ON_BROKERED: LazyLock<Vec<String>> = LazyLock::new(|| {
    let mut paths: IndexSet<String> = LOGIN_PATHS.iter().cloned().collect();
    paths.extend([format!("{GUEST_HOME}/.mcp-auth"), format!("{GUEST_HOME}/.git-credentials")]);
    paths.into_iter().collect()
});

/// signin.ts:578.
pub static GH_LOGIN_PATHS: LazyLock<Vec<String>> = LazyLock::new(|| login_state_paths(sign_in_row("gh")));

/// The variables one row reads: its token variable, then its key variable (signin.ts:587).
fn row_variables(s: &SignIn) -> Vec<String> {
    mints_token(s).map(|t| t.token_env.clone()).into_iter().chain(key_env_of(s).map(str::to_string)).collect()
}

/// Each row's token variable then its key variable, in row order (signin.ts:587).
pub static VAULT_VARIABLES: LazyLock<IndexSet<String>> = LazyLock::new(|| SIGN_IN_ROWS.values().flat_map(row_variables).collect());

/// The record less every vault variable, with each header token's bearer name (signin.ts:591).
pub fn server_values_of(record: &IndexMap<String, String>) -> IndexMap<String, String> {
    let kept: IndexMap<String, String> =
        record.iter().filter(|(name, _)| !VAULT_VARIABLES.contains(name.as_str())).map(|(n, v)| (n.clone(), v.clone())).collect();
    with_bearer_variables(&kept)
}

/// The first row keeping its token or key under `name` (signin.ts:594).
pub fn vault_variable_row(name: &str) -> Option<String> {
    SIGN_IN_ROWS.iter().find(|(_, s)| row_variables(s).iter().any(|v| v == name)).map(|(tool, _)| tool.clone())
}
