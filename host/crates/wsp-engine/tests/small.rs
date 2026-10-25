// SPDX-License-Identifier: AGPL-3.0-only
//! Ports packages/engine/test/key-fingerprint.test.ts, provider-slot.test.ts, project-install.test.ts and
//! vault-cipher.test.ts. drop_template.rs and dotfiles_presets.rs have no TS test of their own to port.
#[path = "rest_common/mod.rs"]
mod common;

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use indexmap::IndexMap;
use wsp_wire::types::init_job::LoginState;
use wsp_wire::views::golden_image::{GoldenLogin, MachineKind, SealedImage, SealedVault, SealedVaultHeaderTo};
use wsp_wire::wire::capabilities::Capabilities;
use x25519_dalek::{PublicKey, StaticSecret};

use wsp_engine::errors::Result;
use wsp_engine::key_fingerprint::key_fingerprint;
use wsp_engine::machine::{
    BackendOffers, BackendPricing, Machine, MachineBackend, MachineListRow, MachineSpec, MachineState, Size, SnapshotRow,
    SnapshotStoragePricing, Waiting, WireSnapshotStoragePricing,
};
use wsp_engine::project_install::{check_scripts, install_script, project_installs, ProjectInstall};
use wsp_engine::provider_slot::ProviderSlot;
use wsp_engine::vault_cipher::{header_of, open, seal, VaultKey};
use wsp_wire::types::JsNumber;

fn strs(xs: &[&str]) -> Vec<String> {
    xs.iter().map(|s| s.to_string()).collect()
}

// ---- key-fingerprint

/// The ed25519 entry of the known_hosts fixture in ssh-backend.test.ts, and what `ssh-keygen -lf` printed for it.
const BLOB: &str = "AAAAC3NzaC1lZDI1NTE5AAAAIC6sV8jQCzynkpUOM40rRIjA7tPstUSjC+A/LVMnAZhP";

// TS: packages/engine/test/key-fingerprint.test.ts:11
#[test]
fn is_the_word_every_ssh_tool_prints_for_the_same_key() {
    assert_eq!(key_fingerprint(BLOB), "SHA256:Ge9MQ9S/Faik2WzsxidRXnoEOJRIHkJKTBkOJ903vq4");
}

/// An ed25519 public key as SPKI DER, base64: the 12 bytes of its header then 32 random ones.
fn spki_ed25519() -> String {
    let mut der = hex::decode("302a300506032b6570032100").unwrap();
    der.extend_from_slice(&rand::random::<[u8; 32]>());
    STANDARD.encode(der)
}

// TS: packages/engine/test/key-fingerprint.test.ts:15
#[test]
fn wears_that_one_form_for_a_place_link_key_too_which_travels_as_spki_der_rather_than_an_ssh_blob() {
    let der = spki_ed25519();
    let other = spki_ed25519();
    let fingerprint = key_fingerprint(&der);
    let body = fingerprint.strip_prefix("SHA256:").expect("the SHA256: prefix");
    assert_eq!(body.len(), 43, "{fingerprint}");
    assert!(body.chars().all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/'), "{fingerprint}");
    assert_ne!(fingerprint, key_fingerprint(&other));
}

// ---- provider-slot

struct StubPricing;

impl BackendPricing for StubPricing {
    fn rate_usd_per_hour(&self, _size: Size) -> f64 {
        0.0
    }
    fn default_size(&self) -> Size {
        Size { cpu: 0.0, mem_mb: 0.0 }
    }
    fn snapshot_storage(&self) -> SnapshotStoragePricing {
        SnapshotStoragePricing {
            wire: WireSnapshotStoragePricing { free_gb: JsNumber(0.0), usd_per_gb_month: JsNumber(0.0), billed_from: String::new() },
            free_named: None,
            usd_per_named_month: None,
        }
    }
}

fn caps(preview_urls: bool, snapshot_listing: bool) -> Capabilities {
    Capabilities {
        live_clone_forks: false,
        pause_mode: None,
        replaces_machine: false,
        preview_urls,
        signed_urls: false,
        callback_relay: false,
        disk_snapshots: false,
        images: false,
        snapshots_any_life: false,
        snapshot_listing,
        templates: false,
        sizes: vec![],
        kept: false,
        copies: false,
        own_network: false,
        sizes_last: false,
    }
}

fn spec() -> MachineSpec {
    MachineSpec {
        kind: MachineKind::Sandbox,
        template: None,
        from_snapshot: None,
        cpu: None,
        mem_mb: None,
        disk_gb: None,
        envs: None,
        labels: None,
        on_idle: None,
        idle_timeout_ms: None,
        idempotency_key: None,
        engine: None,
        copy: None,
        shares: None,
        binds: None,
    }
}

/// What a host with no provider key wires: every capability false, an empty listing, every other road refused.
struct Keyless;

#[async_trait(?Send)]
impl MachineBackend for Keyless {
    fn capabilities(&self) -> Capabilities {
        caps(false, false)
    }
    fn pricing(&self) -> Arc<dyn BackendPricing> {
        Arc::new(StubPricing)
    }
    async fn create(&self, _spec: MachineSpec, _waiting: Option<Waiting>, _sent: Option<String>) -> Result<Arc<dyn Machine>> {
        Err(common::other("no provider key"))
    }
    async fn get(&self, _id: &str) -> Result<Arc<dyn Machine>> {
        Err(common::other("no provider key"))
    }
    async fn list(&self, _labels: Option<&IndexMap<String, String>>) -> Result<Vec<MachineListRow>> {
        Ok(vec![])
    }
    async fn delete_snapshot(&self, _id: &str) -> Result<()> {
        Err(common::other("no provider key"))
    }
}

/// A real provider module: previews and a snapshot listing, and a list that records each call.
struct Real {
    listed: Arc<Mutex<Vec<String>>>,
    rows: Vec<MachineListRow>,
}

#[async_trait(?Send)]
impl MachineBackend for Real {
    fn capabilities(&self) -> Capabilities {
        caps(true, true)
    }
    fn pricing(&self) -> Arc<dyn BackendPricing> {
        Arc::new(StubPricing)
    }
    fn offers(&self) -> BackendOffers {
        BackendOffers { list_snapshots: true, ..BackendOffers::default() }
    }
    async fn create(&self, _spec: MachineSpec, _waiting: Option<Waiting>, _sent: Option<String>) -> Result<Arc<dyn Machine>> {
        Err(common::other("unused"))
    }
    async fn get(&self, _id: &str) -> Result<Arc<dyn Machine>> {
        Err(common::other("unused"))
    }
    async fn list(&self, _labels: Option<&IndexMap<String, String>>) -> Result<Vec<MachineListRow>> {
        self.listed.lock().unwrap().push("list".into());
        Ok(self.rows.clone())
    }
    async fn delete_snapshot(&self, _id: &str) -> Result<()> {
        Ok(())
    }
    async fn list_snapshots(&self, _ids: Option<&[String]>) -> Result<Vec<SnapshotRow>> {
        Ok(vec![])
    }
}

// TS: packages/engine/test/provider-slot.test.ts:12
#[tokio::test]
async fn delegates_every_read_and_call_to_the_module_it_holds_and_a_swap_moves_them_all_at_once() {
    let initial: Arc<dyn MachineBackend> = Arc::new(Keyless);
    let slot = ProviderSlot::new(initial);
    let backend = slot.backend();
    assert!(!backend.capabilities().preview_urls);
    assert!(backend.create(spec(), None, None).await.is_err());
    assert!(!backend.offers().list_snapshots);
    let listed: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let real: Arc<dyn MachineBackend> = Arc::new(Real { listed: listed.clone(), rows: vec![] });
    slot.swap(real.clone());
    let now = backend.capabilities();
    assert!(now.preview_urls);
    assert!(now.snapshot_listing);
    assert_eq!(backend.list(None).await.unwrap(), Vec::<MachineListRow>::new());
    assert_eq!(listed.lock().unwrap().clone(), strs(&["list"]));
    assert!(backend.offers().list_snapshots);
    assert_eq!(backend.list_snapshots(None).await.unwrap(), Vec::<SnapshotRow>::new());
    assert!(Arc::ptr_eq(&slot.current(), &real));
}

// TS: packages/engine/test/provider-slot.test.ts:36
#[tokio::test]
async fn a_backend_taken_off_the_slot_before_a_swap_still_runs_against_the_module_current_when_it_is_called() {
    let initial: Arc<dyn MachineBackend> = Arc::new(Keyless);
    let slot = ProviderSlot::new(initial);
    let taken = slot.backend();
    let rows = vec![MachineListRow { id: "m1".into(), state: MachineState::Running, labels: IndexMap::new(), size: None }];
    let next: Arc<dyn MachineBackend> = Arc::new(Real { listed: Arc::new(Mutex::new(Vec::new())), rows: rows.clone() });
    slot.swap(next);
    assert_eq!(taken.list(None).await.unwrap(), rows);
}

// ---- project-install

fn install(row: &str, command: &str, check: Option<&str>) -> ProjectInstall {
    ProjectInstall { row: row.into(), command: command.into(), check: check.map(String::from) }
}

// TS: packages/engine/test/project-install.test.ts:6
#[test]
fn is_the_lockfiles_command_in_catalog_order_one_per_ecosystem() {
    assert_eq!(
        project_installs(&strs(&["package-lock.json", "Cargo.lock"]), "/root/wsp"),
        vec![install("node", "npm ci", Some("npm --version")), install("rust", "cargo fetch --locked", None)]
    );
}

// TS: packages/engine/test/project-install.test.ts:13
#[test]
fn keeps_a_store_that_hard_links_into_the_checkout_inside_the_checkout() {
    // A store on another btrfs subvolume cannot link across and copies every file instead; a store made a
    // subvolume of its own under the checkout would be dropped by a snapshot of it.
    assert_eq!(
        project_installs(&strs(&["pnpm-lock.yaml"]), "/root/wsp"),
        vec![install("node", "pnpm install --frozen-lockfile --store-dir '/root/wsp/.pnpm-store'", Some("pnpm --version"))]
    );
    let spaced = project_installs(&strs(&["pnpm-lock.yaml"]), "/root/a folder/wsp");
    assert!(spaced[0].command.contains("'/root/a folder/wsp/.pnpm-store'"), "{}", spaced[0].command);
}

// TS: packages/engine/test/project-install.test.ts:20
#[test]
fn checks_a_manager_only_where_its_row_names_a_command_that_exits_0_on_one_that_works() {
    let checks: Vec<Option<String>> =
        project_installs(&strs(&["go.sum", "uv.lock", "Cargo.lock"]), "/root/wsp").into_iter().map(|i| i.check).collect();
    assert_eq!(checks, vec![None, None, None]);
    let vars: IndexMap<String, String> =
        [("PATH".to_string(), "/usr/bin:/bin".to_string()), ("PNPM_HOME".to_string(), "/opt/wsp/pnpm".to_string())].into_iter().collect();
    assert_eq!(
        check_scripts("pnpm --version", "/root/a folder", &vars),
        [
            "export PATH='/usr/bin:/bin'\nexport PNPM_HOME='/opt/wsp/pnpm'\ncd '/root/a folder' && pnpm --version".to_string(),
            "cd '/root/a folder' && env -i HOME='/root' bash -lc 'pnpm --version'".to_string(),
        ]
    );
}

// TS: packages/engine/test/project-install.test.ts:28
#[test]
fn is_nothing_for_a_repo_no_row_names_an_install_for() {
    assert_eq!(project_installs(&strs(&["package.json"]), "/root/wsp"), Vec::<ProjectInstall>::new());
}

// TS: packages/engine/test/project-install.test.ts:34
#[test]
fn runs_in_the_project_folder_and_writes_its_output_to_a_log_outside_it() {
    let script = install_script(&install("node", "npm ci", None), "/root/wsp", "/wsp/run/add-pr_1/install.log");
    assert_eq!(script, "mkdir -p '/wsp/run/add-pr_1' && cd '/root/wsp' && npm ci >> '/wsp/run/add-pr_1/install.log' 2>&1");
    assert!(!script.contains("/root/wsp/install.log"));
}

// ---- vault-cipher

fn image() -> SealedImage {
    SealedImage {
        name: "default".into(),
        version: 1.into(),
        hash: "a".repeat(64),
        recipe_hash: "rh".into(),
        recipe: None,
        pins: None,
        logins: vec![GoldenLogin { name: "codex".into(), state: LoginState::SignedIn }],
        sealed_at: "2026-09-12T00:00:00.000Z".into(),
        sealed_from: "this-mac".into(),
        vault: Some(SealedVault {
            sha256: "b".repeat(64),
            bytes: 128.into(),
            paths: 2.into(),
            held: None,
            taken_at: "2026-09-12T00:00:00.000Z".into(),
        }),
        used_bytes: None,
        place: None,
        no_secrets: None,
    }
}

const PLAIN: &[u8] = b"tar bytes that are the person's logins";

// TS: packages/engine/test/vault-cipher.test.ts:23
#[test]
fn a_places_key_round_trips_and_another_places_key_does_not_open_it() {
    let place = StaticSecret::random_from_rng(rand::rngs::OsRng);
    let other = StaticSecret::random_from_rng(rand::rngs::OsRng);
    let sealed = seal(PLAIN, &image(), &VaultKey::Public(PublicKey::from(&place).to_bytes())).unwrap();
    let header = header_of(&sealed).unwrap();
    assert_eq!(header.to, SealedVaultHeaderTo::Key);
    assert!(header.ephemeral.is_some());
    let opened = open(&sealed, &VaultKey::Private(place.to_bytes())).unwrap();
    assert_eq!(opened.plain, PLAIN.to_vec());
    let err = open(&sealed, &VaultKey::Private(other.to_bytes())).unwrap_err();
    assert!(err.to_string().contains("would not open"), "{err}");
}
