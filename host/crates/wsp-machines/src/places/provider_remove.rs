// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/runtime/src/places/provider-remove.ts` at 65f6b89c7: takes a provider back out, every machine and image
//! this host made there read gone before the key is forgotten. Spec: `spec/places_provider_remove.md`.

use std::cell::{Cell, RefCell};
use std::future::Future;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use futures::future::LocalBoxFuture;
use futures::StreamExt;
use indexmap::{IndexMap, IndexSet};
use tokio::time::{sleep, Instant};
use wsp_engine::drop_template::drop_template;
use wsp_engine::errors::{is_missing, EngineError, ErrorKind, Result as EResult};
use wsp_engine::labels::OWNER_LABEL;
use wsp_engine::machine::{Machine, MachineBackend, MachineState};
use wsp_wire::types::js_number::JsInt;
use wsp_wire::views::place_more::PlaceRemoved;
use wsp_words::exit::{refusal, Kind};
use wsp_words::place_word::provider_key_name;
use wsp_words::words::places::{provider_images_kept_line, provider_kept_line, provider_key_refused, provider_left_words, provider_unlisted_line, LeftThere};

use super::types::{HeldImage, KillConfirm, PlaceKeyForgetter, PlaceResult};
use crate::machines::area_error::AreaError;

/// How many deletes run at once (provider-remove.ts:141).
pub const KILLS_AT_ONCE: usize = 4;

/// How many times the listing is read again after the deletes (provider-remove.ts:144).
pub const RELISTS: usize = 3;

/// The image records this host holds at the provider, and the drop of one once its snapshot is gone.
pub struct ProviderImages {
    pub held: Rc<dyn Fn() -> LocalBoxFuture<'static, PlaceResult<Vec<HeldImage>>>>,
    pub drop: Rc<dyn Fn(String) -> LocalBoxFuture<'static, PlaceResult<()>>>,
}

/// What a provider's remove is handed (provider-remove.ts:152).
pub struct ProviderRemove {
    pub place_id: String,
    pub backend: Arc<dyn MachineBackend>,
    /// This host's owner mark.
    pub owner: String,
    /// The machine ids this host's own records hold there.
    pub known: Vec<String>,
    pub forget_key: Option<Rc<dyn PlaceKeyForgetter>>,
    pub confirm: Option<KillConfirm>,
    pub images: Option<ProviderImages>,
}

/// Deletes every machine this host made there, then every image snapshot, and only then forgets the key
/// (provider-remove.ts:152).
pub async fn remove_provider(o: ProviderRemove) -> PlaceResult<PlaceRemoved> {
    let name = provider_key_name(&o.place_id);
    let confirm = o.confirm.unwrap_or_default();
    let backend = o.backend.as_ref();
    let mut ids = listed_marked(backend, &o.owner, &name).await?;
    ids.extend(o.known.iter().cloned());

    let left: RefCell<Vec<LeftThere>> = RefCell::new(Vec::new());
    let deleted = Cell::new(0usize);
    let mut handled: IndexSet<String> = IndexSet::new();
    let mut pass = 0;
    while !ids.is_empty() {
        if pass > RELISTS {
            left.borrow_mut().extend(ids.iter().map(|id| LeftThere { id: id.clone(), said: provider_left_words::LATE.to_string() }));
            break;
        }
        handled.extend(ids.iter().cloned());
        delete_each(backend, &ids, confirm, &left, &deleted).await;
        match listed_marked(backend, &o.owner, &name).await {
            Ok(listed) => ids = listed.into_iter().filter(|id| !handled.contains(id)).collect(),
            Err(e) => {
                if !left.borrow().is_empty() {
                    break;
                }
                return Ok(answer(false, deleted.get(), Some(provider_unlisted_line(&name, deleted.get() as f64, &e.message)), None));
            }
        }
        pass += 1;
    }
    let deleted_now = deleted.get();
    let left = left.into_inner();
    if !left.is_empty() {
        return Ok(answer(false, deleted_now, Some(provider_kept_line(&name, deleted_now as f64, &left)), None));
    }

    let (images_left, images_deleted) = match &o.images {
        Some(images) => delete_images(backend, images, confirm).await?,
        None => (Vec::new(), 0),
    };
    let snapshots = (images_deleted > 0).then_some(images_deleted);
    if !images_left.is_empty() {
        let note = provider_images_kept_line(&name, deleted_now as f64, &images_left);
        return Ok(answer(false, deleted_now, Some(note), snapshots));
    }
    let still = match &o.forget_key {
        Some(forget) => forget.forget_key(&o.place_id).await?,
        None => None,
    };
    Ok(PlaceRemoved { note: still, ..answer(true, deleted_now, None, snapshots) })
}

fn answer(removed: bool, deleted: usize, note: Option<String>, snapshots: Option<usize>) -> PlaceRemoved {
    PlaceRemoved {
        removed,
        took: None,
        swept: Vec::new(),
        note,
        deleted: Some(JsInt::from(deleted as i64)),
        snapshots: snapshots.map(|n| JsInt::from(n as i64)),
    }
}

/// The marked ids the provider lists, read again on every row (provider-remove.ts:163).
async fn listed_marked(backend: &dyn MachineBackend, owner: &str, name: &str) -> PlaceResult<IndexSet<String>> {
    let labels = IndexMap::from([(OWNER_LABEL.to_string(), owner.to_string())]);
    let rows = backend.list(Some(&labels)).await.map_err(|e| {
        if e.kind() == Some(ErrorKind::Auth) {
            let refused = provider_key_refused(name, &message_of(&e));
            refusal(&refused.said, &refused.fix, Some(Kind::Auth))
        } else {
            AreaError::Engine(e).into_host()
        }
    })?;
    Ok(rows.into_iter().filter(|m| m.labels.get(OWNER_LABEL).map(String::as_str) == Some(owner)).map(|m| m.id).collect())
}

/// Deletes each id, at most `KILLS_AT_ONCE` at a time, and records what did not go (provider-remove.ts:174).
async fn delete_each(backend: &dyn MachineBackend, ids: &IndexSet<String>, confirm: KillConfirm, left: &RefCell<Vec<LeftThere>>, deleted: &Cell<usize>) {
    futures::stream::iter(ids.iter().cloned())
        .for_each_concurrent(KILLS_AT_ONCE, |id| async move { delete_one(backend, id, confirm, left, deleted).await })
        .await;
}

async fn delete_one(backend: &dyn MachineBackend, id: String, confirm: KillConfirm, left: &RefCell<Vec<LeftThere>>, deleted: &Cell<usize>) {
    let seen = match sight_machine(backend, &id).await {
        Ok(seen) => seen,
        Err(e) => {
            left.borrow_mut().push(LeftThere { id, said: provider_left_words::unread(&message_of(&e)) });
            return;
        }
    };
    let Some(machine) = seen.machine else { return };
    let delete_failed = Cell::new(false);
    let ask = || {
        let machine = Arc::clone(&machine);
        let failed = &delete_failed;
        async move {
            machine.kill().await.inspect_err(|e| failed.set(!is_missing(e)))
        }
    };
    match kill_until_gone(backend, &id, ask, confirm).await {
        Ok(()) => deleted.set(deleted.get() + 1),
        Err(failure) => {
            let said = match failure {
                KillFailure::Alive(state) => provider_left_words::running(state_word(state)),
                KillFailure::Engine(e) if delete_failed.get() => provider_left_words::undeleted(&message_of(&e)),
                KillFailure::Engine(e) => provider_left_words::unread(&message_of(&e)),
            };
            left.borrow_mut().push(LeftThere { id, said });
        }
    }
}

/// Deletes each image snapshot and reads it gone (provider-remove.ts:233).
async fn delete_images(backend: &dyn MachineBackend, images: &ProviderImages, confirm: KillConfirm) -> PlaceResult<(Vec<LeftThere>, usize)> {
    let mut left = Vec::new();
    let mut deleted = 0;
    for image in (images.held)().await? {
        let end: Result<ImageEnd, String> = async {
            drop_template(backend, image.template_id.as_deref()).await.map_err(|e| message_of(&e))?;
            match snapshot_until_gone(backend, &image.id, confirm).await.map_err(|e| message_of(&e))? {
                SnapshotGone::Listed { grace_ms } => Ok(ImageEnd::Left(provider_left_words::listed(grace_ms as f64))),
                SnapshotGone::Deleted | SnapshotGone::Missing => {
                    (images.drop)(image.id.clone()).await.map_err(|e| e.message)?;
                    Ok(ImageEnd::Deleted)
                }
            }
        }
        .await;
        match end {
            Ok(ImageEnd::Deleted) => deleted += 1,
            Ok(ImageEnd::Left(said)) => left.push(LeftThere { id: image.id.clone(), said }),
            Err(message) => left.push(LeftThere { id: image.id.clone(), said: provider_left_words::undeleted(&message) }),
        }
    }
    Ok((left, deleted))
}

enum ImageEnd {
    Deleted,
    Left(String),
}

fn message_of(e: &EngineError) -> String {
    e.to_string()
}

fn state_word(state: MachineState) -> &'static str {
    match state {
        MachineState::Starting => "starting",
        MachineState::Running => "running",
        MachineState::Paused => "paused",
        MachineState::Gone => "gone",
    }
}

// golden.ts reads, held here until golden.ts has a home in wsp-engine.

const GONE_READS: u32 = 12;
const KILL_ASKS: u32 = 3;
const KILL_GRACE_MS: u64 = 30_000;
const POLL_MS: u64 = 1_000;

/// One read of a machine: the state it answered, and the handle it came on when the provider found it.
struct Sighting {
    state: MachineState,
    machine: Option<Arc<dyn Machine>>,
}

/// One call per read; a machine the provider no longer has reads gone (golden.ts readOnce).
async fn read_once(backend: &dyn MachineBackend, id: &str) -> EResult<Sighting> {
    let found: EResult<(MachineState, Arc<dyn Machine>)> = async {
        let machine = backend.get(id).await?;
        let state = match machine.seen() {
            Some(seen) => seen.state,
            None => machine.state().await?,
        };
        Ok((state, machine))
    }
    .await;
    match found {
        Ok((state, machine)) => Ok(Sighting { state, machine: Some(machine) }),
        Err(e) if is_missing(&e) => Ok(Sighting { state: MachineState::Gone, machine: None }),
        Err(e) => Err(e),
    }
}

/// Whether the provider still has a machine: gone once GONE_READS reads in a row answer gone (golden.ts sightMachine).
async fn sight_machine(backend: &dyn MachineBackend, id: &str) -> EResult<Sighting> {
    for _ in 0..GONE_READS {
        let seen = read_once(backend, id).await?;
        if seen.state != MachineState::Gone {
            return Ok(seen);
        }
    }
    Ok(Sighting { state: MachineState::Gone, machine: None })
}

enum KillFailure {
    /// A machine that answered KILL_ASKS kills and is still there, in the state it last read.
    Alive(MachineState),
    Engine(EngineError),
}

/// Done once the machine reads gone GONE_READS times in a row (golden.ts killUntilGone).
async fn kill_until_gone<Ask, Fut>(backend: &dyn MachineBackend, id: &str, mut ask: Ask, confirm: KillConfirm) -> Result<(), KillFailure>
where
    Ask: FnMut() -> Fut,
    Fut: Future<Output = EResult<()>>,
{
    let grace = Duration::from_millis(confirm.grace_ms.unwrap_or(KILL_GRACE_MS));
    let poll = Duration::from_millis(confirm.poll_ms.unwrap_or(POLL_MS));
    let mut alive = MachineState::Running;
    for _ in 0..KILL_ASKS {
        if let Err(e) = ask().await {
            if !is_missing(&e) {
                return Err(KillFailure::Engine(e));
            }
        }
        let deadline = Instant::now() + grace;
        let mut gone = 0;
        loop {
            let read = read_once(backend, id).await.map_err(KillFailure::Engine)?;
            if read.state == MachineState::Gone {
                gone += 1;
                if gone >= GONE_READS {
                    return Ok(());
                }
            } else {
                alive = read.state;
                if gone > 0 {
                    break;
                }
                sleep(poll).await;
            }
            if gone == 0 && Instant::now() >= deadline {
                break;
            }
        }
    }
    Err(KillFailure::Alive(alive))
}

enum SnapshotGone {
    Deleted,
    Missing,
    Listed { grace_ms: u64 },
}

/// A snapshot's delete, read back off the listing under the window (golden.ts snapshotUntilGone).
async fn snapshot_until_gone(backend: &dyn MachineBackend, id: &str, confirm: KillConfirm) -> EResult<SnapshotGone> {
    match backend.delete_snapshot(id).await {
        Ok(()) => {}
        Err(e) if is_missing(&e) => return Ok(SnapshotGone::Missing),
        Err(e) => return Err(e),
    }
    if !backend.capabilities().snapshot_listing {
        return Ok(SnapshotGone::Deleted);
    }
    let grace_ms = confirm.grace_ms.unwrap_or(KILL_GRACE_MS);
    let deadline = Instant::now() + Duration::from_millis(grace_ms);
    let ids = [id.to_string()];
    loop {
        if let Ok(rows) = backend.list_snapshots(Some(&ids[..])).await {
            if !rows.iter().any(|r| r.id == id) {
                return Ok(SnapshotGone::Deleted);
            }
        }
        sleep(Duration::from_millis(confirm.poll_ms.unwrap_or(POLL_MS))).await;
        if Instant::now() >= deadline {
            break;
        }
    }
    Ok(SnapshotGone::Listed { grace_ms })
}
