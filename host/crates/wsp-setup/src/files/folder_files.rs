// SPDX-License-Identifier: AGPL-3.0-only
//! `folder-files.ts`: the one rule for the files under a folder a recipe ships or digests, where a link is followed only to a file.
use std::rc::Rc;

/// `FolderFiles` (`folder-files.ts:10`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FolderFiles {
    pub files: Vec<FolderFile>,
    pub links: Vec<String>,
}

/// `FolderFiles.files` (`folder-files.ts:11`), one file folder-relative with the path it is read from.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FolderFile {
    pub rel: String,
    pub path: String,
}

/// `folderFiles` (`folder-files.ts:16`).
pub fn folder_files(_dir: &str, _leave: Rc<dyn Fn(&str) -> bool>) -> FolderFiles {
    todo!()
}

/// `LINKED_FOLDER_NOTE` (`folder-files.ts:37`).
pub const LINKED_FOLDER_NOTE: &str = "links to a folder, which a recipe never sends";
