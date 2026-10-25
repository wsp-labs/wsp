// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/context.ts at 65f6b89c7 (types only), with the values runtime.ts:986-990 fills it
//! with. Spec: spec/context.md.
//!
//! The runtime's one shared object under Proposal 2: one thread owns it, state is reached only through
//! `StateCell::with`, and every area through a slot filled after construction and read at the moment it is called.
use std::cell::{OnceCell, RefCell};
use std::rc::Rc;

use futures::future::{LocalBoxFuture, Shared};
use wsp_wire::views::session::ThreadCapWait;

use crate::clock::Cancel;
use crate::status::StatusTracker;

/// A promise more than one area awaits.
pub type Joined = Shared<LocalBoxFuture<'static, ()>>;

// context.ts:156
// /** The lets more than one area reads or writes, read through ctx.state at the moment they are needed. */
// export interface RuntimeState {
//   sayStops: ((line: string) => void) | undefined;
//   readsSince: number;
//   rootsRecheck: ReturnType<typeof setInterval> | undefined;
//   owner: string;
//   copiesMoving: Promise<void> | undefined;
//   sweepTimer: (() => void) | undefined;
//   sweeping: Promise<void> | undefined;
//   sweepStopped: boolean;
//   beat: (() => void) | undefined;
//   closed: boolean;
//   closing: boolean;
//   ticking: Promise<void> | undefined;
//   preferencesHeld: Preferences | undefined;
// }
// runtime.ts:986
//     state: {
//       sayStops: undefined, readsSince: 0, rootsRecheck: undefined, owner: "", copiesMoving: undefined,
//       sweepTimer: undefined, sweeping: undefined, sweepStopped: false, beat: undefined, closed: false, closing: false,
//       ticking: undefined, preferencesHeld: undefined,
//     },
/// `Default` is exactly what createRuntime fills it with.
#[derive(Default)]
pub struct RuntimeState {
    pub say_stops: Option<Rc<dyn Fn(&str)>>,
    /// When this state file began keeping read stamps.
    pub reads_since: f64,
    pub roots_recheck: Option<Cancel>,
    pub owner: String,
    pub copies_moving: Option<Joined>,
    pub sweep_timer: Option<Cancel>,
    pub sweeping: Option<Joined>,
    pub sweep_stopped: bool,
    /// Marks the record as this process's, now; the sweep and the heartbeat timer refresh it and close() clears it.
    pub beat: Option<Rc<dyn Fn()>>,
    pub closed: bool,
    /// Set as close() starts, before it shuts the daemon channels a card read still out goes over.
    pub closing: bool,
    pub ticking: Option<Joined>,
    /// The preferences record as last read or written. Typed when preferences.ts is ported.
    pub preferences_held: Option<serde_json::Value>,
}

/// The handle over shared state: a borrow lives only inside `with` (rule 3.3). Clones share one state.
pub struct StateCell<S> {
    cell: Rc<RefCell<S>>,
}

impl<S> Clone for StateCell<S> {
    fn clone(&self) -> Self {
        todo!()
    }
}

impl<S> StateCell<S> {
    pub fn new(state: S) -> Self {
        todo!()
    }
    /// Lends the state to `f` and hands back its answer. A `with` inside a `with` on the same cell panics.
    pub fn with<R>(&self, f: impl FnOnce(&mut S) -> R) -> R {
        todo!()
    }
}

/// One member of the context, filled once after construction and read when called.
pub struct Late<T> {
    name: &'static str,
    cell: OnceCell<T>,
}

impl<T: Clone> Late<T> {
    pub fn named(name: &'static str) -> Self {
        todo!()
    }
    /// Panics `<name> was built twice` on a second fill.
    pub fn fill(&self, value: T) {
        todo!()
    }
    /// Panics `the runtime read <name> before it was built` before the fill.
    pub fn get(&self) -> T {
        todo!()
    }
    pub fn is_filled(&self) -> bool {
        todo!()
    }
}

/// context.ts:178-237. Its members join with the types they name (types/internal.ts, types/wiring.ts).
pub trait RuntimeCore {}

/// activity.ts. Members join with this area's port.
pub trait ActivityArea {}

/// context.ts:239-267. Members join with this area's port.
pub trait KindsArea {}

/// context.ts:269-300. Members join with this area's port.
pub trait RulesArea {}

/// context.ts:302-327. Members join with this area's port.
pub trait TranscriptsArea {}

/// context.ts:329-331. Members join with this area's port.
pub trait SlatesArea {}

/// context.ts:333-386. Members join with this area's port.
pub trait ChannelsArea {}

/// context.ts:388-396. Members join with this area's port.
pub trait ProcScopesArea {}

/// context.ts:398-415. Members join with this area's port.
pub trait RecordsArea {}

/// context.ts:417-441. Members join with this area's port.
pub trait ReachArea {}

/// context.ts:443-457. Members join with this area's port.
pub trait PullRequestsArea {}

/// context.ts:459-475. Members join with this area's port.
pub trait DaemonArea {}

/// context.ts:477-503. Members join with this area's port.
pub trait MachinesArea {}

/// context.ts:505-538. Members join with this area's port.
pub trait BootArea {}

/// context.ts:544-560. Members join with this area's port.
pub trait CreateArea {}

/// context.ts:594-606. Members join with this area's port.
pub trait SubSpawnArea {}

/// context.ts:608-632. Members join with this area's port.
pub trait SubQueueArea {}

/// context.ts:562-592. Members join with this area's port.
pub trait SubLifeArea {}

/// context.ts:634-652. Members join with this area's port.
pub trait FoldersArea {}

/// context.ts:664-675. Members join with this area's port.
pub trait StartFromArea {}

/// context.ts:677-679. Members join with this area's port.
pub trait WorkspacesArea {}

/// context.ts:681-718. Members join with this area's port.
pub trait AgentsArea {}

/// context.ts:741-799. Members join with this area's port.
pub trait ThreadsArea {}

/// context.ts:801-853. Members join with this area's port.
pub trait TurnsArea {}

/// context.ts:855-863. Members join with this area's port.
pub trait SessionsArea {}

/// context.ts:865-891. Members join with this area's port.
pub trait BuildersArea {}

/// context.ts:893-895. Members join with this area's port.
pub trait GoldenArea {}

/// context.ts:897-901. Members join with this area's port.
pub trait ImageArea {}

/// context.ts:903-909. Members join with this area's port.
pub trait ProjectsArea {}

/// context.ts:911-922. Members join with this area's port.
pub trait ProjectIconsArea {}

/// context.ts:924-932. Members join with this area's port.
pub trait UsageArea {}

/// context.ts:934-936. Members join with this area's port.
pub trait ConversationsArea {}

/// context.ts:938-943. Members join with this area's port.
pub trait WaitsArea {}

/// context.ts:950-955. Members join with this area's port.
pub trait PreferencesArea {}

// context.ts:945
// export interface StatusArea {
//   readonly status: StatusApi;
//   readonly adoptLost: (listing: ListedMachine[], known: Set<string>, failed: ReapFailure[], at: { place: string; backend: MachineBackend }) => Promise<{ adopted: AdoptedMachine[]; reaped: ReapedMachine[] }>;
// }
/// `adopt_lost` joins with its port in wsp-machines.
pub trait StatusArea {
    fn status(&self) -> Rc<StatusTracker>;
}

// context.ts:957
// /** Every area's members on one object, filled in createRuntime in file order: a member is read at the moment it is
//  * called, never copied into a local while the areas are still being built. */
// export type RuntimeContext = RuntimeCore & ActivityArea & ... & PreferencesArea;
pub struct RuntimeContext {
    pub state: StateCell<RuntimeState>,
    pub core: Late<Rc<dyn RuntimeCore>>,
    pub activity: Late<Rc<dyn ActivityArea>>,
    pub kinds: Late<Rc<dyn KindsArea>>,
    pub rules: Late<Rc<dyn RulesArea>>,
    pub transcripts: Late<Rc<dyn TranscriptsArea>>,
    pub slates: Late<Rc<dyn SlatesArea>>,
    pub channels: Late<Rc<dyn ChannelsArea>>,
    pub proc_scopes: Late<Rc<dyn ProcScopesArea>>,
    pub records: Late<Rc<dyn RecordsArea>>,
    pub reach: Late<Rc<dyn ReachArea>>,
    pub pull_requests: Late<Rc<dyn PullRequestsArea>>,
    pub daemon: Late<Rc<dyn DaemonArea>>,
    pub machines: Late<Rc<dyn MachinesArea>>,
    pub boot: Late<Rc<dyn BootArea>>,
    pub create: Late<Rc<dyn CreateArea>>,
    pub sub_spawn: Late<Rc<dyn SubSpawnArea>>,
    pub sub_queue: Late<Rc<dyn SubQueueArea>>,
    pub sub_life: Late<Rc<dyn SubLifeArea>>,
    pub folders: Late<Rc<dyn FoldersArea>>,
    pub start_from: Late<Rc<dyn StartFromArea>>,
    pub workspaces: Late<Rc<dyn WorkspacesArea>>,
    pub agents: Late<Rc<dyn AgentsArea>>,
    pub threads: Late<Rc<dyn ThreadsArea>>,
    pub turns: Late<Rc<dyn TurnsArea>>,
    pub sessions: Late<Rc<dyn SessionsArea>>,
    pub builders: Late<Rc<dyn BuildersArea>>,
    pub golden: Late<Rc<dyn GoldenArea>>,
    pub image: Late<Rc<dyn ImageArea>>,
    pub projects: Late<Rc<dyn ProjectsArea>>,
    pub project_icons: Late<Rc<dyn ProjectIconsArea>>,
    pub usage: Late<Rc<dyn UsageArea>>,
    pub conversations: Late<Rc<dyn ConversationsArea>>,
    pub waits: Late<Rc<dyn WaitsArea>>,
    pub status: Late<Rc<dyn StatusArea>>,
    pub preferences: Late<Rc<dyn PreferencesArea>>,
}

impl RuntimeContext {
    /// Empty, as `{} as RuntimeContext`, with the state createRuntime starts it with; every slot unfilled.
    pub fn new() -> Rc<RuntimeContext> {
        todo!()
    }
}

// context.ts:720
// export interface DetachedSend {
//   outcome: "steered" | "queued" | "started" | "held";
//   threadId: string;
//   harness: string;
//   capped?: ThreadCapWait;
// }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetachedOutcome {
    Steered,
    Queued,
    Started,
    Held,
}

/// What a message sent without its caller waiting answers.
#[derive(Debug, Clone, PartialEq)]
pub struct DetachedSend {
    pub outcome: DetachedOutcome,
    pub thread_id: String,
    pub harness: String,
    pub capped: Option<ThreadCapWait>,
}
