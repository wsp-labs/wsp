// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/devices.ts at 65f6b89c7. Spec: spec/devices.md.
//!
//! Pairing codes and the device tokens they mint, kept in the store so a restart does not lock a paired computer
//! out. A code is one use; a device token is minted once, handed over once and never stored: only its sha256 is kept,
//! and every reading of it is a timing safe compare of that hash.
use std::rc::Rc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use wsp_wire::views::workspace_more::{ThreadScope, WorkspaceOrigin};
use wsp_wire::wire::devices::{DeviceVia, DeviceView};

use wsp_words::exit::HostError;

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
    todo!()
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
    todo!()
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
    pub expires_at: i64,
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
    todo!()
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
    todo!()
}
