// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/devices.ts at 65f6b89c7. Spec: spec/devices.md.
//!
//! Pairing codes and the device tokens they mint, kept in the store so a restart does not lock a paired computer
//! out. A code is one use; a device token is minted once, handed over once and never stored: only its sha256 is kept,
//! and every reading of it is a timing safe compare of that hash.
use std::rc::Rc;

use async_trait::async_trait;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use wsp_frames::True;
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::views::workspace_more::{ThreadScope, WorkspaceOrigin};
use wsp_wire::wire::devices::{DeviceVia, DeviceView, PAIR_CODE_ALPHABET, PAIR_CODE_LENGTH};

use wsp_words::exit::HostError;
use wsp_words::js::{collate, js_trim};
use wsp_words::js_date::iso_string;

use crate::ids::Ids;
use crate::types::pending::Store;

// /** One document per paired computer, keyed by its id. */
// const DEVICES = "devices";
pub const DEVICES: &str = "devices";
// /** One document per unspent pairing code, keyed by the code itself, so a redeem is one read. */
// const PAIRINGS = "pairings";
pub const PAIRINGS: &str = "pairings";
// /** One document per key this host took away, keyed by the fingerprint: a device admitted through the account
//  * carries the same key when it signs in again while the relay mints it a fresh id, so the key is what a refusal
//  * has to remember. Kept for good: a revocation made here is not the account's to undo, and a code from
//  * wsp host pair is the road back in for that computer. */
// const REVOKED = "revoked-keys";
pub const REVOKED: &str = "revoked-keys";

// /** Two strings of the same bytes, in a time that does not say where they first differ. The one compare every road
//  * into this host makes, whether it holds the host's own token or a device's hash. */
// export function safeEqual(a: string, b: string): boolean {
//   const ab = Buffer.from(a);
//   const bb = Buffer.from(b);
//   return ab.length === bb.length && timingSafeEqual(ab, bb);
// }
pub fn safe_equal(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

// /** The road a thread's token reaches this host by: this computer's own loopback for a turn beside the host, a
//  * machine's road for every other. Never the paired road, which is a computer of the person's and holds no scope. */
// export type ScopedRoad = Exclude<WorkspaceOrigin, "paired">;
/// `here` or `relayed`; never `paired`.
pub type ScopedRoad = WorkspaceOrigin;

// /** A device as the host reads it off a token: what the listing shows, and the road its scope was minted for, which
//  * the listing leaves out since it is the host's reading and nothing a person acts on. */
// export type HeldDevice = DeviceView & { road?: ScopedRoad };
#[derive(Debug, Clone, PartialEq)]
pub struct HeldDevice {
    pub view: DeviceView,
    pub road: Option<ScopedRoad>,
}

/// What `thread_of` answers: the thread a token is, and the road its requests are stamped with.
#[derive(Debug, Clone, PartialEq)]
pub struct ThreadOfDevice {
    pub by: ThreadScope,
    pub road: ScopedRoad,
}

// /** The thread a device's token is, and the road the host stamps on its requests; nothing for a device that is no
//  * thread. A scoped record with no road is one an older host wrote for a machine, so it reads as a machine's. The
//  * one reading, taken by the socket door and the JSON routes alike. */
// export function threadOf(device: HeldDevice): { by: ThreadScope; road: ScopedRoad } | undefined {
//   return device.scope === undefined ? undefined : { by: device.scope, road: device.road ?? "relayed" };
// }
pub fn thread_of(device: &HeldDevice) -> Option<ThreadOfDevice> {
    device.view.scope.clone().map(|by| ThreadOfDevice { by, road: device.road.unwrap_or(WorkspaceOrigin::Relayed) })
}

// /** What the store keeps for a device: never the token, only the digest a presented one is compared against. */
// interface DeviceRecord {
//   id: string;
//   name: string;
//   tokenHash: string;
//   createdAt: string;
//   lastSeenAt: string;
//   /** Set on a token the host minted into one turn's launch rather than one a person's computer redeemed a code
//    * for: what that token may drive. */
//   scope?: ThreadScope;
//   /** Set beside `scope` at the mint, and on no other road: where the turn that holds the token runs. */
//   road?: ScopedRoad;
//   /** Set beside `scope` on a token minted for a side question rather than a turn: it shares the turn's scope, so the
//    * thread running says nothing of whether its answer is still awaited. */
//   aside?: true;
//   /** Set on the browser wsp init opened here, whose code init itself minted: read as the owner. */
//   here?: true;
//   /** Set on a device admitted through the account rather than by a code: which computer it is there, the key it
//    * proved and the key that signed for it. The key is kept because a device admitted here may sign the next one's
//    * admission, and the host trusts no key it has not seen at an admission. */
//   via?: DeviceVia;
// }
/// What the store keeps for a device, keys in this order (spec Behaviour 6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceRecord {
    pub id: String,
    pub name: String,
    pub token_hash: String,
    pub created_at: String,
    pub last_seen_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<ThreadScope>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub road: Option<ScopedRoad>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aside: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub here: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub via: Option<DeviceVia>,
}

// interface PairingRecord {
//   code: string;
//   expiresAt: number;
//   /** Carried onto the device the code admits. */
//   here?: true;
// }
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingRecord {
    pub code: String,
    pub expires_at: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub here: Option<bool>,
}

// /** What the store keeps for a key this host took away: the fingerprint, which is the whole of it. No clock is
//  * read here, since every method of the door below is handed the one the server injects. */
// interface RevokedRecord {
//   fingerprint: string;
// }
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RevokedRecord {
    pub fingerprint: String,
}

// /** The one digest of a token this repo keeps or compares: what the store holds for a device, and what the loopback
//  * page carries of the host's own token for the desktop shell to compare against the token file. */
// export const tokenDigest = (token: string): string => createHash("sha256").update(token).digest("hex");
pub fn token_digest(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

// /** What a redeem hands back: the token, once, and the record every later listing shows. */
// export interface PairedDevice {
//   deviceId: string;
//   deviceToken: string;
//   device: DeviceView;
// }
#[derive(Debug, Clone, PartialEq)]
pub struct PairedDevice {
    pub device_id: String,
    pub device_token: String,
    pub device: DeviceView,
}

/// What `issue` answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssuedCode {
    pub code: String,
    pub expires_at: i64,
}

/// `mint`'s optional fourth argument.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MintMore {
    pub road: Option<ScopedRoad>,
    pub aside: bool,
}

// /** The pairing door the protocol server calls. Every method takes the clock's reading rather than reading one, so
//  * the server's own injectable clock is the only one in the system. */
// export interface DeviceDoor {
//   /** A fresh code, and when it stops being one. `here` is wsp init minting for the browser it opens on this
//    * computer: the device that code admits is read as the owner. */
//   issue(at: { now: number; ttlMs: number; here?: true }): Promise<{ code: string; expiresAt: number }>;
//   /** Spends the code for a device of that name, or nothing when the host holds no such unexpired code. A spent or
//    * expired code is deleted either way, so one guess never gets two tries. */
//   redeem(code: string, name: string, now: number): Promise<PairedDevice | undefined>;
//   /** Spends a code with no device behind it, for a road that proves itself another way: a place holds a key of its
//    * own from the join on, so the code buys the record and never a token. One code store, so a code spent by either
//    * road is spent for both. */
//   spend(code: string, now: number): Promise<boolean>;
//   /** A device with no code and no scope behind it: a road that proved the person's intent another way (a place's
//    * join spent the code the person carried across the room) asks for the token its own window will hold. Stored,
//    * hashed, listed and revoked exactly as every other device is. */
//   admit(name: string, now: number): Promise<PairedDevice>;
//   /** A device the account admitted: no code was spent, and an admission signed by a key this host already trusts
//    * is what let it in. Stored, hashed, listed and revoked exactly as a device that redeemed a code is; `via` is
//    * what tells them apart in a listing and what a revoke here remembers. */
//   admitAccount(name: string, via: DeviceVia, now: number): Promise<PairedDevice>;
//   /** Whether this host took a device holding that key away and still remembers it, which is what keeps an
//    * admission it still holds from letting it back in through the account. */
//   refuses(fingerprint: string): Promise<boolean>;
//   /** A device with no pairing code behind it: the host itself minting a token for a turn it is about to launch,
//    * scoped to that turn's thread. The same door as a redeem, so a scoped token is revoked, listed and read by the
//    * one road every other token takes. */
//   mint(name: string, scope: ThreadScope, now: number, more?: { road?: ScopedRoad; aside?: true }): Promise<PairedDevice>;
//   /** Takes away every side question's token. A host's load calls it before it can mint one of its own, so each it
//    * finds was minted by a process that went down before the answer came. */
//   revokeAsides(): Promise<void>;
//   /** The device this token names, or nothing when no device holds it. Reads only: the JSON routes read a token on
//    * every request, and a write there would rewrite the whole state file each time. */
//   match(token: string): Promise<HeldDevice | undefined>;
//   /** Moves a device's last seen. The auth frame is the one road that calls this; a redeem stamps its own. */
//   seen(id: string, now: number): Promise<HeldDevice | undefined>;
//   list(): Promise<DeviceView[]>;
//   /** True when a device of that id was there to take away. */
//   revoke(id: string): Promise<boolean>;
// }
#[async_trait(?Send)]
pub trait DeviceDoor {
    /// `issue({ now, ttlMs, here })`.
    async fn issue(&self, now: i64, ttl_ms: u64, here: bool) -> Result<IssuedCode, HostError>;
    async fn redeem(&self, code: &str, name: &str, now: i64) -> Result<Option<PairedDevice>, HostError>;
    async fn spend(&self, code: &str, now: i64) -> Result<bool, HostError>;
    async fn admit(&self, name: &str, now: i64) -> Result<PairedDevice, HostError>;
    async fn admit_account(&self, name: &str, via: DeviceVia, now: i64) -> Result<PairedDevice, HostError>;
    async fn refuses(&self, fingerprint: &str) -> Result<bool, HostError>;
    async fn mint(&self, name: &str, scope: ThreadScope, now: i64, more: MintMore) -> Result<PairedDevice, HostError>;
    async fn revoke_asides(&self) -> Result<(), HostError>;
    async fn r#match(&self, token: &str) -> Result<Option<HeldDevice>, HostError>;
    async fn seen(&self, id: &str, now: i64) -> Result<Option<HeldDevice>, HostError>;
    async fn list(&self) -> Result<Vec<DeviceView>, HostError>;
    async fn revoke(&self, id: &str) -> Result<bool, HostError>;
}

// /** The one refusal for a runtime served without a store to keep devices in, so the ops answer plainly rather than
//  * pretending nobody is paired. */
// export const NO_DEVICE_DOOR = "this runtime keeps no devices; the host that serves the app wires one";
pub const NO_DEVICE_DOOR: &str = "this runtime keeps no devices; the host that serves the app wires one";

// export function makeDevices(store: Store): DeviceDoor {
//   const devices = async (): Promise<DeviceRecord[]> => (await store.list(DEVICES)).filter(isDevice);
//
//   // One writer over the device and pairing records: every road that reads a record and writes it back goes through
//   // this chain, so two sockets cannot each spend one code, and a revoke that lands between another road's read and
//   // its write cannot be undone by that write. Reads outside it (match, list) never write, so they need no place here.
//   let writes: Promise<unknown> = Promise.resolve();
//   const oneAtATime = <T>(work: () => Promise<T>): Promise<T> => {
//     const next = writes.then(work);
//     writes = next.catch(() => undefined);
//     return next;
//   };
//
//   /** One device record and the one token it will ever hand over. Both roads that make a device come through here,
//    * so a scoped token is stored, hashed and named by exactly the rule a paired computer's is. */
//   const admit = async (name: string, scope: ThreadScope | undefined, now: number, more: { here?: true; via?: DeviceVia; road?: ScopedRoad; aside?: true } = {}): Promise<PairedDevice> => {
//     const deviceToken = randomBytes(24).toString("base64url");
//     const at = new Date(now).toISOString();
//     const record: DeviceRecord = {
//       // Eight bytes, not four: the id keys the store, so two devices that drew the same one would be one record
//       // and the older computer's access would vanish under the newer.
//       id: `d_${randomBytes(8).toString("hex")}`,
//       name: name.trim() === "" ? "a paired computer" : name.trim(),
//       tokenHash: tokenDigest(deviceToken),
//       createdAt: at,
//       lastSeenAt: at,
//       ...(scope !== undefined ? { scope } : {}),
//       ...(scope !== undefined && more.road !== undefined ? { road: more.road } : {}),
//       ...(scope !== undefined && more.aside === true ? { aside: true } : {}),
//       ...(more.here === true ? { here: true } : {}),
//       ...(more.via !== undefined ? { via: more.via } : {}),
//     };
//     await store.put(DEVICES, record.id, record);
//     return { deviceId: record.id, deviceToken, device: viewOf(record) };
//   };
//
//   /** The code half of a redeem, on its own: the record this host was holding when it had not run out, else
//    * nothing. Spent either way, so one guess never gets two tries, and whatever the caller does with the answer. */
//   const spendCode = async (code: string, now: number): Promise<PairingRecord | undefined> => {
//     const held = await store.get(PAIRINGS, code);
//     if (!isPairing(held)) return undefined;
//     await store.delete(PAIRINGS, code);
//     return now <= held.expiresAt ? held : undefined;
//   };
//
//   const revoke = (id: string): Promise<boolean> =>
//     oneAtATime(async () => {
//       const held = await store.get(DEVICES, id);
//       if (!isDevice(held)) return false;
//       // The key first: a revoke that took the record away and then failed would leave the device free to dial
//       // back in on the admission it still holds.
//       if (held.via !== undefined) await store.put(REVOKED, held.via.fingerprint, { fingerprint: held.via.fingerprint } satisfies RevokedRecord);
//       await store.delete(DEVICES, id);
//       return true;
//     });
//
//   return {
//     issue: ({ now, ttlMs, here }) =>
//       oneAtATime(async () => {
//         // A code nobody redeemed is dead weight in the state file, and minting is the one moment the list is
//         // already worth reading: every host restart and every fresh code clears what has run out.
//         for (const held of await store.list(PAIRINGS)) if (isPairing(held) && now > held.expiresAt) await store.delete(PAIRINGS, held.code);
//         const code = mintCode();
//         const expiresAt = now + ttlMs;
//         await store.put(PAIRINGS, code, { code, expiresAt, ...(here === true ? { here: true } : {}) } satisfies PairingRecord);
//         return { code, expiresAt };
//       }),
//     redeem: (code, name, now) =>
//       oneAtATime(async () => {
//         const spent = await spendCode(code, now);
//         return spent === undefined ? undefined : admit(name, undefined, now, spent.here === true ? { here: true } : {});
//       }),
//     spend: (code, now) => oneAtATime(async () => (await spendCode(code, now)) !== undefined),
//     admit: (name, now) => oneAtATime(() => admit(name, undefined, now)),
//     admitAccount: (name, via, now) => oneAtATime(() => admit(name, undefined, now, { via })),
//     mint: (name, scope, now, more = {}) => oneAtATime(() => admit(name, scope, now, more)),
//     revokeAsides: async () => {
//       for (const record of await devices()) if (record.aside === true) await revoke(record.id);
//     },
//     refuses: async fingerprint => isRevoked(await store.get(REVOKED, fingerprint)),
//     match: async token => {
//       const digest = tokenDigest(token);
//       // Every record is compared, and the first match is kept rather than returned: a loop that leaves early would
//       // say by its own duration how far down the list the token sat.
//       let found: DeviceRecord | undefined;
//       for (const record of await devices()) if (safeEqual(digest, record.tokenHash) && found === undefined) found = record;
//       return found === undefined ? undefined : heldOf(found);
//     },
//     // A read and a write with an await between them: through the chain, so a revoke that lands in that gap is not
//     // undone by this write. A device redialing with backoff while somebody revokes it hits exactly that.
//     seen: (id, now) =>
//       oneAtATime(async () => {
//         const held = await store.get(DEVICES, id);
//         if (!isDevice(held)) return undefined;
//         const moved: DeviceRecord = { ...held, lastSeenAt: new Date(now).toISOString() };
//         await store.put(DEVICES, id, moved);
//         return heldOf(moved);
//       }),
//     list: async () => (await devices()).sort((a, b) => a.createdAt.localeCompare(b.createdAt)).map(viewOf),
//     revoke,
//   };
// }
/// The one door, over `store`, drawing every random byte from `ids` (rule 3.16).
pub fn make_devices(store: Rc<dyn Store>, ids: Rc<dyn Ids>) -> Rc<dyn DeviceDoor> {
    Rc::new(Doors { store, ids, writes: tokio::sync::Mutex::new(()) })
}

/// What `admit` adds to a name and a clock: the flags and road a kind of device carries.
#[derive(Default)]
struct Admit {
    here: bool,
    via: Option<DeviceVia>,
    road: Option<ScopedRoad>,
    aside: bool,
}

struct Doors {
    store: Rc<dyn Store>,
    ids: Rc<dyn Ids>,
    /// The one write chain (TS `oneAtATime`): tokio's mutex is first in, first out, so writes run in call order.
    writes: tokio::sync::Mutex<()>,
}

impl Doors {
    /// Every device record that passes the guard, in the store's order.
    async fn devices(&self) -> Result<Vec<DeviceRecord>, HostError> {
        Ok(self.store.list(DEVICES).await?.into_iter().filter_map(|v| serde_json::from_value(v).ok()).collect())
    }

    async fn device(&self, id: &str) -> Result<Option<DeviceRecord>, HostError> {
        Ok(self.store.get(DEVICES, id).await?.and_then(|v| serde_json::from_value(v).ok()))
    }

    async fn pairing(&self, code: &str) -> Result<Option<PairingRecord>, HostError> {
        Ok(self.store.get(PAIRINGS, code).await?.and_then(|v| serde_json::from_value(v).ok()))
    }

    async fn put_device(&self, id: &str, record: &DeviceRecord) -> Result<(), HostError> {
        self.store.put(DEVICES, id, record_value(record)?).await
    }

    /// The code half of a redeem: the pairing is deleted either way, and answered only while it has not run out.
    async fn spend_code(&self, code: &str, now: i64) -> Result<Option<PairingRecord>, HostError> {
        let Some(held) = self.pairing(code).await? else { return Ok(None) };
        self.store.delete(PAIRINGS, code).await?;
        Ok((now as f64 <= held.expires_at.0).then_some(held))
    }

    /// One device record and the one token it will ever hand over (TS `admit`). Token first, then the id.
    async fn admit(&self, name: &str, scope: Option<ThreadScope>, now: i64, more: Admit) -> Result<PairedDevice, HostError> {
        let token = URL_SAFE_NO_PAD.encode(self.ids.bytes(24));
        let at = iso(now)?;
        let id = format!("d_{}", hex::encode(self.ids.bytes(8)));
        let trimmed = js_trim(name);
        let record = DeviceRecord {
            id: id.clone(),
            name: if trimmed.is_empty() { "a paired computer".into() } else { trimmed.into() },
            token_hash: token_digest(&token),
            created_at: at.clone(),
            last_seen_at: at,
            road: if scope.is_some() { more.road } else { None },
            aside: if scope.is_some() && more.aside { Some(true) } else { None },
            scope,
            here: more.here.then_some(true),
            via: more.via,
        };
        let device = view_of(&record);
        self.put_device(&id, &record).await?;
        Ok(PairedDevice { device_id: id, device_token: token, device })
    }

    fn mint_code(&self) -> String {
        self.ids.bytes(PAIR_CODE_LENGTH as usize).iter().map(|b| PAIR_CODE_ALPHABET.as_bytes()[(b & 31) as usize] as char).collect()
    }
}

fn iso(now: i64) -> Result<String, HostError> {
    iso_string(now as f64).ok_or_else(|| HostError::new("Invalid time value"))
}

fn record_value<T: Serialize>(value: &T) -> Result<serde_json::Value, HostError> {
    serde_json::to_value(value).map_err(|e| HostError::new(e.to_string()))
}

fn view_of(r: &DeviceRecord) -> DeviceView {
    DeviceView {
        id: r.id.clone(),
        name: r.name.clone(),
        created_at: r.created_at.clone(),
        last_seen_at: r.last_seen_at.clone(),
        scope: r.scope.clone(),
        here: (r.here == Some(true)).then_some(True),
        via: r.via.clone(),
    }
}

fn held_of(r: &DeviceRecord) -> HeldDevice {
    HeldDevice { view: view_of(r), road: r.road }
}

#[async_trait(?Send)]
impl DeviceDoor for Doors {
    async fn issue(&self, now: i64, ttl_ms: u64, here: bool) -> Result<IssuedCode, HostError> {
        let _turn = self.writes.lock().await;
        for value in self.store.list(PAIRINGS).await? {
            if let Ok(held) = serde_json::from_value::<PairingRecord>(value) {
                if now as f64 > held.expires_at.0 {
                    self.store.delete(PAIRINGS, &held.code).await?;
                }
            }
        }
        let code = self.mint_code();
        let expires_at = now + ttl_ms as i64;
        let record = PairingRecord { code: code.clone(), expires_at: JsNumber::from(expires_at), here: here.then_some(true) };
        self.store.put(PAIRINGS, &code, record_value(&record)?).await?;
        Ok(IssuedCode { code, expires_at })
    }

    async fn redeem(&self, code: &str, name: &str, now: i64) -> Result<Option<PairedDevice>, HostError> {
        let _turn = self.writes.lock().await;
        let Some(spent) = self.spend_code(code, now).await? else { return Ok(None) };
        let more = Admit { here: spent.here == Some(true), ..Admit::default() };
        Ok(Some(self.admit(name, None, now, more).await?))
    }

    async fn spend(&self, code: &str, now: i64) -> Result<bool, HostError> {
        let _turn = self.writes.lock().await;
        Ok(self.spend_code(code, now).await?.is_some())
    }

    async fn admit(&self, name: &str, now: i64) -> Result<PairedDevice, HostError> {
        let _turn = self.writes.lock().await;
        self.admit(name, None, now, Admit::default()).await
    }

    async fn admit_account(&self, name: &str, via: DeviceVia, now: i64) -> Result<PairedDevice, HostError> {
        let _turn = self.writes.lock().await;
        self.admit(name, None, now, Admit { via: Some(via), ..Admit::default() }).await
    }

    async fn refuses(&self, fingerprint: &str) -> Result<bool, HostError> {
        Ok(self.store.get(REVOKED, fingerprint).await?.and_then(|v| serde_json::from_value::<RevokedRecord>(v).ok()).is_some())
    }

    async fn mint(&self, name: &str, scope: ThreadScope, now: i64, more: MintMore) -> Result<PairedDevice, HostError> {
        let _turn = self.writes.lock().await;
        self.admit(name, Some(scope), now, Admit { road: more.road, aside: more.aside, ..Admit::default() }).await
    }

    async fn revoke_asides(&self) -> Result<(), HostError> {
        for record in self.devices().await? {
            if record.aside == Some(true) {
                self.revoke(&record.id).await?;
            }
        }
        Ok(())
    }

    async fn r#match(&self, token: &str) -> Result<Option<HeldDevice>, HostError> {
        let digest = token_digest(token);
        let mut found: Option<DeviceRecord> = None;
        for record in self.devices().await? {
            if safe_equal(&digest, &record.token_hash) && found.is_none() {
                found = Some(record);
            }
        }
        Ok(found.as_ref().map(held_of))
    }

    async fn seen(&self, id: &str, now: i64) -> Result<Option<HeldDevice>, HostError> {
        let _turn = self.writes.lock().await;
        let Some(mut raw) = self.store.get(DEVICES, id).await? else { return Ok(None) };
        let Some(mut held) = serde_json::from_value::<DeviceRecord>(raw.clone()).ok() else { return Ok(None) };
        held.last_seen_at = iso(now)?;
        raw["lastSeenAt"] = json!(held.last_seen_at);
        self.store.put(DEVICES, id, raw).await?;
        Ok(Some(held_of(&held)))
    }

    async fn list(&self) -> Result<Vec<DeviceView>, HostError> {
        let mut records = self.devices().await?;
        records.sort_by(|a, b| collate(&a.created_at, &b.created_at));
        Ok(records.iter().map(view_of).collect())
    }

    async fn revoke(&self, id: &str) -> Result<bool, HostError> {
        let _turn = self.writes.lock().await;
        let Some(held) = self.device(id).await? else { return Ok(false) };
        if let Some(via) = &held.via {
            let key = RevokedRecord { fingerprint: via.fingerprint.clone() };
            self.store.put(REVOKED, &via.fingerprint, record_value(&key)?).await?;
        }
        self.store.delete(DEVICES, id).await?;
        Ok(true)
    }
}
