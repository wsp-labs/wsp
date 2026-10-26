// SPDX-License-Identifier: AGPL-3.0-only
//! packages/engine/src/image-vault.ts at 65f6b89c7: the image's vault, what the sign-in and secrets stages left on the
//! builder, archived off it before the snapshot so every other place builds its copy of the image from the record.
use std::io::Read;
use std::sync::Arc;

use sha2::{Digest, Sha256};
use wsp_wire::types::shell_quote::shell_quote;
use wsp_wire::wire::machine_link::{vault_member_refusal, VaultRoad};
use wsp_words::js::{js_trim, to_number, utf16};

use crate::errors::EngineError;
use crate::exec_detached::INLINE_EXEC_MS;
use crate::machine::{ExecOptions, Machine};
use crate::vault::{export_paths, import_into, VaultOptions};

/// image-vault.ts:14 `ImageVault`.
#[derive(Clone, Debug, PartialEq)]
pub struct ImageVault {
    pub tar: Vec<u8>,
    pub sha256: String,
    /// How many of the paths asked for were on the machine and went into the archive.
    pub paths: u64,
    /// The paths that went in, absolute: what every member of the archive is judged against.
    pub held: Vec<String>,
}

/// image-vault.ts:28. tar refuses an operand that is not there and the whole export fails with it, so the paths are
/// read first and only the ones that exist are archived. Any exit but 0 is a machine that would not answer.
pub async fn present_paths(machine: &dyn Machine, paths: &[String]) -> Result<Vec<String>, EngineError> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    let quoted: Vec<String> = paths.iter().map(|p| shell_quote(p)).collect();
    let probe = format!("for p in {}; do [ -e \"$p\" ] && echo \"$p\"; done; exit 0", quoted.join(" "));
    let read = machine.exec(&probe, ExecOptions { timeout_ms: Some(INLINE_EXEC_MS), ..ExecOptions::default() }).await?;
    if read.exit_code.get() != 0 {
        return Err(plain(format!(
            "the machine would not say which sign-in paths it holds (exit {}): {}",
            read.exit_code.get(),
            utf16::slice(&read.stderr, -200, None)
        )));
    }
    let found: std::collections::HashSet<&str> = read.stdout.split('\n').map(js_trim).filter(|l| !l.is_empty()).collect();
    Ok(paths.iter().filter(|p| found.contains(p.as_str())).cloned().collect())
}

/// image-vault.ts:42. The tar of the paths that are on the machine, from `/`, and its sha256. Every member is read
/// here, against the paths that were asked for.
pub async fn export_image_vault(machine: &dyn Machine, paths: &[String], opts: &VaultOptions) -> Result<ImageVault, EngineError> {
    let present = present_paths(machine, paths).await?;
    let tar = export_paths(machine, &present, opts).await?;
    refuse_foreign_members(VaultRoad::Seal, &tar, &present)?;
    Ok(ImageVault { sha256: hex::encode(Sha256::digest(&tar)), paths: present.len() as u64, held: present, tar })
}

/// image-vault.ts:52. Lands a vault over `/` on a fresh builder; an overlay, since the pack has already written the
/// config beside the login under the same folders.
pub async fn import_image_vault(machine: Arc<dyn Machine>, tar: &[u8], opts: &VaultOptions) -> Result<(), EngineError> {
    import_into(machine, tar, "/", &VaultOptions { overlay: true, ..opts.clone() }).await?;
    Ok(())
}

/// image-vault.ts:58 `VaultMember["kind"]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemberKind {
    File,
    Dir,
    Symlink,
    Hardlink,
}

/// image-vault.ts:58 `VaultMember`: one member of a vault archive as the guest's tar would extract it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VaultMember {
    /// The name as the header carries it, before any rooting.
    pub name: String,
    pub kind: MemberKind,
    /// A symbolic link's target as written, a hard link's target as the archive names it.
    pub target: Option<String>,
}

const TAR_BLOCK: usize = 512;

fn plain(message: String) -> EngineError {
    EngineError::Other { message, code: None }
}

fn refusal(road: VaultRoad, at: &str, why: &str) -> EngineError {
    plain(vault_member_refusal(road, at, why))
}

/// node's `gunzipSync`: concatenated members read as one, trailing zero bytes ignored, any other byte behind the last
/// member is the start of another and must be one. Its messages are zlib's.
fn gunzip(bytes: &[u8]) -> Result<Vec<u8>, String> {
    if bytes.is_empty() {
        return Err("unexpected end of file".to_string());
    }
    let mut out = Vec::new();
    let mut rest: &[u8] = bytes;
    loop {
        // zlib reads the two magic bytes before anything else: a first byte that is no 0x1f is a bad header at once.
        if rest[0] != 0x1f || (rest.len() >= 2 && rest[1] != 0x8b) {
            return Err("incorrect header check".to_string());
        }
        {
            let mut decoder = flate2::bufread::GzDecoder::new(&mut rest);
            if let Err(e) = decoder.read_to_end(&mut out) {
                return Err(match e.kind() {
                    std::io::ErrorKind::UnexpectedEof => "unexpected end of file".to_string(),
                    _ if e.to_string().contains("invalid gzip header") => "incorrect header check".to_string(),
                    _ => e.to_string(),
                });
            }
        }
        if rest.is_empty() || rest[0] == 0 {
            return Ok(out);
        }
    }
}

/// A NUL-ended string field.
fn field_text(block: &[u8], at: usize, length: usize) -> String {
    let raw = &block[at..(at + length).min(block.len())];
    let end = raw.iter().position(|b| *b == 0).unwrap_or(raw.len());
    String::from_utf8_lossy(&raw[..end]).into_owned()
}

/// A ustar numeric field as tar reads it: octal digits ended by a NUL or a space. A base-256 field is refused.
fn field_number(road: VaultRoad, block: &[u8], at: usize, length: usize, where_: &str, what: &str) -> Result<u64, EngineError> {
    let raw = &block[at..at + length];
    if raw[0] & 0x80 != 0 {
        return Err(refusal(road, where_, &format!("its {what} is written base-256, a form this reading does not read")));
    }
    let latin1: String = raw.iter().map(|b| if *b == 0 { ' ' } else { *b as char }).collect();
    let text = js_trim(&latin1);
    if text.is_empty() {
        return Ok(0);
    }
    if !text.bytes().all(|b| (b'0'..=b'7').contains(&b)) {
        return Err(refusal(road, where_, &format!("its {what} is not an octal number")));
    }
    Ok(u64::from_str_radix(text, 8).unwrap_or(u64::MAX))
}

/// The checksum as tar checks it: the header summed with its own checksum field read as spaces, matched against the
/// field as the unsigned sum and as the signed sum.
fn checksum_ok(road: VaultRoad, block: &[u8], where_: &str) -> Result<bool, EngineError> {
    let want = field_number(road, block, 148, 8, where_, "checksum")? as i64;
    let (mut unsigned, mut signed) = (0i64, 0i64);
    for (i, byte) in block.iter().enumerate().take(TAR_BLOCK) {
        let b = if (148..156).contains(&i) { 0x20 } else { *byte };
        unsigned += b as i64;
        signed += if b > 127 { b as i64 - 256 } else { b as i64 };
    }
    Ok(want == unsigned || want == signed)
}

#[derive(Default, Clone)]
struct Pax {
    path: Option<String>,
    linkpath: Option<String>,
    size: Option<u64>,
}

/// The pax records of an `x` block's data: `<len> <key>=<value>\n`, each read whole.
fn pax_of(road: VaultRoad, data: &[u8], where_: &str) -> Result<Pax, EngineError> {
    let mut held = Pax::default();
    let mut at = 0usize;
    while at < data.len() {
        if data[at] == 0 {
            break;
        }
        let Some(space) = data[at..].iter().position(|b| *b == 0x20).map(|i| at + i) else {
            return Err(refusal(road, where_, "its pax record has no length"));
        };
        let latin1: String = data[at..space].iter().map(|b| *b as char).collect();
        let length = to_number(&latin1);
        if !(length.fract() == 0.0 && length > 0.0 && at as f64 + length <= data.len() as f64) {
            return Err(refusal(road, where_, "its pax record runs past the block it is written in"));
        }
        let end = at + length as usize;
        let raw = if space < end { &data[space + 1..end] } else { &data[..0] };
        let mut record = String::from_utf8_lossy(raw).into_owned();
        if record.ends_with('\n') {
            record.pop();
        }
        let Some(eq) = record.find('=') else {
            return Err(refusal(road, where_, "its pax record carries no key"));
        };
        let (key, value) = (&record[..eq], &record[eq + 1..]);
        if !matches!(key, "path" | "linkpath" | "size") {
            return Err(refusal(road, where_, &format!("its pax record carries {key}, which this reading does not read")));
        }
        match key {
            "size" => {
                let size = to_number(value);
                if !(size.fract() == 0.0 && size >= 0.0 && size.is_finite()) {
                    return Err(refusal(road, where_, "its pax size is not a whole number of bytes"));
                }
                held.size = Some(size as u64);
            }
            "path" => held.path = Some(value.to_string()),
            _ => held.linkpath = Some(value.to_string()),
        }
        at = end;
    }
    Ok(held)
}

/// image-vault.ts:137. Every member of a gzipped tar, read as the guest's tar would extract it. Every divergence is a
/// refusal of the whole archive rather than a skip.
pub fn vault_members(road: VaultRoad, tar: &[u8]) -> Result<Vec<VaultMember>, EngineError> {
    let raw = match gunzip(tar) {
        Ok(raw) => raw,
        Err(e) => return Err(refusal(road, "its first bytes", &format!("they do not decompress as tar's gzip reads them ({e})"))),
    };
    if raw.len() % TAR_BLOCK != 0 {
        return Err(refusal(road, "its last bytes", &format!("the archive is {} bytes, which is not a whole number of tar blocks", raw.len())));
    }
    let mut members = Vec::new();
    let mut pax: Option<Pax> = None;
    let mut long_name: Option<String> = None;
    let mut long_link: Option<String> = None;
    let mut at = 0usize;
    while at < raw.len() {
        let where_ = format!("block {}", at / TAR_BLOCK);
        let block = &raw[at..at + TAR_BLOCK];
        if block.iter().all(|b| *b == 0) {
            if at + 2 * TAR_BLOCK <= raw.len() && raw[at + TAR_BLOCK..at + 2 * TAR_BLOCK].iter().all(|b| *b == 0) {
                return Ok(members);
            }
            return Err(refusal(road, &where_, "a zero block stands where tar reads two to end the archive"));
        }
        let ustar = &raw[at + 257..at + 263] == b"ustar\0";
        let gnu = &raw[at + 257..at + 265] == b"ustar  \0";
        if !ustar && !gnu {
            return Err(refusal(road, &where_, "its header is neither a ustar nor a GNU header"));
        }
        if !checksum_ok(road, block, &where_)? {
            return Err(refusal(road, &where_, "its header fails its own checksum"));
        }
        let kind_byte = if block[156] == 0 { b'0' } else { block[156] };
        let kind = kind_byte as char;
        let size = match pax.as_ref().and_then(|p| p.size) {
            Some(s) => s,
            None => field_number(road, block, 124, 12, &where_, "size")?,
        };
        let data_blocks = size.div_ceil(TAR_BLOCK as u64) as usize;
        let data_at = at + TAR_BLOCK;
        if (data_at as u64) + (data_blocks as u64) * TAR_BLOCK as u64 > raw.len() as u64 {
            return Err(refusal(road, &where_, "its data runs past the end of the archive"));
        }
        let data = &raw[data_at..data_at + size as usize];
        at += data_blocks * TAR_BLOCK;
        if matches!(kind, 'x' | 'L' | 'K') {
            if pax.is_some() || (kind == 'x' && (long_name.is_some() || long_link.is_some())) {
                return Err(refusal(road, &where_, "one member carries both a pax record and a long name entry"));
            }
            if if kind == 'L' { long_name.is_some() } else { kind == 'K' && long_link.is_some() } {
                return Err(refusal(road, &where_, "one member carries two long name entries of the same kind"));
            }
            match kind {
                'x' => pax = Some(pax_of(road, data, &where_)?),
                'L' => long_name = Some(field_text(data, 0, data.len())),
                _ => long_link = Some(field_text(data, 0, data.len())),
            }
            at += TAR_BLOCK;
            continue;
        }
        if !matches!(kind, '0' | '1' | '2' | '5') {
            return Err(refusal(road, &where_, &format!("its type flag is {kind}, which is not a file, a folder or a link")));
        }
        if kind != '0' && size != 0 {
            return Err(refusal(road, &where_, "a folder or a link carries data, so where its next header stands cannot be read"));
        }
        let prefix = if ustar { field_text(block, 345, 155) } else { String::new() };
        let named = field_text(block, 0, 100);
        let held = pax.take().unwrap_or_default();
        let name = held.path.or(long_name.take()).unwrap_or_else(|| if prefix.is_empty() { named } else { format!("{prefix}/{named}") });
        let target = held.linkpath.or(long_link.take()).unwrap_or_else(|| field_text(block, 157, 100));
        long_name = None;
        long_link = None;
        let kind = match kind {
            '5' => MemberKind::Dir,
            '2' => MemberKind::Symlink,
            '1' => MemberKind::Hardlink,
            _ => MemberKind::File,
        };
        let target = matches!(kind, MemberKind::Symlink | MemberKind::Hardlink).then_some(target);
        members.push(VaultMember { name, kind, target });
        at += TAR_BLOCK;
    }
    Err(refusal(road, "its last bytes", "the archive ends without the two zero blocks tar reads to end it"))
}

/// image-vault.ts:196. Where a name lands under `-C /`, as tar roots it: leading slashes and `.` segments gone. Nothing
/// where a `..` segment walks out.
fn lands_at(name: &str) -> Option<String> {
    let parts: Vec<&str> = name.split('/').filter(|p| !p.is_empty() && *p != ".").collect();
    if parts.contains(&"..") {
        None
    } else {
        Some(format!("/{}", parts.join("/")))
    }
}

/// image-vault.ts:203. Where a symbolic link's target points: a path on the guest, absolute as written or read from
/// the folder the link itself lands in.
fn points_at(target: &str, from: &str) -> Option<String> {
    let joined = if target.starts_with('/') { target.to_string() } else { format!("{}/{}", &from[..from.rfind('/').unwrap_or(0)], target) };
    let mut out: Vec<&str> = Vec::new();
    for part in joined.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part != ".." {
            out.push(part);
        } else if out.pop().is_none() {
            return None;
        }
    }
    Some(format!("/{}", out.join("/")))
}

fn under_held(path: &str, held: &[String]) -> bool {
    held.iter().any(|h| lands_at(h).is_some_and(|root| path == root || path.starts_with(&format!("{root}/"))))
}

/// image-vault.ts:224. Why a member may not land, or nothing when it may: the one rule the seal and the copy both read.
pub fn refused_member(member: &VaultMember, held: &[String]) -> Option<String> {
    let Some(lands) = lands_at(&member.name) else {
        return Some("its name walks out of where it lands with ..".to_string());
    };
    if !under_held(&lands, held) {
        return Some(format!("it lands at {lands}, which is not one of the paths the seal asked for or under one"));
    }
    let target = member.target.as_deref().filter(|t| !t.is_empty())?;
    let points = if member.kind == MemberKind::Hardlink { lands_at(target) } else { points_at(target, &lands) };
    match points {
        Some(p) if under_held(&p, held) => None,
        _ => Some(format!("it points at {target}, which is not under the paths the seal asked for")),
    }
}

/// image-vault.ts:236. The archive against the paths the seal asked for, read whole before any of it is handed to a
/// machine: one member outside them refuses all of it.
pub fn refuse_foreign_members(road: VaultRoad, tar: &[u8], held: &[String]) -> Result<(), EngineError> {
    for member in vault_members(road, tar)? {
        if let Some(why) = refused_member(&member, held) {
            return Err(plain(vault_member_refusal(road, &member.name, &why)));
        }
    }
    Ok(())
}

/// image-vault.ts:246. The header the image hash is taken under, so a hash can never be read as one of another rule's.
const IMAGE_HASH_RULE: &str = "wsp-image-2";
/// image-vault.ts:249. What an image with no vault hashes as: its own word, never the empty string.
const NO_VAULT: &str = "none";

/// image-vault.ts:249. One rule for the image hash: the recipe the copy is built from and the vault it imports.
pub fn image_hash(recipe_hash: &str, vault_sha256: Option<&str>) -> String {
    hex::encode(Sha256::digest(format!("{IMAGE_HASH_RULE}\n{recipe_hash}\n{}", vault_sha256.unwrap_or(NO_VAULT)).as_bytes()))
}
