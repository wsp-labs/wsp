// SPDX-License-Identifier: AGPL-3.0-only
//! The images projects wear, kept as files under the wsp home by the SHA-256 of their bytes, never in the preferences
//! record. The host checks that what arrives is the one PNG shape the window makes and writes back IHDR, IDAT and IEND
//! alone. Port of runtime/src/projects/images.ts at 65f6b89c7; spec/images.md.
use std::cell::RefCell;
use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::PathBuf;
use std::rc::Rc;

use async_trait::async_trait;
use base64::Engine;
use flate2::read::ZlibDecoder;
use futures::channel::oneshot;
use futures::future::{FutureExt, LocalBoxFuture, Shared};
use indexmap::{IndexMap, IndexSet};
use sha2::{Digest, Sha256};

use wsp_core::ids::Ids;
use wsp_core::types::wiring::RecipeShelf;
use wsp_wire::types::project_look::{ProjectIconHash, PROJECT_ICON_MAX_BYTES, PROJECT_ICON_PX};
use wsp_wire::views::preferences::Preferences;
use wsp_words::exit::{usage_refusal, HostError, Kind};
use wsp_words::project_look::project_icon_refusal;
use wsp_words::projects::bare_no_such_project_line;

/// The PNG signature. images.ts:15.
pub const SIGNATURE: [u8; 8] = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];

/// One chunk as read off a file: its type (latin1), its data and the whole chunk with length and CRC. images.ts:20-24.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chunk {
    pub r#type: String,
    pub data: Vec<u8>,
    pub whole: Vec<u8>,
}

/// The refusal naming the rule a file broke: said `The host did not take this image: <why>.`, fix
/// `PROJECT_ICON_AGAIN`, kind usage.
pub fn refused(why: &str) -> HostError {
    let said = project_icon_refusal(why);
    usage_refusal(&said.said, &said.fix)
}

/// The chunks from byte 8 up to and with the first IEND; refuses a cut chunk or a bad checksum (spec kept rule 3).
pub fn chunks_of(png: &[u8]) -> Result<Vec<Chunk>, HostError> {
    let mut chunks = Vec::new();
    let mut at = SIGNATURE.len();
    while at < png.len() {
        if at + 12 > png.len() {
            return Err(refused("it ends inside a chunk"));
        }
        let length = u32::from_be_bytes([png[at], png[at + 1], png[at + 2], png[at + 3]]) as usize;
        let end = at + 12 + length;
        if end > png.len() {
            return Err(refused("it ends inside a chunk"));
        }
        let kind: String = png[at + 4..at + 8].iter().map(|&b| b as char).collect();
        let data = png[at + 8..at + 8 + length].to_vec();
        let stored = u32::from_be_bytes([png[at + 8 + length], png[at + 9 + length], png[at + 10 + length], png[at + 11 + length]]);
        if crc32fast::hash(&png[at + 4..at + 8 + length]) != stored {
            return Err(refused(&format!("its {kind} chunk fails its checksum")));
        }
        chunks.push(Chunk { r#type: kind.clone(), data, whole: png[at..end].to_vec() });
        at = end;
        if kind == "IEND" {
            break;
        }
    }
    Ok(chunks)
}

/// Why an inflate stopped: past its cap, or not a stream it can read (`ERR_BUFFER_TOO_LARGE` against any other).
enum InflateFault {
    TooLarge,
    Broken,
}

/// `inflateSync(data, { maxOutputLength: limit })`: the output, or the fault, read at most one byte past the cap.
fn inflate_capped(data: &[u8], limit: usize) -> Result<Vec<u8>, InflateFault> {
    let mut out = Vec::new();
    match ZlibDecoder::new(data).take(limit as u64 + 1).read_to_end(&mut out) {
        Ok(_) if out.len() > limit => Err(InflateFault::TooLarge),
        Ok(_) => Ok(out),
        Err(_) if out.len() > limit => Err(InflateFault::TooLarge),
        Err(_) => Err(InflateFault::Broken),
    }
}

/// The PNG a project's image is kept as, out of what the window sent: the same pixels with IHDR, IDAT and IEND alone.
/// Refused, naming the rule, for anything but a 128 px square at 8 bits, RGBA or RGB, with no interlace, no
/// animation, at most PROJECT_ICON_MAX_BYTES, whose pixels inflate to exactly the rows that size holds.
pub fn kept_project_icon(png: &[u8]) -> Result<Vec<u8>, HostError> {
    if png.len() as u64 > PROJECT_ICON_MAX_BYTES {
        return Err(refused(&format!("it is {} bytes, over the {PROJECT_ICON_MAX_BYTES} an image may be", png.len())));
    }
    if png.len() < SIGNATURE.len() || png[..SIGNATURE.len()] != SIGNATURE {
        return Err(refused("it is not a PNG"));
    }
    let chunks = chunks_of(png)?;
    let head = match chunks.first() {
        Some(head) if head.r#type == "IHDR" && head.data.len() == 13 => head,
        _ => return Err(refused("it does not open with its header")),
    };
    let last = match chunks.last() {
        Some(last) if last.r#type == "IEND" => last,
        _ => return Err(refused("it has no end chunk")),
    };
    let width = u32::from_be_bytes([head.data[0], head.data[1], head.data[2], head.data[3]]);
    let height = u32::from_be_bytes([head.data[4], head.data[5], head.data[6], head.data[7]]);
    let (depth, colour, compression, filter, interlace) = (head.data[8], head.data[9], head.data[10], head.data[11], head.data[12]);
    let px = PROJECT_ICON_PX as u32;
    if width != px || height != px {
        return Err(refused(&format!("it is {width} by {height} pixels, not {px} by {px}")));
    }
    if depth != 8 {
        return Err(refused(&format!("it has {depth} bits a channel, not 8")));
    }
    let channels: usize = match colour {
        6 => 4,
        2 => 3,
        _ => return Err(refused(&format!("its colour type is {colour}, not RGBA or RGB"))),
    };
    if compression != 0 || filter != 0 {
        return Err(refused("its header names a method PNG does not have"));
    }
    if interlace != 0 {
        return Err(refused("it is interlaced"));
    }
    if chunks.iter().any(|c| c.r#type == "acTL") {
        return Err(refused("it is animated"));
    }
    let stride = 1 + PROJECT_ICON_PX as usize * channels;
    let expected = PROJECT_ICON_PX as usize * stride;
    let idat: Vec<&Chunk> = chunks.iter().filter(|c| c.r#type == "IDAT").collect();
    let compressed: Vec<u8> = idat.iter().flat_map(|c| c.data.iter().copied()).collect();
    let pixels = match inflate_capped(&compressed, expected + 1) {
        Ok(pixels) => pixels,
        Err(InflateFault::TooLarge) => return Err(refused(&format!("its pixels inflate past the {expected} bytes {px} px holds"))),
        Err(InflateFault::Broken) => return Err(refused("its pixels do not inflate")),
    };
    if pixels.len() != expected {
        return Err(refused(&format!("its pixels inflate to {} bytes, not {expected}", pixels.len())));
    }
    for row in 0..PROJECT_ICON_PX as usize {
        if pixels[row * stride] > 4 {
            return Err(refused(&format!("row {row} names a filter PNG does not have")));
        }
    }
    let mut kept = SIGNATURE.to_vec();
    kept.extend_from_slice(&head.whole);
    for chunk in &idat {
        kept.extend_from_slice(&chunk.whole);
    }
    kept.extend_from_slice(&last.whole);
    Ok(kept)
}

/// `Buffer.from(text, "base64")`: both alphabets, characters outside them skipped, decoding stopped at the first `=`.
pub fn node_base64(text: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let mut bits: u32 = 0;
    let mut count: u32 = 0;
    for c in text.chars() {
        let value = match c {
            '=' => break,
            'A'..='Z' => c as u32 - 'A' as u32,
            'a'..='z' => c as u32 - 'a' as u32 + 26,
            '0'..='9' => c as u32 - '0' as u32 + 52,
            '+' | '-' => 62,
            '/' | '_' => 63,
            _ => continue,
        };
        bits = (bits << 6) | value;
        count += 6;
        if count >= 8 {
            count -= 8;
            out.push((bits >> count) as u8);
            bits &= (1 << count) - 1;
        }
    }
    out
}

/// `ProjectIconHash.safeParse`: 64 lowercase hex characters.
fn is_hash(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// The hash a kept file's name carries (`^([0-9a-f]{64})\.png$`).
fn kept_hash_of(name: &str) -> Option<&str> {
    let hash = name.strip_suffix(".png")?;
    is_hash(hash).then_some(hash)
}

/// The names in a folder, as a listing reads them; a folder that cannot be read lists nothing.
fn listed(folder: &PathBuf) -> Vec<String> {
    fs::read_dir(folder).map(|entries| entries.filter_map(|entry| entry.ok()?.file_name().into_string().ok()).collect()).unwrap_or_default()
}

fn io_error(e: std::io::Error) -> HostError {
    HostError::new(e.to_string())
}

/// The file store. images.ts:84-134.
pub struct ProjectIcons {
    dir: Rc<dyn Fn() -> Result<PathBuf, HostError>>,
    ids: Rc<dyn Ids>,
}

/// `projectIcons(dir)`: the file store over the folder `dir` names, the random part of a write's name from `ids`.
pub fn project_icons(dir: Rc<dyn Fn() -> Result<PathBuf, HostError>>, ids: Rc<dyn Ids>) -> ProjectIcons {
    ProjectIcons { dir, ids }
}

impl ProjectIcons {
    /// Keeps the PNG the window sent and answers the hash it is kept under.
    pub fn keep(&self, png: &[u8]) -> Result<String, HostError> {
        let bytes = kept_project_icon(png)?;
        let hash = hex::encode(Sha256::digest(&bytes));
        let folder = (self.dir)()?;
        fs::DirBuilder::new().recursive(true).mode(0o700).create(&folder).map_err(io_error)?;
        let part = folder.join(format!(".{hash}.{}.part", hex::encode(self.ids.bytes(4))));
        let mut file = fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&part).map_err(io_error)?;
        file.write_all(&bytes).map_err(io_error)?;
        drop(file);
        fs::rename(&part, folder.join(format!("{hash}.png"))).map_err(io_error)?;
        Ok(hash)
    }

    /// Each hash's image as a data url, read from disk now; None for a hash not kept or not a hash.
    pub fn read(&self, hashes: &[String]) -> Result<IndexMap<String, Option<String>>, HostError> {
        let folder = (self.dir)()?;
        let mut out = IndexMap::new();
        for hash in hashes {
            let image = if is_hash(hash) {
                fs::read(folder.join(format!("{hash}.png")))
                    .ok()
                    .map(|bytes| format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
            } else {
                None
            };
            out.insert(hash.clone(), image);
        }
        Ok(out)
    }

    /// Whether the image is kept: its file is in the folder's listing now.
    pub fn holds(&self, hash: &str) -> Result<bool, HostError> {
        let name = format!("{hash}.png");
        Ok(is_hash(hash) && listed(&(self.dir)()?).contains(&name))
    }

    /// Deletes every kept image whose hash is not in `named`, and every part a cut write left.
    pub fn sweep(&self, named: &IndexSet<String>) -> Result<(), HostError> {
        let folder = (self.dir)()?;
        for name in listed(&folder) {
            if kept_hash_of(&name).is_some_and(|hash| !named.contains(hash)) {
                let _ = fs::remove_file(folder.join(&name));
            }
        }
        for name in listed(&folder) {
            if name.ends_with(".part") {
                let _ = fs::remove_file(folder.join(&name));
            }
        }
        Ok(())
    }
}

/// What the image area reads off the runtime context. images.ts:137-190.
#[async_trait(?Send)]
pub trait ProjectIconsCtx {
    fn state_folder(&self) -> Result<PathBuf, HostError>;
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

/// `inTurn(run)`: `body` runs after the turn before it settles; the caller gets its answer, the chain's tail none.
fn enqueue<T: 'static>(
    work: &RefCell<Option<IconTurn>>,
    body: LocalBoxFuture<'static, Result<T, HostError>>,
) -> LocalBoxFuture<'static, Result<T, HostError>> {
    let prev = work.borrow().clone();
    let (answer, answered) = oneshot::channel::<Result<T, HostError>>();
    let tail: IconTurn = async move {
        if let Some(prev) = prev {
            prev.await;
        }
        let _ = answer.send(body.await);
    }
    .boxed_local()
    .shared();
    *work.borrow_mut() = Some(tail.clone());
    async move {
        tail.await;
        answered.await.unwrap_or_else(|_| Err(HostError::new("the image turn ended before it answered")))
    }
    .boxed_local()
}

/// `changePreferences`: the project's image is `hash`, moved to the end of the map, or none (images.ts:156).
async fn change_named(ctx: &dyn ProjectIconsCtx, project_id: &str, hash: Option<String>) -> Result<(), HostError> {
    let project = project_id.to_string();
    ctx.change_preferences(Box::new(move |mut held: Preferences| {
        held.project_icon.shift_remove(&project);
        if let Some(hash) = hash {
            held.project_icon.insert(project, ProjectIconHash(hash));
        }
        held
    }))
    .await?;
    Ok(())
}

/// `sweep` (images.ts:147): deletes each kept image no project and no recipe names; a shelf that cannot be read
/// deletes nothing.
async fn sweep_now(ctx: &dyn ProjectIconsCtx, images: &ProjectIcons) -> Result<(), HostError> {
    let preferences = ctx.preferences().await?;
    let mut named: IndexSet<String> = preferences.project_icon.values().map(|hash| hash.0.clone()).collect();
    if let Some(shelf) = ctx.recipes() {
        let Ok(list) = shelf.list().await else {
            return Ok(());
        };
        for shelved in list {
            for folder in shelved.file.folders.values() {
                if let Some(image) = &folder.image {
                    named.insert(image.0.clone());
                }
            }
        }
    }
    if ctx.has_state_path() {
        images.sweep(&named)?;
    }
    Ok(())
}

/// Builds the area over `<state folder>/project-icons` and listens for `recipes.changed`.
pub fn project_icons_area(ctx: Rc<dyn ProjectIconsCtx>) -> Rc<ProjectIconsArea> {
    let folder_ctx = ctx.clone();
    let images = Rc::new(project_icons(Rc::new(move || folder_ctx.state_folder().map(|folder| folder.join("project-icons"))), ctx.ids()));
    let work: Rc<RefCell<Option<IconTurn>>> = Rc::new(RefCell::new(None));
    let (listener_ctx, listener_images, listener_work) = (ctx.clone(), images.clone(), work.clone());
    ctx.on_recipes_changed(Rc::new(move || {
        let (ctx, images) = (listener_ctx.clone(), listener_images.clone());
        drop(enqueue(&listener_work, async move { sweep_now(&*ctx, &images).await }.boxed_local()));
    }));
    Rc::new(ProjectIconsArea { ctx, images, work })
}

impl ProjectIconsArea {
    /// Answers `{image}`: the kept hash, None where the image was taken off.
    pub async fn set(&self, project_id: &str, png: Option<String>) -> Result<Option<String>, HostError> {
        let (ctx, images, project) = (self.ctx.clone(), self.images.clone(), project_id.to_string());
        enqueue(
            &self.work,
            async move {
                ctx.ready().await?;
                if !ctx.project_held(&project) {
                    return Err(HostError::with_kind(bare_no_such_project_line(&project), Kind::Usage));
                }
                let hash = match png {
                    Some(png) => Some(images.keep(&node_base64(&png))?),
                    None => None,
                };
                change_named(&*ctx, &project, hash.clone()).await?;
                sweep_now(&*ctx, &images).await?;
                Ok(hash)
            }
            .boxed_local(),
        )
        .await
    }

    /// Each hash's image as a data url; every hash null where the host keeps no files.
    pub async fn read(&self, hashes: Vec<String>) -> Result<IndexMap<String, Option<String>>, HostError> {
        if !self.ctx.has_state_path() {
            return Ok(hashes.into_iter().map(|hash| (hash, None)).collect());
        }
        self.images.read(&hashes)
    }

    /// Drops the project's look and its image, then sweeps.
    pub async fn forget(&self, project_id: &str) -> Result<(), HostError> {
        let (ctx, images, project) = (self.ctx.clone(), self.images.clone(), project_id.to_string());
        enqueue(
            &self.work,
            async move {
                ctx.change_preferences(Box::new(move |mut held: Preferences| {
                    held.project_look.shift_remove(&project);
                    held.project_icon.shift_remove(&project);
                    held
                }))
                .await?;
                sweep_now(&*ctx, &images).await
            }
            .boxed_local(),
        )
        .await
    }

    /// Names a kept image as the project's, where the host keeps files and holds this one.
    pub async fn wear(&self, project_id: &str, hash: Option<String>) -> Result<(), HostError> {
        let (ctx, images, project) = (self.ctx.clone(), self.images.clone(), project_id.to_string());
        enqueue(
            &self.work,
            async move {
                let Some(hash) = hash else {
                    return Ok(());
                };
                if !ctx.has_state_path() || !images.holds(&hash)? {
                    return Ok(());
                }
                change_named(&*ctx, &project, Some(hash)).await
            }
            .boxed_local(),
        )
        .await
    }

    /// Deletes each kept image no project and no recipe names; a shelf that cannot be read deletes nothing. Not in
    /// the chain by itself: callers run it inside their turn.
    pub async fn sweep(&self) -> Result<(), HostError> {
        sweep_now(&*self.ctx, &self.images).await
    }
}

impl wsp_core::context::ProjectIconsArea for ProjectIconsArea {}
