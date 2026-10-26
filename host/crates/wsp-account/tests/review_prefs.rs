// SPDX-License-Identifier: AGPL-3.0-only
//! Review probe: the notice for an icons folder that would not go drops node's call and path as the TS regex
//! `, \w+ '[^']*'$` does (preferences.ts:29), for a path that holds ", ".
#![allow(unused_variables, unused_imports)]
use std::rc::Rc;

use async_trait::async_trait;
use indexmap::IndexMap;
use serde_json::Value;
use wsp_account::account::core_types::{AccountResult, Entry};
use wsp_account::account::preferences::*;
use wsp_core::daemon_channel::DaemonChannel;
use wsp_wire::types::thread_defaults::AccessChoice;
use wsp_wire::views::harness::HarnessCatalog;
use wsp_wire::views::preferences::*;
use wsp_wire::views::workspace::Caller;
use wsp_words::exit::HostError;

struct Icons;
impl ServerIcons for Icons {
    fn folder(&self) -> String {
        "/srv/icons".to_string()
    }
    fn forget(&self) -> Result<(), String> {
        Err("EACCES: permission denied, rmdir '/srv/icons/a, b'".to_string())
    }
}

struct Stub {
    icons: Option<Rc<dyn ServerIcons>>,
}

#[async_trait(?Send)]
impl PreferencesCtx for Stub {
    fn env(&self) -> IndexMap<String, String> { IndexMap::new() }
    fn server_icons(&self) -> Option<Rc<dyn ServerIcons>> { self.icons.clone() }
    fn home_dir(&self) -> String { "/home/me".to_string() }
    fn adapter_ids(&self) -> Vec<String> { unimplemented!() }
    fn access_table(&self, _agent: &str) -> AccessTable { unimplemented!() }
    fn project_held(&self, _id: &str) -> bool { unimplemented!() }
    fn project_of(&self, _id: &str) -> Result<(String, String), HostError> { unimplemented!() }
    fn default_agent_of(&self, _prefs: &Preferences) -> String { unimplemented!() }
    fn session_thread_keys(&self) -> Vec<String> { unimplemented!() }
    fn thread_record_held(&self, _thread_id: &str) -> bool { unimplemented!() }
    async fn ready(&self) { unimplemented!() }
    async fn stored(&self) -> AccountResult<Option<Value>> { unimplemented!() }
    async fn store(&self, _prefs: &Preferences) -> AccountResult<()> { unimplemented!() }
    fn held(&self) -> Option<Preferences> { unimplemented!() }
    fn set_held(&self, _prefs: Preferences) { unimplemented!() }
    fn emit(&self, _event: Value) { unimplemented!() }
    fn on_workspace_deleted(&self, f: Rc<dyn Fn(&str)>) { let _ = f; }
    async fn entry_of(&self, _workspace_id: &str, _origin: Option<&Caller>) -> Result<Entry, HostError> { unimplemented!() }
    fn catalog_agents(&self) -> Vec<CatalogAgentKeys> { unimplemented!() }
    fn thread_env(&self, _entry: &Entry, _agent: &str) -> IndexMap<String, String> { unimplemented!() }
    fn checkout_of(&self, _entry: &Entry) -> Result<String, HostError> { unimplemented!() }
    fn is_local(&self, _entry: &Entry) -> bool { unimplemented!() }
    fn at_provider(&self, _entry: &Entry) -> bool { unimplemented!() }
    fn pause_mode(&self, _entry: &Entry) -> Option<String> { unimplemented!() }
    fn projects_on(&self, _place_id: &str) -> Vec<(String, String, String)> { unimplemented!() }
    async fn workspace_channel( &self, _workspace_id: &str, _on_event: Rc<dyn Fn(Value)>, _origin: Option<&Caller>, ) -> AccountResult<Rc<dyn DaemonChannel>> { unimplemented!() }
    async fn local_channel(&self, _on_event: Rc<dyn Fn(Value)>) -> AccountResult<Rc<dyn DaemonChannel>> { unimplemented!() }
    fn place_channel(&self, _place_id: &str, _on_event: Rc<dyn Fn(Value)>) -> Option<Rc<dyn DaemonChannel>> { unimplemented!() }
    fn place_name(&self, _place_id: &str) -> Option<String> { unimplemented!() }
    async fn setup_write(&self, _place_id: &str, _agent: &str, _change: Value) -> AccountResult<()> { unimplemented!() }
    fn setup_view(&self, _place_id: &str, _agent: &str) -> Value { unimplemented!() }
    fn drop_setup_refusal(&self, _place_id: &str, _agent: &str) -> bool { unimplemented!() }
    fn setup_refusal_moved(&self, _place_id: &str, _agent: &str) { unimplemented!() }
    fn relayed(&self) -> bool { unimplemented!() }
    fn forget_read(&self, _workspace_id: &str) { unimplemented!() }
}

#[test]
fn review_the_icons_notice_drops_nodes_call_and_path_for_a_path_with_a_comma() {
    let area = PreferencesArea::new(Rc::new(Stub { icons: Some(Rc::new(Icons)) }));
    let notice = area.icons_forgotten().expect("a notice");
    // node printed "permission denied" for this message through the TS's two replaces.
    assert!(notice.contains("permission denied") && !notice.contains("rmdir"), "{notice}");
}
