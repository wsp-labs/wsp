// SPDX-License-Identifier: AGPL-3.0-only
//! How a catalog entry is signed into: the shapes, the status readers, and the rules every reader of a row follows.
//! Port of packages/catalog/src/signin.ts:1-342 at a3b870a6e (the rows are in signin_rows); spec/signin.md.
use indexmap::IndexMap;
use serde::Serialize;

use crate::js_regex::JsRegex;
use wsp_wire::types::agents_report::AgentSignInKind;
use wsp_wire::types::init_job::SignInFinish;
use wsp_wire::types::agents_report::SignInRoad;

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
        todo!()
    }

    /// The row's `detail`; `None` where the row has none or it names none. `secrets` maps a name to the file it was cut
    /// from, in insertion order.
    pub fn detail(&self, output: &str, secrets: &IndexMap<String, String>) -> Option<String> {
        todo!()
    }

    /// The row's `kind` (Claude Code, Codex, Gemini CLI).
    pub fn kind(&self, output: &str) -> Option<AgentSignInKind> {
        todo!()
    }

    /// The row's `plan` (Claude Code).
    pub fn plan(&self, output: &str) -> Option<String> {
        todo!()
    }

    /// The row's `why`; no row has one at a3b870a6e, so always `None`.
    pub fn why(&self, output: &str) -> Option<String> {
        todo!()
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    pub sources: Vec<LoginSource>,
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
        todo!()
    }

    pub fn status(&self) -> Option<&StatusCheck> {
        todo!()
    }

    pub fn note(&self) -> Option<&str> {
        todo!()
    }

    pub fn sources(&self) -> &[LoginSource] {
        todo!()
    }

    /// `stateOnMachine`, empty where the row names none.
    pub fn state_on_machine(&self) -> &[String] {
        todo!()
    }
}

/// signin.ts:135.
pub fn has_login(s: &SignIn) -> Option<&LoginSignIn> {
    todo!()
}

/// signin.ts:140.
pub fn mints_token(s: &SignIn) -> Option<&TokenSignIn> {
    todo!()
}

/// The login an agent runs on a computer you own: its own, else the token row's `there` as an oauth login with the
/// row's status and no sources or state (signin.ts:146).
pub fn login_there(s: &SignIn) -> Option<LoginSignIn> {
    todo!()
}

/// signin.ts:155.
pub fn shared_login_of(s: &SignIn) -> Option<&SharedLogin> {
    todo!()
}

/// signin.ts:160.
pub fn lives_on_computer(s: &SignIn) -> bool {
    todo!()
}

/// `<logins>/<dir>` (signin.ts:166).
pub fn login_home_in(logins: &str, shared: &SharedLogin) -> String {
    todo!()
}

/// signin.ts:172.
pub fn key_env_of(s: &SignIn) -> Option<&str> {
    todo!()
}

/// The trimmed paste where the whole of it is the token's shape (signin.ts:178).
pub fn token_in(s: &TokenSignIn, pasted: &str) -> Option<String> {
    todo!()
}

/// signin.ts:185.
pub fn questions_of(s: &SignIn) -> Vec<Question> {
    todo!()
}

/// signin.ts:191.
pub fn asks_the_person(s: &SignIn) -> bool {
    todo!()
}

/// signin.ts:199.
pub fn sign_in_road_of(s: &SignIn) -> SignInRoad {
    todo!()
}

/// The login up to its first ` --` (signin.ts:207).
pub fn login_words(s: &LoginSignIn) -> String {
    todo!()
}

/// signin.ts:214.
pub fn signs_in_by_default(s: &SignIn) -> bool {
    todo!()
}

/// `kube` for kubectl, else the id (signin.ts:221).
pub fn login_id_of(entry_id: &str) -> String {
    todo!()
}

/// signin.ts:228.
pub fn keys_id_of(entry_id: &str) -> String {
    todo!()
}

/// signin.ts:233.
pub fn keys_row_of(keys: &KeyFiles, status: Option<&StatusCheck>) -> SignIn {
    todo!()
}

/// `/root/<p>` for each of the row's `stateOnMachine` (signin.ts:239).
pub fn login_state_paths(sign_in: &SignIn) -> Vec<String> {
    todo!()
}

/// signin.ts:244.
pub fn no_sign_in() -> SignIn {
    todo!()
}

pub const AWS_STATUS: &str = "sh -c 'for p in $(aws configure list-profiles 2>/dev/null); do aws sts get-caller-identity --profile \"$p\" 2>/dev/null && exit 0; done; exit 1'";
pub const GEMINI_STATUS: &str = "if test -s \"$HOME/.gemini/oauth_creds.json\"; then echo oauth_creds.json; elif test -n \"$GEMINI_API_KEY\"; then echo GEMINI_API_KEY; elif test -n \"$GOOGLE_API_KEY\"; then echo GOOGLE_API_KEY; else false; fi";
pub const CLOUDFLARED_STATUS: &str = "if test -s \"$HOME/.cloudflared/cert.pem\"; then echo cert.pem; else false; fi";
pub const TOKEN_SOURCE: &str = "the token from this computer";

/// The first held key the output names as a word (signin.ts:266).
pub fn secret_named(output: &str, secrets: &IndexMap<String, String>) -> Option<String> {
    todo!()
}

/// signin.ts:277.
pub fn gemini_source(output: &str, secrets: &IndexMap<String, String>) -> Option<String> {
    todo!()
}

/// signin.ts:300.
pub fn claude_signed_in(output: &str) -> bool {
    todo!()
}

/// signin.ts:307.
pub fn claude_source(output: &str, secrets: &IndexMap<String, String>) -> Option<String> {
    todo!()
}
