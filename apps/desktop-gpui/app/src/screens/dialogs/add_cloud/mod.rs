// SPDX-License-Identifier: AGPL-3.0-only
//! Add a cloud provider (`settings/add/AddCloudDialog.tsx`, `cloudFlow.ts`, `cloudKey.ts`): one step dialog. The key
//! step says where the key is made, what it needs and what the machines cost, and checks it with the provider on a
//! paste or Enter; Continue saves the key and the dialog closes on the cloud added. The flow's state is
//! [`flow::Flow`]; this view sends what it asks and hands the answers back.

pub mod calls;
pub mod flow;
mod view;
pub mod words;

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{App, AppContext as _, Context, Entity, SharedString, Subscription, Task, Window};
use wsp_host_client::Failure;

use super::layer::{self, Closes};
use super::ADD_CLOUD;
use crate::screens::Host;
use flow::{Ask, Flow, Step};

/// Opens Add a cloud provider where the host's setup says.
pub fn open(host: &Host, window: &mut Window, cx: &mut App) {
    open_on(Some(host), Flow::new(None), window, cx);
}

/// Opens the dialog on `flow` over `host`; with none, on a flow staged from fixture answers, as a shot or a test draws
/// it. The key field takes the keys once the dialog stands, as the web's `autoFocus` does.
pub fn open_on(host: Option<&Host>, flow: Flow, window: &mut Window, cx: &mut App) -> Entity<AddCloud> {
    let view = cx.new(|cx| AddCloud::new(host.cloned(), flow, window, cx));
    layer::open(ADD_CLOUD, view.clone().into(), Closes::Outside, None, window, cx);
    view.update(cx, |this, cx| {
        if this.flow.step == Step::Key {
            this.key_input.update(cx, |s, cx| s.focus(window, cx));
        }
    });
    view
}

pub struct AddCloud {
    host: Option<Host>,
    pub flow: Flow,
    key_input: Entity<InputState>,
    placeholder: SharedString,
    /// The paste chord was pressed in the key field: its change is checked at once.
    pasted: bool,
    _subs: Vec<Subscription>,
    _reads: Task<()>,
}

impl AddCloud {
    fn new(host: Option<Host>, flow: Flow, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let key_input = cx.new(|cx| {
            let mut s = InputState::new(window, cx);
            s.toggle_masked(window, cx);
            s
        });
        if !flow.key.is_empty() {
            key_input.update(cx, |s, cx| s.set_value(flow.key.clone(), window, cx));
        }
        let subs = vec![cx.subscribe_in(&key_input, window, Self::on_key_input)];
        let mut this =
            AddCloud { host, flow, key_input, placeholder: SharedString::default(), pasted: false, _subs: subs, _reads: Task::ready(()) };
        this.read(cx);
        this
    }

    fn on_key_input(&mut self, input: &Entity<InputState>, event: &InputEvent, window: &mut Window, cx: &mut Context<Self>) {
        let text = input.read(cx).value().to_string();
        let ask = match event {
            InputEvent::Change if text != self.flow.key => {
                let now = std::mem::take(&mut self.pasted);
                self.flow.type_key(&text, now)
            }
            InputEvent::PressEnter { .. } => self.flow.type_key(&text, true),
            _ => return,
        };
        if let Some(ask) = ask {
            self.send(ask, window, cx);
        }
        cx.notify();
    }

    /// The read the dialog opens on: the setup.
    fn read(&mut self, cx: &mut Context<Self>) {
        let Some(host) = self.host.clone() else { return };
        let setup = host.run(|c| async move { calls::setup(&c).await });
        self._reads = cx.spawn(async move |this, cx| {
            let setup = setup.await;
            _ = this.update(cx, |this, cx| {
                match setup {
                    Ok(setup) => this.flow.on_setup(setup),
                    Err(f) => this.flow.on_setup_refused(f),
                }
                cx.notify();
            });
        });
    }

    /// Sends one call the flow asked for and hands its answer back. With no host, as a staged flow has, nothing goes.
    fn send(&mut self, ask: Ask, window: &mut Window, cx: &mut Context<Self>) {
        let Some(host) = self.host.clone() else { return };
        match ask {
            Ask::Check { provider, key } => {
                let typed = key.clone();
                let answer = host.run(move |c| async move { calls::check_key(&c, provider, key).await });
                cx.spawn(async move |this, cx| {
                    let answer = answer.await;
                    _ = this.update(cx, |this, cx| {
                        this.flow.on_check(&typed, answer);
                        cx.notify();
                    });
                })
                .detach();
            }
            Ask::Save { provider, key } => {
                let saved = key.map(|key| host.run(move |c| async move { calls::save_key(&c, provider, key).await }));
                cx.spawn_in(window, async move |this, cx| {
                    let answer: Result<Option<flow::Setup>, Failure> = match saved {
                        Some(saved) => saved.await.map(Some),
                        None => Ok(None),
                    };
                    _ = this.update_in(cx, |this, window, cx| {
                        if this.flow.on_saved(answer) {
                            this.close(window, cx);
                        }
                        cx.notify();
                    });
                })
                .detach();
            }
        }
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        layer::close(ADD_CLOUD, window, cx);
        cx.notify();
    }
}
