// SPDX-License-Identifier: AGPL-3.0-only
//! Review of wsp-machines at 2c4f6cb46: the image areas.
#![allow(clippy::await_holding_refcell_ref)]

mod images_support;
use images_support::*;

use wsp_core::types::internal::{BuilderLife, Sealed};
use wsp_core::types::wiring::UpgradeRoad;
use wsp_machines::images::engine::{GoldenDelta, SealResult};
use wsp_machines::images::golden::upgrade::UpgradeRequest;
use wsp_machines::images::golden::GoldenArea;

/// golden.ts:311: a kept builder is the update's only where `(x.record.place ?? places.wired) === place`, so a record
/// with no place is the wired provider's. upgrade.rs:71 reads a missing place as the image's own place, so with the
/// image at `spoo` a builder the wired `boat` kept at the same version number is taken: the delta lands on boat's
/// builder and is sealed as spoo's next version, where the TS forks a builder at spoo.
#[test]
fn a_builder_with_no_place_is_the_wired_providers_not_the_images() {
    run(async {
        let w = World::new();
        w.places
            .rows
            .borrow_mut()
            .insert("spoo".into(), FakeBackend::new("spoo"));
        w.put_image_now(&sealed_image("default", 1, "h1", Some("spoo")));
        w.put_copy_now("spoo", "default", &manifest(&[(1, "snap_v1", None)]));
        w.put_recipe_now("spoo", "default", 1);
        w.roads
            .upgraded
            .borrow_mut()
            .push_back(Ok(engine_builder(FakeMachine::new("f1"))));
        w.seals.borrow_mut().push_back(Ok(SealResult {
            manifest: manifest(&[(1, "snap_v1", None), (2, "snap_v2", None)]),
            version: version(2, "snap_v2", None),
            builder_kept: true,
            vault: None,
        }));
        let m = FakeMachine::new("b_boat");
        let b = live_builder("b_boat", "default", "boat", BuilderLife::Reusable);
        {
            let mut x = b.borrow_mut();
            x.builder = engine_builder(m.clone());
            x.record.place = None;
            x.record.sealed = Some(Sealed {
                at: "2026-10-25T08:55:00.000Z".into(),
                version: 1,
            });
        }
        w.add_builder(b);
        let got = GoldenArea::new(w.clone())
            .upgrade(UpgradeRequest {
                recipe: Some(recipe("true")),
                delta: GoldenDelta::default(),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(
            m.execs().is_empty(),
            "the wired provider's builder was used for spoo's image: {:?}",
            m.execs()
        );
        assert_eq!(got.road, UpgradeRoad::Fork);
    });
}
