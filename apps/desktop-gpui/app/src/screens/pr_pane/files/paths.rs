// SPDX-License-Identifier: AGPL-3.0-only
//! Paths as the daemon takes them (`files/entries.ts`, `files/root.ts`): absolute, inside one of the roots it browses,
//! the daemon's home and the workspace's project folder.

/// `joinPath`.
pub fn join(dir: &str, name: &str) -> String {
    if dir.ends_with('/') {
        format!("{dir}{name}")
    } else {
        format!("{dir}/{name}")
    }
}

/// `parentPath`: None at the filesystem's root.
pub fn parent(path: &str) -> Option<String> {
    if path == "/" {
        return None;
    }
    match path.rfind('/') {
        Some(cut) if cut > 0 => Some(path[..cut].to_owned()),
        _ => Some("/".to_owned()),
    }
}

/// `relativeTo`: `path` below `root` as the tree names it, "" for the root itself.
pub fn relative<'a>(root: &str, path: &'a str) -> &'a str {
    if path == root {
        return "";
    }
    let prefix = if root.ends_with('/') { root.to_owned() } else { format!("{root}/") };
    path.strip_prefix(prefix.as_str()).unwrap_or(path)
}

/// `baseName`.
pub fn base_name(path: &str) -> &str {
    let bare = path.trim_end_matches('/');
    &bare[bare.rfind('/').map_or(0, |cut| cut + 1)..]
}

/// `isMarkdownFile`.
pub fn is_markdown(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".md") || lower.ends_with(".mdx")
}

fn within(path: &str, folder: &str) -> bool {
    path == folder || path.starts_with(&if folder == "/" { "/".to_owned() } else { format!("{folder}/") })
}

/// `rootsOf`: the daemon's home first, then the project folder, each once; none before the daemon's hello.
pub fn roots(home: Option<&str>, project: Option<&str>) -> Vec<String> {
    let Some(home) = home else { return Vec::new() };
    let mut roots = vec![home.to_owned()];
    if let Some(project) = project.filter(|p| *p != home) {
        roots.push(project.to_owned());
    }
    roots
}

/// `rootOf`: the root a path sits in, the nearest where roots nest.
pub fn root_of<'a>(roots: &'a [String], path: &str) -> Option<&'a str> {
    roots.iter().filter(|root| within(path, root)).max_by_key(|root| root.len()).map(String::as_str)
}

/// `parentWithin`: the folder above while a root still holds it; None at a root's edge, where the daemon stops.
pub fn parent_within(roots: &[String], path: &str) -> Option<String> {
    parent(path).filter(|above| root_of(roots, above).is_some())
}

/// One crumb: the name it shows and the folder it goes to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Crumb {
    pub name: String,
    pub path: String,
}

/// `folderCrumbs`: the root first, named by its whole path, then one crumb per folder below it, the last being the
/// folder itself. A folder no root holds is its own single crumb.
pub fn crumbs(roots: &[String], folder: &str) -> Vec<Crumb> {
    let root = root_of(roots, folder).unwrap_or(folder).to_owned();
    let mut out = vec![Crumb { name: root.clone(), path: root.clone() }];
    let mut at = root.clone();
    for part in relative(&root, folder).split('/').filter(|p| !p.is_empty()) {
        at = join(&at, part);
        out.push(Crumb { name: part.to_owned(), path: at.clone() });
    }
    out
}

/// What fs.read answered: a NUL in the text is a binary file.
pub fn is_binary(content: &str) -> bool {
    content.contains('\0')
}
