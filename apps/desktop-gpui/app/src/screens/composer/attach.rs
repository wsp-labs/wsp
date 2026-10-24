// SPDX-License-Identifier: AGPL-3.0-only
//! Files into the composer: a pick, a drop or a paste.

use std::sync::Arc;

use gpui_kit::{prelude::*, ClipboardEntry, Context, Image, ImageFormat, ImageSource, RenderImage, Window};

use super::model::{self, File};
use super::Composer;
use crate::screens::thread_view::cards::{release, thumbnail};

/// An image file as the box draws it: a tile cut to its 56 px square, or a GIF whole.
pub(super) enum Preview {
    Tile(Arc<RenderImage>),
    Whole(Arc<Image>),
}

impl Composer {
    /// Takes files as a pick, a drop or a paste does: each refused one is kept with its reason.
    pub fn attach(&mut self, files: Vec<File>, cx: &mut Context<Self>) {
        let no_images = self.resolved().agent.filter(|a| !a.images).map(|a| a.label);
        for file in files {
            match model::refuse(&file.name, &file.media_type, file.bytes.len(), self.files.len(), no_images.as_deref()) {
                Some(why) => self.refused.push((file.name, why)),
                None => self.files.push(file),
            }
        }
        cx.notify();
    }

    /// Cuts a tile for each image file without one, and lets go of the tiles of files no longer in the box, out of
    /// GPUI's cache and the window's atlas too. A GIF is drawn whole, since its tile would stand still.
    pub(super) fn keep_previews(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let files = &self.files;
        for (_, preview) in self.previews.extract_if(.., |(bytes, _)| !files.iter().any(|f| Arc::ptr_eq(&f.bytes, bytes))) {
            match preview {
                Some(Preview::Tile(tile)) => cx.drop_image(tile, Some(window)),
                Some(Preview::Whole(image)) => release(image, window, cx),
                None => {}
            }
        }
        for file in self.files.iter().filter(|f| f.is_image()) {
            if self.previews.iter().any(|(bytes, _)| Arc::ptr_eq(&file.bytes, bytes)) {
                continue;
            }
            let format = ImageFormat::from_mime_type(&file.media_type).unwrap_or(ImageFormat::Png);
            if format == ImageFormat::Gif {
                let whole = Arc::new(Image::from_bytes(format, file.bytes.to_vec()));
                self.previews.push((file.bytes.clone(), Some(Preview::Whole(whole))));
                continue;
            }
            self.previews.push((file.bytes.clone(), None));
            let (bytes, media) = (file.bytes.clone(), file.media_type.clone());
            let cut = cx.background_spawn(async move { thumbnail(&bytes, &media) });
            let key = file.bytes.clone();
            cx.spawn(async move |this, cx| {
                let Some(tile) = cut.await else { return };
                _ = this.update(cx, |this, cx| {
                    if let Some((_, slot @ None)) = this.previews.iter_mut().find(|(bytes, _)| Arc::ptr_eq(bytes, &key)) {
                        *slot = Some(Preview::Tile(tile));
                        cx.notify();
                    }
                });
            })
            .detach();
        }
    }

    /// What the box draws for an image file, once it is ready.
    pub(super) fn preview(&self, file: &File) -> Option<ImageSource> {
        let (_, preview) = self.previews.iter().find(|(bytes, _)| Arc::ptr_eq(&file.bytes, bytes))?;
        match preview.as_ref()? {
            Preview::Tile(tile) => Some(ImageSource::Render(tile.clone())),
            Preview::Whole(image) => Some(ImageSource::Image(image.clone())),
        }
    }

    pub fn remove_file(&mut self, ix: usize, cx: &mut Context<Self>) {
        if ix < self.files.len() {
            self.files.remove(ix);
        }
        cx.notify();
    }

    /// Files from paths: one past the size cap is refused by its length, unread; the rest are read off the window's
    /// thread.
    pub(super) fn attach_paths(&mut self, paths: &[std::path::PathBuf], cx: &mut Context<Self>) {
        let mut reads = Vec::new();
        for path in paths {
            let Some(name) = path.file_name().map(|n| n.to_string_lossy().into_owned()) else { continue };
            let len = std::fs::metadata(path).map(|m| m.len() as usize).unwrap_or(0);
            if len > model::FILE_MAX_BYTES {
                if let Some(why) = model::refuse(&name, model::media_type(&name), len, self.files.len(), None) {
                    self.refused.push((name, why));
                }
                continue;
            }
            reads.push((name, path.clone()));
        }
        cx.notify();
        if reads.is_empty() {
            return;
        }
        let read = cx.background_spawn(async move {
            reads
                .into_iter()
                .filter_map(|(name, path)| {
                    let bytes = std::fs::read(path).ok()?;
                    Some(File { media_type: model::media_type(&name).to_owned(), name, bytes: Arc::new(bytes) })
                })
                .collect::<Vec<_>>()
        });
        cx.spawn(async move |this, cx| {
            let files = read.await;
            _ = this.update(cx, |this, cx| this.attach(files, cx));
        })
        .detach();
    }

    pub(super) fn pick_files(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(gpui_kit::PathPromptOptions { files: true, directories: false, multiple: true, prompt: None });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = paths.await {
                _ = this.update(cx, |this, cx| this.attach_paths(&paths, cx));
            }
        })
        .detach();
    }

    pub(super) fn on_paste(&mut self, entries: Vec<ClipboardEntry>, cx: &mut Context<Self>) -> bool {
        let mut files = Vec::new();
        let mut paths = Vec::new();
        for entry in entries {
            match entry {
                ClipboardEntry::String(text) => {
                    let taken: Vec<&str> = self.files.iter().chain(&files).map(|f| f.name.as_str()).collect();
                    if let Some(file) = model::pasted_file(text.text(), &taken) {
                        files.push(file);
                    }
                }
                ClipboardEntry::Image(image) => {
                    let n = self.files.len() + files.len() + 1;
                    let media = image.format().mime_type().to_owned();
                    let ext = media.rsplit('/').next().unwrap_or("png").to_owned();
                    files.push(File {
                        name: format!("pasted-image-{n}.{ext}"),
                        media_type: media,
                        bytes: Arc::new(image.bytes().to_vec()),
                    });
                }
                ClipboardEntry::ExternalPaths(p) => paths.extend(p.paths().iter().cloned()),
            }
        }
        if files.is_empty() && paths.is_empty() {
            return false;
        }
        self.attach(files, cx);
        self.attach_paths(&paths, cx);
        true
    }
}
