// SPDX-License-Identifier: AGPL-3.0-only
//! The identity re-asks, the person's computers, recipes, the account, devices and tickets. serve.ts:396-418,
//! 863-868, 880-1169. Spec: spec/serve_places.md.
use std::rc::Rc;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use indexmap::IndexMap;
use rand::rngs::OsRng;
use rand::RngCore;
use serde::Serialize;
use serde_json::{json, Map, Value};

use wsp_core::types::wiring::{RecipeShelf, ShelvedRecipe};
use wsp_machines::places::types::{AddAsk, PlaceAdded, PlaceDoor, PlaceSetUp, RemoveAsk, SetUpAsk, NO_PLACE_DOOR};
use wsp_wire::types::place_word::HERE_PLACE_ID;
use wsp_wire::types::recipe_file::{RecipeView, NO_RECIPE, NO_RECIPES, RECIPES_TICKET_REFUSAL};
use wsp_wire::views::place::PlaceSetAlso;
use wsp_wire::views::place_kind::PlaceKind;
use wsp_wire::views::place_more::{PlaceSettingsAsk, PlaceSettingsSize, PlaceSettingsSpawn, PlaceView};
use wsp_wire::views::workspace::Caller;
use wsp_wire::wire::devices::{
    join_token, ACCOUNT_TICKET_REFUSAL, ACCOUNT_UNSERVED, DEVICES_TICKET_REFUSAL, DEVICE_AUTH_REFUSAL, PAIR_CODE_REFUSAL,
    PAIR_ISSUE_REFUSAL,
};
use wsp_wire::wire::machine_link::{is_joined_computer, no_such_place_refusal};
use wsp_wire::wire::place_link::{
    PlaceDoorView, MINT_JOIN_REFUSAL, PLACES_TICKET_REFUSAL, PLACE_DOOR_REFUSAL, PLACE_DOOR_UNSERVED, PLACE_UNKNOWN_REFUSAL,
    SSH_HOSTS_REFUSAL,
};
use wsp_wire::wire::runtime_ops::{RuntimeOp, RuntimeRequest};
use wsp_words::exit::{HostError, Kind};
use wsp_words::js_date::iso_string;
use wsp_words::place_name::table_name;
use wsp_words::place_word::PlaceRef;
use wsp_words::recipe_file::{no_picks_refusal, recipe_from_here_refusal, recipe_summary};
use wsp_words::words::computer::{RELAY_TICKET_REFUSAL, UNAUTHORIZED};
use wsp_words::words::places::join_roads;
use wsp_words::words::units::{doctor_row_refusal, doctor_running_line, DOCTOR_UNSERVED};

use super::answer::{ok_reply, refusal_reply, Handled};
use super::door::{Served, Ticket};
use super::served::Payload;
use super::socket::Socket;
use crate::serve::Released;
use crate::serve_projects::{thread_place_rows, thread_places};

/// Answers the ops of this unit and sends their replies; `NotMine` for any other op. An error is the switch's to
/// send (spec O2).
pub async fn answer_places(
    socket: &Rc<Socket>,
    frame: &serde_json::Value,
    msg: &RuntimeRequest,
    origin: Option<Caller>,
) -> Result<Handled, HostError> {
    let served = socket.served.clone();
    let road = socket.road();
    let id = serde_json::to_value(msg.op.id()).unwrap_or(Value::Null);
    if let Some(error) = ticket_gate(&msg.op) {
        if !road.own_road() {
            socket.answer(Some(frame), refusal_reply(&id, error, Some("ticket")), None);
            return Ok(Handled::Answered);
        }
    }
    let reply = match &msg.op {
        RuntimeOp::Auth { .. } => ok_reply(&id, released_fields(&served.released)),
        RuntimeOp::PairIssue { here, .. } => {
            if !road.host_only() {
                refusal_reply(&id, PAIR_ISSUE_REFUSAL, None)
            } else {
                let host_key = (*served.rt).places().map(|places| places.host_key());
                let issued = served.issue_code(here.is_some()).await?;
                let mut payload = fields([("code", json!(issued.code)), ("expiresAt", json!(issued.expires_at))]);
                if let Some(host_key) = host_key {
                    payload.insert("hostKey".into(), json!(host_key));
                }
                ok_reply(&id, payload)
            }
        }
        RuntimeOp::PairRedeem { .. } => refusal_reply(&id, PAIR_CODE_REFUSAL, None),
        RuntimeOp::DeviceAuth(_) => refusal_reply(&id, DEVICE_AUTH_REFUSAL, None),
        RuntimeOp::SealOpen(_) => refusal_reply(&id, UNAUTHORIZED, None),
        RuntimeOp::PlaceJoin(_) | RuntimeOp::PlaceAuth(_) | RuntimeOp::PlaceProve(_) => refusal_reply(&id, PLACE_UNKNOWN_REFUSAL, None),
        RuntimeOp::PlacesList { .. } => {
            if road.by.is_some() {
                let places = thread_places(&*served.rt, origin, served.now()).await?;
                ok_reply(&id, fields([("places", thread_place_rows(&places)?), ("adds", json!([])), ("pending", json!([]))]))
            } else if !road.own_road() {
                refusal_reply(&id, PLACES_TICKET_REFUSAL, Some("ticket"))
            } else {
                let door = places_of(&served)?;
                let places = door.list(unsigned(served.now())).await?;
                let adds = door.adds();
                let pending = door.pending().await?;
                ok_reply(&id, fields([("places", to_value(&places)?), ("adds", to_value(&adds)?), ("pending", to_value(&pending)?)]))
            }
        }
        RuntimeOp::PlacesUpdate { place_id, sudo_password, .. } => {
            let update = places_of(&served)?.update(place_id, sudo_ask(sudo_password, None, None)).await?;
            ok_reply(&id, spread(&update)?)
        }
        RuntimeOp::PlacesRemove { place_id, sudo_password, force, forget, .. } => {
            let ask = sudo_ask(sudo_password, force.filter(|set| *set), forget.filter(|set| *set));
            let removed = places_of(&served)?.remove(place_id, ask).await?;
            ok_reply(&id, spread(&removed)?)
        }
        RuntimeOp::PlacesHolds { place_id, .. } => {
            let holds = places_of(&served)?.holds(place_id).await?;
            ok_reply(&id, spread(&holds)?)
        }
        RuntimeOp::PlacesDial { place_id, .. } => {
            let dial = places_of(&served)?.dial(place_id, unsigned(served.now())).await?;
            ok_reply(&id, spread(&dial)?)
        }
        RuntimeOp::PlacesSet {
            place_id,
            reset,
            name,
            ssh,
            recipe,
            threads,
            machines,
            spend_per_day_usd,
            nap_ms,
            turn_limit_ms,
            spawn,
            size,
            delete_after_ms,
            ..
        } => {
            let door = places_of(&served)?;
            let recipe = match recipe {
                Some(word) if word != NO_RECIPE => Some(recipe_named(&served, word).await?.slug),
                other => other.clone(),
            };
            let set = PlaceSettingsAsk {
                threads: *threads,
                machines: *machines,
                spend_per_day_usd: *spend_per_day_usd,
                nap_ms: *nap_ms,
                turn_limit_ms: *turn_limit_ms,
                spawn: spawn.as_ref().map(|spawn| PlaceSettingsSpawn { max_machines: spawn.max_machines, max_depth: spawn.max_depth }),
                size: size.as_ref().map(|size| PlaceSettingsSize { cpu: size.cpu, mem_mb: size.mem_mb }),
                delete_after_ms: *delete_after_ms,
            };
            let also = PlaceSetAlso { name: name.clone(), ssh: ssh.clone(), recipe };
            let view = door.set(place_id, set, reset.clone().unwrap_or_default(), also).await?;
            ok_reply(&id, fields([("place", to_value(&view)?)]))
        }
        RuntimeOp::PlacesFollow { place_id, recipe, .. } => {
            let slug = if recipe == NO_RECIPE { recipe.clone() } else { recipe_named(&served, recipe).await?.slug };
            let place = places_of(&served)?.follow(place_id, &slug).await?;
            ok_reply(&id, fields([("place", to_value(&place)?)]))
        }
        RuntimeOp::PlacesSkip { place_id, row, .. } => {
            let place = places_of(&served)?.skip(place_id, row).await?;
            ok_reply(&id, fields([("place", to_value(&place)?)]))
        }
        RuntimeOp::PlacesEstimate { r#ref, choices, .. } => {
            let estimate = places_of(&served)?.estimate(r#ref, choices.clone()).await?;
            ok_reply(&id, fields([("estimate", to_value(&estimate)?)]))
        }
        RuntimeOp::PlacesSetupLog { place_id, step, .. } => {
            let lines = places_of(&served)?.setup_log(place_id, *step).await?;
            ok_reply(&id, fields([("lines", to_value(&lines)?)]))
        }
        RuntimeOp::PlacesLoginLanded { place_id, agent, .. } => {
            places_of(&served)?.login_landed(place_id, agent).await?;
            ok_reply(&id, Payload::new())
        }
        RuntimeOp::PlacesDoctor { place_id, doctor_id, project, .. } => {
            let Some(doctor) = served.opts.doctor.clone() else {
                return send_refusal(socket, frame, &id, DOCTOR_UNSERVED, None);
            };
            let door = places_of(&served)?;
            let rows = door.list(unsigned(served.now())).await?;
            let Some(row) = rows.iter().find(|place| place.id == *place_id).cloned() else {
                let held: Vec<String> = rows.iter().map(table_name_of).collect();
                return send_refusal(socket, frame, &id, &no_such_place_refusal(place_id, &held), Some("usage"));
            };
            if !is_joined_computer(&row.id, kind_word(&row.kind)) {
                return send_refusal(socket, frame, &id, &doctor_row_refusal(&table_name_of(&row)), Some("usage"));
            }
            if served.doctoring.borrow().contains(&row.id) {
                return send_refusal(socket, frame, &id, &doctor_running_line(&row.name), Some("conflict"));
            }
            served.doctoring.borrow_mut().insert(row.id.clone());
            let ran = doctor.run(&row.id, doctor_id, project.as_deref()).await;
            served.doctoring.borrow_mut().remove(&row.id);
            let code = ran?;
            ok_reply(&id, fields([("code", json!(code))]))
        }
        RuntimeOp::PlacesDoor { .. } => {
            if !road.own_road() {
                refusal_reply(&id, PLACE_DOOR_REFUSAL, None)
            } else {
                let Some(control) = served.opts.door.clone() else {
                    return send_refusal(socket, frame, &id, PLACE_DOOR_UNSERVED, None);
                };
                let opened = control.open().await?;
                let host_key = places_of(&served)?.host_key();
                ok_reply(&id, fields([("door", Value::Object(door_payload(&opened.view, host_key)?))]))
            }
        }
        RuntimeOp::PlacesMint { .. } => {
            if !road.host_only() {
                refusal_reply(&id, MINT_JOIN_REFUSAL, None)
            } else {
                let Some(control) = served.opts.door.clone() else {
                    return send_refusal(socket, frame, &id, PLACE_DOOR_UNSERVED, None);
                };
                let host_key = places_of(&served)?.host_key();
                let door = control.open().await?;
                let issued = served.issue_code(false).await?;
                let token = join_token(&issued.code, &host_key);
                let joins = join_roads(&token, door.view.addresses.as_slice(), door.view.relay.as_deref());
                let expires_at = iso_string(issued.expires_at as f64).ok_or_else(|| HostError::new("Invalid time value"))?;
                ok_reply(&id, fields([("joins", to_value(&joins)?), ("expiresAt", Value::from(expires_at))]))
            }
        }
        RuntimeOp::PlacesSshHosts { .. } => {
            if !road.host_only() {
                refusal_reply(&id, SSH_HOSTS_REFUSAL, None)
            } else {
                let rows = match (*served.rt).places() {
                    Some(places) => places.list(unsigned(served.now())).await?,
                    None => Vec::new(),
                };
                let ssh_hosts = served.opts.ssh_hosts.clone();
                let hosts = match ssh_hosts {
                    Some(ssh_hosts) => ssh_hosts(rows).await?,
                    None => Vec::new(),
                };
                ok_reply(&id, fields([("hosts", to_value(&hosts)?)]))
            }
        }
        RuntimeOp::PlacesAdd { add_id, address, name, ssh_port, key_path, host_key, sudo_password, recipe, .. } => {
            let Some(control) = served.opts.door.clone() else {
                return send_refusal(socket, frame, &id, PLACE_DOOR_UNSERVED, None);
            };
            let picked = match recipe {
                Some(word) => Some(recipe_named(&served, word).await?),
                None => None,
            };
            let at = control.open().await?;
            let mut host_urls = at.view.addresses.clone();
            host_urls.extend(at.view.relay.clone());
            let added = places_of(&served)?
                .add(
                    AddAsk {
                        add_id: add_id.clone(),
                        address: address.clone(),
                        name: name.clone(),
                        ssh_port: ssh_port.and_then(|port| u16::try_from(port.get()).ok()),
                        key_path: key_path.clone(),
                        host_key: host_key.clone(),
                        sudo_password: sudo_password.as_ref().map(|sudo| sudo.0.clone()),
                        host_urls,
                        door_port: at.back_port,
                        relay: at.view.relay.clone(),
                        choices: picked.as_ref().map(|picked| picked.file.clone()),
                        recipe: picked.as_ref().map(|picked| picked.slug.clone()),
                    },
                    unsigned(served.now()),
                )
                .await?;
            ok_reply(&id, added_fields(&added)?)
        }
        RuntimeOp::PlacesSetup { r#ref, recipe, choices, add_id, row, .. } => {
            let picked = match recipe {
                Some(word) => Some(recipe_named(&served, word).await?),
                None => None,
            };
            let (choices, recipe) = match picked {
                Some(picked) => (Some(picked.file), Some(picked.slug)),
                None => (choices.clone(), None),
            };
            let ask = SetUpAsk { choices, recipe, add_id: add_id.clone(), row: row.clone() };
            let set_up = places_of(&served)?.set_up(r#ref, ask).await?;
            ok_reply(&id, set_up_fields(&set_up)?)
        }
        RuntimeOp::PlacesChoose { r#ref, choices, recipe, .. } => {
            let pending = places_of(&served)?.choose(r#ref, choices.clone(), recipe.clone()).await?;
            ok_reply(&id, fields([("pending", to_value(&pending)?)]))
        }
        RuntimeOp::RecipesList { .. } => {
            let by = followers_of(&served).await?;
            let held = shelf_of(&served)?.list().await?;
            let recipes = held.iter().map(|recipe| recipe_view(recipe, &by)).collect::<Result<Vec<Value>, HostError>>()?;
            ok_reply(&id, fields([("recipes", Value::Array(recipes))]))
        }
        RuntimeOp::RecipesGet { name, .. } => {
            let (held, hash) = shelf_of(&served)?.get(name).await?;
            let by = followers_of(&served).await?;
            ok_reply(&id, fields([("recipe", recipe_view(&held, &by)?), ("hash", Value::from(hash))]))
        }
        RuntimeOp::RecipesSave { name, file, from, .. } => {
            let place = match from {
                Some(word) => Some((word, places_of(&served)?.place_for(word).await?.place_id)),
                None => None,
            };
            if let Some((word, None)) = &place {
                return Err(usage(recipe_from_here_refusal(word)));
            }
            let place_id = place.as_ref().and_then(|(_, place_id)| place_id.clone());
            let picks = match &place_id {
                Some(place_id) => places_of(&served)?.picks_of(place_id).await?,
                None => None,
            };
            if let (Some((word, _)), None) = (&place, &picks) {
                return Err(usage(no_picks_refusal(word)));
            }
            let base = match (&picks, file) {
                (Some(picks), _) => to_value(picks)?,
                (None, Some(Some(kept))) => kept.0.clone(),
                (None, _) => Value::Object(Map::new()),
            };
            let mut file = match base {
                Value::Object(fields) => fields,
                _ => Map::new(),
            };
            file.insert("name".into(), Value::from(name.as_str()));
            let shelf = shelf_of(&served)?;
            let held = shelf.save(Value::Object(file)).await?;
            match &place_id {
                Some(place_id) => {
                    places_of(&served)?.follow(place_id, &held.slug).await?;
                }
                None => {
                    if let Some(door) = (*served.rt).places() {
                        door.recipe_changed(&held.slug, 0).await?;
                    }
                }
            }
            let by = followers_of(&served).await?;
            ok_reply(&id, fields([("recipe", recipe_view(&held, &by)?)]))
        }
        RuntimeOp::RecipesRemove { name, check, .. } => {
            let by = followers_of(&served).await?;
            let shelf = shelf_of(&served)?;
            let checking = *check == Some(true);
            let held = if checking { shelf.read(name).await? } else { shelf.remove(name).await? };
            if !checking {
                if let Some(door) = (*served.rt).places() {
                    door.unfollow(&held.slug).await?;
                }
            }
            ok_reply(&id, fields([("recipe", recipe_view(&held, &by)?)]))
        }
        RuntimeOp::RecipesOptions { .. } => {
            let here: Vec<(String, String)> = served
                .rt
                .projects()
                .list(None)
                .await?
                .into_iter()
                .filter(|project| project.computer == HERE_PLACE_ID)
                .map(|project| (project.name, project.path))
                .collect();
            let options = shelf_of(&served)?.options(Some(here)).await?;
            ok_reply(&id, fields([("options", options)]))
        }
        RuntimeOp::AccountGet { .. } => {
            let Some(account) = served.opts.account.clone() else {
                return send_refusal(socket, frame, &id, ACCOUNT_UNSERVED, None);
            };
            let view = account.read().await?;
            ok_reply(&id, fields([("account", to_value(&view)?)]))
        }
        RuntimeOp::DevicesList { .. } => {
            let devices = served.devices()?.list().await?;
            ok_reply(&id, fields([("devices", to_value(&devices)?)]))
        }
        RuntimeOp::DevicesRevoke { device_id, .. } => {
            let answer_socket = socket.clone();
            let answer_id = id.clone();
            served
                .revoke_device(
                    device_id,
                    Some(Box::new(move |revoked: bool| {
                        answer_socket.send(ok_reply(&answer_id, fields([("revoked", json!(revoked))])));
                    })),
                )
                .await?;
            return Ok(Handled::Answered);
        }
        RuntimeOp::TicketIssue { purpose, .. } => {
            let mut bytes = [0u8; 24];
            OsRng.fill_bytes(&mut bytes);
            let ticket = URL_SAFE_NO_PAD.encode(bytes);
            let ttl = i64::try_from(served.ticket_ttl_ms).unwrap_or(i64::MAX);
            let expires_at = served.now().saturating_add(ttl);
            let held = Ticket { purpose: *purpose, expires_at, paired: road.paired_road };
            served.tickets.borrow_mut().insert(ticket.clone(), held);
            ok_reply(&id, fields([("ticket", Value::from(ticket)), ("expiresAt", json!(expires_at))]))
        }
        _ => return Ok(Handled::NotMine),
    };
    socket.answer(Some(frame), reply, None);
    Ok(Handled::Answered)
}

/// `refusedOffOwnRoad()`: a socket off the person's own road is sent the ticket refusal and the op runs nothing
/// (serve.ts:864-868). Answers whether it refused.
pub fn refused_off_own_road(socket: &Rc<Socket>, frame: &serde_json::Value, msg: &RuntimeRequest) -> bool {
    if socket.road().own_road() {
        return false;
    }
    let id = serde_json::to_value(msg.op.id()).unwrap_or(Value::Null);
    socket.answer(Some(frame), refusal_reply(&id, PLACES_TICKET_REFUSAL, Some("ticket")), None);
    true
}

/// The refusal an op asked off the person's own road is sent with, where it has one (serve.ts:864-868, 1090-1158).
/// The ops that gate by their own road or host check (the listing, the door, the mint, the ssh hosts, the pairing)
/// answer in their arms.
fn ticket_gate(op: &RuntimeOp) -> Option<&'static str> {
    Some(match op {
        RuntimeOp::PlacesUpdate { .. }
        | RuntimeOp::PlacesRemove { .. }
        | RuntimeOp::PlacesHolds { .. }
        | RuntimeOp::PlacesDial { .. }
        | RuntimeOp::PlacesSet { .. }
        | RuntimeOp::PlacesFollow { .. }
        | RuntimeOp::PlacesSkip { .. }
        | RuntimeOp::PlacesEstimate { .. }
        | RuntimeOp::PlacesSetupLog { .. }
        | RuntimeOp::PlacesLoginLanded { .. }
        | RuntimeOp::PlacesDoctor { .. }
        | RuntimeOp::PlacesAdd { .. }
        | RuntimeOp::PlacesSetup { .. }
        | RuntimeOp::PlacesChoose { .. } => PLACES_TICKET_REFUSAL,
        RuntimeOp::RecipesList { .. }
        | RuntimeOp::RecipesGet { .. }
        | RuntimeOp::RecipesSave { .. }
        | RuntimeOp::RecipesRemove { .. }
        | RuntimeOp::RecipesOptions { .. } => RECIPES_TICKET_REFUSAL,
        RuntimeOp::AccountGet { .. } => ACCOUNT_TICKET_REFUSAL,
        RuntimeOp::DevicesList { .. } | RuntimeOp::DevicesRevoke { .. } => DEVICES_TICKET_REFUSAL,
        RuntimeOp::TicketIssue { .. } => RELAY_TICKET_REFUSAL,
        _ => return None,
    })
}

/// Sends a refusal the arm answers with, and says the frame was answered.
fn send_refusal(socket: &Rc<Socket>, frame: &Value, id: &Value, error: &str, kind: Option<&str>) -> Result<Handled, HostError> {
    socket.answer(Some(frame), refusal_reply(id, error, kind), None);
    Ok(Handled::Answered)
}

fn usage(message: String) -> HostError {
    HostError::with_kind(message, Kind::Usage)
}

/// `...released`: `version`, then `road`, each only where given (serve.ts:825-847).
fn released_fields(released: &Released) -> Payload {
    let mut fields = Payload::new();
    if let Some(version) = &released.version {
        fields.insert("version".into(), Value::from(version.as_str()));
    }
    if let Some(road) = &released.road {
        fields.insert("road".into(), Value::from(road.as_str()));
    }
    fields
}

/// The door the runtime serves, or the refusal naming there is none (serve.ts:396-397).
fn places_of(served: &Served) -> Result<Rc<dyn PlaceDoor>, HostError> {
    (*served.rt).places().ok_or_else(|| HostError::new(NO_PLACE_DOOR))
}

/// The shelf the host or the runtime wires, or the refusal naming there is none (serve.ts:399-402).
fn shelf_of(served: &Served) -> Result<Rc<dyn RecipeShelf>, HostError> {
    served.opts.recipes.clone().or_else(|| (*served.rt).recipes()).ok_or_else(|| HostError::new(NO_RECIPES))
}

/// `recipeNamed(word)`: the saved recipe a word names (serve.ts:404-406).
async fn recipe_named(served: &Served, word: &str) -> Result<ShelvedRecipe, HostError> {
    shelf_of(served)?.read(word).await
}

/// `followers()`: the computers that follow each recipe, none on a runtime that holds no places (serve.ts:413-414).
async fn followers_of(served: &Served) -> Result<IndexMap<String, Vec<String>>, HostError> {
    match (*served.rt).places() {
        Some(places) => places.followers().await,
        None => Ok(IndexMap::new()),
    }
}

/// `recipeView(held, followers)`: one recipe as a client reads it (serve.ts:408-411).
fn recipe_view(held: &ShelvedRecipe, followers: &IndexMap<String, Vec<String>>) -> Result<Value, HostError> {
    to_value(&RecipeView {
        name: held.file.name.clone(),
        slug: held.slug.clone(),
        summary: recipe_summary(&held.file),
        machines: followers.get(&held.slug).cloned().unwrap_or_default(),
        saved_at: held.saved_at.clone(),
        file: held.file.clone(),
    })
}

/// `sudoPassword`, `force` and `forget` as `RemoveAsk` takes them, each only where sent (serve.ts:923-931).
fn sudo_ask(sudo_password: &Option<wsp_wire::views::place::SudoPassword>, force: Option<bool>, forget: Option<bool>) -> RemoveAsk {
    RemoveAsk { sudo_password: sudo_password.as_ref().map(|sudo| sudo.0.clone()), force, forget }
}

/// `door` as the reply writes it: the view's own fields, then `hostKey` last (serve.ts:1008-1022).
fn door_payload(view: &PlaceDoorView, host_key: String) -> Result<Map<String, Value>, HostError> {
    let mut door: Payload = spread(view)?.into_iter().filter(|(key, _)| key.as_str() != "hostKey").collect();
    door.insert("hostKey".into(), Value::from(host_key));
    Ok(door)
}

/// `{addId, place, hostKey?, said?, pending?}`: what an install answers (types.ts:793-799).
fn added_fields(added: &PlaceAdded) -> Result<Payload, HostError> {
    let mut out = fields([("addId", json!(added.add_id)), ("place", to_value(&added.place)?)]);
    if let Some(host_key) = &added.host_key {
        out.insert("hostKey".into(), json!(host_key));
    }
    if let Some(said) = &added.said {
        out.insert("said".into(), json!(said));
    }
    if let Some(pending) = &added.pending {
        out.insert("pending".into(), to_value(pending)?);
    }
    Ok(out)
}

/// `{addId, place, setup?, said?}`: what a set up answers (types.ts:802-807).
fn set_up_fields(set_up: &PlaceSetUp) -> Result<Payload, HostError> {
    let mut out = fields([("addId", json!(set_up.add_id)), ("place", to_value(&set_up.place)?)]);
    if let Some(setup) = &set_up.setup {
        out.insert("setup".into(), to_value(setup)?);
    }
    if let Some(said) = &set_up.said {
        out.insert("said".into(), json!(said));
    }
    Ok(out)
}

fn kind_word(kind: &PlaceKind) -> &'static str {
    match kind {
        PlaceKind::Computer => "computer",
        PlaceKind::Provider => "provider",
    }
}

/// `tableName(place)`: what a table calls a computer (place-name.ts:35).
fn table_name_of(place: &PlaceView) -> String {
    table_name(&PlaceRef { id: place.id.clone(), name: place.name.clone(), kind: Some(kind_word(&place.kind).to_string()) })
}

fn unsigned(ms: i64) -> u64 {
    u64::try_from(ms).unwrap_or(0)
}

fn fields<const N: usize>(pairs: [(&str, Value); N]) -> Payload {
    pairs.into_iter().map(|(key, value)| (key.to_string(), value)).collect()
}

fn to_value<T: Serialize>(answer: &T) -> Result<Value, HostError> {
    serde_json::to_value(answer).map_err(|e| HostError::new(e.to_string()))
}

/// The fields of an answer spread beside id and ok, in the answer's own order.
fn spread<T: Serialize>(answer: &T) -> Result<Payload, HostError> {
    match to_value(answer)? {
        Value::Object(fields) => Ok(fields),
        _ => Err(HostError::new("the answer is not an object")),
    }
}
