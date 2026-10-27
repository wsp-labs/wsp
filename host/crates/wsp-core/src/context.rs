// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/context.ts at 65f6b89c7 (types only), with the values runtime.ts:986-990 fills it
//! with. Spec: spec/context.md.
//!
//! The runtime's one shared object under Proposal 2: one thread owns it, state is reached only through
//! `StateCell::with`, and every area through a slot filled after construction and read at the moment it is called.
use std::cell::RefCell;
use std::rc::Rc;

use futures::future::{LocalBoxFuture, Shared};
use wsp_wire::views::preferences::Preferences;
use wsp_wire::views::session::ThreadCapWait;

pub use crate::clock::Cancel;
use crate::status::StatusTracker;

/// A promise more than one area awaits. The context's `Joined` and `Cancel`: wsp-core's `types::wiring::Joined<T>`,
/// wsp-machines' `Cancel` and the engine's `Cancel` are other types for other callers.
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
    /// The preferences record as last read or written.
    pub preferences_held: Option<Preferences>,
}

/// The handle over shared state: a borrow lives only inside `with` (rule 3.3). Clones share one state.
pub struct StateCell<S> {
    cell: Rc<RefCell<S>>,
}

impl<S> Clone for StateCell<S> {
    fn clone(&self) -> Self {
        StateCell { cell: self.cell.clone() }
    }
}

impl<S> StateCell<S> {
    pub fn new(state: S) -> Self {
        StateCell { cell: Rc::new(RefCell::new(state)) }
    }
    /// Lends the state to `f` and hands back its answer. A `with` inside a `with` on the same cell panics.
    pub fn with<R>(&self, f: impl FnOnce(&mut S) -> R) -> R {
        f(&mut self.cell.borrow_mut())
    }
}

/// One member of the context, filled once after construction and read when called.
pub struct Late<T> {
    name: &'static str,
    cell: RefCell<Option<T>>,
}

impl<T: Clone> Late<T> {
    pub fn named(name: &'static str) -> Self {
        Late { name, cell: RefCell::new(None) }
    }
    /// Panics `<name> was built twice` on a second fill.
    pub fn fill(&self, value: T) {
        let mut cell = self.cell.borrow_mut();
        if cell.is_some() {
            drop(cell);
            panic!("{} was built twice", self.name);
        }
        *cell = Some(value);
    }
    /// Empties the slot, so the value it held is dropped: a context that holds its areas and is held by them is freed
    /// only once the close has cleared each slot. A read after is a read before the build.
    pub fn clear(&self) {
        let held = self.cell.borrow_mut().take();
        drop(held);
    }
    /// Panics `the runtime read <name> before it was built` before the fill.
    pub fn get(&self) -> T {
        let held = self.cell.borrow().clone();
        match held {
            Some(v) => v,
            None => panic!("the runtime read {} before it was built", self.name),
        }
    }
    pub fn is_filled(&self) -> bool {
        self.cell.borrow().is_some()
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
        Rc::new(RuntimeContext {
            state: StateCell::new(RuntimeState::default()),
            core: Late::named("core"),
            activity: Late::named("activity"),
            kinds: Late::named("kinds"),
            rules: Late::named("rules"),
            transcripts: Late::named("transcripts"),
            slates: Late::named("slates"),
            channels: Late::named("channels"),
            proc_scopes: Late::named("proc_scopes"),
            records: Late::named("records"),
            reach: Late::named("reach"),
            pull_requests: Late::named("pull_requests"),
            daemon: Late::named("daemon"),
            machines: Late::named("machines"),
            boot: Late::named("boot"),
            create: Late::named("create"),
            sub_spawn: Late::named("sub_spawn"),
            sub_queue: Late::named("sub_queue"),
            sub_life: Late::named("sub_life"),
            folders: Late::named("folders"),
            start_from: Late::named("start_from"),
            workspaces: Late::named("workspaces"),
            agents: Late::named("agents"),
            threads: Late::named("threads"),
            turns: Late::named("turns"),
            sessions: Late::named("sessions"),
            builders: Late::named("builders"),
            golden: Late::named("golden"),
            image: Late::named("image"),
            projects: Late::named("projects"),
            project_icons: Late::named("project_icons"),
            usage: Late::named("usage"),
            conversations: Late::named("conversations"),
            waits: Late::named("waits"),
            status: Late::named("status"),
            preferences: Late::named("preferences"),
        })
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
