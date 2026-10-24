// SPDX-License-Identifier: AGPL-3.0-only
//! What an AppImage holds, read off its squashfs without running or mounting it. A type 2 AppImage is its runtime,
//! an ELF file, with the squashfs right after it: at the end of the runtime's section headers or of its last section,
//! whichever is further, as the runtime finds its own image. Only squashfs 4.0 with gzip is read, which is what
//! `scripts/package.sh` packs; anything else is not the GPUI app.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Component, Path};

const MAGIC: u32 = 0x7371_7368;
const GZIP: u16 = 1;
const METADATA_MAX: usize = 8192;
/// A listing past this is not one `package.sh` writes.
const LISTING_MAX: usize = 1024 * 1024;

/// A file's kind and permission bits as its directory entry and inode give them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry {
    pub file: bool,
    pub mode: u16,
}

/// Where a folder's listing starts in the directory table, its offset into that block, and its size as stored.
type Listing = (u64, usize, usize);

pub struct Image {
    file: File,
    base: u64,
    root: u64,
    inodes: u64,
    dirs: u64,
}

fn u16_at(b: &[u8], at: usize) -> Result<u16, String> {
    b.get(at..at + 2).map(|s| u16::from_le_bytes([s[0], s[1]])).ok_or_else(|| "the image ends early".to_owned())
}

fn u32_at(b: &[u8], at: usize) -> Result<u32, String> {
    b.get(at..at + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]])).ok_or_else(|| "the image ends early".to_owned())
}

fn u64_at(b: &[u8], at: usize) -> Result<u64, String> {
    b.get(at..at + 8).map(|s| u64::from_le_bytes(s.try_into().expect("eight bytes"))).ok_or_else(|| "the image ends early".to_owned())
}

impl Image {
    pub fn open(path: &Path) -> Result<Image, String> {
        let mut file = File::open(path).map_err(|e| e.to_string())?;
        let mut elf = [0u8; 64];
        file.read_exact(&mut elf).map_err(|_| "not an ELF file")?;
        if elf[..4] != *b"\x7fELF" || elf[4] != 2 || elf[5] != 1 {
            return Err("not a 64-bit little-endian ELF file".into());
        }
        let (shoff, shentsize, shnum) = (u64_at(&elf, 0x28)?, u64::from(u16_at(&elf, 0x3a)?), u64::from(u16_at(&elf, 0x3c)?));
        let mut base = shentsize.checked_mul(shnum).and_then(|n| n.checked_add(shoff)).ok_or("broken section headers")?;
        if shnum > 0 {
            let mut last = [0u8; 0x28];
            file.seek(SeekFrom::Start(shoff + (shnum - 1) * shentsize)).map_err(|e| e.to_string())?;
            file.read_exact(&mut last).map_err(|_| "broken section headers")?;
            let end = u64_at(&last, 0x18)?.checked_add(u64_at(&last, 0x20)?).ok_or("broken section headers")?;
            base = base.max(end);
        }
        let mut sb = [0u8; 96];
        file.seek(SeekFrom::Start(base)).map_err(|e| e.to_string())?;
        file.read_exact(&mut sb).map_err(|_| "no squashfs after the runtime")?;
        if u32_at(&sb, 0)? != MAGIC {
            return Err("no squashfs after the runtime".into());
        }
        if (u16_at(&sb, 28)?, u16_at(&sb, 30)?) != (4, 0) {
            return Err("a squashfs other than 4.0".into());
        }
        if u16_at(&sb, 20)? != GZIP {
            return Err("a squashfs not compressed with gzip".into());
        }
        Ok(Image { file, base, root: u64_at(&sb, 32)?, inodes: u64_at(&sb, 64)?, dirs: u64_at(&sb, 72)? })
    }

    /// One metadata block at `at` within the image: its bytes, and where the next one starts.
    fn block(&mut self, at: u64) -> Result<(Vec<u8>, u64), String> {
        let mut head = [0u8; 2];
        self.file.seek(SeekFrom::Start(self.base + at)).map_err(|e| e.to_string())?;
        self.file.read_exact(&mut head).map_err(|_| "the image ends early")?;
        let head = u16::from_le_bytes(head);
        let size = usize::from(head & 0x7fff);
        if size == 0 || size > METADATA_MAX {
            return Err("a broken metadata block".into());
        }
        let mut raw = vec![0u8; size];
        self.file.read_exact(&mut raw).map_err(|_| "the image ends early")?;
        let data = if head & 0x8000 != 0 {
            raw
        } else {
            let mut out = Vec::with_capacity(METADATA_MAX);
            flate2::read::ZlibDecoder::new(&raw[..]).take(METADATA_MAX as u64 + 1).read_to_end(&mut out).map_err(|e| e.to_string())?;
            out
        };
        if data.len() > METADATA_MAX {
            return Err("a metadata block past its size".into());
        }
        Ok((data, at + 2 + size as u64))
    }

    /// `len` bytes of the metadata stream of the table at `table`, from `offset` into its block at `block`.
    fn meta(&mut self, table: u64, block: u64, offset: usize, len: usize) -> Result<Vec<u8>, String> {
        let mut at = table.checked_add(block).ok_or("a broken reference")?;
        let mut bytes = Vec::new();
        while bytes.len() < offset + len {
            let (data, next) = self.block(at)?;
            bytes.extend_from_slice(&data);
            at = next;
        }
        Ok(bytes[offset..offset + len].to_vec())
    }

    /// The inode a reference names: its type, its permission bits and, for a folder, where its listing is and how long.
    fn inode(&mut self, reference: u64) -> Result<(u16, u16, Option<Listing>), String> {
        let (block, offset) = (reference >> 16, (reference & 0xffff) as usize);
        let head = self.meta(self.inodes, block, offset, 16)?;
        let (kind, mode) = (u16_at(&head, 0)?, u16_at(&head, 2)?);
        let listing = match kind {
            1 => {
                let d = self.meta(self.inodes, block, offset, 32)?;
                Some((u64::from(u32_at(&d, 16)?), usize::from(u16_at(&d, 26)?), usize::from(u16_at(&d, 24)?)))
            }
            8 => {
                let d = self.meta(self.inodes, block, offset, 40)?;
                Some((u64::from(u32_at(&d, 24)?), usize::from(u16_at(&d, 34)?), u32_at(&d, 20)? as usize))
            }
            _ => None,
        };
        Ok((kind, mode, listing))
    }

    /// The reference of `name` in the folder whose listing is at `listing`, or nothing where it holds no such name.
    fn find(&mut self, (block, offset, size): Listing, name: &str) -> Result<Option<u64>, String> {
        // A listing's size counts three bytes more than it holds.
        let len = size.saturating_sub(3);
        if len > LISTING_MAX {
            return Err("a listing past any this app packs".into());
        }
        let listing = self.meta(self.dirs, block, offset, len)?;
        let mut at = 0;
        while at + 12 <= listing.len() {
            let count = u32_at(&listing, at)? as usize + 1;
            let start = u64::from(u32_at(&listing, at + 4)?);
            at += 12;
            for _ in 0..count {
                let inode_offset = u64::from(u16_at(&listing, at)?);
                let name_len = usize::from(u16_at(&listing, at + 6)?) + 1;
                let here = listing.get(at + 8..at + 8 + name_len).ok_or("the image ends early")?;
                at += 8 + name_len;
                if here == name.as_bytes() {
                    return Ok(Some((start << 16) | inode_offset));
                }
            }
        }
        Ok(None)
    }

    /// What stands at `path` inside the image, links not followed, or nothing where nothing does.
    pub fn entry(&mut self, path: &Path) -> Result<Option<Entry>, String> {
        let mut reference = self.root;
        for part in path.components() {
            let Component::Normal(name) = part else { return Err(format!("{} is not a path inside the image", path.display())) };
            let (_, _, listing) = self.inode(reference)?;
            let Some(listing) = listing else { return Ok(None) };
            match self.find(listing, &name.to_string_lossy())? {
                Some(next) => reference = next,
                None => return Ok(None),
            }
        }
        let (kind, mode, _) = self.inode(reference)?;
        Ok(Some(Entry { file: matches!(kind, 2 | 9), mode }))
    }
}
