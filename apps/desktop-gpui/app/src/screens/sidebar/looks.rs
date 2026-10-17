// SPDX-License-Identifier: AGPL-3.0-only
//! A project's glyph in the look the person picked (`projects/look.tsx`) or its own image (`projects.icons`), and a
//! computer's glyph (`settings/ComputerGlyph.tsx`).

use std::collections::HashMap;
use std::sync::Arc;

use base64::Engine as _;
use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::{div, img, prelude::*, AnyElement, App, Hsla, Image, ImageFormat, Pixels, Window};
use serde_json::{json, Value};
use wsp_host_client::protocol::{PlaceView, Preferences};

use crate::ui::computer_glyph::ComputerIcon;
use crate::ui::{Glyph, Palette};

/// A project's look: its glyph by `ProjectIcon`, the hue it is drawn in where one is picked.
fn project_icon(id: &str) -> Glyph {
    match id {
        "code" => Glyph::Code,
        "terminal" => Glyph::Terminal,
        "globe" => Glyph::Globe,
        "rocket" => Glyph::Rocket,
        "box" => Glyph::Box,
        "database" => Glyph::Database,
        "server" => Glyph::Server,
        "cpu" => Glyph::Cpu,
        "zap" => Glyph::Zap,
        "flame" => Glyph::Flame,
        "leaf" => Glyph::Leaf,
        "star" => Glyph::Star,
        "heart" => Glyph::Heart,
        "book" => Glyph::Book,
        "music" => Glyph::Music,
        "camera" => Glyph::Camera,
        "gamepad" => Glyph::Gamepad2,
        "shield" => Glyph::Shield,
        "wrench" => Glyph::Wrench,
        _ => Glyph::Folder,
    }
}

fn hue(id: &str, p: &Palette) -> Option<Hsla> {
    Some(match id {
        "red" => p.hue_red,
        "orange" => p.hue_orange,
        "amber" => p.hue_amber,
        "green" => p.hue_green,
        "teal" => p.hue_teal,
        "blue" => p.hue_blue,
        "violet" => p.hue_violet,
        "pink" => p.hue_pink,
        _ => return None,
    })
}

/// The project images the host keeps, by the hash `preferences.projectIcon` names, decoded once each. Holds only the
/// hashes the preferences name, so it is bounded by the number of projects.
#[derive(Default)]
pub struct Images {
    loaded: HashMap<String, Arc<Image>>,
    /// Hashes the host said it does not keep, asked again only on the next connection.
    missing: Vec<String>,
    /// Hashes asked for whose answer has not come.
    asked: Vec<String>,
    /// Images no longer named, which GPUI's image cache and the window's atlas keep until told.
    retired: Vec<Arc<Image>>,
}

impl Images {
    /// The hashes the preferences name that are neither loaded nor known to be missing.
    pub fn wanted(&self, preferences: &Value) -> Vec<String> {
        let named = preferences.get("projectIcon").and_then(Value::as_object);
        named
            .into_iter()
            .flat_map(|m| m.values())
            .filter_map(Value::as_str)
            .filter(|h| !self.loaded.contains_key(*h) && !self.missing.iter().chain(&self.asked).any(|m| m == h))
            .map(str::to_owned)
            .collect()
    }

    pub fn asking(&mut self, hashes: &[String]) {
        self.asked.extend_from_slice(hashes);
    }

    /// An ask that failed: its hashes wait for the next connection, as a missing one does.
    pub fn failed(&mut self, hashes: &[String]) {
        self.asked.retain(|h| !hashes.contains(h));
        self.missing.extend_from_slice(hashes);
    }

    /// Takes `projects.icons`'s answer: each hash's PNG as a data url, or null where the host keeps none.
    pub fn take(&mut self, icons: &Value, preferences: &Value) {
        for (hash, url) in icons.as_object().into_iter().flatten() {
            self.asked.retain(|h| h != hash);
            let png = url
                .as_str()
                .and_then(|u| u.strip_prefix("data:image/png;base64,"))
                .and_then(|b| base64::engine::general_purpose::STANDARD.decode(b).ok());
            match png {
                Some(bytes) => {
                    self.loaded.insert(hash.clone(), Arc::new(Image::from_bytes(ImageFormat::Png, bytes)));
                }
                None => self.missing.push(hash.clone()),
            }
        }
        self.keep_named(preferences);
    }

    /// Forgets what the preferences no longer name.
    pub fn keep_named(&mut self, preferences: &Value) {
        let named: Vec<&str> = preferences
            .get("projectIcon")
            .and_then(Value::as_object)
            .into_iter()
            .flat_map(|m| m.values())
            .filter_map(Value::as_str)
            .collect();
        self.retired.extend(self.loaded.extract_if(|h, _| !named.contains(&h.as_str())).map(|(_, image)| image));
        self.missing.retain(|h| named.contains(&h.as_str()));
    }

    /// Takes the images no longer named out of GPUI's image cache and the window's atlas.
    pub fn drop_retired(&mut self, window: &mut Window, cx: &mut App) {
        for image in self.retired.drain(..) {
            if let Some(drawn) = image.clone().get_render_image(window, cx) {
                cx.drop_image(drawn, Some(window));
            }
            image.remove_asset(cx);
        }
    }

    /// A new connection asks again for what the last one said was missing.
    pub fn reconnected(&mut self) {
        self.missing.clear();
        self.asked.clear();
    }

    fn of(&self, preferences: &Value, project: &str) -> Option<Arc<Image>> {
        let hash = preferences.get("projectIcon")?.get(project)?.as_str()?;
        self.loaded.get(hash).cloned()
    }
}

/// A project's glyph at `size`: its image where it has one, else its look's glyph in its hue, else in `ink`. Without
/// `hued` the hue gives way to `ink`, as a settled row's `grayscale` glyph does.
pub fn project_glyph(
    project: Option<&str>,
    preferences: &Value,
    images: &Images,
    size: Pixels,
    ink: Hsla,
    hued: bool,
    p: &Palette,
) -> AnyElement {
    if let Some(image) = project.and_then(|id| images.of(preferences, id)) {
        // `rounded-icon` on a `bg-foreground/4%` ground.
        return div()
            .flex_none()
            .size(size)
            .rounded(size * 0.22)
            .bg(p.foreground.opacity(0.04))
            .child(img(image).size(size).rounded(size * 0.22))
            .into_any_element();
    }
    let look = project.and_then(|id| preferences.get("projectLook")?.get(id));
    let icon = look.and_then(|l| l.get("icon")?.as_str()).map_or(Glyph::Folder, project_icon);
    let ink = look.and_then(|l| l.get("hue")?.as_str()).and_then(|h| hue(h, p)).filter(|_| hued).unwrap_or(ink);
    Icon::new(icon).with_size(size).text_color(ink).into_any_element()
}

/// Whether a project's look carries a hue, which keeps its glyph's ink when the row's ink changes.
pub fn has_hue(project: Option<&str>, preferences: &Value) -> bool {
    project.and_then(|id| preferences.get("projectLook")?.get(id)?.get("hue")?.as_str()).is_some_and(|h| h != "neutral")
}

/// A computer's glyph through `ui::computer_glyph`: the person's pick, else what the computer is, a cloud as its
/// provider's mark.
pub fn computer_glyph(place: &Value, preferences: &Value, size: Pixels, ink: Hsla) -> AnyElement {
    let Ok(view) = serde_json::from_value::<PlaceView>(place.clone()) else {
        return Icon::new(Glyph::Server).with_size(size).text_color(ink).into_any_element();
    };
    let look = preferences.get("computerLook").and_then(|l| l.get(&view.id)).cloned();
    let picked = look.and_then(|look| serde_json::from_value::<Preferences>(json!({ "computerLook": { view.id.clone(): look } })).ok());
    ComputerIcon::of(&view, picked.as_ref()).draw(ink, size)
}

/// A computer's name as the app says it (`place-name.ts` placeName): its label, else its name.
pub fn place_name(place: &Value) -> String {
    let name = place.get("name").and_then(Value::as_str).unwrap_or_default();
    match place.get("label").and_then(Value::as_str) {
        Some(label) => label.to_owned(),
        None if place.get("kind").and_then(Value::as_str) == Some("provider") => provider_key_name(name),
        None => name.to_owned(),
    }
}

/// `PROVIDER_KEY_WORDS`' names: each cloud's id and the name its key rows give it.
pub const PROVIDER_NAMES: [(&str, &str); 2] = [("box", "Boat"), ("solari", "Solari")];

/// `providerKeyName`: a provider by the name its key rows give it, an id with no row as itself.
pub fn provider_key_name(provider: &str) -> String {
    PROVIDER_NAMES.iter().find(|(id, _)| *id == provider).map_or(provider, |&(_, name)| name).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn images_ask_for_the_named_hashes_once_and_keep_only_those() {
        let prefs = json!({ "projectIcon": { "p1": "aa", "p2": "bb" } });
        let mut images = Images::default();
        let mut wanted = images.wanted(&prefs);
        wanted.sort();
        assert_eq!(wanted, ["aa", "bb"]);
        images.take(&json!({ "aa": null, "bb": null }), &prefs);
        assert!(images.wanted(&prefs).is_empty(), "a missing hash is not asked again on this connection");
        images.reconnected();
        assert_eq!(images.wanted(&prefs).len(), 2);
    }

    #[test]
    fn a_provider_row_goes_by_its_key_name_and_a_label_wins() {
        assert_eq!(place_name(&json!({ "kind": "provider", "name": "box" })), "Boat");
        assert_eq!(place_name(&json!({ "kind": "provider", "name": "none" })), "none");
        assert_eq!(place_name(&json!({ "kind": "place", "name": "box" })), "box");
        assert_eq!(place_name(&json!({ "kind": "provider", "name": "box", "label": "Work" })), "Work");
    }

    #[test]
    fn a_computer_goes_by_its_label_else_its_name() {
        assert_eq!(place_name(&json!({ "id": "x", "name": "boat", "label": "Boat" })), "Boat");
        assert_eq!(place_name(&json!({ "id": "x", "name": "boat" })), "boat");
    }
}
