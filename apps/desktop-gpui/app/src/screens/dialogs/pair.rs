// SPDX-License-Identifier: AGPL-3.0-only
//! The pair screen (`PairScreen.tsx`, `BootGate.tsx`): the window of a host the app holds no token for. One heading,
//! the hint, the code field, Pair and two lines for the refusal; blank while it asks again whether a token is held,
//! which it does each time the window comes forward. A code is spent once and the window becomes the app.

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::{v_flex, Root};
use gpui_kit::{div, prelude::*, px, App, Context, Entity, FontWeight, Global, Subscription, Task, Window};
use tokio::runtime::Runtime;
use wsp_host_client::{Env, Failure, HostClient};

use crate::shell::{Launch, Shell};
use crate::ui::theme::{self, Palette};
use crate::ui::{text, Button, Field, Refusal};

/// `PAIR_HEADING`, as Electron's window shows it too: its pair screen is the web's own page.
pub const HEADING: &str = "Pair this browser";
pub const HINT: &str = "Run wsp host pair on the computer the host runs on and type the code it prints.";
pub const LABEL: &str = "Pairing code";
pub const PAIR: &str = "Pair";
pub const PAIRING: &str = "Pairing";
/// `PAIR_CODE_SHAPE_REFUSAL`: a paste that is no code, refused before any host is asked.
pub const SHAPE_REFUSAL: &str =
    "that is not a pairing code, which is 8 letters and digits like ABCD-EFGH; paste only the code wsp host pair printed";

/// `PAIR_CODE_ALPHABET` and `PAIR_CODE_LENGTH`.
const ALPHABET: &str = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const LENGTH: usize = 8;

/// The code as the host takes it, read as `readJoinToken` reads a paste: what stands before a `.` and its key, the
/// dashes out, upper case; or the shape refusal for anything that is no code.
pub fn sent_code(typed: &str) -> Result<String, &'static str> {
    let code: String = typed.trim().split('.').next().unwrap_or_default().replace('-', "").to_uppercase();
    if code.chars().count() == LENGTH && code.chars().all(|c| ALPHABET.contains(c)) {
        Ok(code)
    } else {
        Err(SHAPE_REFUSAL)
    }
}

/// `deviceName`: what this computer calls itself to the host, which `wsp host devices` shows beside the id. Electron's
/// window sends the same "in a browser" name: its page runs `deviceName` on its own user agent.
pub fn device_name(host: &str) -> String {
    let platform = if cfg!(target_os = "macos") {
        "a Mac"
    } else if cfg!(target_os = "windows") {
        "a Windows computer"
    } else if cfg!(target_os = "linux") {
        "a Linux computer"
    } else {
        "a computer"
    };
    format!("{platform} in a browser at {host}")
}

pub type Redeem = Rc<dyn Fn(String, &mut App) -> Task<Result<(), String>>>;
pub type Ask = Rc<dyn Fn(&mut App) -> Task<bool>>;
pub type Paired = Rc<dyn Fn(&mut Window, &mut App)>;

/// What the screen asks of the host it stands for.
#[derive(Clone)]
pub struct Road {
    /// Spends a code that passed the shape check; the host's refusal in its own words, or nothing once a token is held.
    pub redeem: Redeem,
    /// Whether a token for the host is held now.
    pub ask: Ask,
    /// What the window becomes once a token is held.
    pub paired: Paired,
}

impl Road {
    /// The host the shell dials off `state`, which then opens as the app on the token a code bought.
    pub fn host(state: PathBuf, env: Env, runtime: Arc<Runtime>) -> Road {
        let dial = {
            let (state, env, runtime) = (state.clone(), env.clone(), runtime.clone());
            move |cx: &mut App| -> Task<bool> {
                let (state, env) = (state.clone(), env.clone());
                let dialled = runtime.spawn(async move { HostClient::connect(&state, &env).await.is_ok() });
                cx.background_spawn(async move { dialled.await.unwrap_or(false) })
            }
        };
        let redeem = {
            let (state, env, runtime) = (state.clone(), env.clone(), runtime.clone());
            move |code: String, cx: &mut App| -> Task<Result<(), String>> {
                let host = crate::screens::sidebar::host_page(&state).unwrap_or_default();
                let name = device_name(host.trim_start_matches("http://").trim_end_matches('/'));
                let (state, env) = (state.clone(), env.clone());
                let spent = runtime.spawn(async move { HostClient::redeem(&state, &env, &code, &name).await });
                cx.spawn(async move |cx| {
                    let client = spent.await.unwrap_or_else(|e| Err(Failure::new(e.to_string()))).map_err(|f| f.message)?;
                    if let Some(token) = client.paired_token() {
                        cx.update(|cx| cx.set_global(Bought(Some(token))));
                    }
                    Ok(())
                })
            }
        };
        let paired = move |window: &mut Window, cx: &mut App| {
            let (state, env, runtime) = (state.clone(), env.clone(), runtime.clone());
            window.replace_root(cx, |window, cx| {
                let shell = cx.new(|cx| Shell::new(state, env, runtime, Launch::default(), window, cx));
                Root::new(shell, window, cx)
            });
        };
        Road { redeem: Rc::new(redeem), ask: Rc::new(dial), paired: Rc::new(paired) }
    }
}

/// The device token a code bought for the host here, kept for the life of the process so a window opened again dials
/// with it: only a new launch asks for a code again. An account host keeps its token in its record instead.
struct Bought(Option<String>);

impl Global for Bought {}

/// The token a pairing bought in this process, which every shell after it presents.
pub fn bought(cx: &App) -> Option<String> {
    cx.try_global::<Bought>().and_then(|b| b.0.clone())
}

/// Stands the pair screen as the window's whole content.
pub fn open(road: Road, window: &mut Window, cx: &mut App) {
    window.replace_root(cx, |window, cx| {
        let pair = cx.new(|cx| PairScreen::new(road, window, cx));
        Root::new(pair, window, cx)
    });
}

pub struct PairScreen {
    road: Road,
    dark: Palette,
    code: Entity<InputState>,
    error: Option<String>,
    busy: bool,
    /// Asking whether a token is held now: the page is blank for that round trip.
    asking: bool,
    _asked: Option<Task<()>>,
    _spent: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl PairScreen {
    /// Stands in Graphite whatever the theme, as the web's page is `<html class="dark">` before pairing: no host is there
    /// yet to name the person's theme. The app's own theme is left as it is.
    pub fn new(road: Road, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let dark = theme::theme_by_id(theme::DARK).expect("the dark default is a theme").palette();
        let code = cx.new(|cx| InputState::new(window, cx));
        let typed = cx.subscribe_in(&code, window, |this, _, event: &InputEvent, window, cx| match event {
            InputEvent::Change => cx.notify(),
            InputEvent::PressEnter { .. } => this.submit(window, cx),
            _ => {}
        });
        let forward = cx.observe_window_activation(window, |this, window, cx| {
            if window.is_window_active() {
                this.ask(window, cx);
            }
        });
        code.update(cx, |code, cx| code.focus(window, cx));
        PairScreen {
            road,
            dark,
            code,
            error: None,
            busy: false,
            asking: false,
            _asked: None,
            _spent: None,
            _subscriptions: vec![typed, forward],
        }
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn busy(&self) -> bool {
        self.busy
    }

    pub fn asking(&self) -> bool {
        self.asking
    }

    pub fn set_code(&mut self, code: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.code.update(cx, |field, cx| field.set_value(code.to_owned(), window, cx));
    }

    /// The code screen asks again whether a token is held, as `BootGate` asks the shell on focus.
    fn ask(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.asking {
            return;
        }
        self.asking = true;
        cx.notify();
        let asked = (self.road.ask)(cx);
        self._asked = Some(cx.spawn_in(window, async move |this, cx| {
            let held = asked.await;
            _ = this.update_in(cx, |this, window, cx| {
                this.asking = false;
                cx.notify();
                if held {
                    (this.road.paired.clone())(window, cx);
                }
            });
        }));
    }

    pub fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let typed = self.code.read(cx).value().to_string();
        if self.busy || typed.trim().is_empty() {
            return;
        }
        self.error = None;
        let code = match sent_code(&typed) {
            Ok(code) => code,
            Err(refused) => {
                self.error = Some(refused.to_owned());
                cx.notify();
                return;
            }
        };
        self.busy = true;
        cx.notify();
        let spent = (self.road.redeem)(code, cx);
        self._spent = Some(cx.spawn_in(window, async move |this, cx| {
            let spent = spent.await;
            _ = this.update_in(cx, |this, window, cx| {
                this.busy = false;
                match spent {
                    Ok(()) => (this.road.paired.clone())(window, cx),
                    Err(said) => this.error = Some(said),
                }
                cx.notify();
            });
        }));
    }
}

impl Render for PairScreen {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.dark.clone();
        let page = div().debug_selector(|| "pair-screen".into()).size_full().bg(p.background).text_color(p.foreground);
        if self.asking {
            return theme::scoped_theme(theme::DARK, page.debug_selector(|| "booting".into()), cx);
        }
        let empty = self.code.read(cx).value().trim().is_empty();
        let me = cx.weak_entity();
        let pair =
            Button::primary("pair", if self.busy { PAIRING } else { PAIR }).held(self.busy || empty).on_click(move |_, window, cx| {
                _ = me.update(cx, |this, cx| this.submit(window, cx));
            });
        let refusal = match &self.error {
            Some(said) => Refusal::new().said(said.clone()),
            None => Refusal::new(),
        };
        let page = page.flex().items_center().justify_center().p(px(24.)).child(
            v_flex()
                .w(px(320.))
                .gap(px(12.))
                .child(div().text_size(px(18.)).line_height(px(28.)).font_weight(FontWeight::MEDIUM).child(HEADING))
                .child(div().text_size(text::NOTE.0).line_height(text::NOTE.1).text_color(p.muted_foreground).child(HINT))
                .child(div().text_size(text::SETTING_TITLE.0).line_height(text::SETTING_TITLE.1).child(LABEL))
                .child(Field::new(&self.code).sheet_words().height(px(32.)))
                .child(pair)
                .child(refusal),
        );
        theme::scoped_theme(theme::DARK, page, cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_code_is_read_as_the_host_takes_it() {
        assert_eq!(sent_code(" abcd-efgh ").as_deref(), Ok("ABCDEFGH"));
        assert_eq!(sent_code("ABCD-EFGH.c2lnbmVk").as_deref(), Ok("ABCDEFGH"));
        for paste in ["ABCD EFGH", "ABCD-EFG", "ABCD-EFGHJ", "ILOU-ABCD", "code: ABCD-EFGH"] {
            assert_eq!(sent_code(paste), Err(SHAPE_REFUSAL), "{paste:?}");
        }
    }

    #[test]
    fn the_device_names_the_host_it_is_at() {
        assert!(device_name("10.0.0.2:7777").ends_with(" in a browser at 10.0.0.2:7777"));
    }
}
