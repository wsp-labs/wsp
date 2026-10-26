// SPDX-License-Identifier: AGPL-3.0-only
//! Add a cloud provider as the dialog walks it (`cloudFlow.ts`): the provider where more than one is offered, then its
//! key checked live and saved. Every host answer lands here through a method named for it, so a test moves the flow
//! with fixture answers; the view's thin layer sends the calls and hands their answers back.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use wsp_host_client::Failure;

use super::words::{self, FIRST_CLOUD, KEY_REFUSED};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Provider,
    Key,
}

/// What a key that works read off the provider.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct Facts {
    pub account: String,
    pub plan: String,
    pub running: u64,
    pub limit: u64,
}

/// A refusal as the dialog shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refused {
    pub said: String,
    pub fix: Option<String>,
    pub kind: Option<String>,
}

impl From<Failure> for Refused {
    fn from(f: Failure) -> Self {
        Refused { said: f.said().to_owned(), fix: f.fix.as_deref().map(str::to_owned), kind: f.kind }
    }
}

/// The live check of the key in the field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Check {
    Idle,
    Checking,
    Works(Facts),
    /// The provider answered and refused the key.
    Refused(Refused),
    /// Nothing came back about the key at all: worth pressing again.
    Unreachable(Refused),
}

/// The words a key that works reads as: the account, the plan and the machines running of the most it may, each
/// left out where the provider did not say it.
pub fn key_works_line(facts: &Facts) -> String {
    let parts: Vec<String> = [
        (!facts.account.is_empty()).then(|| format!("Account {}", facts.account)),
        (!facts.plan.is_empty()).then(|| facts.plan.clone()),
        (facts.limit != 0).then(|| format!("{} of {} running at once", facts.running, facts.limit)),
    ]
    .into_iter()
    .flatten()
    .collect();
    if parts.is_empty() {
        "Works.".into()
    } else {
        format!("Works. {}.", parts.join(", "))
    }
}

/// `InitSetup`, as much of it as the dialog reads.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct Setup {
    #[serde(default)]
    pub keys: BTreeMap<String, bool>,
}

/// The clouds this host could be given a key for: every cloud its setup keys a row by, none before it has said.
pub fn clouds_offered(setup: Option<&Setup>) -> Vec<&'static str> {
    words::clouds().iter().map(|c| c.id).filter(|id| setup.is_some_and(|s| s.keys.contains_key(*id))).collect()
}

/// A call the flow asks the view's thin layer to send.
#[derive(Clone, Debug, PartialEq)]
pub enum Ask {
    /// `providers.checkKey`.
    Check { provider: String, key: String },
    /// `init.keys` where a key was typed; the dialog closes on the cloud added.
    Save { provider: String, key: Option<String> },
}

/// The dialog's whole state, for one open.
#[derive(Clone, Debug)]
pub struct Flow {
    pub provider: String,
    pub step: Step,
    pub check: Check,
    /// What the key field holds.
    pub key: String,
    pub setup: Option<Setup>,
    pub setup_refused: Option<String>,
    /// The key step's refusal.
    pub refused: Option<Refused>,
    /// Continue from the key is with the host.
    pub busy: bool,
    /// The step the flow opened on is decided once the setup is read.
    placed: bool,
}

impl Flow {
    /// A flow on `provider`'s key, waiting for the setup; none named, it opens where the setup says.
    pub fn new(provider: Option<&str>) -> Flow {
        Flow {
            provider: provider.unwrap_or(FIRST_CLOUD).to_owned(),
            step: Step::Key,
            check: Check::Idle,
            key: String::new(),
            setup: None,
            setup_refused: None,
            refused: None,
            busy: false,
            placed: provider.is_some(),
        }
    }

    pub fn cloud(&self) -> String {
        words::cloud_name(&self.provider)
    }

    pub fn offered(&self) -> Vec<&'static str> {
        clouds_offered(self.setup.as_ref())
    }

    /// The steps the dialog counts: the provider where more than one is offered, then the key.
    pub fn steps(&self) -> Vec<Step> {
        if self.offered().len() > 1 {
            vec![Step::Provider, Step::Key]
        } else {
            vec![Step::Key]
        }
    }

    /// The host holds a key for this provider.
    pub fn held(&self) -> bool {
        self.setup.as_ref().is_some_and(|s| s.keys.get(&self.provider) == Some(&true))
    }

    /// The setup read (`init.get`). The first one places a flow named by nobody: on the provider step where the host
    /// offers more than one, else at the one cloud's key.
    pub fn on_setup(&mut self, setup: Setup) {
        self.setup = Some(setup);
        self.setup_refused = None;
        if !self.placed {
            self.placed = true;
            let offered = self.offered();
            self.provider = offered.first().copied().unwrap_or(FIRST_CLOUD).to_owned();
            self.step = if offered.len() > 1 { Step::Provider } else { Step::Key };
        }
    }

    pub fn on_setup_refused(&mut self, failure: Failure) {
        self.setup_refused = Some(failure.message);
    }

    pub fn pick_provider(&mut self, id: &str) {
        self.provider = id.to_owned();
        self.check = Check::Idle;
    }

    pub fn go(&mut self, step: Step) {
        self.step = step;
    }

    /// The field changed: checked at once on a paste or Enter, else back to nothing until then.
    pub fn type_key(&mut self, text: &str, now: bool) -> Option<Ask> {
        self.key = text.to_owned();
        self.refused = None;
        if !now {
            self.check = Check::Idle;
            return None;
        }
        let typed = text.trim();
        if typed.is_empty() {
            self.check = Check::Idle;
            return None;
        }
        self.check = Check::Checking;
        Some(Ask::Check { provider: self.provider.clone(), key: typed.to_owned() })
    }

    /// The provider's answer for `typed`; dropped where the field has changed since, so a slow check never stands over
    /// a newer one.
    pub fn on_check(&mut self, typed: &str, answer: Result<Facts, Failure>) {
        if self.key.trim() != typed {
            return;
        }
        self.check = match answer {
            Ok(facts) => Check::Works(facts),
            Err(f) if f.kind.as_deref() == Some(KEY_REFUSED) => Check::Refused(f.into()),
            Err(f) => Check::Unreachable(f.into()),
        };
    }

    /// Continue from the key may be pressed: a key the provider took, or none typed and one held.
    pub fn key_ready(&self) -> bool {
        if self.key.trim().is_empty() {
            self.held()
        } else {
            matches!(self.check, Check::Works(_))
        }
    }

    /// Continue from the key: the key saved where one was typed, then the dialog closes on the cloud added.
    pub fn key_on(&mut self) -> Option<Ask> {
        if !self.key_ready() || self.busy {
            return None;
        }
        self.busy = true;
        self.refused = None;
        let key = Some(self.key.trim().to_owned()).filter(|k| !k.is_empty());
        Some(Ask::Save { provider: self.provider.clone(), key })
    }

    /// Continue from the key ended: the dialog closes on `Ok`, and a refusal stands under the field.
    pub fn on_saved(&mut self, answer: Result<Option<Setup>, Failure>) -> bool {
        self.busy = false;
        match answer {
            Ok(setup) => {
                if let Some(setup) = setup {
                    self.setup = Some(setup);
                }
                self.key.clear();
                self.check = Check::Idle;
                true
            }
            Err(f) => {
                self.refused = Some(f.into());
                false
            }
        }
    }

    /// The head: the step's title and its one line.
    pub fn head(&self) -> (String, Option<String>) {
        let cloud = self.cloud();
        match self.step {
            Step::Provider => ("Add a cloud provider".into(), Some("Where your sub-threads run.".into())),
            Step::Key => (format!("Add {cloud}"), Some(format!("wsp runs your sub-threads on your {cloud} account with this key."))),
        }
    }

    /// The count at the head's right: the step among the steps.
    pub fn figure(&self) -> String {
        let steps = self.steps();
        let at = steps.iter().position(|s| *s == self.step).map_or(0, |a| a + 1);
        format!("{at} of {}", steps.len())
    }
}
