// SPDX-License-Identifier: AGPL-3.0-only
//! Review of the changed `a_start_that_fails_before_its_launch_leaves_no_row_and_ends_the_wait`
//! (tests/sessions_launch.rs:339). sessions.ts:569-571 asks the agent's lists (launchPicks) once and only then reads
//! the thread's folder (threadFolder); the changed test says only that the lists were asked at least once.
#![allow(dead_code, unused_imports)]

#[path = "support_a/mod.rs"]
mod support;
include!("sessions_launch/world.rs");

#[test]
fn the_agents_lists_are_asked_once_and_before_the_folder_is_read() {
    run_local(async {
        let r = rig();
        r.host.hooks.set::<dyn Fn(&str) -> bool>("LaunchCtx::folder_exists", Rc::new(|_p: &str| false));
        let mut p = prepared(&r);
        p.opts.cwd = Some("/gone/dir".to_string());
        let join = r.spawn(p);
        settle().await;
        {
            let seen = r.seen.borrow();
            assert_eq!(seen.len(), 1, "the agent's lists were asked once");
            assert!(
                !seen[0].calls.iter().any(|c| c.starts_with("thread_folder") || c.starts_with("folder_exists")),
                "nothing read the folder before the lists were asked: {:?}",
                seen[0].calls
            );
        }
        let e = err(join.await.expect("the launch ran"));
        assert_eq!(said(&e).0, no_cwd_line(&home_shortened("/gone/dir", None)));
        assert_eq!(r.host.hooks.called("thread_folder"), vec!["thread_folder(Some(\"/gone/dir\"))".to_string()], "the folder is read after the lists");
    });
}
