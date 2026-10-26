// SPDX-License-Identifier: AGPL-3.0-only
//! backend_of_kind_cloud may fail (TS backendOfKind throws with no backend) and forkingAt has no catch.
mod images_support;

use std::rc::Rc;

use images_support::*;
use wsp_machines::images::builders::BuildersArea;
use wsp_words::exit::HostError;

fn area(w: &Rc<World>) -> Rc<BuildersArea> {
    BuildersArea::new(w.clone())
}

#[test]
fn a_cloud_kind_with_no_backend_refuses_the_fork_as_it_is() {
    // builders.ts:237: backendOfKind("cloud", place) in forkingAt, with no catch.
    let w = World::new();
    *w.backend_of_kind_failure.borrow_mut() =
        Some(plain_error("no backend is wired for the kind cloud"));
    let a = area(&w);
    run(async {
        let got: Result<(), HostError> = a.forking_at(None).await.map(|_| ());
        assert_eq!(
            got,
            Err(plain_error("no backend is wired for the kind cloud"))
        );
    });
}
