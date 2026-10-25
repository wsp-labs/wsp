// SPDX-License-Identifier: AGPL-3.0-only
//! The images projects wear, kept as files under the wsp home by the SHA-256 of their bytes, never in the preferences
//! record. The host checks that what arrives is the one PNG shape the window makes and writes back IHDR, IDAT and IEND
//! alone. Port of runtime/src/projects/images.ts at 65f6b89c7; spec/images.md.
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use async_trait::async_trait;
use futures::future::{LocalBoxFuture, Shared};
use indexmap::{IndexMap, IndexSet};
use wsp_core::ids::Ids;
use wsp_core::types::wiring::RecipeShelf;
use wsp_wire::views::preferences::Preferences;
use wsp_words::exit::HostError;

/// The PNG signature. images.ts:15.
pub const SIGNATURE: [u8; 8] = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];

/// One chunk as read off a file: its type (latin1), its data and the whole chunk with length and CRC. images.ts:20-24.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chunk {
    pub r#type: String,
    pub data: Vec<u8>,
    pub whole: Vec<u8>,
}

// const refused = (why: string): Error => {
//   const { said, fix } = projectIconRefusal(why);
//   return usageRefusal(said, fix);
// };
/// The refusal naming the rule a file broke: said `The host did not take this image: <why>.`, fix
/// `PROJECT_ICON_AGAIN`, kind usage.
pub fn refused(why: &str) -> HostError {
    todo!()
}

// function chunksOf(png: Buffer): Chunk[] { ... }  (images.ts:31-47)
/// The chunks from byte 8 up to and with the first IEND; refuses a cut chunk or a bad checksum (spec kept rule 3).
pub fn chunks_of(png: &[u8]) -> Result<Vec<Chunk>, HostError> {
    todo!()
}

// export function keptProjectIcon(png: Buffer): Buffer { ... }  (images.ts:52-82)
/// The PNG a project's image is kept as: the same pixels with IHDR, IDAT and IEND alone. Refused, naming the rule,
/// for anything but a 128 px square at 8 bits, RGBA or RGB, no interlace, no animation, at most
/// `PROJECT_ICON_MAX_BYTES`, whose pixels inflate to exactly the rows that size holds (spec kept rules 1-15).
pub fn kept_project_icon(png: &[u8]) -> Result<Vec<u8>, HostError> {
    todo!()
}

/// `Buffer.from(text, "base64")`: both alphabets, characters outside them skipped, decoding stopped at the first `=`.
pub fn node_base64(text: &str) -> Vec<u8> {
    todo!()
}

/// The file store. images.ts:84-134.
pub struct ProjectIcons {
    dir: Rc<dyn Fn() -> PathBuf>,
    ids: Rc<dyn Ids>,
}

// export function projectIcons(dir: () => string): ProjectIcons {
pub fn project_icons(dir: Rc<dyn Fn() -> PathBuf>, ids: Rc<dyn Ids>) -> ProjectIcons {
    todo!()
}

impl ProjectIcons {
    // keep(png) {
    //   const bytes = keptProjectIcon(png);
    //   const hash = createHash("sha256").update(bytes).digest("hex");
    //   mkdirSync(dir(), { recursive: true, mode: 0o700 });
    //   const part = join(dir(), `.${hash}.${randomBytes(4).toString("hex")}.part`);
    //   writeFileSync(part, bytes, { mode: 0o600 });
    //   renameSync(part, pathOf(hash));
    //   return hash;
    // },
    /// Keeps the PNG the window sent and answers the hash it is kept under.
    pub fn keep(&self, png: &[u8]) -> Result<String, HostError> {
        todo!()
    }

    // read(hashes) { ... }  (images.ts:115-126)
    /// Each hash's image as a data url, read from disk now; None for a hash not kept or not a hash.
    pub fn read(&self, hashes: &[String]) -> IndexMap<String, Option<String>> {
        todo!()
    }

    // holds: hash => kept().includes(hash),
    pub fn holds(&self, hash: &str) -> bool {
        todo!()
    }

    // sweep(named) {
    //   for (const hash of kept()) if (!named.has(hash)) rmSync(pathOf(hash), { force: true });
    //   for (const name of listed()) if (name.endsWith(".part")) rmSync(join(dir(), name), { force: true });
    // },
    /// Deletes every kept image whose hash is not in `named`, and every part a cut write left.
    pub fn sweep(&self, named: &IndexSet<String>) {
        todo!()
    }
}

/// What the image area reads off the runtime context. images.ts:137-190.
#[async_trait(?Send)]
pub trait ProjectIconsCtx {
    fn state_folder(&self) -> PathBuf;
    /// `ctx.opts.statePath !== undefined`: a host with no state file keeps no files.
    fn has_state_path(&self) -> bool;
    async fn ready(&self) -> Result<(), HostError>;
    /// `ctx.projectsHeld.has(id)`.
    fn project_held(&self, project_id: &str) -> bool;
    async fn preferences(&self) -> Result<Preferences, HostError>;
    /// `ctx.changePreferences(fn)`: the change runs on the preferences as they stand, in the preferences' own turn.
    async fn change_preferences(&self, change: Box<dyn FnOnce(Preferences) -> Preferences>) -> Result<Preferences, HostError>;
    /// `ctx.opts.recipes`, none where the host wires no shelf.
    fn recipes(&self) -> Option<Rc<dyn RecipeShelf>>;
    /// `ctx.bus.on("recipes.changed", f)`.
    fn on_recipes_changed(&self, listener: Rc<dyn Fn()>);
    fn ids(&self) -> Rc<dyn Ids>;
}

/// The chain every write and sweep runs on: the tail, with its failure swallowed.
pub type IconTurn = Shared<LocalBoxFuture<'static, ()>>;

/// A project's image as every road of the runtime moves it. images.ts:137-190.
pub struct ProjectIconsArea {
    ctx: Rc<dyn ProjectIconsCtx>,
    images: Rc<ProjectIcons>,
    work: Rc<RefCell<Option<IconTurn>>>,
}

// export function projectIconsArea(ctx: RuntimeContext): ProjectIconsArea {
//   const images = projectIcons(() => join(ctx.stateFolder(), "project-icons"));
//   ...
//   ctx.bus.on("recipes.changed", () => void inTurn(sweep).catch(() => undefined));
/// Builds the area over `<state folder>/project-icons` and listens for `recipes.changed`.
pub fn project_icons_area(ctx: Rc<dyn ProjectIconsCtx>) -> Rc<ProjectIconsArea> {
    todo!()
}

impl ProjectIconsArea {
    // set: (projectId, png) => inTurn(async () => {
    //   await ctx.ready();
    //   if (!ctx.projectsHeld.has(projectId)) throw Object.assign(new Error(bareNoSuchProjectLine(projectId)), { kind: "usage" });
    //   const hash = png === null ? null : images.keep(Buffer.from(png, "base64"));
    //   await named(projectId, hash);
    //   await sweep();
    //   return { image: hash };
    // }),
    /// Answers `{image}`: the kept hash, None where the image was taken off.
    pub async fn set(&self, project_id: &str, png: Option<String>) -> Result<Option<String>, HostError> {
        todo!()
    }

    // read: async hashes => (ctx.opts.statePath === undefined ? Object.fromEntries(hashes.map(h => [h, null])) : images.read(hashes)),
    pub async fn read(&self, hashes: Vec<String>) -> Result<IndexMap<String, Option<String>>, HostError> {
        todo!()
    }

    // forget: projectId => inTurn(async () => { ...drop projectLook and projectIcon...; await sweep(); }),
    pub async fn forget(&self, project_id: &str) -> Result<(), HostError> {
        todo!()
    }

    // wear: (projectId, hash) => inTurn(async () => {
    //   if (hash === undefined || ctx.opts.statePath === undefined || !images.holds(hash)) return;
    //   await named(projectId, hash);
    // }),
    pub async fn wear(&self, project_id: &str, hash: Option<String>) -> Result<(), HostError> {
        todo!()
    }

    // const sweep = async (): Promise<void> => { ... }  (images.ts:147-155)
    /// Deletes each kept image no project and no recipe names; a shelf that cannot be read deletes nothing. Not in
    /// the chain by itself: callers run it inside their turn.
    pub async fn sweep(&self) -> Result<(), HostError> {
        todo!()
    }
}

impl wsp_core::context::ProjectIconsArea for ProjectIconsArea {}
