// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/test/slate-images.test.ts. Spec: spec/slate_images.md.
//!
//! An image read for the window: from this computer's disk, from another computer's daemon, and from a remote
//! address the host fetches only after its domain is allowed. Answers are read through serde_json alone.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::future::Future;
use std::path::Path;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine;
use futures::future::LocalBoxFuture;
use futures::FutureExt;
use regex::Regex;
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use tokio::task::LocalSet;
use wsp_slates::pending::wire::SlatesImageAnswer;
use wsp_slates::slate_image_fetch::{fetch_slate_image, local_address, FetchRoad, LookedUp};
use wsp_slates::slate_images::{slate_image, FsImageReply, ImageOn, ImageRefusal, SlateImageRoads};

const PNG_HEX: &str = "89504e470d0a1a0a0000000d49484452000000010000000108060000001f15c4890000000d4944415478da63f8cfc0f01f0005000201a3b0d8f60000000049454e44ae426082";
const CAP: usize = 10 * 1024 * 1024;
const SVG: &str = "<?xml version=\"1.0\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\"><image href=\"https://acme.test/x.png\"/></svg>";
const NOT_AN_IMAGE_TAIL: &str = "is not a PNG, JPEG, GIF or WebP image";

fn png() -> Vec<u8> {
    hex_bytes(PNG_HEX)
}

fn hex_bytes(h: &str) -> Vec<u8> {
    (0..h.len()).step_by(2).map(|i| u8::from_str_radix(&h[i..i + 2], 16).unwrap()).collect()
}

fn jpeg() -> Vec<u8> {
    vec![0xff, 0xd8, 0xff, 0xe0, 0, 16, 0x4a, 0x46, 0x49, 0x46]
}

fn gif() -> Vec<u8> {
    let mut v = b"GIF89a".to_vec();
    v.extend_from_slice(&[1, 0, 1, 0, 0, 0, 0]);
    v.push(b';');
    v
}

fn webp() -> Vec<u8> {
    let mut v = b"RIFF".to_vec();
    v.extend_from_slice(&[26, 0, 0, 0]);
    v.extend_from_slice(b"WEBPVP8 ");
    v
}

/// The PNG followed by zero bytes up to `len` bytes in all.
fn padded(len: usize) -> Vec<u8> {
    let mut v = png();
    v.resize(len, 0);
    v
}

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn said(answer: &SlatesImageAnswer) -> String {
    let v = serde_json::to_value(answer).unwrap();
    if let Some(p) = v.get("problem") {
        return format!("{} {}", p["code"].as_str().unwrap(), p["message"].as_str().unwrap());
    }
    if let Some(a) = v.get("ask") {
        return format!("ask {}", a["domain"].as_str().unwrap());
    }
    if v.get("unchanged").is_some() {
        return format!("unchanged {}", v["version"].as_str().unwrap());
    }
    format!("shown {}", v["mediaType"].as_str().unwrap())
}

fn json_of(answer: &SlatesImageAnswer) -> Value {
    serde_json::to_value(answer).unwrap()
}

async fn local<F: Future>(f: F) -> F::Output {
    LocalSet::new().run_until(f).await
}

fn tmp() -> tempfile::TempDir {
    // /tmp itself, not a per-user temp folder: a screenshot an agent saves to /tmp shows.
    tempfile::Builder::new().prefix("wsp-slate-images-").tempdir_in("/tmp").unwrap()
}

fn put(dir: &Path, name: &str, bytes: &[u8]) -> String {
    let path = dir.join(name);
    std::fs::write(&path, bytes).unwrap();
    path.to_str().unwrap().to_string()
}

async fn here(src: &str, folder: Option<&str>, have: Option<&str>) -> SlatesImageAnswer {
    slate_image(
        src,
        SlateImageRoads {
            thread: "t-here".into(),
            have: have.map(String::from),
            folder: folder.map(String::from),
            on: None,
            allowed: Rc::new(|_| false),
            fetch: None,
        },
    )
    .await
}

// TS slate-images.test.ts:49
#[tokio::test(flavor = "current_thread")]
async fn shows_a_real_image_of_each_type_by_whole_path_and_relative_under_the_folder() {
    local(async {
        let dir = tmp();
        let d = dir.path();
        let home = put(d, "home.png", &png());
        let photo = put(d, "photo.jpg", &jpeg());
        let spin = put(d, "spin.gif", &gif());
        let card = put(d, "card.webp", &webp());
        let edge = put(d, "edge.png", &padded(CAP));
        std::fs::create_dir(d.join("shots")).unwrap();
        put(&d.join("shots"), "a.png", &png());
        let folder = d.to_str().unwrap();

        let first = json_of(&here(&home, None, None).await);
        assert_eq!(first["mediaType"], "image/png");
        assert_eq!(first["bytes"], b64(&png()));
        assert!(Regex::new(r"^\d+:70:\d+:\d+$").unwrap().is_match(first["version"].as_str().unwrap()), "{first}");
        assert_eq!(said(&here(&photo, None, None).await), "shown image/jpeg");
        assert_eq!(said(&here(&spin, None, None).await), "shown image/gif");
        assert_eq!(said(&here(&card, None, None).await), "shown image/webp");
        assert_eq!(said(&here("shots/a.png", Some(folder), None).await), "shown image/png");
        assert_eq!(said(&here("./shots/../home.png", Some(folder), None).await), "shown image/png");
        assert_eq!(said(&here(&edge, None, None).await), "shown image/png");
    })
    .await
}

// TS slate-images.test.ts:59
#[tokio::test(flavor = "current_thread")]
async fn refuses_a_text_file_named_png_by_its_bytes() {
    local(async {
        let dir = tmp();
        let notes = put(dir.path(), "notes.png", b"a text file named like a picture\n");
        assert_eq!(said(&here(&notes, None, None).await), format!("R916 {notes} {NOT_AN_IMAGE_TAIL}"));
    })
    .await
}

// TS slate-images.test.ts:63
#[tokio::test(flavor = "current_thread")]
async fn refuses_an_svg_by_name_or_bytes() {
    local(async {
        let dir = tmp();
        let by_name = put(dir.path(), "logo.svg", SVG.as_bytes());
        let by_bytes = put(dir.path(), "logo.png", SVG.as_bytes());
        assert_eq!(
            said(&here(&by_name, None, None).await),
            format!("R916 {by_name} is an SVG, which a slate does not show since it can load other things; save it as a PNG")
        );
        let got = said(&here(&by_bytes, None, None).await);
        assert!(got.starts_with("R916 ") && got.contains("logo.png is an SVG"), "{got}");
    })
    .await
}

// TS slate-images.test.ts:68
#[tokio::test(flavor = "current_thread")]
async fn refuses_a_file_over_10_mb_with_its_size() {
    local(async {
        let dir = tmp();
        let mut huge = png();
        huge.resize(png().len() + CAP, 0);
        let path = put(dir.path(), "huge.png", &huge);
        assert_eq!(
            said(&here(&path, None, None).await),
            format!("R915 {path} is 10.1 MB, over the 10 MB an image may weigh; save it smaller, or show a part of it")
        );
    })
    .await
}

// TS slate-images.test.ts:72
#[tokio::test(flavor = "current_thread")]
async fn refuses_a_folder_a_device_a_pipe_and_a_socket_without_waiting_on_the_pipe() {
    local(async {
        let dir = tmp();
        let d = dir.path();
        let shots = d.join("shots");
        std::fs::create_dir(&shots).unwrap();
        let shots = shots.to_str().unwrap().to_string();
        let pipe = d.join("pipe.png").to_str().unwrap().to_string();
        let made = std::process::Command::new("mkfifo").arg(&pipe).status().unwrap();
        assert!(made.success());
        let sock = d.join("s.png").to_str().unwrap().to_string();
        let _listener = std::os::unix::net::UnixListener::bind(&sock).unwrap();

        let not_a_file = |p: &str| format!("R914 {p} is not a file; an image is a regular file");
        assert_eq!(said(&here(&shots, None, None).await), not_a_file(&shots));
        assert_eq!(said(&here("/dev/null", None, None).await), "R914 /dev/null is not a file; an image is a regular file");
        assert_eq!(said(&here("/dev/zero", None, None).await), "R914 /dev/zero is not a file; an image is a regular file");
        // A read that waited on the pipe would hold this test forever.
        assert_eq!(said(&here(&pipe, None, None).await), not_a_file(&pipe));
        assert_eq!(said(&here(&sock, None, None).await), not_a_file(&sock));
    })
    .await
}

// TS slate-images.test.ts:85
#[tokio::test(flavor = "current_thread")]
async fn says_one_byte_over_as_10_1_mb() {
    local(async {
        let dir = tmp();
        let path = put(dir.path(), "edge-over.png", &padded(CAP + 1));
        assert_eq!(
            said(&here(&path, None, None).await),
            format!("R915 {path} is 10.1 MB, over the 10 MB an image may weigh; save it smaller, or show a part of it")
        );
    })
    .await
}

// TS slate-images.test.ts:100
#[tokio::test(flavor = "current_thread")]
async fn answers_unchanged_for_the_held_version_and_the_new_picture_after_a_write() {
    local(async {
        let dir = tmp();
        let path = put(dir.path(), "again.png", &png());
        let first = json_of(&here(&path, None, None).await);
        let version = first["version"].as_str().expect("a shown image carries a version").to_string();
        assert!(Regex::new(r"^\d+:70:\d+:\d+$").unwrap().is_match(&version), "{version}");
        assert_eq!(json_of(&here(&path, None, Some(&version)).await), json!({"unchanged": true, "version": version}));
        let mut more = png();
        more.extend_from_slice(b"more");
        std::fs::write(&path, &more).unwrap();
        let again = json_of(&here(&path, None, Some(&version)).await);
        assert_eq!(again["mediaType"], "image/png");
        assert_eq!(again["bytes"], b64(&more));
        assert!(Regex::new(r"^\d+:74:").unwrap().is_match(again["version"].as_str().unwrap()), "{again}");
    })
    .await
}

// TS slate-images.test.ts:111
#[tokio::test(flavor = "current_thread")]
async fn shows_a_same_size_picture_copied_with_cp_p() {
    local(async {
        let dir = tmp();
        let d = dir.path();
        let mut other_png = png();
        let n = other_png.len();
        other_png[n - 5] = 0x42;
        let shown = put(d, "kept-time.png", &png());
        let other = put(d, "other-same-size.png", &other_png);
        let when = std::time::UNIX_EPOCH + Duration::from_secs(1_790_856_000);
        for p in [&shown, &other] {
            std::fs::File::options().write(true).open(p).unwrap().set_modified(when).unwrap();
        }
        let first = json_of(&here(&shown, None, None).await);
        let version = first["version"].as_str().unwrap().to_string();
        tokio::time::sleep(Duration::from_millis(20)).await;
        let cp = std::process::Command::new("cp").args(["-p", &other, &shown]).status().unwrap();
        assert!(cp.success());
        assert_eq!(std::fs::metadata(&shown).unwrap().modified().unwrap(), when);
        let after = json_of(&here(&shown, None, Some(&version)).await);
        assert_eq!(after["mediaType"], "image/png");
        assert_eq!(after["bytes"], b64(&other_png));
    })
    .await
}

// TS slate-images.test.ts:129
#[tokio::test(flavor = "current_thread")]
async fn refuses_a_missing_file_a_path_through_a_file_an_empty_src_and_a_home_path() {
    local(async {
        let dir = tmp();
        let d = dir.path();
        let home = put(d, "home.png", &png());
        let gone = d.join("gone.png").to_str().unwrap().to_string();
        let through = format!("{home}/x.png");
        assert_eq!(said(&here(&gone, None, None).await), format!("R900 no file at {gone}"));
        assert_eq!(said(&here(&through, None, None).await), format!("R900 no file at {through}"));
        let empty = said(&here("  ", None, None).await);
        assert!(empty.starts_with("R900 an image names no file"), "{empty}");
        assert_eq!(
            said(&here("~/shots/a.png", None, None).await),
            "R900 ~/shots/a.png starts at a home folder, which wsp does not guess; write the whole path"
        );
        assert_eq!(
            said(&here("shots/a.png", None, None).await),
            "R903 shots/a.png is not a whole path, and this host knows no folder for the thread to read it under"
        );
    })
    .await
}

// TS slate-images.test.ts:137
#[tokio::test(flavor = "current_thread")]
async fn refuses_an_address_that_is_not_http_or_carries_a_login() {
    local(async {
        let file = said(&here("file:///etc/hosts", None, None).await);
        assert!(file.starts_with("R917 file:///etc/hosts is not an http or https address"), "{file}");
        let data = said(&here("data:image/png;base64,AAAA", None, None).await);
        assert!(data.starts_with("R917 data:image"), "{data}");
        assert_eq!(
            said(&here("https://me:secret@acme.test/a.png", None, None).await),
            "R917 https://me:secret@acme.test/a.png carries a login, which wsp never sends"
        );
    })
    .await
}

// The thread's other computer.

type Answer = Result<Value, ImageRefusal>;

fn on(asked: &Rc<RefCell<Vec<String>>>, answer: impl Fn(&str) -> Answer + 'static) -> ImageOn {
    let asked = asked.clone();
    ImageOn {
        name: "dev4".into(),
        read: Rc::new(move |path: String| {
            asked.borrow_mut().push(path.clone());
            let a = answer(&path).map(|v| serde_json::from_value::<FsImageReply>(v).unwrap());
            async move { a }.boxed_local()
        }),
    }
}

fn refusal(code: Option<&str>, message: &str) -> Answer {
    Err(ImageRefusal { code: code.map(String::from), message: message.into() })
}

async fn there(src: &str, o: ImageOn, have: Option<&str>) -> SlatesImageAnswer {
    slate_image(
        src,
        SlateImageRoads {
            thread: "t-there".into(),
            have: have.map(String::from),
            folder: Some("/root/acme".into()),
            on: Some(o),
            allowed: Rc::new(|_| false),
            fetch: None,
        },
    )
    .await
}

// TS slate-images.test.ts:159
#[tokio::test(flavor = "current_thread")]
async fn reads_a_whole_path_there_and_a_relative_one_under_the_folder_there() {
    local(async {
        let asked = Rc::new(RefCell::new(Vec::new()));
        let content = b64(&png());
        let reply = json!({"size": png().len(), "mediaType": "image/png", "content": content});
        let box_ = on(&asked, move |_| Ok(reply.clone()));
        let first = json_of(&there("/tmp/home.png", box_.clone(), None).await);
        assert_eq!(first["mediaType"], "image/png");
        assert_eq!(first["bytes"], b64(&png()));
        assert_eq!(said(&there("shots/a.png", box_, None).await), "shown image/png");
        assert_eq!(*asked.borrow(), vec!["/tmp/home.png".to_string(), "/root/acme/shots/a.png".to_string()]);
    })
    .await
}

// TS slate-images.test.ts:166
#[tokio::test(flavor = "current_thread")]
async fn names_an_svg_the_same_on_another_computer() {
    local(async {
        let dir = tmp();
        let path = put(dir.path(), "logo.png", SVG.as_bytes());
        let asked = Rc::new(RefCell::new(Vec::new()));
        let size = SVG.len();
        let on_this = said(&here(&path, None, None).await);
        let on_box = said(&there(&path, on(&asked, move |_| Ok(json!({"size": size, "svg": true}))), None).await);
        assert_eq!(on_box, on_this);
        assert!(on_this.starts_with("R916 ") && on_this.contains("logo.png is an SVG, which a slate does not show"), "{on_this}");
    })
    .await
}

// TS slate-images.test.ts:179
#[tokio::test(flavor = "current_thread")]
async fn answers_unchanged_for_a_daemon_version() {
    local(async {
        let asked = Rc::new(RefCell::new(Vec::new()));
        let reply = json!({"size": 70, "modified": 1_760_000_000_000u64, "mediaType": "image/png", "content": b64(&png())});
        let box_ = on(&asked, move |_| Ok(reply.clone()));
        let version = "1760000000000:70";
        let first = json_of(&there("/tmp/home.png", box_.clone(), None).await);
        assert_eq!(first["version"], version);
        assert_eq!(
            json_of(&there("/tmp/home.png", box_, Some(version)).await),
            json!({"unchanged": true, "version": version})
        );
    })
    .await
}

// TS slate-images.test.ts:185
#[tokio::test(flavor = "current_thread")]
async fn reads_at_most_four_of_one_threads_images_at_once() {
    local(async {
        let running: Rc<RefCell<HashMap<String, i32>>> = Rc::default();
        let most: Rc<RefCell<HashMap<String, i32>>> = Rc::default();
        let held: Rc<RefCell<Vec<oneshot::Sender<()>>>> = Rc::default();
        let reply = json!({"size": png().len(), "mediaType": "image/png", "content": b64(&png())});
        let slow = |name: &str| {
            let (running, most, held, reply, name) = (running.clone(), most.clone(), held.clone(), reply.clone(), name.to_string());
            ImageOn {
                name: name.clone(),
                read: Rc::new(move |_path: String| {
                    let (running, most, held, reply, name) = (running.clone(), most.clone(), held.clone(), reply.clone(), name.clone());
                    async move {
                        let now = {
                            let mut r = running.borrow_mut();
                            let n = r.entry(name.clone()).or_insert(0);
                            *n += 1;
                            *n
                        };
                        {
                            let mut m = most.borrow_mut();
                            let top = m.entry(name.clone()).or_insert(0);
                            *top = (*top).max(now);
                        }
                        let (tx, rx) = oneshot::channel();
                        held.borrow_mut().push(tx);
                        let _ = rx.await;
                        *running.borrow_mut().get_mut(&name).unwrap() -= 1;
                        Ok(serde_json::from_value::<FsImageReply>(reply).unwrap())
                    }
                    .boxed_local()
                }),
            }
        };
        let read = |thread: &str, i: usize| {
            let roads = SlateImageRoads {
                thread: thread.into(),
                have: None,
                folder: Some("/root".into()),
                on: Some(slow(thread)),
                allowed: Rc::new(|_| false),
                fetch: None,
            };
            tokio::task::spawn_local(async move { slate_image(&format!("/tmp/{i}.png"), roads).await })
        };
        let mut all: Vec<_> = (0..10).map(|i| read("t-busy", i)).collect();
        all.push(read("t-other", 0));
        for _ in 0..20 {
            if held.borrow().len() >= 5 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        let now = {
            let r = running.borrow();
            json!({"busy": r.get("t-busy"), "other": r.get("t-other")})
        };
        assert_eq!(now, json!({"busy": 4, "other": 1}));
        while !all.iter().all(|h| h.is_finished()) {
            let go: Vec<_> = held.borrow_mut().drain(..).collect();
            for tx in go {
                let _ = tx.send(());
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        let mut shown = Vec::new();
        for h in all {
            shown.push(said(&h.await.unwrap()));
        }
        assert_eq!(shown, vec!["shown image/png".to_string(); 11]);
        assert_eq!(most.borrow().get("t-busy"), Some(&4));
    })
    .await
}

// TS slate-images.test.ts:211
#[tokio::test(flavor = "current_thread")]
async fn says_each_daemon_refusal_and_names_the_update() {
    local(async {
        let asked = Rc::new(RefCell::new(Vec::new()));
        let big = said(&there("/tmp/big.png", on(&asked, |_| Ok(json!({"size": CAP + 5}))), None).await);
        assert!(big.starts_with("R915 /tmp/big.png is 10.1 MB, over the 10 MB"), "{big}");
        assert_eq!(
            said(&there("/tmp/notes.png", on(&asked, |_| Ok(json!({"size": 30}))), None).await),
            format!("R916 /tmp/notes.png {NOT_AN_IMAGE_TAIL}")
        );
        let svg = said(&there("/tmp/logo.svg", on(&asked, |_| Ok(json!({"size": 30}))), None).await);
        assert!(svg.starts_with("R916 /tmp/logo.svg is an SVG"), "{svg}");
        assert_eq!(
            said(&there("/tmp/gone.png", on(&asked, |_| refusal(Some("not-found"), "/tmp/gone.png does not exist")), None).await),
            "R900 no file at /tmp/gone.png"
        );
        assert_eq!(
            said(&there("/dev/null", on(&asked, |_| refusal(Some("not-a-file"), "/dev/null is not a regular file")), None).await),
            "R914 /dev/null is not a file; an image is a regular file"
        );
        assert_eq!(
            said(&there("/tmp/a.png", on(&asked, |_| refusal(None, "unknown op: fs.image")), None).await),
            "R903 dev4 runs a daemon too old to read images; wsp add dev4 --update updates it"
        );
    })
    .await
}

// A web server on this computer, which the tests reach under made-up names through a road that lets 127.0.0.1
// alone through; every other address is judged by the host's own rule.

struct Seen {
    url: String,
    headers: Vec<(String, String)>,
}

struct WebState {
    seen: Mutex<Vec<Seen>>,
    /// How many bytes the endless route had sent when its client went away; -1 until it has.
    closed_after: AtomicI64,
    /// Set when the client of the silent route closed its socket.
    silent_closed: AtomicBool,
}

struct Web {
    port: u16,
    st: Arc<WebState>,
}

impl Web {
    fn urls(&self) -> Vec<String> {
        self.st.seen.lock().unwrap().iter().map(|s| s.url.clone()).collect()
    }

    fn at(&self, path: &str) -> String {
        at(self.port, path, "img.acme.test")
    }
}

fn at(port: u16, path: &str, host: &str) -> String {
    format!("http://{host}:{port}{path}")
}

async fn serve() -> Web {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let st = Arc::new(WebState { seen: Mutex::new(Vec::new()), closed_after: AtomicI64::new(-1), silent_closed: AtomicBool::new(false) });
    let shared = st.clone();
    tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else { return };
            tokio::spawn(handle(stream, port, shared.clone()));
        }
    });
    Web { port, st }
}

async fn reply(s: &mut TcpStream, status: &str, headers: &[(&str, String)], body: &[u8]) {
    let mut head = format!("HTTP/1.1 {status}\r\n");
    for (k, v) in headers {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    head.push_str(&format!("content-length: {}\r\nconnection: close\r\n\r\n", body.len()));
    let _ = s.write_all(head.as_bytes()).await;
    let _ = s.write_all(body).await;
    let _ = s.shutdown().await;
}

async fn handle(mut s: TcpStream, port: u16, st: Arc<WebState>) {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
        let n = s.read(&mut chunk).await.unwrap_or(0);
        if n == 0 {
            return;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let text = String::from_utf8_lossy(&buf).to_string();
    let mut lines = text.split("\r\n");
    let url = lines.next().unwrap_or("").split(' ').nth(1).unwrap_or("").to_string();
    let headers: Vec<(String, String)> = lines
        .take_while(|l| !l.is_empty())
        .filter_map(|l| l.split_once(':').map(|(k, v)| (k.trim().to_lowercase(), v.trim().to_string())))
        .collect();
    st.seen.lock().unwrap().push(Seen { url: url.clone(), headers });
    let ct = |t: &str| vec![("content-type", t.to_string())];
    let moved = |to: String| vec![("location", to)];
    match url.as_str() {
        "/a.png" => reply(&mut s, "200 OK", &ct("image/png"), &png()).await,
        "/tagged.png" => reply(&mut s, "200 OK", &[ct("image/png"), vec![("etag", "\"v7\"".to_string())]].concat(), &png()).await,
        "/notes.png" => reply(&mut s, "200 OK", &ct("image/png"), b"not a picture").await,
        "/logo.svg" => reply(&mut s, "200 OK", &ct("image/svg+xml"), SVG.as_bytes()).await,
        "/missing.png" => reply(&mut s, "404 Not Found", &[], b"").await,
        "/declared-big.png" => {
            let head = format!("HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n", CAP + 1);
            let _ = s.write_all(head.as_bytes()).await;
            let _ = s.shutdown().await;
        }
        "/to-a.png" => reply(&mut s, "302 Found", &moved("/a.png".into()), b"").await,
        "/to-other.png" => reply(&mut s, "302 Found", &moved(format!("http://other.acme.test:{port}/a.png")), b"").await,
        "/to-metadata.png" => reply(&mut s, "302 Found", &moved("http://169.254.169.254/latest/meta-data/".into()), b"").await,
        "/to-file.png" => reply(&mut s, "302 Found", &moved("file:///etc/passwd".into()), b"").await,
        "/loop.png" => reply(&mut s, "302 Found", &moved("/loop.png".into()), b"").await,
        "/silent.png" => loop {
            match s.read(&mut chunk).await {
                Ok(0) | Err(_) => {
                    st.silent_closed.store(true, Ordering::SeqCst);
                    return;
                }
                Ok(_) => {}
            }
        },
        "/endless.png" => {
            let head = "HTTP/1.1 200 OK\r\ncontent-type: image/png\r\ntransfer-encoding: chunked\r\n\r\n";
            let mut sent: usize = 0;
            let mut alive = s.write_all(head.as_bytes()).await.is_ok();
            let first = png();
            let zeros = vec![0u8; 64 * 1024];
            let mut piece: &[u8] = &first;
            while alive && sent < 40 * 1024 * 1024 {
                let mut frame = format!("{:x}\r\n", piece.len()).into_bytes();
                frame.extend_from_slice(piece);
                frame.extend_from_slice(b"\r\n");
                alive = s.write_all(&frame).await.is_ok();
                if alive {
                    sent += piece.len();
                }
                piece = &zeros;
            }
            if alive {
                let _ = s.write_all(b"0\r\n\r\n").await;
            }
            st.closed_after.store(sent as i64, Ordering::SeqCst);
        }
        _ => reply(&mut s, "404 Not Found", &[], b"").await,
    }
}

type Lookup = Rc<dyn Fn(&str) -> LocalBoxFuture<'static, Result<Vec<LookedUp>, String>>>;

struct TestRoad {
    names: HashMap<&'static str, Vec<&'static str>>,
    lookup: Option<Lookup>,
    refuses: fn(&str) -> Option<String>,
    timeout: u64,
}

impl FetchRoad for TestRoad {
    fn lookup(&self, host: &str) -> LocalBoxFuture<'static, Result<Vec<LookedUp>, String>> {
        if let Some(f) = &self.lookup {
            return f(host);
        }
        let found: Vec<LookedUp> = self
            .names
            .get(host)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|a| LookedUp { address: a.to_string(), family: if a.contains(':') { 6 } else { 4 } })
            .collect();
        async move { Ok(found) }.boxed_local()
    }

    fn refuses(&self, address: &str) -> Option<String> {
        (self.refuses)(address)
    }

    fn timeout_ms(&self) -> u64 {
        self.timeout
    }
}

fn judged(address: &str) -> Option<String> {
    if address == "127.0.0.1" {
        None
    } else {
        local_address(address)
    }
}

fn road_with(lookup: Option<Lookup>, timeout: u64) -> Rc<dyn FetchRoad> {
    let names = HashMap::from([
        ("img.acme.test", vec!["127.0.0.1"]),
        ("other.acme.test", vec!["127.0.0.1"]),
        ("inner.acme.test", vec!["10.0.0.7"]),
        ("both.acme.test", vec!["93.184.215.14", "fd00::7"]),
    ]);
    Rc::new(TestRoad { names, lookup, refuses: judged, timeout })
}

fn road() -> Rc<dyn FetchRoad> {
    road_with(None, 5_000)
}

fn two_step(first: &'static str, then: &'static str, asks: &Rc<Cell<u32>>) -> Lookup {
    let asks = asks.clone();
    Rc::new(move |_host: &str| {
        let n = asks.get();
        asks.set(n + 1);
        let address = if n == 0 { first } else { then };
        async move { Ok(vec![LookedUp { address: address.to_string(), family: 4 }]) }.boxed_local()
    })
}

async fn fetched(url: &str, domain: &str, road: Rc<dyn FetchRoad>) -> String {
    said(&fetch_slate_image(url, domain, Some(road)).await)
}

async fn fetched_at(web: &Web, path: &str, road: Rc<dyn FetchRoad>) -> String {
    fetched(&web.at(path), "img.acme.test", road).await
}

// TS slate-images.test.ts:281
#[tokio::test(flavor = "current_thread")]
async fn is_asked_for_and_fetches_nothing_until_allowed() {
    local(async {
        let web = serve().await;
        let allowed = Rc::new(Cell::new(false));
        let gate = allowed.clone();
        let roads = SlateImageRoads {
            thread: "t-remote".into(),
            have: None,
            folder: None,
            on: None,
            allowed: Rc::new(move |d: &str| gate.get() && d == "img.acme.test"),
            fetch: Some(road()),
        };
        assert_eq!(json_of(&slate_image(&web.at("/a.png"), roads.clone()).await), json!({"ask": {"domain": "img.acme.test"}}));
        assert_eq!(web.urls(), Vec::<String>::new());
        allowed.set(true);
        let shown = json_of(&slate_image(&web.at("/a.png"), roads).await);
        assert_eq!(shown["mediaType"], "image/png");
        assert_eq!(shown["bytes"], b64(&png()));
        assert_eq!(web.urls(), vec!["/a.png".to_string()]);
    })
    .await
}

// TS slate-images.test.ts:291
#[tokio::test(flavor = "current_thread")]
async fn sends_no_cookie_and_no_login() {
    local(async {
        let web = serve().await;
        assert_eq!(fetched_at(&web, "/a.png", road()).await, "shown image/png");
        let seen = web.st.seen.lock().unwrap();
        let mut keys: Vec<&str> = seen[0].headers.iter().map(|(k, _)| k.as_str()).collect();
        keys.sort();
        assert_eq!(keys, vec!["accept", "connection", "host", "user-agent"]);
        let header = |name: &str| seen[0].headers.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone());
        assert_eq!(header("user-agent").as_deref(), Some("wsp"));
        assert_eq!(header("accept").as_deref(), Some("image/png,image/jpeg,image/gif,image/webp"));
    })
    .await
}

// TS slate-images.test.ts:296 (the 22 rows of the it.each table)
#[tokio::test(flavor = "current_thread")]
async fn refuses_each_local_address() {
    local(async {
        let rows: [(&str, Option<&str>); 22] = [
            ("localhost", None),
            ("127.0.0.1", Some("127.0.0.1, which is this computer")),
            ("127.8.9.10", Some("127.8.9.10, which")),
            ("0.0.0.0", Some("0.0.0.0, which")),
            ("10.1.2.3", Some("10.1.2.3, which")),
            ("172.16.0.1", Some("172.16.0.1, which")),
            ("172.31.255.254", Some("172.31.255.254, which")),
            ("192.168.1.1", Some("192.168.1.1, which")),
            ("100.64.0.1", Some("100.64.0.1, which")),
            ("169.254.169.254", Some("169.254.169.254, which")),
            ("[::1]", Some("::1, which")),
            ("[::]", Some("resolves to ::, which")),
            ("[fe80::1]", Some("fe80::1, which")),
            ("[fd00::1]", Some("fd00::1, which")),
            ("[fc00::1]", Some("fc00::1, which")),
            ("[::ffff:127.0.0.1]", Some("::ffff:7f00:1, which")),
            ("[::ffff:169.254.169.254]", Some("::ffff:a9fe:a9fe, which")),
            ("[64:ff9b::a00:1]", Some("64:ff9b::a00:1, which")),
            ("[::7f00:1]", Some("::7f00:1, which")),
            ("[::ffff:0:7f00:1]", Some("::ffff:0:7f00:1, which")),
            ("[64:ff9b:1::7f00:1]", Some("64:ff9b:1::7f00:1, which")),
            ("[2002:7f00:1::]", Some("2002:7f00:1::, which")),
        ];
        let strict: Rc<dyn FetchRoad> = Rc::new(TestRoad { names: HashMap::new(), lookup: None, refuses: local_address, timeout: 5_000 });
        for (host, says) in rows {
            let url = format!("http://{host}/a.png");
            let parsed = url::Url::parse(&url).unwrap();
            let domain = parsed.host_str().unwrap().to_string();
            let with_road = said(&fetch_slate_image(&url, &domain, Some(strict.clone())).await);
            let with_default = said(&fetch_slate_image(&url, &domain, None).await);
            for (which, got) in [("test road", &with_road), ("default road", &with_default)] {
                assert!(got.starts_with("R917 "), "{host} on the {which}: {got}");
                if let Some(says) = says {
                    assert!(got.contains(says), "{host} on the {which}: {got}");
                }
            }
            if host == "localhost" {
                let ok = with_default.contains("resolves to 127.0.0.1, which is this computer or its network")
                    || with_default.contains("resolves to ::1, which is this computer or its network");
                assert!(ok, "localhost on the default road: {with_default}");
            }
        }
    })
    .await
}

// Not in the TS test; spec rules 7 and 8, with the values the TS localAddress gives.
#[test]
fn local_address_reads_carried_and_zoned_addresses() {
    let local = Some("is this computer or its network".to_string());
    let not = Some("is not an address".to_string());
    let rows: Vec<(&str, Option<String>)> = vec![
        ("not-an-ip", not.clone()),
        ("", not.clone()),
        ("example.com", not.clone()),
        ("[::1]", not.clone()),
        ("1.2.3", not.clone()),
        ("fe80::1%", not.clone()),
        ("8.8.8.8", None),
        ("93.184.215.14", None),
        ("0.0.0.1", local.clone()),
        ("10.0.0.1", local.clone()),
        ("100.63.255.255", None),
        ("100.64.0.1", local.clone()),
        ("100.127.255.255", local.clone()),
        ("100.128.0.1", None),
        ("127.0.0.1", local.clone()),
        ("169.254.1.1", local.clone()),
        ("172.15.0.1", None),
        ("172.16.0.1", local.clone()),
        ("172.31.255.255", local.clone()),
        ("172.32.0.1", None),
        ("192.0.0.5", local.clone()),
        ("192.0.1.5", None),
        ("192.168.0.1", local.clone()),
        ("198.18.0.1", local.clone()),
        ("198.19.255.255", local.clone()),
        ("198.20.0.1", None),
        ("224.0.0.1", local.clone()),
        ("239.255.255.255", local.clone()),
        ("240.0.0.1", local.clone()),
        ("255.255.255.255", local.clone()),
        ("::", local.clone()),
        ("::1", local.clone()),
        ("::2", local.clone()),
        ("::0:2", local.clone()),
        ("::0.0.0.1", local.clone()),
        ("::0.0.0.2", local.clone()),
        ("::1:0", local.clone()),
        ("::7f00:1", local.clone()),
        ("::127.0.0.1", local.clone()),
        ("::808:808", None),
        ("::101:101", None),
        ("::8.8.8.8", None),
        ("::ffff:7f00:1", local.clone()),
        ("::ffff:127.0.0.1", local.clone()),
        ("::ffff:10.0.0.1", local.clone()),
        ("::ffff:808:808", None),
        ("::ffff:8.8.8.8", None),
        ("::ffff:0:7f00:1", local.clone()),
        ("::ffff:0:10.0.0.1", local.clone()),
        ("::ffff:0:808:808", None),
        ("64:ff9b::7f00:1", local.clone()),
        ("64:ff9b::10.0.0.1", local.clone()),
        ("64:ff9b::808:808", None),
        ("64:ff9b::8.8.8.8", None),
        ("64:ff9b::1.2.3.4", None),
        ("64:ff9b:1::7f00:1", local.clone()),
        ("64:ff9b:1::808:808", local.clone()),
        ("2002:7f00:1::", local.clone()),
        ("2002:a00:1::", local.clone()),
        ("2002:a00:1:1::", local.clone()),
        ("2002:808:808::", None),
        ("2002:808:808::1", None),
        ("2001:db8::1", None),
        ("2606:4700::1111", None),
        ("fc00::1", local.clone()),
        ("fd00::1", local.clone()),
        ("fe80::1", local.clone()),
        ("fe80::1%eth0", local.clone()),
        ("::1%lo", local.clone()),
        ("fec0::1", local.clone()),
        ("ff02::1", local.clone()),
        ("100::1", local.clone()),
        ("100:0:0:1::1", None),
        ("100:0:0:0:1::1", local.clone()),
    ];
    for (address, want) in rows {
        assert_eq!(local_address(address), want, "local_address({address:?})");
    }
}

// TS slate-images.test.ts:325
#[tokio::test(flavor = "current_thread")]
async fn refuses_a_name_resolving_to_any_private_address() {
    local(async {
        let web = serve().await;
        assert_eq!(
            fetched(&at(web.port, "/a.png", "inner.acme.test"), "inner.acme.test", road()).await,
            format!(
                "R917 {} resolves to 10.0.0.7, which is this computer or its network; a slate fetches images from the internet alone",
                at(web.port, "/a.png", "inner.acme.test")
            )
        );
        let both = fetched(&at(web.port, "/a.png", "both.acme.test"), "both.acme.test", road()).await;
        assert!(both.contains("resolves to fd00::7, which is this computer"), "{both}");
        assert_eq!(web.urls(), Vec::<String>::new());
    })
    .await
}

// TS slate-images.test.ts:331
#[tokio::test(flavor = "current_thread")]
async fn dials_the_address_it_checked() {
    local(async {
        let web = serve().await;
        let asks = Rc::new(Cell::new(0));
        let flips = two_step("127.0.0.1", "10.0.0.7", &asks);
        assert_eq!(fetched_at(&web, "/a.png", road_with(Some(flips), 5_000)).await, "shown image/png");
        assert_eq!(asks.get(), 1);
    })
    .await
}

// TS slate-images.test.ts:338
#[tokio::test(flavor = "current_thread")]
async fn follows_a_redirect_within_the_domain_and_refuses_others() {
    local(async {
        let web = serve().await;
        assert_eq!(fetched_at(&web, "/to-a.png", road()).await, "shown image/png");
        assert_eq!(
            fetched_at(&web, "/to-other.png", road()).await,
            format!("R917 {} redirects to other.acme.test, a domain other than the img.acme.test you allowed", web.at("/to-other.png"))
        );
        let metadata = fetched_at(&web, "/to-metadata.png", road()).await;
        assert!(metadata.contains("redirects to 169.254.169.254, a domain other than"), "{metadata}");
        let file = fetched_at(&web, "/to-file.png", road()).await;
        assert!(file.contains("leads to file: by a redirect"), "{file}");
        let looped = fetched_at(&web, "/loop.png", road()).await;
        assert!(looped.contains("redirects more than 5 times"), "{looped}");
        assert!(!web.urls().contains(&"/latest/meta-data/".to_string()));
    })
    .await
}

// TS slate-images.test.ts:347
#[tokio::test(flavor = "current_thread")]
async fn checks_the_address_again_at_a_redirect() {
    local(async {
        let web = serve().await;
        let asks = Rc::new(Cell::new(0));
        let rebinds = two_step("127.0.0.1", "192.168.0.10", &asks);
        let got = fetched_at(&web, "/to-a.png", road_with(Some(rebinds), 5_000)).await;
        assert!(got.starts_with("R917 "), "{got}");
        assert!(got.contains("/a.png resolves to 192.168.0.10, which is this computer or its network"), "{got}");
        assert_eq!(web.urls(), vec!["/to-a.png".to_string()]);
    })
    .await
}

// TS slate-images.test.ts:354
#[tokio::test(flavor = "current_thread")]
async fn stops_at_10_mb_as_bytes_stream_and_refuses_a_declared_length() {
    local(async {
        let web = serve().await;
        assert_eq!(
            fetched_at(&web, "/endless.png", road()).await,
            format!("R915 {} is more than the 10 MB an image may weigh; save it smaller, or show a part of it", web.at("/endless.png"))
        );
        for _ in 0..200 {
            if web.st.closed_after.load(Ordering::SeqCst) != -1 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let closed_after = web.st.closed_after.load(Ordering::SeqCst);
        assert!(closed_after > 0, "the server never saw its client go: {closed_after}");
        assert!(closed_after < 20 * 1024 * 1024, "the server had sent {closed_after} bytes when the client closed");
        let declared = fetched_at(&web, "/declared-big.png", road()).await;
        assert!(declared.starts_with("R915 ") && declared.contains(" is 10.1 MB, over the 10 MB"), "{declared}");
    })
    .await
}

// TS slate-images.test.ts:362 (no unhandled rejection; here nothing may outlive the call)
#[tokio::test(flavor = "current_thread")]
async fn leaves_nothing_running_after_a_deadline() {
    let web = serve().await;
    let local = LocalSet::new();
    let url = format!("http://127.0.0.1:{}/silent.png", web.port);
    let got = local.run_until(fetched(&url, "127.0.0.1", road_with(None, 500))).await;
    assert_eq!(got, format!("R903 127.0.0.1 did not answer within 1 s for {url}"));
    for _ in 0..200 {
        if web.st.silent_closed.load(Ordering::SeqCst) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(web.st.silent_closed.load(Ordering::SeqCst), "the socket to the silent server was left open");
    assert!(tokio::time::timeout(Duration::from_secs(2), local).await.is_ok(), "a local task was still running 2 s after the call");
}

// TS slate-images.test.ts:376
#[tokio::test(flavor = "current_thread")]
async fn gives_up_on_a_lookup_that_never_answers() {
    local(async {
        let web = serve().await;
        let never: Lookup = Rc::new(|_host: &str| futures::future::pending().boxed_local());
        assert_eq!(
            fetched_at(&web, "/a.png", road_with(Some(never), 1_000)).await,
            format!("R903 img.acme.test did not answer within 1 s for {}", web.at("/a.png"))
        );
    })
    .await
}

// TS slate-images.test.ts:380
#[tokio::test(flavor = "current_thread")]
async fn carries_the_etag_as_version() {
    local(async {
        let web = serve().await;
        let roads = |have: Option<&str>| SlateImageRoads {
            thread: "t-tag".into(),
            have: have.map(String::from),
            folder: None,
            on: None,
            allowed: Rc::new(|_| true),
            fetch: Some(road()),
        };
        let first = json_of(&slate_image(&web.at("/tagged.png"), roads(None)).await);
        assert_eq!(first["version"], "\"v7\"");
        assert_eq!(
            json_of(&slate_image(&web.at("/tagged.png"), roads(Some("\"v7\""))).await),
            json!({"unchanged": true, "version": "\"v7\""})
        );
    })
    .await
}

// TS slate-images.test.ts:386
#[tokio::test(flavor = "current_thread")]
async fn gives_up_on_a_silent_server() {
    local(async {
        let web = serve().await;
        assert_eq!(
            fetched_at(&web, "/silent.png", road_with(None, 1_000)).await,
            format!("R903 img.acme.test did not answer within 1 s for {}", web.at("/silent.png"))
        );
    })
    .await
}

// TS slate-images.test.ts:390
#[tokio::test(flavor = "current_thread")]
async fn refuses_what_is_not_an_image_an_svg_and_a_failed_status() {
    local(async {
        let web = serve().await;
        assert_eq!(
            fetched_at(&web, "/notes.png", road()).await,
            format!("R916 {} {NOT_AN_IMAGE_TAIL}", web.at("/notes.png"))
        );
        let svg = fetched_at(&web, "/logo.svg", road()).await;
        assert!(svg.starts_with("R916 ") && svg.contains(" is an SVG, which a slate does not show"), "{svg}");
        assert_eq!(
            fetched_at(&web, "/missing.png", road()).await,
            format!("R903 img.acme.test answered 404 for {}", web.at("/missing.png"))
        );
    })
    .await
}
