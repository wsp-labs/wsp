// SPDX-License-Identifier: AGPL-3.0-only
//! Recording a project: the landing roads by kind, what they may ask of the runtime, the seed menu and picks, and the
//! add. projects.ts:108-341 at 65f6b89c7; spec/projects_add.md. `LandingRoad` and `ProjectSourceModule` are the shapes
//! of runtime/src/project-landing.ts and project-sources.ts, which wsp-core holds and has not written yet.
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use indexmap::IndexMap;
use wsp_engine::machine::{Machine, MachineBackend};
use wsp_wire::views::place_kind::PlaceKind;
use wsp_wire::views::workspace::{Caller, ProjectSeed, ProjectSource, ProjectView, SeedChoice, SeedPlan};
use wsp_wire::views::workspace_more::ProjectBranch;
use wsp_words::exit::HostError;

use super::Projects;

/// `ProjectLanding["kind"]`: the computer the app runs on, a computer the person joined, or a provider.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LandingKind {
    Mac,
    Box,
    Provider,
}

/// `ctx.computerRows()`'s row.
pub use wsp_machines::machines::boot::ComputerRow;

/// A computer the person joined as its login runs every command there, and that login's home.
#[derive(Clone)]
pub struct JoinedComputer {
    pub machine: Arc<dyn Machine>,
    pub home: String,
}

/// What a landing road may ask of the runtime for one computer (project-landing.ts:52-69), its data fields; the
/// host's functions (`land`, `scratch`, `branchHere`, `now`) are the road's to reach.
#[derive(Clone)]
pub struct LandingDeps {
    pub computer: Option<JoinedComputer>,
    pub mac_state_home: String,
    pub computer_name: String,
    /// `cloneEnv()`: `{GITHUB_TOKEN}` only where GitHub was set up from the vault there.
    pub clone_env: IndexMap<String, String>,
}

/// `landingDeps(computer)`'s answer: the deps, the backend a fork there would use (none where it cannot be read),
/// and the place id.
#[derive(Clone)]
pub struct LandingAt {
    pub deps: LandingDeps,
    pub at: Option<Arc<dyn MachineBackend>>,
    pub place_id: Option<String>,
}

/// What `land` hands back. project-landing.ts:34-48.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Landed {
    pub fields: serde_json::Map<String, serde_json::Value>,
    pub notice: Option<String>,
}

/// What the add hears as it lands: a stage and a line.
pub type AddReport = Rc<dyn Fn(wsp_wire::views::workspace::ProjectAddStage, &str)>;

/// `land(o, deps)`'s request. projects.ts:319-326.
pub struct LandAsk {
    pub project: ProjectView,
    pub seed: Option<(Vec<u8>, SeedChoice, SeedPlan)>,
    pub report: AddReport,
}

/// `ProjectLanding` (project-landing.ts:71-104). A method answering `None` is one the road does not have.
#[async_trait(?Send)]
pub trait LandingRoad {
    fn kind(&self) -> LandingKind;
    fn path(&self, name: &str, source: &ProjectSource, deps: &LandingDeps) -> String;
    /// `places({project, memoryKey, deps}).memoryDir`.
    fn memory_dir(&self, project: &ProjectView, memory_key: &str, deps: &LandingDeps) -> String;
    fn lands_at_add(&self, seeding: bool) -> bool;
    async fn land(&self, ask: LandAsk, deps: &LandingDeps) -> Result<Landed, HostError>;
    async fn remove(&self, project: &ProjectView, deps: &LandingDeps) -> Result<String, HostError>;
    async fn folder_stands(&self, project: &ProjectView, deps: &LandingDeps) -> Option<Result<bool, HostError>>;
    async fn seed_into(&self, project: &ProjectView, tar: Vec<u8>, deps: &LandingDeps) -> Option<Result<(), HostError>>;
    async fn unsaved(&self, project: &ProjectView, deps: &LandingDeps) -> Option<Result<Option<String>, HostError>>;
    async fn branch(&self, project: &ProjectView, deps: &LandingDeps) -> Result<ProjectBranch, HostError>;
}

/// What a source resolves to: the remote the holding computer clones, its default branch, the name, and for a folder
/// the seed menu (with the memory key a seed keeps).
#[derive(Clone, Debug, PartialEq)]
pub struct SourceResolved {
    pub name: String,
    pub remote: String,
    pub default_branch: String,
    pub seed: Option<SeedPlan>,
    pub seed_memory_key: Option<String>,
}

/// The callbacks a source's resolve reads.
pub struct ResolveAsk {
    pub seeding: bool,
    pub seed_plan: Rc<dyn Fn(String) -> LocalBoxFuture<'static, Result<SeedPlan, HostError>>>,
    pub folder_remote: Rc<dyn Fn(String) -> LocalBoxFuture<'static, Result<String, HostError>>>,
}

/// `projectSource(kind)` (project-sources.ts).
#[async_trait(?Send)]
pub trait ProjectSourceModule {
    async fn resolve(&self, source: &ProjectSource, ask: ResolveAsk) -> Result<SourceResolved, HostError>;
}

/// What `add` is asked. projects.ts:201.
#[derive(Clone, Default)]
pub struct AddAsk {
    pub source: String,
    pub on: Option<String>,
    pub name: Option<String>,
    pub base: Option<String>,
    pub into: Option<String>,
    pub seed: Option<SeedChoice>,
    /// Named only by a recipe folder moving a project wsp made from it.
    pub id: Option<String>,
    pub created_at: Option<String>,
    pub report: Option<Rc<dyn Fn(&str)>>,
}

/// `ProjectView & { notice? }`.
#[derive(Clone, Debug, PartialEq)]
pub struct AddedProject {
    pub project: ProjectView,
    pub notice: Option<String>,
}

/// What `seed` is asked. projects.ts:180.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SeedWords {
    pub keep: Option<Vec<String>>,
    pub cut: Option<Vec<String>>,
    pub none: bool,
    pub save: bool,
}

impl Projects {
    // ts: projects.ts:108-114
    //|   /** The seed half of an add, which the host wires because it reads a folder of the person's: a runtime without it
    //|    * records a folder as a project on this computer and clones a repo anywhere else, and a folder seeding another
    //|    * computer is refused rather than sent unread. */
    //|   const seedWiring = (): SeedWiring => {
    //|     if (opts.seed === undefined) throw new Error(NO_SEED_WIRING);
    //|     return opts.seed;
    //|   };
    pub fn seed_wiring(&self) -> Result<Rc<dyn wsp_core::types::wiring::SeedWiring>, HostError> {
        todo!()
    }

    // ts: projects.ts:116-122
    //|   /** Which landing road a computer takes, off the kind of workspace it makes rather than off its id: the computer
    //|    * the app runs on works the folder beside itself, a computer the person joined holds the project as a folder in
    //|    * its login's home, and everything else is a provider, which holds nothing of a project. */
    //|   const landingKind = (computer: string, _at?: MachineBackend): ProjectLanding["kind"] => {
    //|     const kind = ctx.kindOf(computer);
    //|     return copiesFolder(kind) ? "mac" : runsInFolder(kind) ? "box" : "provider";
    //|   };
    /// Which landing road a computer takes, off the kind of workspace it makes.
    pub fn landing_kind(&self, computer: &str) -> LandingKind {
        todo!()
    }

    // ts: projects.ts:124-150
    //|   /** What a landing road may ask of this runtime, for one computer: the road that puts the seed archive on it, and
    //|    * where that computer and this Mac keep what a project needs. */
    //|   const landingDeps = async (computer: string): Promise<{ deps: LandingDeps; at: MachineBackend | undefined; placeId: string | undefined }> => {
    //|     const { placeId } = await ctx.landingPlace(computer);
    //|     // A computer this host cannot read a backend for holds nothing of a project: the record still stands, as it
    //|     // did before this road existed, and the road that would have to fork there says so itself when it is asked.
    //|     const at = await ctx.forkingAt(placeId).catch(() => undefined);
    //|     const joined = placeId === undefined ? undefined : placeDoor?.folderComputer(placeId);
    //|     const deps: LandingDeps = {
    //|       land: async (machine, path, bytes) => void (await landBytes(machine, path, bytes)),
    //|       scratch: () => GUEST_TMP,
    //|       ...(joined !== undefined ? { computer: { machine: joined.machine, home: joined.home } } : {}),
    //|       // The vault's GitHub token only where GitHub was set up from the vault there, as that computer's turns read it.
    //|       cloneEnv: (): Record<string, string> => {
    //|         const token = opts.vault?.()[GITHUB_TOKEN_ENV];
    //|         return token === undefined || placeId === undefined || placeDoor?.githubFromVault(placeId) !== true ? {} : { [GITHUB_TOKEN_ENV]: token };
    //|       },
    //|       // Where Claude Code keeps its projects on this computer, which is the memory folder of a project worked in
    //|       // place here; read the way every other road on this computer reads that store.
    //|       macStateHome: local?.home("claude") ?? "",
    //|       branchHere: path => ctx.branchAt(path),
    //|       // The name this wsp holds for that computer, read the way a fork's own line reads it.
    //|       computerName: ctx.placeName(computer),
    //|       now: () => clock.now(),
    //|     };
    //|     return { deps, at, placeId };
    //|   };
    /// What a landing road may ask of this runtime, for one computer (spec rule 3).
    pub async fn landing_deps(&self, computer: &str) -> Result<LandingAt, HostError> {
        todo!()
    }

    // ts: projects.ts:152-158
    //|   /** A computer somebody joined that this host cannot reach right now holds nothing of a project: the sentence is
    //|    * that computer's own absent one, said before anything is made or removed. Read apart from the deps above
    //|    * because a computer with no backend at all is what this Mac looks like to a host with no provider key, and a
    //|    * folder here is still a project. */
    //|   const readableComputer = async (at: MachineBackend | undefined, placeId: string | undefined): Promise<void> => {
    //|     if (at === undefined && placeId !== undefined) await ctx.landingBackend(placeId);
    //|   };
    /// A joined computer this host cannot reach right now holds nothing of a project: its own absent sentence.
    pub async fn readable_computer(&self, at: Option<&Arc<dyn MachineBackend>>, place_id: Option<&str>) -> Result<(), HostError> {
        todo!()
    }

    // ts: projects.ts:164-176
    //|     /** What a seed of a folder on this computer would carry to the computer `on` names, with the ticks a choice
    //|      * remembered for that folder leaves on it. Nothing is read whole and nothing leaves this computer: the menu is
    //|      * git's own listing of what it ignores, one size pass and the agent's memory folder. */
    //|     async seedPlan(source: string, on?: string): Promise<SeedPlan> {
    //|       await ctx.ready();
    //|       const folder = folderNamed(source);
    //|       const read = await seedWiring().plan(folder, await ctx.homesHere());
    //|       const place = on === undefined ? undefined : (await ctx.computerRows()).find(r => namesPlace(r, on));
    //|       const plan = place !== undefined && isProviderPlace(place) ? { ...read, provider: true } : read;
    //|       const remembered = (await store.get(SEED_CHOICES, folder)) as SeedChoice | undefined;
    //|       const ticked = (f: SeedFile): boolean => seedTravels(plan, f) && (remembered === undefined ? f.ticked : remembered.files.includes(f.path));
    //|       return { ...plan, ...(remembered === undefined ? {} : { remembered: true }), files: plan.files.map(f => ({ ...f, ticked: ticked(f) })) };
    //|     },
    /// What a seed of a folder on this computer would carry to the computer `on` names (spec rule 5).
    pub async fn seed_plan(&self, source: &str, on: Option<&str>) -> Result<SeedPlan, HostError> {
        todo!()
    }

    // ts: projects.ts:178-197
    //|     /** The seed picks of a project whose folder is on this computer, moved by the person's words and, with `save`,
    //|      * kept as the folder's remembered choice: the record an add with remember writes and the next add reads. */
    //|     async seed(ref: string, o: { keep?: readonly string[]; cut?: readonly string[]; none?: boolean; save?: boolean }, origin?: Caller): Promise<ProjectSeed> {
    //|       const project = await projectsDoor.resolve(ref, origin);
    //|       if (project.computer !== HERE_PLACE_ID || project.source.kind !== "folder") throw seedNotHereRefusal(project.name, ctx.nameOfComputer(project.computer, await ctx.computerRows()));
    //|       if (o.save === true && scopeOf(origin) !== undefined) throw seedSaveRefusal(project.name, o);
    //|       const folder = folderNamed(project.source.path);
    //|       // These picks feed sub-threads alone, and a sub-thread only runs on a provider.
    //|       const plan = { ...(await projectsDoor.seedPlan(folder)), provider: true };
    //|       const prior = (await store.get(SEED_CHOICES, folder)) as SeedChoice | undefined;
    //|       // The saved picks, else nothing: nothing git ignores goes to a sub-thread's machine before the person picked it.
    //|       const picks = seedPicks(plan, o.none === true ? [] : (prior?.files ?? []), o.keep ?? [], o.cut ?? []);
    //|       if (o.save === true) {
    //|         // The record is the folder's for every add too, so a box's files of values stay in it unless cut by name.
    //|         const cuts = (o.cut ?? []).map(path => path.replace(/\/+$/, ""));
    //|         const held = heldBackPicks(plan, prior?.files ?? []).filter(path => !cuts.includes(path));
    //|         await store.put(SEED_CHOICES, folder, { files: [...picks.files, ...picks.missing, ...held], memory: prior?.memory ?? false, commits: prior?.commits ?? false, remember: true } satisfies SeedChoice);
    //|       }
    //|       return projectSeedOf(project.name, plan, picks, o.save === true);
    //|     },
    /// The seed picks of a project whose folder is here, moved and, with `save`, kept (spec rule 6).
    pub async fn seed(&self, reference: &str, o: SeedWords, origin: Option<Caller>) -> Result<ProjectSeed, HostError> {
        todo!()
    }

    // ts: projects.ts:199-341
    //|     /** `report` hears each line the add says as it goes, beside the stream every client watches: a caller that
    //|      * keeps its own log of the add, as a computer's setup does. */
    //|     async add(o: { source: string; on?: string; name?: string; base?: string; into?: string; seed?: SeedChoice; id?: string; createdAt?: string; report?: (line: string) => void }, origin?: Caller): Promise<ProjectView & { notice?: string }> {
    //|       await ctx.ready();
    //|       // A project is this computer's to record: the folder and the computer named are read here, and a machine
    //|       // that asked would be naming paths on a computer it cannot see.
    //|       ctx.refuseRecording(o.source, origin);
    //|       const rows = await ctx.computerRows();
    //|       const clones = rows.filter(r => r.id !== HERE_PLACE_ID && kindWords(kindForComputer(r.id)).projectSources.includes("git")).map(r => r.name);
    //|       const kind = sourceKind(o.source);
    //|       if (kind === "computer") throw new Error(ADD_IS_A_COMPUTER_LINE);
    //|       if (kind === "folder" && o.into !== undefined) throw new Error(INTO_TAKES_A_REPO_LINE);
    //|       const source: ProjectSource = projectSourceOf(o.source, kind, kind === "folder" ? folderNamed(o.source) : undefined);
    //|       // No --on: a folder is worked here and so is a repo given a folder to clone into; a repo with neither is the
    //|       // person's to place, here or on a computer that clones, which the kind table says are which.
    //|       const on = o.on;
    //|       const computer = on === undefined ? (kind === "folder" || o.into !== undefined ? HERE_PLACE_ID : undefined) : rows.find(r => namesPlace(r, on))?.id;
    //|       if (computer === undefined) {
    //|         if (o.on === undefined) throw new Error(noComputerForSourceLine(o.source, clones));
    //|         throw new Error(noSuchPlaceRefusal(o.on, rows.map(tableName)));
    //|       }
    //|       if (o.into !== undefined && computer !== HERE_PLACE_ID) throw new Error(intoIsHereLine(ctx.nameOfComputer(HERE_PLACE_ID, rows)));
    //|       const place = rows.find(r => r.id === computer);
    //|       if (opts.cloudProjects === false && place !== undefined && isProviderPlace(place)) {
    //|         const refused = cloudProjectRefusal(tableName(place));
    //|         throw usageRefusal(refused.happened, refused.fix);
    //|       }
    //|       const computerKind = kindForComputer(computer);
    //|       const takes = kindWords(computerKind).projectSources;
    //|       if (!takes.includes(source.kind)) throw new Error(source.kind === "folder" ? folderOnCopyRefusal(ctx.nameOfComputer(computer, rows)) : noComputerForSourceLine(o.source, clones));
    //|       // A repo here is cloned into the folder the person named, and that folder is then the project: every road
    //|       // after the clone is the one a folder of theirs already takes.
    //|       if (source.kind !== "folder" && copiesFolder(computerKind)) {
    //|         if (o.into === undefined) throw new Error(cloneIntoNeeded(o.source));
    //|         const into = await ctx.cloneHere(o.source, source, o.into);
    //|         const { into: _into, on: _on, ...rest } = o;
    //|         return projectsDoor.add({ ...rest, source: into }, origin);
    //|       }
    //|       const held = [...projectsHeld.values()].find(p => p.computer === computer && sameSource(p.source, source));
    //|       if (held !== undefined) throw Object.assign(new Error(sameSourceRefusal(held.name, ctx.nameOfComputer(computer, rows))), { kind: "conflict" });
    //|       // A folder here is a project whether or not git holds it; the repo it sits in, its own or one above it, is
    //|       // what its branches and worktrees are read off.
    //|       const top = source.kind === "folder" && copiesFolder(computerKind) ? await ctx.gitTopOf(source.path) : undefined;
    //|       const { deps, at, placeId } = await landingDeps(computer);
    //|       await readableComputer(at, placeId);
    //|       const road = projectLanding(landingKind(computer, at));
    //|       // What the source resolves to on this computer: the remote whichever computer holds the project will clone,
    //|       // and, for a folder here, the menu of what a seed of it would carry.
    //|       const module = projectSource(source.kind);
    //|       const resolved = await module.resolve(source, {
    //|         // A folder is seeded only onto a computer that clones it; the computer the app runs on copies it beside
    //|         // itself and reads nothing of it but its own remote.
    //|         seeding: road.kind !== "mac",
    //|         seedPlan: folder => projectsDoor.seedPlan(folder, computer),
    //|         folderRemote: folder => ctx.remoteHere(folder),
    //|       });
    //|       // Nothing of the person's folder leaves this computer unasked: a seed onto a computer that clones needs the
    //|       // choice they made off the menu, and the road that copies the folder here seeds nothing at all.
    //|       const seeding = resolved.seed !== undefined && road.kind !== "mac";
    //|       if (seeding && o.seed === undefined) throw Object.assign(new Error(seedChoiceNeeded(source.kind === "folder" ? source.path : o.source)), { kind: "invalid" });
    //|       const name = o.name ?? resolved.name;
    //|       // An id and an age are named only by a recipe folder moving the project wsp made from it, which went first.
    //|       const id = o.id ?? `pr_${randomBytes(4).toString("hex")}`;
    //|       const path = road.path({ name, source, deps });
    //|       // The key the agent's memory sits under, fixed here and never recomputed: the folder's own key where a folder
    //|       // on this computer seeded the project, so the memory it already has is the memory it keeps, else the key of
    //|       // the path on the computer holding it.
    //|       const memoryKey = resolved.seed?.memory?.key ?? claudeProjectKey(source.kind === "folder" ? source.path : path);
    //|       const places = road.places({ project: { id, name, path, source }, memoryKey, deps });
    //|       const project: ProjectView = {
    //|         id,
    //|         name,
    //|         computer,
    //|         source,
    //|         path,
    //|         remote: resolved.remote,
    //|         defaultBranch: resolved.defaultBranch,
    //|         memoryKey,
    //|         memoryDir: places.memoryDir,
    //|         // The branch a workspace of this project starts on: the one they named, and otherwise none, which the
    //|         // clone reads as the remote's own default. The branch a seed's unpushed commits were on is never this: a
    //|         // branch the remote has never seen is nothing a clone can ask for, so those commits land on a branch of
    //|         // their own after the clone and the record stays on the branch the remote has.
    //|         ...(o.base !== undefined ? { base: o.base } : {}),
    //|         ...(top !== undefined ? { git: { top } } : {}),
    //|         createdAt: o.createdAt ?? new Date(clock.now()).toISOString(),
    //|       };
    //|       const began = clock.now();
    //|       const report = (stage: ProjectAddStage, message: string): void => {
    //|         bus.emit({ type: "project.add", projectId: project.id, computer, stage, message, elapsedMs: clock.now() - began });
    //|         o.report?.(message);
    //|       };
    //|       // Work runs on the computer when the add has something of the person's to put there, which is a seed: the
    //|       // clone, the files they ticked, the install and, on a provider, the image every workspace of the project
    //|       // forks. A repo the computer can clone by itself is recorded here and cloned by the workspace's own create,
    //|       // which is what it did before this road existed.
    //|       const keep = async (landed: Landed): Promise<ProjectView & { notice?: string }> => {
    //|         // The notice is what the person is told about this add, not a field of the project: the record is written
    //|         // once here with the schema's own fields, so it is taken off before anything is stored or emitted.
    //|         const { notice: _said, ...fields } = landed;
    //|         const recorded: ProjectView = { ...project, ...fields };
    //|         await ctx.rememberProject(recorded);
    //|         // Kept under the folder as this host resolved it, which is the word the next menu is looked up by.
    //|         if (o.seed?.remember === true && source.kind === "folder") {
    //|           // A provider's add holds a box's files of values back rather than unticking them for the next box.
    //|           const prior = (await store.get(SEED_CHOICES, source.path)) as SeedChoice | undefined;
    //|           const held = resolved.seed === undefined ? [] : heldBackPicks(resolved.seed, prior?.files ?? []);
    //|           await store.put(SEED_CHOICES, source.path, { ...o.seed, files: [...new Set([...o.seed.files, ...held])] } satisfies SeedChoice);
    //|         }
    //|         bus.emit({ type: "project.added", project: recorded });
    //|         return recorded;
    //|       };
    //|       const seed = seeding && resolved.seed !== undefined && o.seed !== undefined ? { plan: resolved.seed, choice: o.seed } : undefined;
    //|       if (!road.landsAtAdd({ seeding: seed !== undefined })) return keep({});
    //|       report("planned", addingProjectLine(project.name, source, seed !== undefined));
    //|       try {
    //|         const packed = seed === undefined ? undefined : await seedWiring().pack({ ...seed, homes: await ctx.homesHere() });
    //|         // A login inside a folder they ticked stays on this computer: said as the pack finds it, so the terminal
    //|         // watching the add reads it there and the answer's notice is the tool door's copy of the same fact.
    //|         if (packed !== undefined && packed.left.length > 0) report("seeding", leftBehindLine(packed.left));
    //|         const landed = await road.land(
    //|           {
    //|             project,
    //|             source: module,
    //|             ...(seed !== undefined && packed !== undefined ? { seed: { tar: packed.tar, choice: seed.choice, plan: seed.plan } } : {}),
    //|             report,
    //|           },
    //|           deps,
    //|         );
    //|         const recorded = await keep(landed);
    //|         // The one sentence about where the project is, which every door reads from here: the terminal prints this
    //|         // stage and says nothing of its own after it, so the fact is said once.
    //|         report("done", addedProjectOn(recorded, ctx.nameOfComputer(computer, rows)));
    //|         // What landed and is not what they asked for, each in its own sentence: the commits a computer's git
    //|         // refused, and a login found inside a folder they ticked, which the pack leaves here. Both are theirs to
    //|         // know about on an add that otherwise stands.
    //|         const notices = [...(landed.notice !== undefined ? [landed.notice] : []), ...(packed !== undefined && packed.left.length > 0 ? [leftBehindLine(packed.left)] : [])];
    //|         return notices.length === 0 ? recorded : { ...recorded, notice: notices.join("; ") };
    //|       } catch (e) {
    //|         report("failed", e instanceof Error ? e.message : String(e));
    //|         throw e;
    //|       }
    //|     },
    /// Records a project (spec rule 7).
    pub async fn add(self: &Rc<Self>, o: AddAsk, origin: Option<Caller>) -> Result<AddedProject, HostError> {
        todo!()
    }
}
