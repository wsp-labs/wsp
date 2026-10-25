// SPDX-License-Identifier: AGPL-3.0-only
//! How a catalog entry is signed into: the shapes, the status readers, and the rules every reader of a row follows.
//! Port of packages/catalog/src/signin.ts:1-342 at a3b870a6e (the rows are in signin_rows); spec/signin.md.
use indexmap::IndexMap;
use serde::Serialize;
use serde_json::Value;

use crate::js_regex::JsRegex;
use crate::json_parse::parse_json;
use crate::jsonc::is_js_space;
use crate::roads::GUEST_HOME;
use wsp_wire::types::agents_report::AgentSignInKind;
use wsp_wire::types::agents_report::SignInRoad;
use wsp_wire::types::init_job::SignInFinish;

/// One status reader per sign-in row, the closures of signin.ts:344-536 by the row they sit on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum StatusReader {
    Gh,
    Gcloud,
    Aws,
    Wrangler,
    Vercel,
    Netlify,
    Fly,
    Supabase,
    Railway,
    Doppler,
    Claude,
    Codex,
    Gemini,
    Opencode,
    Cursor,
    Cloudflared,
    Kubectl,
    Pi,
    Hermes,
}

/// signin.ts:11.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StatusCheck {
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub typed: Option<String>,
    pub reader: StatusReader,
}

impl StatusCheck {
    /// The row's own `signedIn` (signin.ts:17).
    pub fn signed_in(&self, output: &str, exit_code: i32) -> bool {
        let exit_ok = exit_code == 0;
        let says = |source: &'static str| JsRegex::new(source).is_match(output);
        match self.reader {
            StatusReader::Gh => says("Logged in to") && !says("Failed to log in"),
            StatusReader::Gcloud | StatusReader::Fly => exit_ok && says("@"),
            StatusReader::Aws => exit_ok && says(r#""Arn""#),
            StatusReader::Wrangler => says("You are logged in"),
            StatusReader::Vercel => exit_ok && !says("No existing credentials"),
            StatusReader::Netlify => exit_ok && !JsRegex::with_flags("Not logged in", "i").is_match(output),
            StatusReader::Supabase | StatusReader::Doppler | StatusReader::Gemini | StatusReader::Cloudflared => exit_ok,
            StatusReader::Railway => exit_ok && says("Logged in as"),
            StatusReader::Claude => claude_signed_in(output),
            StatusReader::Codex => exit_ok && says("Logged in using"),
            StatusReader::Opencode => exit_ok && says(r"[1-9]\d* (credentials|environment variable)"),
            StatusReader::Cursor => exit_ok && says("Logged in as|Login successful"),
            StatusReader::Kubectl => exit_ok && says(r"\S"),
            StatusReader::Pi => exit_ok && JsRegex::with_flags(r"^provider\s+model\b", "m").is_match(output),
            StatusReader::Hermes => exit_ok && says(r"\(\d+ credentials\):"),
        }
    }

    /// The row's `detail`; `None` where the row has none or it names none. `secrets` maps a name to the file it was cut
    /// from, in insertion order.
    pub fn detail(&self, output: &str, secrets: &IndexMap<String, String>) -> Option<String> {
        match self.reader {
            StatusReader::Claude => claude_source(output, secrets),
            StatusReader::Gemini => gemini_source(output, secrets),
            StatusReader::Opencode | StatusReader::Hermes => secret_named(output, secrets),
            StatusReader::Kubectl => Some(format!("context {}", js_trim(output))),
            _ => None,
        }
    }

    /// The row's `kind` (Claude Code, Codex, Gemini CLI).
    pub fn kind(&self, output: &str) -> Option<AgentSignInKind> {
        match self.reader {
            StatusReader::Claude => claude_kind(output),
            StatusReader::Codex => codex_kind(output),
            StatusReader::Gemini => gemini_kind(output),
            _ => None,
        }
    }

    /// The row's `plan` (Claude Code).
    pub fn plan(&self, output: &str) -> Option<String> {
        match self.reader {
            StatusReader::Claude => claude_plan(output),
            _ => None,
        }
    }

    /// The row's `why`; no row has one at a3b870a6e, so always `None`.
    pub fn why(&self, _output: &str) -> Option<String> {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum LoginSource {
    Keychain,
    RcKey,
    File,
    Helper,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SignInKind {
    Oauth,
    Device,
    Key,
    Token,
    None,
}

/// How a question is settled (signin.ts:51): serialized as the one key the TS object carries beside `asks`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Settle {
    Flag(String),
    Answer(String),
    Person(bool),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Question {
    pub asks: JsRegex,
    #[serde(flatten)]
    pub settle: Settle,
}

/// signin.ts:55.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KeyFiles {
    pub paths: Vec<String>,
    pub note: String,
}

/// signin.ts:66.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedLogin {
    pub dir: String,
    pub file: String,
    pub target: String,
    pub home_env: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Headless {
    Device,
    Code,
}

/// The tool's own login on a computer you own, beside a token (signin.ts:87).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TokenThere {
    pub login: String,
    pub finish: SignInFinish,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headless: Option<Headless>,
}

/// signin.ts:75. `kind` is always `Token`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenSignIn {
    pub kind: SignInKind,
    pub mint: String,
    pub token_env: String,
    pub token: JsRegex,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_env: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<StatusCheck>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub there: Option<TokenThere>,
    pub sources: Vec<LoginSource>,
    pub state_on_machine: Vec<String>,
}

/// A login the wizard runs on the machine (signin.ts:94); `kind` is Oauth, Device or Key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginSignIn {
    pub kind: SignInKind,
    pub login: String,
    pub sources: Vec<LoginSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headless: Option<Headless>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub questions: Option<Vec<Question>>,
    pub finish: SignInFinish,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<JsRegex>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_env: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<StatusCheck>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keys: Option<KeyFiles>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shared: Option<SharedLogin>,
    pub state_on_machine: Vec<String>,
}

/// Nothing to run on a headless machine (signin.ts:130); `kind` is None.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoSignIn {
    pub kind: SignInKind,
    pub sources: Vec<LoginSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<StatusCheck>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_on_machine: Option<Vec<String>>,
}

/// signin.ts:92, serialized as the inner shape, which carries its own `kind`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum SignIn {
    Token(TokenSignIn),
    Login(LoginSignIn),
    None(NoSignIn),
}

impl SignIn {
    pub fn kind(&self) -> SignInKind {
        match self {
            SignIn::Token(_) => SignInKind::Token,
            SignIn::Login(l) => l.kind,
            SignIn::None(_) => SignInKind::None,
        }
    }

    pub fn status(&self) -> Option<&StatusCheck> {
        match self {
            SignIn::Token(t) => t.status.as_ref(),
            SignIn::Login(l) => l.status.as_ref(),
            SignIn::None(n) => n.status.as_ref(),
        }
    }

    pub fn note(&self) -> Option<&str> {
        match self {
            SignIn::Token(t) => t.note.as_deref(),
            SignIn::Login(l) => l.note.as_deref(),
            SignIn::None(n) => n.note.as_deref(),
        }
    }

    pub fn sources(&self) -> &[LoginSource] {
        match self {
            SignIn::Token(t) => t.sources.as_slice(),
            SignIn::Login(l) => l.sources.as_slice(),
            SignIn::None(n) => n.sources.as_slice(),
        }
    }

    /// `stateOnMachine`, empty where the row names none.
    pub fn state_on_machine(&self) -> &[String] {
        match self {
            SignIn::Token(t) => t.state_on_machine.as_slice(),
            SignIn::Login(l) => l.state_on_machine.as_slice(),
            SignIn::None(n) => n.state_on_machine.as_deref().unwrap_or(&[]),
        }
    }
}

/// signin.ts:135.
pub fn has_login(s: &SignIn) -> Option<&LoginSignIn> {
    match s {
        SignIn::Login(l) => Some(l),
        _ => None,
    }
}

/// signin.ts:140.
pub fn mints_token(s: &SignIn) -> Option<&TokenSignIn> {
    match s {
        SignIn::Token(t) => Some(t),
        _ => None,
    }
}

/// The login an agent runs on a computer you own: its own, else the token row's `there` as an oauth login with the
/// row's status and no sources or state (signin.ts:146).
pub fn login_there(s: &SignIn) -> Option<LoginSignIn> {
    match s {
        SignIn::Login(l) => Some(l.clone()),
        SignIn::Token(t) => t.there.as_ref().map(|there| LoginSignIn {
            kind: SignInKind::Oauth,
            login: there.login.clone(),
            sources: Vec::new(),
            fallback: None,
            headless: there.headless,
            questions: None,
            finish: there.finish,
            code: None,
            key_env: None,
            status: t.status.clone(),
            note: None,
            keys: None,
            shared: None,
            state_on_machine: Vec::new(),
        }),
        SignIn::None(_) => None,
    }
}

/// signin.ts:155.
pub fn shared_login_of(s: &SignIn) -> Option<&SharedLogin> {
    has_login(s).and_then(|l| l.shared.as_ref())
}

/// signin.ts:160.
pub fn lives_on_computer(s: &SignIn) -> bool {
    shared_login_of(s).is_some()
}

/// `<logins>/<dir>` (signin.ts:166).
pub fn login_home_in(logins: &str, shared: &SharedLogin) -> String {
    format!("{logins}/{}", shared.dir)
}

/// signin.ts:172.
pub fn key_env_of(s: &SignIn) -> Option<&str> {
    match s {
        SignIn::Token(t) => t.key_env.as_deref(),
        SignIn::Login(l) => l.key_env.as_deref(),
        SignIn::None(_) => None,
    }
}

/// The trimmed paste where the whole of it is the token's shape (signin.ts:178).
pub fn token_in(s: &TokenSignIn, pasted: &str) -> Option<String> {
    let value = js_trim(pasted);
    s.token.is_whole_match(value).then(|| value.to_string())
}

/// signin.ts:185.
pub fn questions_of(s: &SignIn) -> Vec<Question> {
    has_login(s).and_then(|l| l.questions.clone()).unwrap_or_default()
}

/// signin.ts:191.
pub fn asks_the_person(s: &SignIn) -> bool {
    has_login(s).and_then(|l| l.questions.as_deref()).is_some_and(|qs| qs.iter().any(|q| matches!(q.settle, Settle::Person(_))))
}

/// signin.ts:199.
pub fn sign_in_road_of(s: &SignIn) -> SignInRoad {
    if asks_the_person(s) {
        return SignInRoad::Terminal;
    }
    match s {
        SignIn::Token(_) => SignInRoad::Token,
        SignIn::None(_) => SignInRoad::None,
        SignIn::Login(l) => match (l.kind, l.headless) {
            (SignInKind::Key, _) => SignInRoad::Key,
            (SignInKind::Device, _) => SignInRoad::Device,
            (_, Some(Headless::Device)) => SignInRoad::Device,
            _ => SignInRoad::Code,
        },
    }
}

/// The login up to its first ` --` (signin.ts:207).
pub fn login_words(s: &LoginSignIn) -> String {
    match s.login.find(" --") {
        Some(at) => s.login[..at].to_string(),
        None => s.login.clone(),
    }
}

/// signin.ts:214.
pub fn signs_in_by_default(s: &SignIn) -> bool {
    matches!(s.kind(), SignInKind::Oauth | SignInKind::Device)
}

/// `kube` for kubectl, else the id (signin.ts:221).
pub fn login_id_of(entry_id: &str) -> String {
    if entry_id == "kubectl" {
        "kube".to_string()
    } else {
        entry_id.to_string()
    }
}

/// signin.ts:228.
pub fn keys_id_of(entry_id: &str) -> String {
    format!("{}-keys", login_id_of(entry_id))
}

/// signin.ts:233.
pub fn keys_row_of(keys: &KeyFiles, status: Option<&StatusCheck>) -> SignIn {
    SignIn::None(NoSignIn {
        kind: SignInKind::None,
        note: Some(keys.note.clone()),
        sources: vec![LoginSource::File],
        status: status.cloned(),
        state_on_machine: None,
    })
}

/// `/root/<p>` for each of the row's `stateOnMachine` (signin.ts:239).
pub fn login_state_paths(sign_in: &SignIn) -> Vec<String> {
    sign_in.state_on_machine().iter().map(|p| format!("{GUEST_HOME}/{p}")).collect()
}

/// signin.ts:244.
pub fn no_sign_in() -> SignIn {
    SignIn::None(NoSignIn { kind: SignInKind::None, note: None, sources: Vec::new(), status: None, state_on_machine: None })
}

pub const AWS_STATUS: &str = "sh -c 'for p in $(aws configure list-profiles 2>/dev/null); do aws sts get-caller-identity --profile \"$p\" 2>/dev/null && exit 0; done; exit 1'";
pub const GEMINI_STATUS: &str = "if test -s \"$HOME/.gemini/oauth_creds.json\"; then echo oauth_creds.json; elif test -n \"$GEMINI_API_KEY\"; then echo GEMINI_API_KEY; elif test -n \"$GOOGLE_API_KEY\"; then echo GOOGLE_API_KEY; else false; fi";
pub const CLOUDFLARED_STATUS: &str = "if test -s \"$HOME/.cloudflared/cert.pem\"; then echo cert.pem; else false; fi";
pub const TOKEN_SOURCE: &str = "the token from this computer";

fn js_trim(s: &str) -> &str {
    s.trim_matches(is_js_space)
}

/// A key named by the status, by the file the secrets step cut it from, or as the machine's own when that step did not set it.
fn key_from(name: &str, secrets: &IndexMap<String, String>) -> String {
    match secrets.get(name) {
        Some(from) => format!("API key from {from}, set on the machine as a secret"),
        None => format!("API key from {name} on the machine"),
    }
}

/// The first key the secrets step set that the status output names as a word: that key is the login's source.
pub fn secret_named(output: &str, secrets: &IndexMap<String, String>) -> Option<String> {
    secrets
        .iter()
        .find(|(name, _)| output.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).any(|word| word == name.as_str()))
        .map(|(_, from)| format!("API key from {from}, set on the machine as a secret"))
}

/// The one word the gemini line echoed last: the cached sign-in's file, or the key's name.
fn gemini_found(output: &str) -> Option<&str> {
    js_trim(output).split(is_js_space).next_back()
}

/// signin.ts:277.
pub fn gemini_source(output: &str, secrets: &IndexMap<String, String>) -> Option<String> {
    match gemini_found(output)? {
        "oauth_creds.json" => Some("OAuth credentials".to_string()),
        found @ ("GEMINI_API_KEY" | "GOOGLE_API_KEY") => Some(key_from(found, secrets)),
        _ => None,
    }
}

fn gemini_kind(output: &str) -> Option<AgentSignInKind> {
    match gemini_found(output)? {
        "oauth_creds.json" => Some(AgentSignInKind::Oauth),
        "GEMINI_API_KEY" | "GOOGLE_API_KEY" => Some(AgentSignInKind::ApiKey),
        _ => None,
    }
}

/// The JSON claude auth status printed, when it did: the text from its first `{` to its last `}`.
fn claude_status(output: &str) -> Option<Value> {
    let start = output.find('{')?;
    let end = output.rfind('}')?;
    if end <= start {
        return None;
    }
    parse_json(&output[start..=end]).ok()
}

fn field<'a>(status: &'a Value, key: &str) -> Option<&'a str> {
    status.get(key).and_then(Value::as_str)
}

fn logged_in(status: &Value) -> bool {
    matches!(status.get("loggedIn"), Some(Value::Bool(true)))
}

/// signin.ts:300.
pub fn claude_signed_in(output: &str) -> bool {
    claude_status(output).is_some_and(|status| logged_in(&status))
}

/// signin.ts:307.
pub fn claude_source(output: &str, secrets: &IndexMap<String, String>) -> Option<String> {
    let status = claude_status(output)?;
    if field(&status, "apiKeySource") == Some("ANTHROPIC_API_KEY") {
        return Some(key_from("ANTHROPIC_API_KEY", secrets));
    }
    if !logged_in(&status) {
        return None;
    }
    match field(&status, "authMethod") {
        Some("oauth_token") => Some(TOKEN_SOURCE.to_string()),
        Some("claude.ai") => Some("OAuth credentials".to_string()),
        _ => None,
    }
}

/// The plan claude auth status names a browser sign-in's account by (subscriptionType, measured "max" on 2.1.259).
fn claude_plan(output: &str) -> Option<String> {
    let status = claude_status(output)?;
    if status.get("apiKeySource").is_some() {
        return None;
    }
    field(&status, "subscriptionType").filter(|plan| !plan.is_empty()).map(str::to_string)
}

/// A key where the status names one; the long-lived token only a plan mints, and a browser sign-in whose account names
/// its plan, are a subscription; a browser sign-in naming none is plain OAuth.
fn claude_kind(output: &str) -> Option<AgentSignInKind> {
    let status = claude_status(output)?;
    if field(&status, "apiKeySource") == Some("ANTHROPIC_API_KEY") {
        return Some(AgentSignInKind::ApiKey);
    }
    if !logged_in(&status) {
        return None;
    }
    match field(&status, "authMethod") {
        Some("oauth_token") => Some(AgentSignInKind::Subscription),
        Some("claude.ai") if claude_plan(output).is_some() => Some(AgentSignInKind::Subscription),
        Some("claude.ai") => Some(AgentSignInKind::Oauth),
        _ => None,
    }
}

/// codex login status's own two lines for a login (codex-rs cli/src/login.rs): a ChatGPT plan, or a key.
fn codex_kind(output: &str) -> Option<AgentSignInKind> {
    if JsRegex::new("Logged in using ChatGPT").is_match(output) {
        Some(AgentSignInKind::Subscription)
    } else if JsRegex::new("Logged in using an API key").is_match(output) {
        Some(AgentSignInKind::ApiKey)
    } else {
        None
    }
}
