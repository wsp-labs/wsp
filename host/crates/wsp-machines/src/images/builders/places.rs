// SPDX-License-Identifier: AGPL-3.0-only
//! builders.ts:57-78, 190-281 at 65f6b89c7: where an image is built, and how a builder is shown. Spec:
//! `spec/builders_places.md`.
use std::sync::Arc;

use indexmap::IndexMap;

use wsp_core::types::internal::{BuilderLife, BuilderRecord, NO_COPY_RECIPE};
use wsp_core::types::wiring::{at_provider, GoldenRecipe};
use wsp_engine::labels::{BUILDER_LABEL, CREATED_AT_LABEL, OWNER_LABEL, WSP_LABEL};
use wsp_engine::machine::{MachineBackend, Size};
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::types::place_word::HERE_PLACE_ID;
use wsp_wire::views::golden_image::{GoldenBuilderView, GoldenBuilderViewHeldBy, GoldenBuilderViewScreen, GoldenBuilderViewSealed, GoldenBuilderViewSize};
use wsp_wire::views::place_kind::PlaceKind;
use wsp_wire::wire::capabilities::forks_no_machines;
use wsp_wire::wire::machine_link::{is_joined_computer, no_such_place_refusal, place_builds_no_image_line};
use wsp_words::exit::{HostError, Kind};
use wsp_words::place_word::{cloud_named, place_said, provider_key_name, PlaceRef};
use wsp_words::words::thread::{place_forks_nothing_pick_line, NO_PROVIDER_LINE};

use super::{iso_at, BuildPlace, BuildersArea, CopyRecipe};
use crate::images::LiveBuilder;

/// `placeAt`'s answer.
#[derive(Clone)]
pub struct PlaceAt {
    pub place: String,
    pub at: Arc<dyn MachineBackend>,
}

fn kind_word(kind: &PlaceKind) -> &'static str {
    match kind {
        PlaceKind::Computer => "computer",
        PlaceKind::Provider => "provider",
    }
}

impl BuildersArea {
    /// builders.ts:57-78 (P1).
    pub fn builder_view(&self, r: &BuilderRecord, b: &LiveBuilder) -> GoldenBuilderView {
        let place = r.place.clone().unwrap_or_else(|| self.ctx.places().wired());
        let rate = self.backend_of_place(&place).map(|at| at.pricing().rate_usd_per_hour(Size { cpu: r.size.cpu.0, mem_mb: r.size.mem_mb.0 }));
        let foreign_owner = (b.life == BuilderLife::Foreign).then(|| b.builder.machine.labels().and_then(|l| l.get(OWNER_LABEL)).cloned().unwrap_or_default());
        let held_by = match (b.life, r.held_by.as_ref()) {
            (BuilderLife::Held, Some(h)) => Some(GoldenBuilderViewHeldBy { host: h.host.clone(), pid: JsNumber::from(f64::from(h.pid)), heartbeat: h.heartbeat.clone() }),
            _ => None,
        };
        GoldenBuilderView {
            id: r.id.clone(),
            name: r.name.clone(),
            kind: r.kind,
            created_at: r.created_at.clone(),
            place_name: self.place_name(&place),
            place_id: place,
            size: GoldenBuilderViewSize { cpu: r.size.cpu, mem_mb: r.size.mem_mb },
            rate_usd_per_hour: rate.map(JsNumber::from),
            screen: r.stream_url.clone().map(|stream_url| GoldenBuilderViewScreen { stream_url }),
            sealable: Some(b.sealable),
            recipe_hash: r.import.as_ref().map(|i| i.recipe_hash.clone()),
            recipe: r.import.as_ref().and_then(|i| i.recipe.clone()),
            foreign_owner,
            held_by,
            building: (r.building == Some(true)).then_some(true),
            sealed: r.sealed.as_ref().map(|s| GoldenBuilderViewSealed { at: s.at.clone(), version: JsNumber::from(f64::from(s.version)) }),
        }
    }

    /// The backend a place id resolves to: the provider row, else the door's backend for it.
    fn backend_of_place(&self, place: &str) -> Option<Arc<dyn MachineBackend>> {
        self.ctx.places().backend(place).or_else(|| self.ctx.door().and_then(|d| d.backend_of(place)))
    }

    /// builders.ts:191-195 (P2).
    pub fn backend_at(&self, place: &str) -> Result<Arc<dyn MachineBackend>, HostError> {
        self.backend_of_place(place).ok_or_else(|| {
            let held: Vec<String> = self.ctx.places().list().iter().map(|p| provider_key_name(p)).collect();
            HostError::with_kind(no_such_place_refusal(place, &held), Kind::Missing)
        })
    }

    /// builders.ts:200-221 (P3).
    pub async fn place_at(&self, word: &str) -> Result<PlaceAt, HostError> {
        let places = self.ctx.places();
        if let Some(at) = places.backend(word) {
            return Ok(PlaceAt { place: word.to_string(), at });
        }
        let Some(door) = self.ctx.door() else {
            let cloud = cloud_named(&places.list(), word).unwrap_or_else(|| word.to_string());
            let at = self.backend_at(&cloud)?;
            return Ok(PlaceAt { place: cloud, at });
        };
        let place_id = door.place_for(word).await.map_err(|mut e| {
            e.kind = Some(Kind::Missing);
            e
        })?;
        match place_id {
            Some(id) => {
                let at = door.forking_backend(&id).await?;
                Ok(PlaceAt { place: id, at })
            }
            None => {
                let wired = cloud_named(&[places.wired()], word).and_then(|w| places.backend(&w).map(|at| (w, at)));
                match wired {
                    Some((place, at)) => Ok(PlaceAt { place, at }),
                    None => Err(HostError::with_kind(place_builds_no_image_line(word), Kind::Conflict)),
                }
            }
        }
    }

    /// builders.ts:225-230 (P5).
    pub async fn landing_backend(&self, place_id: Option<&str>) -> Result<Arc<dyn MachineBackend>, HostError> {
        if let Some(at) = self.forking_at(place_id).await? {
            return Ok(at);
        }
        let running: Vec<String> = self
            .build_places()
            .await?
            .iter()
            .map(|p| place_said(&PlaceRef { id: p.place.clone(), name: p.name.clone(), kind: Some(kind_word(&p.kind).to_string()) }))
            .collect();
        if running.is_empty() {
            let line = self.ctx.opts().no_machines_line.unwrap_or_else(|| NO_PROVIDER_LINE.to_string());
            return Err(HostError::with_kind(line, Kind::Conflict));
        }
        let place = place_id.map_or_else(|| self.ctx.places().wired(), str::to_string);
        Err(HostError::with_kind(place_forks_nothing_pick_line(&self.place_name(&place), &running), Kind::Conflict))
    }

    /// builders.ts:233-239 (P4).
    pub async fn forking_at(&self, place_id: Option<&str>) -> Result<Option<Arc<dyn MachineBackend>>, HostError> {
        if let Some(id) = place_id {
            self.ctx.place_door_of()?.forking_backend(id).await?;
        }
        let at = self.ctx.backend_of_kind_cloud(place_id);
        Ok((!forks_no_machines(&at.capabilities())).then_some(at))
    }

    /// builders.ts:242 (P6).
    pub fn place_name(&self, place: &str) -> String {
        self.ctx.door().and_then(|d| d.name_of(place)).unwrap_or_else(|| if place == HERE_PLACE_ID { self.ctx.hostname() } else { provider_key_name(place) })
    }

    /// builders.ts:245 (P7).
    pub async fn image_place(&self, name: &str) -> Result<String, HostError> {
        Ok(self.ctx.record_of(name).await?.and_then(|r| r.place).unwrap_or_else(|| self.ctx.places().wired()))
    }

    /// builders.ts:247 (P8).
    pub fn place_kind_of(&self, place: &str) -> PlaceKind {
        let places = self.ctx.places();
        if at_provider(Some(place), &*places) {
            PlaceKind::Provider
        } else {
            PlaceKind::Computer
        }
    }

    /// builders.ts:250-263 (P9).
    pub async fn build_places(&self) -> Result<Vec<BuildPlace>, HostError> {
        let places = self.ctx.places();
        let mut rows = Vec::new();
        for id in places.list() {
            if let Some(backend) = places.backend(&id) {
                if !forks_no_machines(&backend.capabilities()) {
                    rows.push(BuildPlace { place: id.clone(), name: id, kind: PlaceKind::Provider, backend });
                }
            }
        }
        let Some(door) = self.ctx.door() else { return Ok(rows) };
        for view in door.list(self.ctx.clock().now()).await? {
            if !is_joined_computer(&view.id, kind_word(&view.kind)) {
                continue;
            }
            if let Ok(backend) = door.forking_backend(&view.id).await {
                rows.push(BuildPlace { place: view.id, name: view.name, kind: PlaceKind::Computer, backend });
            }
        }
        Ok(rows)
    }

    /// builders.ts:267-271 (P10).
    pub fn copy_recipe_or_throw(&self) -> Result<CopyRecipe, HostError> {
        self.ctx.opts().copy_recipe.ok_or_else(|| HostError::new(NO_COPY_RECIPE))
    }

    /// builders.ts:275-279 (P10).
    pub fn recipe_or_throw(&self, named: Option<GoldenRecipe>) -> Result<GoldenRecipe, HostError> {
        named
            .or_else(|| self.ctx.opts().golden_recipe)
            .ok_or_else(|| HostError::new("this runtime has no golden recipe; the host wires one (setup + smoke) before the wizard can run"))
    }

    /// builders.ts:281 (P11).
    pub fn builder_labels(&self, extra: Option<&IndexMap<String, String>>) -> IndexMap<String, String> {
        let mut labels = extra.cloned().unwrap_or_default();
        labels.insert(WSP_LABEL.to_string(), "1".to_string());
        labels.insert(BUILDER_LABEL.to_string(), "1".to_string());
        labels.insert(OWNER_LABEL.to_string(), self.ctx.owner());
        labels.insert(CREATED_AT_LABEL.to_string(), iso_at(self.ctx.clock().now()));
        labels
    }
}
