// SPDX-License-Identifier: AGPL-3.0-only
//! Hard tests of the small modules against the TS's own output (tests/fixtures/oracle/small.json, vault.json):
//! fingerprints, presets, the template drop, the provider slot, install lines and a vault node sealed. spec/small.md.
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use base64::Engine;
use indexmap::IndexMap;
use serde_json::{json, Value};

use wsp_engine::dotfiles_presets::{apt, PRELUDE};
use wsp_engine::drop_template::drop_template;
use wsp_engine::errors::{classify, EngineError, Result};
use wsp_engine::key_fingerprint::key_fingerprint;
use wsp_engine::machine::{
    BackendOffers, BackendPricing, Capabilities, Machine, MachineBackend, MachineListRow, MachineSpec, Size, SnapshotStoragePricing,
    Waiting,
};
use wsp_engine::project_install::{check_scripts, install_script, project_installs, ProjectInstall, INSTALL_STORES};
use wsp_engine::provider_slot::ProviderSlot;
use wsp_engine::vault_cipher::{open, seal, sealed_image_of, VaultKey};

fn oracle(name: &str) -> Value {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/oracle").join(name);
    serde_json::from_str(&fs::read_to_string(p).unwrap()).unwrap()
}

fn b64(s: &str) -> Vec<u8> {
    base64::engine::general_purpose::STANDARD.decode(s).unwrap()
}

// spec/small.md B3
#[test]
fn fingerprints_decode_base64_as_node_does() {
    for c in oracle("small.json")["fingerprints"].as_array().unwrap() {
        assert_eq!(key_fingerprint(c["key"].as_str().unwrap()), c["fp"].as_str().unwrap(), "{:?}", c["key"]);
    }
}

// spec/small.md B4
#[test]
fn apt_and_prelude_write_the_ts_text() {
    assert_eq!(apt("ripgrep", Some("rg")), "command -v rg >/dev/null 2>&1 || { apt-get update -qq; apt-get install -y -qq ripgrep; }");
    assert_eq!(apt("jq", None), "command -v jq >/dev/null 2>&1 || { apt-get update -qq; apt-get install -y -qq jq; }");
    assert_eq!(PRELUDE, "set -euo pipefail\nexport HOME=\"${HOME:-$(getent passwd \"$(id -u)\" | cut -d: -f6)}\"");
}

struct Priced;
impl BackendPricing for Priced {
    fn rate_usd_per_hour(&self, _size: Size) -> f64 {
        0.0
    }
    fn default_size(&self) -> Size {
        Size { cpu: 0.0, mem_mb: 0.0 }
    }
    fn snapshot_storage(&self) -> SnapshotStoragePricing {
        SnapshotStoragePricing {
            wire: serde_json::from_value(json!({ "freeGb": 0, "usdPerGbMonth": 0, "billedFrom": "" })).unwrap(),
            free_named: None,
            usd_per_named_month: None,
        }
    }
}

fn caps(templates: bool) -> Capabilities {
    serde_json::from_value(
        json!({ "liveCloneForks": false, "replacesMachine": false, "previewUrls": templates, "signedUrls": false, "callbackRelay": false,
        "diskSnapshots": false, "images": false, "snapshotsAnyLife": false, "snapshotListing": false, "templates": templates, "sizes": [],
        "kept": false, "copies": false, "ownNetwork": false }),
    )
    .unwrap()
}

/// A backend that records template deletes and lists, answering each delete with `answer`.
struct Templates {
    name: &'static str,
    caps: bool,
    offers: bool,
    answer: Option<EngineError>,
    calls: Mutex<Vec<String>>,
}

impl Templates {
    fn new(name: &'static str, caps: bool, offers: bool, answer: Option<EngineError>) -> Arc<Self> {
        Arc::new(Templates { name, caps, offers, answer, calls: Mutex::new(vec![]) })
    }
}

#[async_trait]
impl MachineBackend for Templates {
    fn capabilities(&self) -> Capabilities {
        caps(self.caps)
    }
    fn pricing(&self) -> Arc<dyn BackendPricing> {
        Arc::new(Priced)
    }
    fn offers(&self) -> BackendOffers {
        BackendOffers { templates: self.offers, ..BackendOffers::default() }
    }
    async fn create(&self, _spec: MachineSpec, _waiting: Option<Waiting>, _sent: Option<String>) -> Result<Arc<dyn Machine>> {
        Err(EngineError::Other { message: "no".into(), code: None })
    }
    async fn get(&self, _id: &str) -> Result<Arc<dyn Machine>> {
        Err(EngineError::Other { message: "no".into(), code: None })
    }
    async fn list(&self, _labels: Option<&IndexMap<String, String>>) -> Result<Vec<MachineListRow>> {
        self.calls.lock().unwrap().push(format!("{} list", self.name));
        Ok(vec![])
    }
    async fn delete_snapshot(&self, _id: &str) -> Result<()> {
        Ok(())
    }
    async fn delete_template(&self, id: &str) -> Result<()> {
        self.calls.lock().unwrap().push(format!("{} delete {id}", self.name));
        match &self.answer {
            Some(e) => Err(e.clone()),
            None => Ok(()),
        }
    }
}

// spec/small.md B5
#[tokio::test]
async fn drop_template_ignores_a_missing_template_only() {
    let fine = Templates::new("a", true, true, None);
    drop_template(fine.as_ref(), Some("tpl_1")).await.unwrap();
    drop_template(fine.as_ref(), None).await.unwrap();
    assert_eq!(*fine.calls.lock().unwrap(), vec!["a delete tpl_1".to_string()]);
    let missing = Templates::new("b", true, true, Some(EngineError::Provider(classify(404, None, Some("Template not found"), None))));
    drop_template(missing.as_ref(), Some("tpl_2")).await.unwrap();
    let refused = Templates::new("c", true, true, Some(EngineError::Provider(classify(409, None, Some("in use"), None))));
    assert_eq!(drop_template(refused.as_ref(), Some("tpl_3")).await.unwrap_err().to_string(), "in use");
    for (caps, offers) in [(false, true), (true, false)] {
        let without = Templates::new("d", caps, offers, None);
        drop_template(without.as_ref(), Some("tpl_4")).await.unwrap();
        assert!(without.calls.lock().unwrap().is_empty(), "a backend without templates is asked nothing");
    }
}

// spec/small.md B6
#[tokio::test]
async fn a_handle_taken_before_a_swap_reaches_the_new_module() {
    let first = Templates::new("first", false, false, None);
    let second = Templates::new("second", true, true, None);
    let slot = ProviderSlot::new(first.clone());
    let handle = slot.backend();
    assert!(!handle.capabilities().templates);
    handle.list(None).await.unwrap();
    slot.swap(second.clone());
    assert!(handle.capabilities().templates);
    assert!(handle.offers().templates);
    handle.list(None).await.unwrap();
    handle.delete_template("t").await.unwrap();
    assert_eq!(*first.calls.lock().unwrap(), vec!["first list".to_string()]);
    assert_eq!(*second.calls.lock().unwrap(), vec!["second list".to_string(), "second delete t".to_string()]);
    let current: Arc<dyn MachineBackend> = second.clone();
    assert!(Arc::ptr_eq(&slot.current(), &current));
}

// spec/small.md B7 to B9
#[test]
fn install_lines_match_the_ts_oracle() {
    let o = &oracle("small.json")["installs"];
    let roots: Vec<String> =
        ["pnpm-lock.yaml", "package-lock.json", "bun.lockb", "uv.lock", "Cargo.lock"].iter().map(|s| s.to_string()).collect();
    let want: Vec<ProjectInstall> = o["projectInstalls"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| ProjectInstall {
            row: i["row"].as_str().unwrap().into(),
            command: i["command"].as_str().unwrap().into(),
            check: i["check"].as_str().map(String::from),
        })
        .collect();
    assert_eq!(project_installs(&roots, "/root/my proj//"), want);
    assert_eq!(*INSTALL_STORES, o["stores"].as_array().unwrap().iter().map(|s| s.as_str().unwrap().to_string()).collect::<Vec<_>>());
    let i = ProjectInstall { row: "r".into(), command: "pnpm install".into(), check: None };
    assert_eq!(install_script(&i, "/root/my proj", "/root/.wsp/logs/install's.log"), o["installScript"].as_str().unwrap());
    assert_eq!(
        install_script(&ProjectInstall { row: "r".into(), command: "x".into(), check: None }, "d", "plain.log"),
        o["installScriptNoSlash"].as_str().unwrap()
    );
    let vars: IndexMap<String, String> =
        [("PNPM_HOME", "/root/.local/share/pnpm"), ("A", "it's")].iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    let want: Vec<String> = o["checkScripts"].as_array().unwrap().iter().map(|s| s.as_str().unwrap().to_string()).collect();
    assert_eq!(check_scripts("pnpm --version", "/root/p", &vars).to_vec(), want);
}

fn key32(s: &str) -> [u8; 32] {
    b64(s).try_into().unwrap()
}

// spec/small.md B10 to B12
#[test]
fn a_vault_node_sealed_opens_here() {
    let o = oracle("vault.json");
    let sealed = b64(o["sealed"].as_str().unwrap());
    let opened = open(&sealed, &VaultKey::Private(key32(o["privateKey"].as_str().unwrap()))).unwrap();
    assert_eq!(opened.plain, b64(o["plain"].as_str().unwrap()));
    assert_eq!(serde_json::to_value(&opened.image).unwrap()["recipeHash"], json!("rh"));
    assert_eq!(sealed_image_of(&sealed).unwrap(), opened.image);
    // Sealed here to the same public key: the header keeps the TS's key order and the place's key opens it.
    let mine = seal(b"plain", &opened.image, &VaultKey::Public(key32(o["publicKey"].as_str().unwrap()))).unwrap();
    let line = String::from_utf8(mine[..mine.iter().position(|b| *b == b'\n').unwrap()].to_vec()).unwrap();
    let header: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(
        header.as_object().unwrap().keys().cloned().collect::<Vec<_>>(),
        vec!["format", "cipher", "to", "salt", "nonce", "ephemeral", "image"]
    );
    assert_eq!(b64(header["ephemeral"].as_str().unwrap())[..12], [0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x6e, 0x03, 0x21, 0x00]);
    assert_eq!(b64(header["salt"].as_str().unwrap()).len(), 16);
    assert_eq!(b64(header["nonce"].as_str().unwrap()).len(), 12);
    assert_eq!(open(&mine, &VaultKey::Private(key32(o["privateKey"].as_str().unwrap()))).unwrap().plain, b"plain");
}

// spec/small.md B11
#[test]
fn each_refusal_says_its_words() {
    let o = oracle("vault.json");
    let private = VaultKey::Private(key32(o["privateKey"].as_str().unwrap()));
    let public = VaultKey::Public(key32(o["publicKey"].as_str().unwrap()));
    let sealed = b64(o["sealed"].as_str().unwrap());
    let words = |r: Result<wsp_engine::vault_cipher::OpenedVault>| r.unwrap_err().to_string();
    assert_eq!(words(open(&sealed, &public)), "opening a vault sealed to a place needs that place's private key");
    assert_eq!(words(open(b"no newline here", &private)), "this file is not a sealed wsp image: it has no header line");
    assert_eq!(words(open(b"{not json\nbody", &private)), "this file is not a sealed wsp image: its header is not one wsp wrote");
    assert_eq!(
        words(open(b"{\"format\":\"other\"}\nbody", &private)),
        "this file is not a sealed wsp image: its header is not one wsp wrote"
    );
    let line = o["headerLine"].as_str().unwrap();
    let mut header: Value = serde_json::from_str(line).unwrap();
    header["to"] = json!("passphrase");
    assert_eq!(
        words(open(format!("{header}\nbody-bytes-long-enough").as_bytes(), &private)),
        "this vault was sealed to a passphrase, not to a place's key"
    );
    let mut header: Value = serde_json::from_str(line).unwrap();
    header.as_object_mut().unwrap().remove("ephemeral");
    assert_eq!(
        words(open(format!("{header}\nbody-bytes-long-enough").as_bytes(), &private)),
        "this vault names no ephemeral key and cannot be opened"
    );
    assert_eq!(words(open(format!("{line}\nshort").as_bytes(), &private)), "the sealed vault is shorter than its own tag");
    let mut edited = sealed.clone();
    let last = edited.len() - 1;
    edited[last] ^= 1;
    assert_eq!(
        words(open(&edited, &private)),
        "the sealed vault would not open: the passphrase or the key is not the one it was sealed to, or the file was changed"
    );
    let other = x25519_dalek::StaticSecret::from([7u8; 32]);
    assert_eq!(
        words(open(&sealed, &VaultKey::Private(other.to_bytes()))),
        "the sealed vault would not open: the passphrase or the key is not the one it was sealed to, or the file was changed"
    );
    assert_eq!(
        seal(b"x", &sealed_image_of(&sealed).unwrap(), &private).unwrap_err().to_string(),
        "sealing to a place needs its public key"
    );
}
