// SPDX-License-Identifier: AGPL-3.0-only
//! The Projects step: the folders a person may import, each ticked one's name, icon and colour.

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::{h_flex, v_flex, Icon, Sizable as _};
use gpui_kit::{div, prelude::*, px, AnyElement, App, ClickEvent, Context, Hsla, PathPromptOptions, SharedString, Window};
use serde_json::{json, Value};

use super::super::model::{github_pick, str_of};
use super::super::{choices, words, AddComputer, Folder, FolderFields, List, HUES, ICONS};
use super::{icon_word, list, SizeCell};
use crate::screens::dialogs::picks::{pick_line, Changes, PickItem, PickList, Tick, RANGE_HOVER};
use crate::screens::sidebar::looks;
use crate::ui::card::CardItem;
use crate::ui::{size, text, ActivePalette, Button, Field, PickRow, Select, SelectItem, SelectState};

impl AddComputer {
    /// The folders a person may import: ones added by hand, ones the picks hold that are no project here, then every
    /// project here.
    fn folders(&self, cx: &App) -> Vec<Folder> {
        let picks = self.picks_now(cx);
        let facts = self.options.as_ref().map(|o| list(o, "folders").to_vec()).unwrap_or_default();
        let fact_of = |path: &str| facts.iter().find(|f| str_of(f, "path") == Some(path)).cloned().unwrap_or(json!({}));
        let here = self.here_projects();
        let looks = self.preferences.get("projectLook").cloned().unwrap_or(json!({}));
        let with_facts = |mut f: Folder| {
            let fact = fact_of(&f.path);
            f.bytes = fact.get("bytes").and_then(Value::as_u64);
            f.private = fact.get("private") == Some(&json!(true));
            f.unpushed = fact.get("unpushed").and_then(Value::as_u64);
            f
        };
        let mut out: Vec<Folder> = self.added.iter().cloned().map(with_facts).collect();
        if let Some(table) = picks.get("folders").and_then(Value::as_object) {
            for (key, row) in table {
                if here.iter().any(|p| choices::folder_key(str_of(p, "name").unwrap_or_default()) == *key)
                    || out.iter().any(|f| &f.key == key)
                {
                    continue;
                }
                out.push(with_facts(Folder {
                    key: key.clone(),
                    name: str_of(row, "name").unwrap_or(key).to_owned(),
                    path: str_of(row, "from").unwrap_or_default().to_owned(),
                    remote: None,
                    icon: str_of(row, "icon").unwrap_or("folder").to_owned(),
                    hue: str_of(row, "hue").unwrap_or("neutral").to_owned(),
                    bytes: None,
                    private: false,
                    unpushed: None,
                }));
            }
        }
        for p in here {
            let id = str_of(&p, "id").unwrap_or_default();
            out.push(with_facts(Folder {
                key: choices::folder_key(str_of(&p, "name").unwrap_or_default()),
                name: str_of(&p, "name").unwrap_or_default().to_owned(),
                path: str_of(&p, "path").unwrap_or_default().to_owned(),
                remote: Some(str_of(&p, "remote").unwrap_or_default().to_owned()),
                icon: looks.pointer(&format!("/{id}/icon")).and_then(Value::as_str).unwrap_or("folder").to_owned(),
                hue: looks.pointer(&format!("/{id}/hue")).and_then(Value::as_str).unwrap_or("neutral").to_owned(),
                bytes: None,
                private: false,
                unpushed: None,
            }));
        }
        out
    }

    /// The fields a ticked folder opens, made on first draw and kept by its key.
    fn folder_fields(&mut self, key: &str, name: &str, icon: &str, hue: &str, window: &mut Window, cx: &mut Context<Self>) {
        if !self.folders.contains_key(key) {
            let field = cx.new(|cx| InputState::new(window, cx).default_value(name.to_owned()));
            let icons = ICONS.iter().map(|i| SelectItem::new(i.to_string(), icon_word(i)).glyph(looks::project_icon(i))).collect();
            let icon_state = cx.new(|_| SelectState::new(icons, Some(&icon.to_owned())));
            let hues = HUES
                .iter()
                .map(|h| {
                    let h2 = h.to_string();
                    SelectItem::new(h.to_string(), icon_word(h))
                        .mark(move |cx| div().size(px(12.)).rounded_full().bg(hue_ink(&h2, cx)).into_any_element())
                })
                .collect();
            let hue_state = cx.new(|_| SelectState::new(hues, Some(&hue.to_owned())));
            let k = key.to_owned();
            let sub = cx.subscribe(&field, move |this, field, e: &InputEvent, cx| {
                if matches!(e, InputEvent::Change) {
                    let name = field.read(cx).value().to_string();
                    this.put_folder(&k, |row| row["name"] = json!(name), cx);
                }
            });
            self.folders.insert(key.to_owned(), FolderFields { name: field, icon: icon_state, hue: hue_state, _subs: vec![sub] });
        }
    }

    /// One field of a ticked folder's row changed.
    fn put_folder(&mut self, key: &str, change: impl FnOnce(&mut Value), cx: &mut Context<Self>) {
        let Some(picks) = self.picks.clone() else { return };
        let Some(mut row) = picks.pointer(&format!("/folders/{key}")).cloned() else { return };
        change(&mut row);
        self.set_picks(choices::tick_folder(&picks, key, Some(row)), None, cx);
    }

    pub(super) fn projects_body(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let picks = self.picks_now(cx);
        let name = self.box_name(cx);
        let no_github = github_pick(&picks) == "skip";
        let folders = self.folders(cx);
        let on_box: Vec<String> = self
            .projects
            .iter()
            .filter(|p| str_of(p, "computer") == self.place_id.as_deref())
            .filter_map(|p| str_of(p, "name").map(str::to_owned))
            .collect();
        let row_of = |f: &Folder| -> Value {
            let held = picks.pointer(&format!("/folders/{}", f.key));
            let mut row = json!({
                "from": f.path,
                "name": held.and_then(|h| str_of(h, "name")).unwrap_or(&f.name),
                "icon": held.and_then(|h| str_of(h, "icon")).unwrap_or(&f.icon),
                "hue": held.and_then(|h| str_of(h, "hue")).unwrap_or(&f.hue),
                "keep": held.and_then(|h| h.get("keep")).cloned().unwrap_or(json!([])),
            });
            if let Some(image) = held.and_then(|h| h.get("image")).filter(|i| !i.is_null()) {
                row["image"] = image.clone();
            }
            row
        };
        let rows: Vec<(Folder, Value, bool)> =
            folders.iter().map(|f| (f.clone(), row_of(f), choices::has(&picks, "folders", &f.key))).collect();
        for (f, row, on) in &rows {
            if *on {
                self.folder_fields(
                    &f.key,
                    str_of(row, "name").unwrap_or_default(),
                    str_of(row, "icon").unwrap_or("folder"),
                    str_of(row, "hue").unwrap_or("neutral"),
                    window,
                    cx,
                );
            }
        }
        let items: Vec<PickItem> =
            rows.iter().map(|(f, row, on)| PickItem::new(f.key.clone(), str_of(row, "name").unwrap_or_default().to_owned(), *on)).collect();
        let fields: std::collections::HashMap<String, Fields> =
            self.folders.iter().map(|(k, f)| (k.clone(), (f.name.clone(), f.icon.clone(), f.hue.clone()))).collect();
        let weak = cx.entity().downgrade();
        let row = {
            let rows = rows.clone();
            move |item: &PickItem, tick: Tick| -> Vec<CardItem> {
                let Some((f, row, on)) = rows.iter().find(|(f, _, _)| f.key.as_str() == item.key().as_ref()).cloned() else {
                    return Vec::new();
                };
                let shown = str_of(&row, "name").unwrap_or_default().to_owned();
                let clash = on && on_box.contains(&shown);
                let note = if on && no_github && f.private {
                    Some(words::NEEDS_GITHUB.to_owned())
                } else {
                    f.remote.as_deref().map(|r| if r.is_empty() { words::NO_REMOTE.to_owned() } else { words::remote_line(r, f.unpushed) })
                };
                let icon = str_of(&row, "icon").unwrap_or("folder").to_owned();
                let hue = str_of(&row, "hue").unwrap_or("neutral").to_owned();
                let mut pick = PickRow::new(SharedString::from(format!("folder-{}", f.key)), shown.clone(), on)
                    .framed(HuedGlyph { icon: icon.clone(), hue: hue.clone() })
                    .tag(f.path.clone())
                    .on_click(tick.press())
                    .hover(RANGE_HOVER)
                    .locked(item.is_locked());
                if let Some(note) = note {
                    pick = pick.note(note);
                }
                if let Some(size) = f.bytes {
                    pick = pick.slot(SizeCell(size));
                }
                let mut out: Vec<CardItem> = vec![pick.into()];
                if let (true, Some((field, icon_state, hue_state))) = (on, fields.get(&f.key)) {
                    let taken = clash.then(|| format!("{name} already has a project called {shown}."));
                    out.push(pick_line(
                        format!("{}-name", f.key),
                        "Name",
                        v_flex().items_end().gap_1().child(Field::new(field).invalid(clash)).children(taken.map(TakenLine)),
                    ));
                    let (k1, k2, w1, w2) = (f.key.clone(), f.key.clone(), weak.clone(), weak.clone());
                    out.push(pick_line(
                        format!("{}-icon", f.key),
                        "Icon",
                        Select::new(icon_state).on_change(move |next: &String, _, cx| {
                            let next = next.clone();
                            let k = k1.clone();
                            let _ = w1.update(cx, |this, cx| {
                                this.put_folder(
                                    &k,
                                    |row| {
                                        row["icon"] = json!(next);
                                        if let Some(o) = row.as_object_mut() {
                                            o.remove("image");
                                        }
                                    },
                                    cx,
                                )
                            });
                        }),
                    ));
                    out.push(pick_line(
                        format!("{}-hue", f.key),
                        "Colour",
                        Select::new(hue_state).on_change(move |next: &String, _, cx| {
                            let next = next.clone();
                            let k = k2.clone();
                            let _ = w2.update(cx, |this, cx| this.put_folder(&k, |row| row["hue"] = json!(next), cx));
                        }),
                    ));
                }
                out
            }
        };
        let put = {
            let rows = rows.clone();
            move |picks: &Value, _: &Value, changes: &Changes| -> Value {
                changes.iter().fold(picks.clone(), |next, (key, on)| match rows.iter().find(|(f, _, _)| f.key.as_str() == key.as_ref()) {
                    Some((f, row, _)) => choices::tick_folder(&next, &f.key, on.then(|| row.clone())),
                    None => next,
                })
            }
        };
        let state = self.lists[&List::Projects].clone();
        let weak = cx.entity().downgrade();
        let set = move |changes: Changes, _: &mut Window, cx: &mut App| {
            let _ = weak.update(cx, |this, cx| {
                let picks = this.picks_now(cx);
                this.set_picks(put(&picks, &Value::Null, &changes), None, cx);
            });
        };
        vec![
            PickList::new("projects", &state, items, set, row).into_any_element(),
            h_flex()
                .child(Button::add("add-folder", "Add a folder…").on_click(cx.listener(|_, _: &ClickEvent, _, cx| {
                    let picked = cx.prompt_for_paths(PathPromptOptions { files: false, directories: true, multiple: false, prompt: None });
                    cx.spawn(async move |this, cx| {
                        let Ok(Ok(Some(paths))) = picked.await else { return };
                        let Some(path) = paths.into_iter().next() else { return };
                        let _ = this.update(cx, |this, cx| this.add_folder(&path.to_string_lossy(), cx));
                    })
                    .detach();
                })))
                .into_any_element(),
        ]
    }

    /// A folder picked by hand: it stands at the top, ticked.
    pub fn add_folder(&mut self, path: &str, cx: &mut Context<Self>) {
        let name = path.split('/').rfind(|s| !s.is_empty()).unwrap_or(path).to_owned();
        let folder = Folder {
            key: choices::folder_key(&name),
            name: name.clone(),
            path: path.to_owned(),
            remote: None,
            icon: "folder".into(),
            hue: "neutral".into(),
            bytes: None,
            private: false,
            unpushed: None,
        };
        self.added.retain(|f| f.key != folder.key);
        self.added.insert(0, folder.clone());
        let picks = self.picks_now(cx);
        let row = json!({ "from": path, "name": name, "icon": "folder", "hue": "neutral", "keep": [] });
        self.set_picks(choices::tick_folder(&picks, &folder.key, Some(row)), None, cx);
    }
}

/// A project hue's ink, the muted one for none (`PROJECT_HUES`).
fn hue_ink(hue: &str, cx: &App) -> Hsla {
    looks::hue(hue, cx.palette()).unwrap_or(cx.palette().muted_foreground)
}

/// A ticked folder's name field and its icon and colour selects.
type Fields = (gpui_kit::Entity<InputState>, gpui_kit::Entity<SelectState<String>>, gpui_kit::Entity<SelectState<String>>);

/// "X already has a project called Y." under the name field, in the refusal's ink.
#[derive(IntoElement)]
struct TakenLine(String);

impl RenderOnce for TakenLine {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .debug_selector(|| "name-taken".into())
            .text_size(text::FACT.0)
            .line_height(px(18.))
            .text_color(cx.palette().error_foreground)
            .child(self.0)
    }
}

/// A folder's glyph in its hue, at 16 px.
#[derive(IntoElement)]
struct HuedGlyph {
    icon: String,
    hue: String,
}

impl RenderOnce for HuedGlyph {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let ink = if self.hue == "neutral" { cx.palette().foreground.opacity(0.8) } else { hue_ink(&self.hue, cx) };
        Icon::new(looks::project_icon(&self.icon)).with_size(size::GLYPH).text_color(ink)
    }
}
