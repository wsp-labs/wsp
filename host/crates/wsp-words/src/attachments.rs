// SPDX-License-Identifier: AGPL-3.0-only
//! Images a message carries: their types and sizes, the prompt that names the files, and the refusals of one too big. Port of the functions of packages/protocol/src/attachments.ts at
//! a3b870a6e; its shapes and flat constants are wsp_wire::types::attachments. See spec/attachments.md.

use indexmap::IndexSet;

use wsp_wire::types::attachments::Attachment;
use wsp_wire::types::attachments::AttachmentRecord;
use wsp_wire::types::attachments::{FILES_DIR, FILES_MAX, FILE_MAX_BYTES, FILE_MAX_WORDS, IMAGE_MAX_BYTES, IMAGE_MAX_WORDS};
use wsp_wire::types::js_number::{JsInt, JsNumber};
use wsp_wire::types::shell_quote::shell_quote;

use crate::exit::HostError;
use crate::words::units::fmt_bytes;

/// The image types a message carries, each with the word a transcript prints and the extension its copy on a machine
/// lands under. A type outside this table is refused before it travels: the harnesses take these four.
// attachments.ts:18
// export const IMAGE_TYPES: Readonly<Record<string, { word: string; ext: string }>> = {
//   "image/png": { word: "png", ext: "png" },
//   "image/jpeg": { word: "jpeg", ext: "jpg" },
//   "image/gif": { word: "gif", ext: "gif" },
//   "image/webp": { word: "webp", ext: "webp" },
// };
/// The TS IMAGE_TYPES: (media type, word, extension), in its order.
pub const IMAGE_TYPES: &[(&str, &str, &str)] = &[
    ("image/png", "png", "png"),
    ("image/jpeg", "jpeg", "jpg"),
    ("image/gif", "gif", "gif"),
    ("image/webp", "webp", "webp"),
];

// What one image may weigh. A harness reads an image whole into its request, and the bytes travel base64 on our own
// wire, so the cap bounds both.
// attachments.ts:27
// in wsp_wire::types::attachments: IMAGE_MAX_BYTES

// What one file that is not an image may weigh. Five of them travel base64 in one message on the host's socket,
// which takes 100 MiB a message, so the cap is the image's.
// attachments.ts:31
// in wsp_wire::types::attachments: FILE_MAX_BYTES

// attachments.ts:34
// in wsp_wire::types::attachments: FILES_MAX

// One file as the wire carries it: its type and the bytes themselves, base64. The name is what the person's file or
// paste was called: the transcript's words for an image, and the name a file lands under, made plain first.
// attachments.ts:38
// in wsp_wire::types::attachments: Attachment

// attachments.ts:43
// in wsp_wire::types::attachments: Attachment

// attachments.ts:46
// in wsp_wire::types::attachments: UNTYPED_FILE

/// Whether this type travels as an image, on its harness's own road, rather than landing as a file.
// attachments.ts:49
// export const isImage = (mediaType: string): boolean => IMAGE_TYPES[mediaType] !== undefined;
pub fn is_image(media_type: &str) -> bool {
    IMAGE_TYPES.iter().any(|(media, _, _)| *media == media_type)
}

/// What an attachment weighs, from the base64 the wire carries, without decoding it.
// attachments.ts:52
// export function attachmentBytes(image: { bytes: string }): number {
//   const b64 = image.bytes;
//   const pad = b64.endsWith("==") ? 2 : b64.endsWith("=") ? 1 : 0;
//   return Math.max(0, Math.floor(b64.length / 4) * 3 - pad);
// }
/// `bytes` is the image's base64 text.
pub fn attachment_bytes(bytes: &str) -> f64 {
    let len = bytes.encode_utf16().count() as f64;
    let pad = if bytes.ends_with("==") {
        2.0
    } else if bytes.ends_with('=') {
        1.0
    } else {
        0.0
    };
    ((len / 4.0).floor() * 3.0 - pad).max(0.0)
}

// One file of a person's message as a transcript keeps it: what it was, what it weighed and what it was called.
// The bytes are not here, so a transcript costs the same however large the file was; the host keeps an image's bytes
// beside it, under attachmentKey, until its thread goes, and a file's go to the machine alone.
// attachments.ts:61
// in wsp_wire::types::attachments: AttachmentRecord

// attachments.ts:66
// in wsp_wire::types::attachments: AttachmentRecord

/// What a transcript keeps of one file the wire carried.
// attachments.ts:69
// export function attachmentRecord(file: Attachment): AttachmentRecord {
//   return { mediaType: file.mediaType, bytes: attachmentBytes(file), ...(file.name !== undefined ? { name: file.name } : {}) };
// }
pub fn attachment_record(file: &Attachment) -> AttachmentRecord {
    AttachmentRecord {
        media_type: file.media_type.clone(),
        bytes: JsInt(JsNumber(attachment_bytes(&file.bytes))),
        name: file.name.clone(),
    }
}

// attachments.ts:75
// in wsp_wire::types::attachments: IMAGE_TYPE_WORDS

// The size cap as a person reads it. Every sentence that names the cap reads this rather than spelling a number
// beside it, so the words a person is told and the rule the code enforces cannot drift apart.
// attachments.ts:79
// in wsp_wire::types::attachments: IMAGE_MAX_WORDS

// attachments.ts:80
// in wsp_wire::types::attachments: FILE_MAX_WORDS

// The first bytes each of the four types starts with; WebP's is a RIFF header whose length comes before the word
// that names the format, so its check is the head and a second run eight bytes in.
// attachments.ts:84
// private in the TS: port it as a private helper
// const MAGIC: readonly { mediaType: string; head: readonly number[]; at8?: readonly number[] }[] = [
//   { mediaType: "image/png", head: [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a] },
//   { mediaType: "image/jpeg", head: [0xff, 0xd8, 0xff] },
//   { mediaType: "image/gif", head: [0x47, 0x49, 0x46, 0x38] },
//   { mediaType: "image/webp", head: [0x52, 0x49, 0x46, 0x46], at8: [0x57, 0x45, 0x42, 0x50] },
// ];

/// What image these bytes are, read off the bytes themselves and never off a file's name, which lies; null when they
/// are none of the four. The command line and the MCP server read a person's file with this, so a screenshot saved
/// as .txt still travels as an image and a .png that is a PDF travels as a file.
// attachments.ts:94
// export function imageTypeOf(bytes: Uint8Array): string | null {
//   const starts = (at: number, want: readonly number[]): boolean => want.every((byte, i) => bytes[at + i] === byte);
//   return MAGIC.find(m => starts(0, m.head) && (m.at8 === undefined || starts(8, m.at8)))?.mediaType ?? null;
// }
pub fn image_type_of(bytes: &[u8]) -> Option<String> {
    let starts = |at: usize, want: &[u8]| want.iter().enumerate().all(|(i, byte)| bytes.get(at + i) == Some(byte));
    let magic: [(&str, &[u8], Option<&[u8]>); 4] = [
        ("image/png", &[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a], None),
        ("image/jpeg", &[0xff, 0xd8, 0xff], None),
        ("image/gif", &[0x47, 0x49, 0x46, 0x38], None),
        ("image/webp", &[0x52, 0x49, 0x46, 0x46], Some(&[0x57, 0x45, 0x42, 0x50])),
    ];
    magic
        .iter()
        .find(|(_, head, at8)| starts(0, head) && (*at8).map_or(true, |at8| starts(8, at8)))
        .map(|(media, _, _)| media.to_string())
}

/// The refusal of a path the person named that this computer has no file at, or has something other than a file at:
/// the road answers in a sentence, as every other refusal on it does, rather than in the reader's own error.
// attachments.ts:101
// export function notAFileLine(path: string): string {
//   return `there is no file at ${path} on this computer`;
// }
pub fn not_a_file_line(path: &str) -> String {
    format!("there is no file at {path} on this computer")
}

/// What a transcript prints in place of an attachment, in one bracket: an image's weight and type, a file's weight
/// and name. The command line prints this on the person's turn, and the app wherever it has nothing of its own to
/// draw.
// attachments.ts:108
// export function attachmentLine(file: AttachmentRecord): string {
//   const type = IMAGE_TYPES[file.mediaType];
//   if (type !== undefined) return `[image ${fmtBytes(file.bytes)} ${type.word}]`;
//   return `[file ${fmtBytes(file.bytes)}${file.name !== undefined ? ` ${file.name}` : ""}]`;
// }
pub fn attachment_line(file: &AttachmentRecord) -> String {
    let bytes = fmt_bytes(file.bytes.0 .0);
    match IMAGE_TYPES.iter().find(|(media, _, _)| *media == file.media_type) {
        Some((_, word, _)) => format!("[image {bytes} {word}]"),
        None => format!("[file {bytes}{}]", file.name.as_ref().map(|name| format!(" {name}")).unwrap_or_default()),
    }
}

/// Why these files cannot travel, in the person's words, or null when they can: too many for one message, one over
/// its cap, or an empty one. The first thing wrong is the whole answer, since a person fixes one file at a time. Takes
/// the records, so a road that has the file's size and not its bytes yet (the command line reading a path) asks the
/// same question as one holding the bytes.
// attachments.ts:118
// export function filesRefusal(files: readonly AttachmentRecord[]): string | null {
//   if (files.length > FILES_MAX) return `only ${FILES_MAX} files fit one message; this one carries ${files.length}`;
//   for (const [index, file] of files.entries()) {
//     const image = isImage(file.mediaType);
//     const at = file.name ?? `${image ? "image" : "file"} ${index + 1}`;
//     if (file.bytes === 0) return `${at} is empty`;
//     if (image && file.bytes > IMAGE_MAX_BYTES) return `${at} is ${fmtBytes(file.bytes)}, over the ${IMAGE_MAX_WORDS} an image may be`;
//     if (!image && file.bytes > FILE_MAX_BYTES) return `${at} is ${fmtBytes(file.bytes)}, over the ${FILE_MAX_WORDS} a file may be`;
//   }
//   return null;
// }
pub fn files_refusal(files: &[AttachmentRecord]) -> Option<String> {
    if files.len() as u64 > FILES_MAX {
        return Some(format!("only {FILES_MAX} files fit one message; this one carries {}", files.len()));
    }
    for (index, file) in files.iter().enumerate() {
        let image = is_image(&file.media_type);
        let kind = if image { "image" } else { "file" };
        let at = file.name.clone().unwrap_or_else(|| format!("{kind} {}", index + 1));
        let bytes = file.bytes.0 .0;
        if bytes == 0.0 {
            return Some(format!("{at} is empty"));
        }
        if image && bytes > IMAGE_MAX_BYTES as f64 {
            return Some(format!("{at} is {}, over the {IMAGE_MAX_WORDS} an image may be", fmt_bytes(bytes)));
        }
        if !image && bytes > FILE_MAX_BYTES as f64 {
            return Some(format!("{at} is {}, over the {FILE_MAX_WORDS} a file may be", fmt_bytes(bytes)));
        }
    }
    None
}

/// Why these files cannot go into a running turn of this agent, or null when they can: an image goes where the
/// agent's steer reads one, and any other file lands in the thread's folder, which only a message that starts a turn
/// does. A message that cannot go waits for the turn to end.
// attachments.ts:133
// export function steerFilesBlocked(files: readonly AttachmentRecord[], steersImages: boolean, agent: string): string | null {
//   if (files.length === 0) return null;
//   if (files.some(f => !isImage(f.mediaType))) return "a file that is not an image goes only with a message that starts a turn";
//   return steersImages ? null : `${agent} takes no image into a running turn`;
// }
pub fn steer_files_blocked(files: &[AttachmentRecord], steers_images: bool, agent: &str) -> Option<String> {
    if files.is_empty() {
        return None;
    }
    if files.iter().any(|file| !is_image(&file.media_type)) {
        return Some("a file that is not an image goes only with a message that starts a turn".to_string());
    }
    if steers_images {
        None
    } else {
        Some(format!("{agent} takes no image into a running turn"))
    }
}

/// The refusal of a message with an image to an agent that takes none, said before the machine is asked.
// attachments.ts:140
// export function noImagesLine(harness: string): string {
//   return `${harness} takes no image with a message; describe it in words, or open the thread on an agent that reads images`;
// }
pub fn no_images_line(harness: &str) -> String {
    format!("{harness} takes no image with a message; describe it in words, or open the thread on an agent that reads images")
}

/// Why these files cannot go to this agent, or null when they can: the caps first, since a person fixes those
/// whatever the agent is, then the agent itself, whose adapter may read no image at all; a file that is not an image
/// lands in the thread's folder, which every agent reads. Every road that carries a file asks this before a machine is
/// asked for anything: the composer before the send, the command line and the MCP server before the request, and the
/// runtime again before the turn.
// attachments.ts:149
// export function filesBlocked(files: readonly AttachmentRecord[], road: AttachmentRoad | undefined, harness: string): string | null {
//   if (files.length === 0) return null;
//   return filesRefusal(files) ?? (road === undefined && files.some(f => isImage(f.mediaType)) ? noImagesLine(harness) : null);
// }
/// How an adapter takes an image with a turn (adapter-port.ts:248). Declared here, where the words that read it
/// live; wsp-adapters uses this one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AttachmentRoad {
    Inline,
    File,
}

pub fn files_blocked(files: &[AttachmentRecord], road: Option<&AttachmentRoad>, harness: &str) -> Option<String> {
    if files.is_empty() {
        return None;
    }
    files_refusal(files).or_else(|| {
        let image = files.iter().any(|file| is_image(&file.media_type));
        (road.is_none() && image).then(|| no_images_line(harness))
    })
}

// A folder name that is one path segment and nothing else. A request id is a string a client chose, and it travels
// into a path on a machine, so one shaped like anything else is not used.
// attachments.ts:156
// private in the TS: port it as a private helper
// const PLAIN_ID = /^[A-Za-z0-9][A-Za-z0-9._-]*$/;

/// What the host keeps one image of a message under, so any client draws it again after a restart, whoever sent it:
/// the thread, the request id the transcript's start carries and the image's place in the message. None where the
/// request id or the thread id is not a plain id, since no client could name that send by a path-safe key.
// attachments.ts:161
// export function attachmentKey(threadId: string, requestId: string | undefined, index: number): string | undefined {
//   return requestId !== undefined && PLAIN_ID.test(requestId) && PLAIN_ID.test(threadId) ? `${threadId}.${requestId}.${index}` : undefined;
// }
pub fn attachment_key(thread_id: &str, request_id: Option<&str>, index: f64) -> Option<String> {
    let request_id = request_id.filter(|id| plain_id(id))?;
    if !plain_id(thread_id) {
        return None;
    }
    Some(format!("{thread_id}.{request_id}.{}", crate::js::number_string(index)))
}

// One image of a message as the host kept it: its type and the bytes, base64, as the wire carried them.
// attachments.ts:166
// in wsp_wire::types::attachments: KeptAttachment

// attachments.ts:167
// in wsp_wire::types::attachments: KeptAttachment

/// Where one send's images land for an agent that reads them as files: a folder of their own beside the send's
/// files in the folder the thread works in, which every kind of machine has, where a computer the person owns has no
/// /root to land under. Named by the request id the client minted, so two sends on one thread never write to the
/// same path and the first turn cannot be handed the second's picture, and the turn's end removes it leaving nothing
/// empty behind. `minted` stands in when the send carried no request id, and when the one it carried is not a plain
/// id, since that string would otherwise be a path of the client's choosing.
// attachments.ts:175
// export function turnImagesDir(folder: string, threadId: string, requestId: string | undefined, minted: string): string {
//   return `${threadFilesDir(folder, threadId)}/${plainOr(requestId, minted)}.images`;
// }
pub fn turn_images_dir(folder: &str, thread_id: &str, request_id: Option<&str>, minted: &str) -> String {
    format!("{}/{}.images", thread_files_dir(folder, thread_id), plain_or(request_id, minted))
}

/// Where one image of a message lands inside its send's folder: named by its place in the message and its own type,
/// so the folder read in order is the message read in order. A type outside the table has no name here, since it
/// lands as a file, and this throws for a caller that never asked.
// attachments.ts:182
// export function imagePathIn(dir: string, index: number, mediaType: string): string {
//   const type = IMAGE_TYPES[mediaType];
//   if (type === undefined) throw new Error(`${mediaType} is not an image type a message carries`);
//   return `${dir}/${index + 1}.${type.ext}`;
// }
/// The TS throw is the Err.
pub fn image_path_in(dir: &str, index: f64, media_type: &str) -> Result<String, crate::exit::HostError> {
    let Some((_, _, ext)) = IMAGE_TYPES.iter().find(|(media, _, _)| *media == media_type) else {
        return Err(HostError::new(format!("{media_type} is not an image type a message carries")));
    };
    Ok(format!("{dir}/{}.{ext}", crate::js::number_string(index + 1.0)))
}

// attachments.ts:190
// in wsp_wire::types::attachments: FILES_DIR

// The line the copy's exclude file carries for FILES_DIR, matching it in any folder of the checkout.
// attachments.ts:193
// private in the TS: port it as a private helper
// const EXCLUDE_LINE = `${FILES_DIR}/`;

// attachments.ts:195
// private in the TS: port it as a private helper
// const plainOr = (id: string | undefined, fallback: string): string => (id !== undefined && PLAIN_ID.test(id) ? id : fallback);

/// One thread's files under a folder: removing it removes every send of that thread and nothing of another.
// attachments.ts:198
// export function threadFilesDir(folder: string, threadId: string): string {
//   return `${folder.replace(/\/+$/, "")}/${FILES_DIR}/${plainOr(threadId, "thread")}`;
// }
pub fn thread_files_dir(folder: &str, thread_id: &str) -> String {
    format!("{}/{FILES_DIR}/{}", folder.trim_end_matches('/'), plain_or(Some(thread_id), "thread"))
}

/// Where one send's files land: its own folder under its thread's, named by the request id the client minted where
/// that is a plain id, for the reason turnImagesDir gives.
// attachments.ts:204
// export function sendFilesDir(folder: string, threadId: string, requestId: string | undefined, minted: string): string {
//   return `${threadFilesDir(folder, threadId)}/${plainOr(requestId, minted)}`;
// }
pub fn send_files_dir(folder: &str, thread_id: &str, request_id: Option<&str>, minted: &str) -> String {
    format!("{}/{}", thread_files_dir(folder, thread_id), plain_or(request_id, minted))
}

/// The longest name a file lands under, its extension kept.
// attachments.ts:209
const FILE_NAME_MAX: i64 = 120;

/// A file's name as one plain path segment: the last segment of what the client sent, every character outside a
/// plain set made a dash and no leading dot, so it can name nothing outside its send's folder and no hidden file.
// attachments.ts:213
// export function safeFileName(name: string | undefined): string {
//   const last = (name ?? "").split(/[\\/]/).at(-1) ?? "";
//   const plain = last.replace(/[^A-Za-z0-9._-]/g, "-").replace(/^[.-]+/, "");
//   if (plain.replace(/[.-]/g, "") === "") return "file";
//   if (plain.length <= FILE_NAME_MAX) return plain;
//   const dot = plain.lastIndexOf(".");
//   const ext = dot > 0 && plain.length - dot <= 10 ? plain.slice(dot) : "";
//   return plain.slice(0, FILE_NAME_MAX - ext.length) + ext;
// }
pub fn safe_file_name(name: Option<&str>) -> String {
    let last = name.unwrap_or("").rsplit(['/', '\\']).next().unwrap_or("");
    let mapped: String = last
        .chars()
        .flat_map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                vec![c]
            } else {
                vec!['-'; c.len_utf16()]
            }
        })
        .collect();
    let plain = mapped.trim_start_matches(['.', '-']);
    if plain.chars().all(|c| c == '.' || c == '-') {
        return "file".to_string();
    }
    if plain.len() <= FILE_NAME_MAX as usize {
        return plain.to_string();
    }
    let ext = match plain.rfind('.') {
        Some(dot) if dot > 0 && plain.len() - dot <= 10 => &plain[dot..],
        _ => "",
    };
    format!("{}{ext}", &plain[..FILE_NAME_MAX as usize - ext.len()])
}

/// Where one file lands in its send's folder: under its own plain name, and a second file of the same name in that
/// send under that name led by its place, so neither overwrites the other. `taken` is the send's names so far.
// attachments.ts:225
// export function filePathIn(dir: string, name: string | undefined, taken: Set<string>): string {
//   const plain = safeFileName(name);
//   let landed = plain;
//   for (let n = 2; taken.has(landed); n++) landed = `${n}-${plain}`;
//   taken.add(landed);
//   return `${dir}/${landed}`;
// }
pub fn file_path_in(dir: &str, name: Option<&str>, taken: &mut IndexSet<String>) -> String {
    let plain = safe_file_name(name);
    let mut landed = plain.clone();
    let mut n = 2;
    while taken.contains(&landed) {
        landed = format!("{n}-{plain}");
        n += 1;
    }
    taken.insert(landed.clone());
    format!("{dir}/{landed}")
}

/// The one command that readies a send's folder before its files land. FILES_DIR and the thread's folder are
/// refused where either is a link, since the bytes would follow it out of the copy. In a checkout the copy's own
/// exclude file gains FILES_DIR once, so git's status stays clean without touching a file the person commits; an
/// exclude file that is a link is left alone, and a folder that is no checkout has no status to keep. The send's own
/// folder is made fresh, never one already there.
// attachments.ts:238
// export function landFilesLine(folder: string, dir: string): string {
//   const thread = shellQuote(dir.slice(0, dir.lastIndexOf("/")));
//   const exclude = `x=$(git -c core.fsmonitor=false rev-parse --git-path info/exclude 2>/dev/null) && [ ! -L "$x" ] && { grep -qxF '${EXCLUDE_LINE}' "$x" 2>/dev/null || { mkdir -p "$(dirname "$x")" && printf '%s\\n' '${EXCLUDE_LINE}' >> "$x"; }; }`;
//   return `cd ${shellQuote(folder)} && [ ! -L ${FILES_DIR} ] && mkdir -p ${FILES_DIR} && { ${exclude} || true; } && [ ! -L ${thread} ] && mkdir -p ${thread} && mkdir ${shellQuote(dir)}`;
// }
pub fn land_files_line(folder: &str, dir: &str) -> String {
    let thread = match dir.rfind('/') {
        Some(slash) => dir[..slash].to_string(),
        None => {
            let mut cut = dir.to_string();
            cut.pop();
            cut
        }
    };
    let thread = shell_quote(&thread);
    let exclude = format!(
        r#"x=$(git -c core.fsmonitor=false rev-parse --git-path info/exclude 2>/dev/null) && [ ! -L "$x" ] && {{ grep -qxF '{EXCLUDE_LINE}' "$x" 2>/dev/null || {{ mkdir -p "$(dirname "$x")" && printf '%s\n' '{EXCLUDE_LINE}' >> "$x"; }}; }}"#
    );
    format!(
        "cd {} && [ ! -L {FILES_DIR} ] && mkdir -p {FILES_DIR} && {{ {exclude} || true; }} && [ ! -L {thread} ] && mkdir -p {thread} && mkdir {}",
        shell_quote(folder),
        shell_quote(dir)
    )
}

/// The one command that takes these threads' files off a folder: each thread's own folder under FILES_DIR, and
/// nothing where FILES_DIR is a link, since the removal would follow it out of the copy. An id that is not a plain
/// one names no folder.
// attachments.ts:247
// export function dropFilesLine(folder: string, threadIds: readonly string[]): string {
//   const dirs = threadIds.filter(id => PLAIN_ID.test(id)).map(id => shellQuote(`${FILES_DIR}/${id}`));
//   return dirs.length === 0 ? "true" : `cd ${shellQuote(folder)} && [ ! -L ${FILES_DIR} ] && rm -rf -- ${dirs.join(" ")}`;
// }
pub fn drop_files_line(folder: &str, thread_ids: &[impl AsRef<str>]) -> String {
    let dirs: Vec<String> = thread_ids
        .iter()
        .map(|id| id.as_ref())
        .filter(|id| plain_id(id))
        .map(|id| shell_quote(&format!("{FILES_DIR}/{id}")))
        .collect();
    if dirs.is_empty() {
        return "true".to_string();
    }
    format!("cd {} && [ ! -L {FILES_DIR} ] && rm -rf -- {}", shell_quote(folder), dirs.join(" "))
}

/// The refusal of a message whose files could not land in the thread's folder: its FILES_DIR is a link, or the
/// folder is not one this machine can write in.
// attachments.ts:254
// export function filesNotLandedLine(folder: string): string {
//   return `the attached files could not land in ${folder}/${FILES_DIR}; make sure it is a folder the agent can write in, not a link, and send again`;
// }
pub fn files_not_landed_line(folder: &str) -> String {
    format!("the attached files could not land in {folder}/{FILES_DIR}; make sure it is a folder the agent can write in, not a link, and send again")
}

/// The prompt the agent is handed for a message with files: the person's words, then every landed path.
// attachments.ts:259
// export function attachedFilesPrompt(prompt: string, paths: readonly string[]): string {
//   return paths.length === 0 ? prompt : `${prompt}\n\n${ATTACHED_FILES_HEAD}\n${paths.map(p => `- ${p}`).join("\n")}`;
// }
pub fn attached_files_prompt(prompt: &str, paths: &[impl AsRef<str>]) -> String {
    if paths.is_empty() {
        return prompt.to_string();
    }
    let lines: Vec<String> = paths.iter().map(|path| format!("- {}", path.as_ref())).collect();
    format!("{prompt}\n\nAttached files:\n{}", lines.join("\n"))
}

// The plain-id rule (attachments.ts:156): one ASCII letter or digit, then letters, digits, dots, underscores and dashes.
fn plain_id(id: &str) -> bool {
    let mut chars = id.chars();
    matches!(chars.next(), Some(first) if first.is_ascii_alphanumeric())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

fn plain_or(id: Option<&str>, fallback: &str) -> String {
    match id {
        Some(id) if plain_id(id) => id.to_string(),
        _ => fallback.to_string(),
    }
}

const EXCLUDE_LINE: &str = ".wsp-files/";
