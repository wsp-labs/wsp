// SPDX-License-Identifier: AGPL-3.0-only
//! The sign-ins the add flows run under a row (`useAgentActs.ts`, `agentsRows.ts`, `SignInFlowView.tsx`): how each
//! agent's Sign in goes, and each one as it stands, fed by the host's steps for a watched run, the paste for a token or
//! key, and the line for the person's terminal; the calls that drive one; and the view under its row. Held by the
//! dialog: closing it stops every run it started, as a run the host was still starting is stopped once its id lands.

use std::collections::BTreeMap;
use std::rc::Rc;

use gpui_kit::component::input::InputState;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _};
use gpui_kit::{div, prelude::*, px, AnyElement, App, Entity, SharedString, Window};
use serde::Deserialize;
use serde_json::json;
use wsp_host_client::protocol::{
    AgentsKeyRequest, AgentsSignInCodeRequest, AgentsSignInEvent, AgentsSignInRequest, AgentsSignInStopRequest,
};
use wsp_host_client::{Failure, HostClient, Op};

use crate::ui::{catalog, spinner, text, ActivePalette, Button, CopyRow, Field, Glyph, Refusal};

pub const SIGN_IN: &str = "Sign in";
pub const CANCEL: &str = "Cancel";
pub const OPEN: &str = "Open";
pub const RUN_IN_TERMINAL: &str = "Run in your terminal";
pub const PASTE_TOKEN: &str = "Paste the token";
pub const PASTE_KEY: &str = "Paste the key";
pub const SAVE: &str = "Save";
pub const CODE_SUBMIT: &str = "Submit";

/// How an agent's Sign in goes from the summary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Start {
    /// The agent's own sign-in, run on the host and watched; `pastes` where its page hands a code back.
    Run { agent: String, pastes: bool },
    /// A token or key pasted into this host's vault, under the line that mints it.
    Vault { agent: String, token: bool, mint: Option<&'static str> },
    /// A line the person runs in their own terminal.
    Copy { line: String },
}

/// How a sign-in road reads from the summary, which stands on the computer the host runs on with no terminal of a
/// task to type in: a terminal's sign-in is the line for the person's own.
pub fn start_of(agent: &str, road: &str) -> Option<Start> {
    let agent = agent.to_owned();
    match road {
        "none" => None,
        "token" => Some(Start::Vault { mint: catalog::sign_in(&agent).and_then(|s| s.mint), agent, token: true }),
        "key" => Some(Start::Vault { agent, token: false, mint: None }),
        "terminal" => Some(Start::Copy { line: format!("wsp agents signin {agent}") }),
        _ => Some(Start::Run { pastes: road == "code", agent }),
    }
}

/// A watched run's state, as the host's steps say it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunState {
    Running,
    Waiting,
    Failed,
}

/// One sign-in as it stands under its row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Flow {
    /// `fix` is what to do about a refusal `said` carries, where the host said.
    Run {
        state: RunState,
        pastes: bool,
        url: Option<String>,
        code: Option<String>,
        paste: bool,
        said: Option<String>,
        fix: Option<String>,
        id: Option<String>,
    },
    Vault {
        agent: String,
        token: bool,
        mint: Option<&'static str>,
        saving: bool,
        refused: Option<String>,
        fix: Option<String>,
    },
    Copy {
        line: String,
    },
}

/// One step the host pushes for a run (`agents.signIn`).
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Heard {
    pub sign_in_id: String,
    pub state: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub paste: Option<bool>,
    #[serde(default)]
    pub said: Option<String>,
}

/// A call a sign-in asks the view's thin layer to send.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ask {
    /// `agents.signIn` on the computer the host runs on.
    Begin { row: String, agent: String },
    /// `agents.signInStop`.
    Stop { id: String },
    /// `agents.signInCode`.
    Code { row: String, id: String, code: String },
    /// `agents.key`.
    Key { row: String, agent: String, key: String, token: bool },
}

/// Every sign-in the summary stands, by its row: at most one per ticked agent.
#[derive(Clone, Debug, Default)]
pub struct SignIns {
    flows: BTreeMap<String, Flow>,
}

impl SignIns {
    pub fn of(&self, row: &str) -> Option<&Flow> {
        self.flows.get(row)
    }

    /// Whether a watched run still goes, so its act is Cancel.
    pub fn running(&self, row: &str) -> bool {
        matches!(self.flows.get(row), Some(Flow::Run { state, .. }) if *state != RunState::Failed)
    }

    /// Sign in pressed: a line or a field stands under the row, or a run starts on the host.
    pub fn start(&mut self, row: &str, start: &Start) -> Option<Ask> {
        match start {
            Start::Copy { line } => {
                self.flows.insert(row.into(), Flow::Copy { line: line.clone() });
                None
            }
            Start::Vault { agent, token, mint } => {
                let stop = self.stop_of(row);
                self.flows.insert(
                    row.into(),
                    Flow::Vault { agent: agent.clone(), token: *token, mint: *mint, saving: false, refused: None, fix: None },
                );
                stop
            }
            Start::Run { agent, pastes } => {
                self.flows.insert(
                    row.into(),
                    Flow::Run {
                        state: RunState::Running,
                        pastes: *pastes,
                        url: None,
                        code: None,
                        paste: false,
                        said: None,
                        fix: None,
                        id: None,
                    },
                );
                Some(Ask::Begin { row: row.into(), agent: agent.clone() })
            }
        }
    }

    fn stop_of(&self, row: &str) -> Option<Ask> {
        match self.flows.get(row) {
            Some(Flow::Run { id: Some(id), state, .. }) if *state != RunState::Failed => Some(Ask::Stop { id: id.clone() }),
            _ => None,
        }
    }

    /// The host took the run, by its id, or refused it. A run cancelled before its id came is stopped then.
    pub fn on_begun(&mut self, row: &str, answer: Result<String, Failure>) -> Option<Ask> {
        match (self.flows.get_mut(row), answer) {
            (Some(Flow::Run { id, .. }), Ok(signed)) => {
                *id = Some(signed);
                None
            }
            (Some(Flow::Run { state, said, fix, .. }), Err(f)) => {
                *state = RunState::Failed;
                *said = Some(f.said().to_owned());
                *fix = f.fix.as_deref().map(str::to_owned);
                None
            }
            (_, Ok(signed)) => Some(Ask::Stop { id: signed }),
            _ => None,
        }
    }

    /// A step the host pushed for a run. Returns true where a run signed in: its row leaves, and the report is read
    /// again to say so.
    pub fn hear(&mut self, heard: &Heard) -> bool {
        let Some(row) = self
            .flows
            .iter()
            .find(|(_, f)| matches!(f, Flow::Run { id: Some(id), .. } if *id == heard.sign_in_id))
            .map(|(row, _)| row.clone())
        else {
            return false;
        };
        if heard.state == "signed-in" {
            self.flows.remove(&row);
            return true;
        }
        if let Some(Flow::Run { state, url, code, paste, said, fix, .. }) = self.flows.get_mut(&row) {
            *state = match heard.state.as_str() {
                "waiting" => RunState::Waiting,
                "failed" => RunState::Failed,
                _ => RunState::Running,
            };
            if heard.url.is_some() {
                *url = heard.url.clone();
            }
            *code = heard.code.clone();
            if let Some(p) = heard.paste {
                *paste = p;
            }
            *said = heard.said.clone();
            *fix = None;
        }
        false
    }

    /// The dialog closed: every sign-in under it goes, and each run with the host stops there.
    pub fn end(&mut self) -> Vec<Ask> {
        let stops = self.flows.keys().filter_map(|row| self.stop_of(row)).collect();
        self.flows.clear();
        stops
    }

    /// Cancel: the row goes back to its Sign in, and a run stops on the host.
    pub fn cancel(&mut self, row: &str) -> Option<Ask> {
        let stop = self.stop_of(row);
        self.flows.remove(row);
        stop
    }

    /// The code a run's page handed back.
    pub fn code(&mut self, row: &str, typed: &str) -> Option<Ask> {
        let typed = typed.trim();
        match self.flows.get(row) {
            Some(Flow::Run { id: Some(id), .. }) if !typed.is_empty() => {
                Some(Ask::Code { row: row.into(), id: id.clone(), code: typed.into() })
            }
            _ => None,
        }
    }

    pub fn on_code(&mut self, row: &str, answer: Result<(), Failure>) {
        if let (Some(Flow::Run { state, said, fix, .. }), Err(f)) = (self.flows.get_mut(row), answer) {
            *state = RunState::Failed;
            *said = Some(f.said().to_owned());
            *fix = f.fix.as_deref().map(str::to_owned);
        }
    }

    /// Save on a pasted token or key.
    pub fn save(&mut self, row: &str, key: &str) -> Option<Ask> {
        let key = key.trim();
        match self.flows.get_mut(row) {
            Some(Flow::Vault { agent, token, saving, refused, fix, .. }) if !key.is_empty() && !*saving => {
                *fix = None;
                *saving = true;
                *refused = None;
                Some(Ask::Key { row: row.into(), agent: agent.clone(), key: key.into(), token: *token })
            }
            _ => None,
        }
    }

    /// The key was saved, and its row leaves; or the refusal under the field.
    pub fn on_saved(&mut self, row: &str, answer: Result<(), Failure>) {
        match answer {
            Ok(()) => {
                self.flows.remove(row);
            }
            Err(f) => {
                if let Some(Flow::Vault { saving, refused, fix, .. }) = self.flows.get_mut(row) {
                    *fix = f.fix.as_deref().map(str::to_owned);
                    *saving = false;
                    *refused = Some(f.said().to_owned());
                }
            }
        }
    }
}

async fn call<O>(host: &HostClient, params: serde_json::Value) -> Result<O::Reply, Failure>
where
    O: Op + serde::de::DeserializeOwned,
{
    let op: O = serde_json::from_value(params).map_err(|e| Failure::new(e.to_string()))?;
    host.call(op).await
}

/// `agents.signIn` for `agent` on the computer `place` names: the run's id, which its pushed steps carry.
pub async fn begin(host: &HostClient, place: String, agent: String) -> Result<String, Failure> {
    Ok(call::<AgentsSignInRequest>(host, json!({ "target": { "placeId": place }, "agent": agent })).await?.sign_in_id)
}

pub async fn stop(host: &HostClient, id: String) -> Result<(), Failure> {
    call::<AgentsSignInStopRequest>(host, json!({ "signInId": id })).await.map(drop)
}

pub async fn code(host: &HostClient, id: String, code: String) -> Result<(), Failure> {
    call::<AgentsSignInCodeRequest>(host, json!({ "signInId": id, "code": code })).await.map(drop)
}

/// A pasted token or key into this host's vault.
pub async fn key(host: &HostClient, agent: String, key: String, token: bool) -> Result<(), Failure> {
    call::<AgentsKeyRequest>(host, json!({ "agent": agent, "key": key, "way": if token { "token" } else { "key" } })).await.map(drop)
}

/// A run's step as the host pushed it.
pub fn heard_of(event: &AgentsSignInEvent) -> Option<Heard> {
    serde_json::to_value(event).ok().and_then(|v| serde_json::from_value(v).ok())
}

type Typed = Rc<dyn Fn(String, &mut Window, &mut App) + 'static>;

/// A refusal under a sign-in: the sentence in the refusal's ink, its fix after it in the foreground (`RefusalSlot`).
fn refusal(said: String, fix: Option<String>) -> Refusal {
    let r = Refusal::new().said(said);
    match fix {
        Some(fix) => r.fix(fix),
        None => r,
    }
}

/// One sign-in as it stands under its row (`SignInFlowView` with no room kept): a line for the person's terminal, the
/// field a token or key is pasted in with Save, or a watched run's page and code with the field its code goes in.
/// `field` is the row's own field, made and kept by the dialog; `on_save` and `on_code` take what was typed in it.
#[derive(IntoElement)]
pub struct SignInView {
    id: SharedString,
    flow: Flow,
    field: Option<Entity<InputState>>,
    on_save: Option<Typed>,
    on_code: Option<Typed>,
}

impl SignInView {
    pub fn new(id: impl Into<SharedString>, flow: &Flow) -> Self {
        SignInView { id: id.into(), flow: flow.clone(), field: None, on_save: None, on_code: None }
    }

    pub fn field(mut self, field: &Entity<InputState>) -> Self {
        self.field = Some(field.clone());
        self
    }

    pub fn on_save(mut self, f: impl Fn(String, &mut Window, &mut App) + 'static) -> Self {
        self.on_save = Some(Rc::new(f));
        self
    }

    pub fn on_code(mut self, f: impl Fn(String, &mut Window, &mut App) + 'static) -> Self {
        self.on_code = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for SignInView {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette().clone();
        let xs = |words: &str| div().text_size(text::XS.0).line_height(text::XS.1).text_color(p.muted_foreground).child(words.to_owned());
        let id = self.id;
        let typed_into = |f: Option<Typed>, field: Option<Entity<InputState>>| {
            move |_: &gpui_kit::ClickEvent, window: &mut Window, cx: &mut App| {
                let (Some(f), Some(field)) = (&f, &field) else { return };
                let typed = field.read(cx).value().to_string();
                f(typed, window, cx);
            }
        };
        let body: AnyElement = match &self.flow {
            Flow::Copy { line } => {
                v_flex().gap_2().child(xs(RUN_IN_TERMINAL)).child(CopyRow::new(format!("{id}-line"), line.clone())).into_any_element()
            }
            Flow::Vault { token, mint, saving, refused, fix, .. } => {
                let typed = self.field.as_ref().is_some_and(|f| !f.read(cx).value().trim().is_empty());
                let save = Button::outline(format!("{id}-save"), SAVE)
                    .xs()
                    .glyph(Glyph::Check)
                    .busy(*saving)
                    .held(!typed)
                    .on_click(typed_into(self.on_save.clone(), self.field.clone()));
                v_flex()
                    .gap_3()
                    .children(mint.map(|mint| v_flex().gap_2().child(xs(RUN_IN_TERMINAL)).child(CopyRow::new(format!("{id}-mint"), mint))))
                    .child(
                        v_flex().gap_2().child(xs(if *token { PASTE_TOKEN } else { PASTE_KEY })).child(
                            h_flex()
                                .gap_3()
                                .children(self.field.as_ref().map(|f| div().flex_1().min_w_0().child(Field::new(f).sheet())))
                                .child(save),
                        ),
                    )
                    .children(refused.clone().map(|said| refusal(said, fix.clone())))
                    .into_any_element()
            }
            Flow::Run { state, url, code, paste, said, fix, .. } => {
                let open = url.clone().map(|url| {
                    Button::outline(format!("{id}-open"), OPEN).xs().glyph(Glyph::ExternalLink).on_click(move |_, _, cx| cx.open_url(&url))
                });
                let line = match (url, *state) {
                    (None, RunState::Running) => {
                        Some(h_flex().h(px(40.)).gap_3().child(spinner(cx)).children(said.clone().map(|s| xs(&s))).into_any_element())
                    }
                    (None, _) => None,
                    (Some(_), _) => Some(
                        h_flex()
                            .h(px(40.))
                            .gap_3()
                            .children(code.clone().map(|code| {
                                div()
                                    .debug_selector(|| "sign-in-code".into())
                                    .font_family(cx.theme().mono_font_family.clone())
                                    .text_size(px(20.))
                                    .line_height(px(28.))
                                    .text_color(p.foreground)
                                    .child(code)
                            }))
                            .children(open)
                            .into_any_element(),
                    ),
                };
                let paste_line = (*paste && *state == RunState::Waiting).then(|| {
                    h_flex()
                        .h(px(40.))
                        .gap_3()
                        .children(self.field.as_ref().map(|f| div().flex_1().min_w_0().child(Field::new(f).sheet())))
                        .child(
                            Button::outline(format!("{id}-code"), CODE_SUBMIT)
                                .xs()
                                .on_click(typed_into(self.on_code.clone(), self.field.clone())),
                        )
                });
                let failed = (*state == RunState::Failed).then(|| said.clone()).flatten();
                v_flex()
                    .gap_2()
                    .children(line)
                    .children(paste_line)
                    .children(failed.map(|said| refusal(said, fix.clone())))
                    .into_any_element()
            }
        };
        div().debug_selector(|| "sign-in-view".into()).child(body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sign_ins_refusal_draws_its_sentence_in_the_refusals_ink_and_its_fix_in_the_foreground() {
        let (red, ink, muted) = (gpui_kit::red(), gpui_kit::black(), gpui_kit::blue());
        let runs = refusal("The sign-in ran out.".into(), Some("Press Sign in again.".into())).runs(red, ink, muted);
        assert_eq!(runs, [("The sign-in ran out.".into(), red), (" Press Sign in again.".into(), ink)]);
    }

    #[test]
    fn a_close_stops_each_run_the_host_took_and_nothing_else() {
        let mut ins = SignIns::default();
        ins.start("signins/codex", &Start::Run { agent: "codex".into(), pastes: false });
        ins.on_begun("signins/codex", Ok("si_1".into()));
        ins.start("signins/gemini", &Start::Run { agent: "gemini".into(), pastes: false });
        ins.start("signins/claude", &Start::Vault { agent: "claude".into(), token: true, mint: None });
        assert_eq!(ins.end(), [Ask::Stop { id: "si_1".into() }], "a run not yet taken is stopped once its id lands");
        assert!(ins.of("signins/codex").is_none() && ins.of("signins/claude").is_none());
    }
}
