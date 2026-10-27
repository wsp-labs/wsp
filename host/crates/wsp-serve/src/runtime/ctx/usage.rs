// SPDX-License-Identifier: AGPL-3.0-only
// Each body is todo!() until its packet writes it; the allow goes with the last one.
#![allow(unused_variables)]

use super::Ctx;

/// What `usageArea` builds once and shares (usage.ts:46-50, 292-303): the price table, the ledger, the plan alerts, the
/// burn and the resets. The usage area, the status tracker and the turns' filings all read these one values.
pub struct UsageParts {
    pub prices: std::rc::Rc<wsp_machines::usage::accounts::PriceTable>,
    pub ledger: std::rc::Rc<wsp_machines::usage::ledger::UsageLedger>,
    pub alerts: std::rc::Rc<wsp_core::plan_alerts::PlanAlerts>,
    pub burn: wsp_machines::usage::accounts::Burn,
    pub resets: wsp_account::usage_reset::UsageResets,
}

impl Ctx {
    /// The usage parts of this context, built on first use and then the same ones.
    pub fn usage_parts(&self) -> std::rc::Rc<UsageParts> {
        todo!()
    }

    /// `void alerts.resume().catch(warn)` (usage.ts:49): arms the plan alerts the state holds.
    pub fn usage_arm(self: &std::rc::Rc<Ctx>) {
        todo!()
    }

    /// `createStatusTracker({..})` (status.ts:11-69): the tracker over the status rows and the poll's callbacks.
    pub fn usage_status_tracker(self: &std::rc::Rc<Ctx>) -> std::rc::Rc<wsp_core::status::StatusTracker> {
        todo!()
    }
}

#[async_trait::async_trait(?Send)]
impl wsp_account::account::usage::UsageCtx for Ctx {
    fn now(&self) -> i64 {
        todo!()
    }
    fn schedule(&self, ms: i64, f: Box<dyn FnOnce()>) -> wsp_core::clock::Cancel {
        todo!()
    }
    async fn usage_logs_on(&self) -> bool {
        todo!()
    }
    async fn places(&self, now: i64) -> Vec<wsp_wire::views::place_more::PlaceView> {
        todo!()
    }
    fn has_door(&self) -> bool {
        todo!()
    }
    fn place_away(&self, place_id: &str) -> bool {
        todo!()
    }
    fn name_of(&self, place_id: &str) -> String {
        todo!()
    }
    fn linked(&self, place_id: &str) -> bool {
        todo!()
    }
    fn signed_in_at(&self, place_id: &str, agent: &str) -> bool {
        todo!()
    }
    async fn place_for(&self, word: &str) -> Option<String> {
        todo!()
    }
    async fn box_login_path(&self, place_id: &str) -> Option<String> {
        todo!()
    }
    async fn box_exec(
        &self,
        place_id: &str,
        script: &str,
        timeout_ms: i64,
        stdin: &str,
    ) -> wsp_account::account::core_types::AccountResult<String> {
        todo!()
    }
    fn local_wired(&self) -> bool {
        todo!()
    }
    fn local_has_daemon(&self) -> bool {
        todo!()
    }
    fn local_env(&self) -> indexmap::IndexMap<String, String> {
        todo!()
    }
    fn local_home(&self, agent: &str) -> String {
        todo!()
    }
    async fn local_exec_lines(
        &self,
        script: &str,
        env: indexmap::IndexMap<String, String>,
        idle_ms: i64,
        deadline_ms: i64,
    ) -> wsp_account::account::core_types::AccountResult<Vec<String>> {
        todo!()
    }
    async fn read_logs_on(
        &self,
        place_id: &str,
        stores: &[wsp_frames::UsageStore],
    ) -> wsp_account::account::core_types::AccountResult<wsp_frames::UsageLogsReply> {
        todo!()
    }
    async fn sys_history(
        &self,
        target: &wsp_core::types::wiring::ReadingsTarget,
        frame: wsp_account::account::core_types::Frame,
        origin: Option<&wsp_wire::views::workspace::Caller>,
    ) -> wsp_account::account::core_types::AccountResult<serde_json::Value> {
        todo!()
    }
    async fn ready(&self) {
        todo!()
    }
    fn wsp_sessions(&self) -> Vec<String> {
        todo!()
    }
    fn projects_on(&self, place_id: &str) -> Vec<(String, String)> {
        todo!()
    }
    fn project_name(&self, id: &str) -> Option<String> {
        todo!()
    }
    fn vaulted_for(&self, agent: &str) -> Option<serde_json::Value> {
        todo!()
    }
    fn launch_of(&self, place_id: &str, agent: &str) -> (indexmap::IndexMap<String, String>, Option<String>, Option<String>) {
        todo!()
    }
    async fn login_home(&self, place_id: &str, agent: &str) -> Option<String> {
        todo!()
    }
    async fn read_sign_ins_here(&self) -> wsp_account::account::core_types::AccountResult<Option<Vec<(String, String)>>> {
        todo!()
    }
    fn on_agents_changed(&self, f: std::rc::Rc<dyn Fn()>) {
        let _ = self.hear("agents.changed", move |_| f());
    }
    async fn limits(&self) -> wsp_account::account::core_types::AccountResult<Vec<serde_json::Value>> {
        todo!()
    }
    async fn file_logs(
        &self,
        entries: Vec<wsp_account::account::usage::UsageEntry>,
        read: Vec<String>,
    ) -> wsp_account::account::core_types::AccountResult<()> {
        todo!()
    }
    async fn file_limit(
        &self,
        reading: wsp_account::account::usage::LimitReading,
    ) -> wsp_account::account::core_types::AccountResult<(serde_json::Value, serde_json::Value)> {
        todo!()
    }
    async fn alerts_read(
        &self,
        before: serde_json::Value,
        after: serde_json::Value,
    ) -> wsp_account::account::core_types::AccountResult<()> {
        todo!()
    }
    async fn used(
        &self,
        range: wsp_wire::types::usage::UsageRange,
        split: wsp_wire::types::usage::UsageSplit,
        outside: bool,
        labels: wsp_account::account::usage::LabelOf,
    ) -> wsp_account::account::core_types::AccountResult<wsp_wire::types::usage::UsedAnswer> {
        todo!()
    }
    async fn account_labels(&self) -> wsp_account::account::core_types::AccountResult<indexmap::IndexMap<String, String>> {
        todo!()
    }
    fn account_on_computer(
        &self,
        agent: &str,
        place_id: &str,
        computer_name: &str,
        limits: &[serde_json::Value],
    ) -> wsp_account::account::usage::AccountKey {
        todo!()
    }
    fn account_of(
        &self,
        agent: &str,
        named: Option<&wsp_wire::types::usage::HarnessLimitAccount>,
        place_id: &str,
        computer_name: &str,
    ) -> wsp_account::account::usage::AccountKey {
        todo!()
    }
    fn agent_label(&self, agent: &str) -> String {
        todo!()
    }
    fn model_label(&self, model: &str) -> String {
        todo!()
    }
    fn log_limit(&self, agent: &str, snapshot: &serde_json::Value) -> Option<wsp_wire::types::usage::HarnessLimit> {
        todo!()
    }
    fn plan_agents(&self) -> Vec<String> {
        todo!()
    }
    fn plan_read_command(&self, agent: &str, road: &wsp_account::account::usage::PlanRoad) -> String {
        todo!()
    }
    fn plan_parse(&self, agent: &str, out: &str) -> Option<wsp_wire::types::usage::HarnessLimit> {
        todo!()
    }
    async fn account_rows(
        &self,
        limits: Vec<serde_json::Value>,
        places: Vec<(String, String, Option<serde_json::Value>)>,
        name_of: wsp_account::account::usage::NameOf,
    ) -> wsp_account::account::core_types::AccountResult<wsp_wire::types::usage::AccountsAnswer> {
        todo!()
    }
    async fn account_names(
        &self,
        limits: Vec<serde_json::Value>,
        name_of: wsp_account::account::usage::NameOf,
    ) -> wsp_account::account::core_types::AccountResult<indexmap::IndexMap<String, String>> {
        todo!()
    }
    async fn kept_computer_names(
        &self,
        places: &[wsp_wire::views::place_more::PlaceView],
    ) -> wsp_account::account::core_types::AccountResult<indexmap::IndexMap<String, String>> {
        todo!()
    }
    fn computer_name(
        &self,
        places: &[wsp_wire::views::place_more::PlaceView],
        place_id: &str,
        kept: Option<&indexmap::IndexMap<String, String>>,
    ) -> String {
        todo!()
    }
    async fn spend_reset(
        &self,
        account: &str,
        credit_id: Option<&str>,
        on: Option<&str>,
    ) -> Result<wsp_wire::types::usage::ResetAnswer, wsp_words::exit::HostError> {
        todo!()
    }
    fn nap_refused_reason(&self, entry: &wsp_account::account::core_types::Entry) -> Option<String> {
        todo!()
    }
    fn stop_refused_reason(&self, entry: &wsp_account::account::core_types::Entry) -> Option<String> {
        todo!()
    }
    fn disk_reason(&self, entry: &wsp_account::account::core_types::Entry) -> Option<String> {
        todo!()
    }
    fn spawn(&self, f: futures::future::LocalBoxFuture<'static, ()>) {
        todo!()
    }
    fn warn(&self, line: &str) {
        todo!()
    }
}

#[async_trait::async_trait(?Send)]
impl wsp_account::usage_reset::UsageResetDeps for Ctx {
    fn store(&self) -> std::rc::Rc<dyn wsp_core::types::pending::Store> {
        todo!()
    }
    async fn limits(&self) -> Result<Vec<wsp_wire::types::usage::AccountLimit>, wsp_words::exit::HostError> {
        todo!()
    }
    fn agent_name(&self, agent: &str) -> String {
        todo!()
    }
    fn resets(&self, agent: &str) -> Option<std::rc::Rc<dyn wsp_account::usage_reset::Resets>> {
        todo!()
    }
    async fn places(&self) -> Result<wsp_account::usage_reset::PlaceOf, wsp_words::exit::HostError> {
        todo!()
    }
    async fn road(
        &self,
        agent: &str,
        place: &wsp_account::usage_reset::ResetPlace,
    ) -> Result<wsp_adapters::port::ResetRoad, wsp_words::exit::HostError> {
        todo!()
    }
    async fn run(
        &self,
        place: &wsp_account::usage_reset::ResetPlace,
        script: &str,
        agent: &str,
    ) -> Result<String, wsp_words::exit::HostError> {
        todo!()
    }
    async fn file(&self, ask: wsp_account::usage_reset::FileAsk<'_>) -> Result<String, wsp_words::exit::HostError> {
        todo!()
    }
    async fn row(&self, key: &str) -> Result<Option<wsp_wire::types::usage::AccountRow>, wsp_words::exit::HostError> {
        todo!()
    }
    fn uuid(&self) -> String {
        todo!()
    }
}

impl wsp_account::usage_reset::Resets for Ctx {
    fn read_command(&self, road: &wsp_adapters::port::ResetRoad) -> Result<String, String> {
        todo!()
    }
    fn parse_read(&self, stdout: &str) -> Option<wsp_adapters::port::ResetReading> {
        todo!()
    }
    fn spend_command(
        &self,
        road: &wsp_adapters::port::ResetRoad,
        idempotency_key: &str,
        credit_id: Option<&str>,
    ) -> Result<String, String> {
        todo!()
    }
    fn parse_spend(&self, stdout: &str) -> wsp_adapters::port::ResetSpend {
        todo!()
    }
    fn too_old(&self) -> String {
        todo!()
    }
}

#[async_trait::async_trait(?Send)]
impl wsp_account::account::status::StatusCtx for Ctx {
    async fn ready(&self) {
        todo!()
    }
    fn held(&self) -> Vec<wsp_account::account::core_types::Entry> {
        todo!()
    }
    fn view(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> serde_json::Map<String, serde_json::Value> {
        todo!()
    }
    fn rate_usd_per_hour(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> f64 {
        todo!()
    }
    fn row_reason(&self, entry: &wsp_account::account::core_types::Entry) -> Option<String> {
        todo!()
    }
    fn idle_at(&self, workspace_id: &str) -> Option<f64> {
        todo!()
    }
    fn away_line(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> Option<String> {
        todo!()
    }
    fn unreached_of(&self, entry: &wsp_account::account::core_types::Entry) -> Option<serde_json::Value> {
        todo!()
    }
    fn reach_of(&self, entry: &wsp_account::account::core_types::Entry) -> String {
        todo!()
    }
    fn has_daemon(&self, entry: &wsp_account::account::core_types::Entry) -> bool {
        todo!()
    }
    fn machine_reads(&self, entry: &wsp_account::account::core_types::Entry) -> (bool, Option<f64>, bool, bool) {
        todo!()
    }
    fn polled_reach_set(&self, workspace_id: &str, machine_id: &str, reach: &str) {
        todo!()
    }
    fn polled_reach_delete(&self, workspace_id: &str) {
        todo!()
    }
    fn boot_work_has(&self, workspace_id: &str) -> bool {
        todo!()
    }
    fn revive_daemon(&self, entry: &wsp_account::account::core_types::Entry, reach: &str) {
        todo!()
    }
    fn offer_daemon_again(&self, entry: &wsp_account::account::core_types::Entry, status: &serde_json::Value) {
        todo!()
    }
    fn read_version_again(&self, entry: &wsp_account::account::core_types::Entry, status: &serde_json::Value) {
        todo!()
    }
    async fn adopt_gone(
        &self,
        entry: &wsp_account::account::core_types::Entry,
        reason: &str,
    ) -> wsp_account::account::core_types::AccountResult<()> {
        todo!()
    }
    fn live(&self, workspace_id: &str) -> Option<wsp_account::account::core_types::Entry> {
        todo!()
    }
    async fn store_get(&self, collection: &str, id: &str) -> wsp_account::account::core_types::AccountResult<Option<serde_json::Value>> {
        todo!()
    }
    async fn store_put(&self, collection: &str, id: &str, value: serde_json::Value) -> wsp_account::account::core_types::AccountResult<()> {
        todo!()
    }
    async fn store_list(&self, collection: &str) -> wsp_account::account::core_types::AccountResult<Vec<serde_json::Value>> {
        todo!()
    }
    async fn store_delete(&self, collection: &str, id: &str) -> wsp_account::account::core_types::AccountResult<()> {
        todo!()
    }
    fn now(&self) -> i64 {
        todo!()
    }
    fn wall_now(&self) -> i64 {
        todo!()
    }
    fn owner(&self) -> String {
        todo!()
    }
    fn lost_workspace(&self, row: &wsp_account::account::status::ListedRow, owner: &str, now: i64) -> bool {
        todo!()
    }
    fn project_held(&self, id: &str) -> bool {
        todo!()
    }
    fn name_refusal(&self, name: &str) -> Option<String> {
        todo!()
    }
    fn places_wired(&self) -> String {
        todo!()
    }
    async fn golden_head(&self) -> Option<String> {
        todo!()
    }
    async fn confirm(
        &self,
        place: &str,
        machine_id: &str,
    ) -> Result<wsp_account::account::status::Confirmed, wsp_account::account::status::ReadFailure> {
        todo!()
    }
    async fn stop_unrecorded(&self, place: &str, machine_id: &str) -> Result<(), wsp_account::account::status::ReadFailure> {
        todo!()
    }
    fn attach(&self, record: wsp_core::types::wiring::WorkspaceRecord) -> wsp_account::account::core_types::Entry {
        todo!()
    }
    async fn persist(&self, record: &wsp_core::types::wiring::WorkspaceRecord) -> wsp_account::account::core_types::AccountResult<()> {
        todo!()
    }
    fn sync_daemon(&self, entry: &wsp_account::account::core_types::Entry) {
        todo!()
    }
    fn emit(&self, event: serde_json::Value) {
        todo!()
    }
    async fn emit_status(&self, entry: &wsp_account::account::core_types::Entry, reach: &str, reason: &str) {
        todo!()
    }
    fn spawn(&self, f: futures::future::LocalBoxFuture<'static, ()>) {
        todo!()
    }
    fn warn(&self, line: &str) {
        todo!()
    }
}
