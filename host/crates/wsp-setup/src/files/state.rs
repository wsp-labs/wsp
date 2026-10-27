// SPDX-License-Identifier: AGPL-3.0-only
//! `cli/state.ts`: which state file a run works on, and the store that keeps it.
use std::fs;
use std::rc::Rc;

use wsp_cli::serving_home::{home_named, real_state, serving_home};
use wsp_store::store::Store;
use wsp_wire::types::env::HOME_ENV;

use crate::context::Env;
use crate::paths::{join, resolve};

/// The name this repository's own root package.json carries.
const CHECKOUT_PACKAGE: &str = "wsp";


/// `devCheckoutState` (`cli/state.ts:29`): a checkout's own `.wsp/state.json`, where the folder's package.json names the workspace.
pub fn dev_checkout_state(cwd: &str) -> Option<String> {
    let text = fs::read_to_string(join(&[cwd, "package.json"])).ok()?;
    let parsed: serde_json::Value = serde_json::from_str(&text).ok()?;
    (parsed.get("name")?.as_str()? == CHECKOUT_PACKAGE).then(|| join(&[cwd, ".wsp", "state.json"]))
}

/// `StatePick` (`cli/state.ts:34`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StatePick {
    pub path: String,
    pub note: Option<String>,
}

fn passed_over_line(chosen: &str, by: &str, cwd: &str, dev: &str) -> String {
    format!("this runs against {chosen}, named by {by}; the checkout in {cwd} marks {dev}, which this run does not use.")
}

/// `statePick(flag, cwd, env)`: `--state`, then `WSP_HOME`, then a checkout's own, then the serving home's.
pub fn state_pick(flag: Option<&str>, cwd: &str, env: &Env, user: &str) -> StatePick {
    let dev = dev_checkout_state(cwd);
    let home = home_named(env.get(HOME_ENV).map(String::as_str));
    let named = match (flag, home) {
        (Some(path), _) => Some((path.to_string(), "--state")),
        (None, Some(home)) => Some((join(&[&home, "state.json"]), HOME_ENV)),
        (None, None) => None,
    };
    let Some((path, by)) = named else {
        return StatePick { path: dev.unwrap_or_else(|| join(&[&serving_home(env, user), "state.json"])), note: None };
    };
    match dev {
        Some(dev) if real_state(&resolve(cwd, &dev)) != real_state(&resolve(cwd, &path)) => {
            let note = passed_over_line(&resolve(cwd, &path), by, cwd, &dev);
            StatePick { path, note: Some(note) }
        }
        _ => StatePick { path, note: None },
    }
}

/// `defaultStatePath` (`cli/state.ts:58`).
pub fn default_state_path(_cwd: &str, _env: &Env, _user: &str) -> String {
    todo!()
}

/// `statePathFrom(flag, env, note)`: absolute, so the lock, the token and the recipe name one path whatever the cwd is.
pub fn state_path_from(flag: Option<&str>, env: &Env, cwd: &str, user: &str, note: &dyn Fn(&str)) -> String {
    let pick = state_pick(flag, cwd, env, user);
    if let Some(line) = &pick.note {
        note(line);
    }
    resolve(cwd, &pick.path)
}

/// `statesHere` (`cli/state.ts:72`).
pub fn states_here(_state_path: &str, _cwd: &str, _env: &Env, _user: &str) -> Vec<String> {
    todo!()
}

/// `stateStore` (`cli/state.ts:80`).
pub fn state_store(_state_path: &str, _env: &Env) -> Rc<dyn Store> {
    todo!()
}
