// SPDX-License-Identifier: AGPL-3.0-only
//! The images projects wear, as Settings holds them (`images.ts`): each read from the host by the hash
//! `preferences.projectIcon` names, decoded once, and held while Settings stands open, since the sidebar's marks are a
//! view of their own that paints them again without asking. At most `HELD_MAX` are held: past that a new one takes the
//! place of the one drawn longest ago, and one drawn on the frame itself shows its glyph.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use base64::Engine as _;
use gpui_kit::RenderImage;

/// More than Settings draws at once: the projects list and the sidebar's rows under Projects.
pub const HELD_MAX: usize = 64;

enum Got {
    Asking,
    Image(Arc<RenderImage>),
    /// The host keeps no image under the hash, or would not say: the glyph stands.
    Missing,
}

struct Entry {
    got: Got,
    /// The frame that last drew it.
    seen: u64,
}

#[derive(Default)]
struct Held {
    by_hash: HashMap<String, Entry>,
    ask: Vec<String>,
    frame: u64,
    /// Images the bound let go, for the window to drop after the frame.
    gone: Vec<Arc<RenderImage>>,
}

/// Held inside the records each page reads, so a draw can take an image through a shared borrow.
#[derive(Default)]
pub struct Images {
    held: RefCell<Held>,
}

impl Images {
    /// The image under `hash` once it is here; one not asked for yet is asked after this frame.
    pub fn draw(&self, hash: &str) -> Option<Arc<RenderImage>> {
        let mut held = self.held.borrow_mut();
        let held = &mut *held;
        let frame = held.frame;
        if let Some(entry) = held.by_hash.get_mut(hash) {
            entry.seen = frame;
            return match &entry.got {
                Got::Image(image) => Some(image.clone()),
                Got::Asking | Got::Missing => None,
            };
        }
        if held.by_hash.len() >= HELD_MAX {
            let oldest = held.by_hash.iter().filter(|(_, e)| e.seen < frame).min_by_key(|(_, e)| e.seen).map(|(h, _)| h.clone())?;
            if let Some(Entry { got: Got::Image(image), .. }) = held.by_hash.remove(&oldest) {
                held.gone.push(image);
            }
        }
        held.by_hash.insert(hash.to_owned(), Entry { got: Got::Asking, seen: frame });
        held.ask.push(hash.to_owned());
        None
    }

    /// After a frame: the images the bound let go, for the window to drop, and the hashes to ask the host for.
    pub fn settle(&self) -> (Vec<String>, Vec<Arc<RenderImage>>) {
        let mut held = self.held.borrow_mut();
        held.frame += 1;
        let gone = std::mem::take(&mut held.gone);
        let ask: Vec<String> = std::mem::take(&mut held.ask).into_iter().filter(|h| held.by_hash.contains_key(h)).collect();
        (ask, gone)
    }

    /// `projects.icons`' answer: each hash's PNG as a data url, or none. Whether anything drawn moved.
    pub fn landed(&self, icons: HashMap<String, Option<String>>) -> bool {
        let mut held = self.held.borrow_mut();
        let mut moved = false;
        for (hash, url) in icons {
            let Some(entry) = held.by_hash.get_mut(&hash) else { continue };
            entry.got = match url.as_deref().and_then(decode) {
                Some(image) => {
                    moved = true;
                    Got::Image(image)
                }
                None => Got::Missing,
            };
        }
        moved
    }

    /// An ask the host did not answer: its hashes show the glyph until Settings opens again.
    pub fn failed(&self, hashes: &[String]) {
        let mut held = self.held.borrow_mut();
        for hash in hashes {
            if let Some(entry) = held.by_hash.get_mut(hash) {
                entry.got = Got::Missing;
            }
        }
    }

    /// Everything, for the window to drop as Settings goes.
    pub fn clear(&self) -> Vec<Arc<RenderImage>> {
        let mut held = self.held.borrow_mut();
        held.ask.clear();
        let mut gone = std::mem::take(&mut held.gone);
        gone.extend(held.by_hash.drain().filter_map(|(_, entry)| match entry.got {
            Got::Image(image) => Some(image),
            Got::Asking | Got::Missing => None,
        }));
        gone
    }

    #[cfg(test)]
    fn held(&self) -> usize {
        self.held.borrow().by_hash.len()
    }

    #[cfg(test)]
    fn holds(&self, hash: &str) -> bool {
        self.held.borrow().by_hash.contains_key(hash)
    }
}

/// A PNG data url as the window draws it.
fn decode(url: &str) -> Option<Arc<RenderImage>> {
    let png = base64::engine::general_purpose::STANDARD.decode(url.strip_prefix("data:image/png;base64,")?).ok()?;
    Some(Arc::new(render_image(image::load_from_memory_with_format(&png, image::ImageFormat::Png).ok()?.to_rgba8())))
}

/// A picture as GPUI draws one: straight BGRA.
pub fn render_image(mut picture: image::RgbaImage) -> RenderImage {
    for pixel in picture.pixels_mut() {
        pixel.0.swap(0, 2);
    }
    RenderImage::new(vec![image::Frame::new(picture)])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url() -> String {
        let mut png = Vec::new();
        image::RgbaImage::from_pixel(2, 2, image::Rgba([10, 20, 30, 255]))
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png))
    }

    #[test]
    fn an_image_is_asked_once_and_held_while_settings_stands_open() {
        let images = Images::default();
        assert!(images.draw("a").is_none() && images.draw("b").is_none());
        let (ask, gone) = images.settle();
        assert_eq!((ask, gone.len()), (vec!["a".to_owned(), "b".to_owned()], 0));
        assert!(images.landed(HashMap::from([("a".to_owned(), Some(url())), ("b".to_owned(), None)])));
        let a = images.draw("a").expect("a landed");
        assert_eq!(a.as_bytes(0).unwrap()[..4], [30, 20, 10, 255], "drawn as BGRA");
        assert!(images.draw("b").is_none(), "the host keeps none under b, so its glyph stands");
        assert_eq!(images.settle().0, Vec::<String>::new(), "nothing asked twice");
        let (_, gone) = images.settle();
        assert!(gone.is_empty() && images.held() == 2, "a frame that draws no project lets nothing go: the sidebar may still paint it");
        assert!(images.draw("a").is_some(), "and a drawn again is still here");
        assert_eq!((images.clear().len(), images.held()), (1, 0), "Settings going lets everything go");
    }

    #[test]
    fn past_the_bound_a_new_image_takes_the_place_of_the_one_drawn_longest_ago() {
        let images = Images::default();
        for n in 0..HELD_MAX + 10 {
            images.draw(&n.to_string());
        }
        let (ask, _) = images.settle();
        assert_eq!((ask.len(), images.held()), (HELD_MAX, HELD_MAX), "past the bound on one frame, a project shows its glyph");
        assert!(images.landed(HashMap::from([("0".to_owned(), Some(url()))])));
        images.draw("0");
        images.settle();
        images.draw("new");
        let (ask, gone) = images.settle();
        assert_eq!((ask, gone.len(), images.held()), (vec!["new".to_owned()], 0, HELD_MAX));
        assert!(images.draw("0").is_some(), "0, drawn last, is kept");
        let made_room: Vec<String> = (1..HELD_MAX).map(|n| n.to_string()).filter(|h| !images.holds(h)).collect();
        assert_eq!(made_room.len(), 1, "one drawn on the first frame made room");
        assert!(!images.landed(HashMap::from([(made_room[0].clone(), Some(url()))])), "and its late answer holds nothing");
        images.settle();
        for n in 0..HELD_MAX {
            images.draw(&format!("other-{n}"));
        }
        let (_, gone) = images.settle();
        assert_eq!((gone.len(), images.held()), (1, HELD_MAX), "0's image, let go for the bound, goes back to the window");
    }
}
