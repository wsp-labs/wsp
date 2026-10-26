// SPDX-License-Identifier: AGPL-3.0-only
//! A live check of the https road of the image fetch against the internet; run it with `--ignored`.
use wsp_slates::slate_image_fetch::{fetch_road, fetch_slate_image};

#[tokio::test(flavor = "current_thread")]
#[ignore = "needs the internet"]
async fn an_https_image_is_fetched_and_shown() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let answer =
                fetch_slate_image("https://github.githubassets.com/favicons/favicon.png", "github.githubassets.com", Some(fetch_road()))
                    .await;
            let text = serde_json::to_string(&answer).unwrap();
            assert!(text.contains("image/png"), "{}", &text[..text.len().min(300)]);
        })
        .await;
}
