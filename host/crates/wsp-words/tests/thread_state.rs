// SPDX-License-Identifier: AGPL-3.0-only
//! thread-state.ts and wsp-tools.ts have no TS tests of their own; every expected value here is what those files
//! returned under node at a3b870a6e.

mod support;

// Cases the spec pins; every expected value is what the TS function returned under node at a3b870a6e.
mod thread_states {
    use wsp_words::thread_state::*;

    use serde_json::{json, Value};
    use wsp_wire::views::session::{SubagentState, ThreadView};

    fn tv(v: Value) -> ThreadView {
        serde_json::from_value(crate::support::over(crate::support::thread_view(), v)).unwrap()
    }

    fn state_word(s: wsp_words::thread_state::ThreadState) -> String {
        serde_json::to_value(s).unwrap().as_str().unwrap().to_string()
    }

    fn moment(m: Option<SettleMoment>) -> String {
        match m {
            None => "null".into(),
            Some(m) => wsp_words::exit::json_line(&json!({ "ms": m.ms, "byHand": m.by_hand }), None),
        }
    }

    #[test]
    fn a_threads_state_reads_asking_waiting_and_unseen_ends() {
        assert_eq!(thread_unseen_at(&tv(json!({"status":"running"}))), None); // threadUnseenAt({"status":"running"})
        assert_eq!(thread_unread(&tv(json!({"status":"running"}))), false); // threadUnread({"status":"running"})
        assert_eq!(thread_needs_you(&tv(json!({"status":"running"}))), false); // threadNeedsYou({"status":"running"})
        assert_eq!(state_word(thread_state(&tv(json!({"status":"running"})))), "running"); // threadState({"status":"running"})
        assert_eq!(waiting_line(&tv(json!({"status":"running"}))), None); // waitingLine({"status":"running"})
        assert_eq!(thread_word_of(&tv(json!({"status":"running"}))), "Working"); // threadWordOf({"status":"running"})
        assert_eq!(thread_unseen_at(&tv(json!({"status":"running","endedAt":5,"wokeAt":9}))), None); // threadUnseenAt({"status":"running","endedAt":5,"wokeAt":9})
        assert_eq!(thread_unread(&tv(json!({"status":"running","endedAt":5,"wokeAt":9}))), false); // threadUnread({"status":"running","endedAt":5,"wokeAt":9})
        assert_eq!(thread_needs_you(&tv(json!({"status":"running","endedAt":5,"wokeAt":9}))), false); // threadNeedsYou({"status":"running","endedAt":5,"wokeAt":9})
        assert_eq!(state_word(thread_state(&tv(json!({"status":"running","endedAt":5,"wokeAt":9})))), "running"); // threadState({"status":"running","endedAt":5,"wokeAt":9})
        assert_eq!(waiting_line(&tv(json!({"status":"running","endedAt":5,"wokeAt":9}))), None); // waitingLine({"status":"running","endedAt":5,"wokeAt":9})
        assert_eq!(thread_word_of(&tv(json!({"status":"running","endedAt":5,"wokeAt":9}))), "Working"); // threadWordOf({"status":"running","endedAt":5,"wokeAt":9})
        assert_eq!(thread_unseen_at(&tv(json!({"status":"completed"}))), None); // threadUnseenAt({"status":"completed"})
        assert_eq!(thread_unread(&tv(json!({"status":"completed"}))), false); // threadUnread({"status":"completed"})
        assert_eq!(thread_needs_you(&tv(json!({"status":"completed"}))), false); // threadNeedsYou({"status":"completed"})
        assert_eq!(state_word(thread_state(&tv(json!({"status":"completed"})))), "completed"); // threadState({"status":"completed"})
        assert_eq!(waiting_line(&tv(json!({"status":"completed"}))), None); // waitingLine({"status":"completed"})
        assert_eq!(thread_word_of(&tv(json!({"status":"completed"}))), "Idle"); // threadWordOf({"status":"completed"})
        assert_eq!(thread_unseen_at(&tv(json!({"status":"completed","endedAt":10}))), Some(10.0)); // threadUnseenAt({"status":"completed","endedAt":10})
        assert_eq!(thread_unread(&tv(json!({"status":"completed","endedAt":10}))), true); // threadUnread({"status":"completed","endedAt":10})
        assert_eq!(thread_needs_you(&tv(json!({"status":"completed","endedAt":10}))), true); // threadNeedsYou({"status":"completed","endedAt":10})
        assert_eq!(state_word(thread_state(&tv(json!({"status":"completed","endedAt":10})))), "done"); // threadState({"status":"completed","endedAt":10})
        assert_eq!(waiting_line(&tv(json!({"status":"completed","endedAt":10}))), None); // waitingLine({"status":"completed","endedAt":10})
        assert_eq!(thread_word_of(&tv(json!({"status":"completed","endedAt":10}))), "Done"); // threadWordOf({"status":"completed","endedAt":10})
        assert_eq!(thread_unseen_at(&tv(json!({"status":"completed","endedAt":10,"readAt":10}))), None); // threadUnseenAt({"status":"completed","endedAt":10,"readAt":10})
        assert_eq!(thread_unread(&tv(json!({"status":"completed","endedAt":10,"readAt":10}))), false); // threadUnread({"status":"completed","endedAt":10,"readAt":10})
        assert_eq!(thread_needs_you(&tv(json!({"status":"completed","endedAt":10,"readAt":10}))), false); // threadNeedsYou({"status":"completed","endedAt":10,"readAt":10})
        assert_eq!(state_word(thread_state(&tv(json!({"status":"completed","endedAt":10,"readAt":10})))), "completed"); // threadState({"status":"completed","endedAt":10,"readAt":10})
        assert_eq!(waiting_line(&tv(json!({"status":"completed","endedAt":10,"readAt":10}))), None); // waitingLine({"status":"completed","endedAt":10,"readAt":10})
        assert_eq!(thread_word_of(&tv(json!({"status":"completed","endedAt":10,"readAt":10}))), "Idle"); // threadWordOf({"status":"completed","endedAt":10,"readAt":10})
        assert_eq!(thread_unseen_at(&tv(json!({"status":"completed","endedAt":10,"readAt":9}))), Some(10.0)); // threadUnseenAt({"status":"completed","endedAt":10,"readAt":9})
        assert_eq!(thread_unread(&tv(json!({"status":"completed","endedAt":10,"readAt":9}))), true); // threadUnread({"status":"completed","endedAt":10,"readAt":9})
        assert_eq!(thread_needs_you(&tv(json!({"status":"completed","endedAt":10,"readAt":9}))), true); // threadNeedsYou({"status":"completed","endedAt":10,"readAt":9})
        assert_eq!(state_word(thread_state(&tv(json!({"status":"completed","endedAt":10,"readAt":9})))), "done"); // threadState({"status":"completed","endedAt":10,"readAt":9})
        assert_eq!(waiting_line(&tv(json!({"status":"completed","endedAt":10,"readAt":9}))), None); // waitingLine({"status":"completed","endedAt":10,"readAt":9})
        assert_eq!(thread_word_of(&tv(json!({"status":"completed","endedAt":10,"readAt":9}))), "Done"); // threadWordOf({"status":"completed","endedAt":10,"readAt":9})
        assert_eq!(thread_unseen_at(&tv(json!({"status":"completed","endedAt":10,"readAt":12,"wokeAt":11}))), None); // threadUnseenAt({"status":"completed","endedAt":10,"readAt":12,"wokeAt":11})
        assert_eq!(thread_unread(&tv(json!({"status":"completed","endedAt":10,"readAt":12,"wokeAt":11}))), false); // threadUnread({"status":"completed","endedAt":10,"readAt":12,"wokeAt":11})
        assert_eq!(thread_needs_you(&tv(json!({"status":"completed","endedAt":10,"readAt":12,"wokeAt":11}))), false); // threadNeedsYou({"status":"completed","endedAt":10,"readAt":12,"wokeAt":11})
        assert_eq!(state_word(thread_state(&tv(json!({"status":"completed","endedAt":10,"readAt":12,"wokeAt":11})))), "completed"); // threadState({"status":"completed","endedAt":10,"readAt":12,"wokeAt":11})
        assert_eq!(waiting_line(&tv(json!({"status":"completed","endedAt":10,"readAt":12,"wokeAt":11}))), None); // waitingLine({"status":"completed","endedAt":10,"readAt":12,"wokeAt":11})
        assert_eq!(thread_word_of(&tv(json!({"status":"completed","endedAt":10,"readAt":12,"wokeAt":11}))), "Idle"); // threadWordOf({"status":"completed","endedAt":10,"readAt":12,"wokeAt":11})
        assert_eq!(thread_unseen_at(&tv(json!({"status":"completed","endedAt":10,"readAt":12,"wokeAt":13}))), Some(13.0)); // threadUnseenAt({"status":"completed","endedAt":10,"readAt":12,"wokeAt":13})
        assert_eq!(thread_unread(&tv(json!({"status":"completed","endedAt":10,"readAt":12,"wokeAt":13}))), true); // threadUnread({"status":"completed","endedAt":10,"readAt":12,"wokeAt":13})
        assert_eq!(thread_needs_you(&tv(json!({"status":"completed","endedAt":10,"readAt":12,"wokeAt":13}))), true); // threadNeedsYou({"status":"completed","endedAt":10,"readAt":12,"wokeAt":13})
        assert_eq!(state_word(thread_state(&tv(json!({"status":"completed","endedAt":10,"readAt":12,"wokeAt":13})))), "done"); // threadState({"status":"completed","endedAt":10,"readAt":12,"wokeAt":13})
        assert_eq!(waiting_line(&tv(json!({"status":"completed","endedAt":10,"readAt":12,"wokeAt":13}))), None); // waitingLine({"status":"completed","endedAt":10,"readAt":12,"wokeAt":13})
        assert_eq!(thread_word_of(&tv(json!({"status":"completed","endedAt":10,"readAt":12,"wokeAt":13}))), "Done"); // threadWordOf({"status":"completed","endedAt":10,"readAt":12,"wokeAt":13})
        assert_eq!(thread_unseen_at(&tv(json!({"status":"interrupted","endedAt":10}))), Some(10.0)); // threadUnseenAt({"status":"interrupted","endedAt":10})
        assert_eq!(thread_unread(&tv(json!({"status":"interrupted","endedAt":10}))), true); // threadUnread({"status":"interrupted","endedAt":10})
        assert_eq!(thread_needs_you(&tv(json!({"status":"interrupted","endedAt":10}))), false); // threadNeedsYou({"status":"interrupted","endedAt":10})
        assert_eq!(state_word(thread_state(&tv(json!({"status":"interrupted","endedAt":10})))), "done"); // threadState({"status":"interrupted","endedAt":10})
        assert_eq!(waiting_line(&tv(json!({"status":"interrupted","endedAt":10}))), None); // waitingLine({"status":"interrupted","endedAt":10})
        assert_eq!(thread_word_of(&tv(json!({"status":"interrupted","endedAt":10}))), "Done"); // threadWordOf({"status":"interrupted","endedAt":10})
        assert_eq!(thread_unseen_at(&tv(json!({"status":"interrupted","endedAt":10,"wokeAt":12}))), Some(12.0)); // threadUnseenAt({"status":"interrupted","endedAt":10,"wokeAt":12})
        assert_eq!(thread_unread(&tv(json!({"status":"interrupted","endedAt":10,"wokeAt":12}))), true); // threadUnread({"status":"interrupted","endedAt":10,"wokeAt":12})
        assert_eq!(thread_needs_you(&tv(json!({"status":"interrupted","endedAt":10,"wokeAt":12}))), true); // threadNeedsYou({"status":"interrupted","endedAt":10,"wokeAt":12})
        assert_eq!(state_word(thread_state(&tv(json!({"status":"interrupted","endedAt":10,"wokeAt":12})))), "done"); // threadState({"status":"interrupted","endedAt":10,"wokeAt":12})
        assert_eq!(waiting_line(&tv(json!({"status":"interrupted","endedAt":10,"wokeAt":12}))), None); // waitingLine({"status":"interrupted","endedAt":10,"wokeAt":12})
        assert_eq!(thread_word_of(&tv(json!({"status":"interrupted","endedAt":10,"wokeAt":12}))), "Done"); // threadWordOf({"status":"interrupted","endedAt":10,"wokeAt":12})
        assert_eq!(thread_unseen_at(&tv(json!({"status":"failed","endedAt":10}))), Some(10.0)); // threadUnseenAt({"status":"failed","endedAt":10})
        assert_eq!(thread_unread(&tv(json!({"status":"failed","endedAt":10}))), false); // threadUnread({"status":"failed","endedAt":10})
        assert_eq!(thread_needs_you(&tv(json!({"status":"failed","endedAt":10}))), true); // threadNeedsYou({"status":"failed","endedAt":10})
        assert_eq!(state_word(thread_state(&tv(json!({"status":"failed","endedAt":10})))), "failed"); // threadState({"status":"failed","endedAt":10})
        assert_eq!(waiting_line(&tv(json!({"status":"failed","endedAt":10}))), None); // waitingLine({"status":"failed","endedAt":10})
        assert_eq!(thread_word_of(&tv(json!({"status":"failed","endedAt":10}))), "Failed"); // threadWordOf({"status":"failed","endedAt":10})
        assert_eq!(thread_unseen_at(&tv(json!({"status":"completed","asking":"Allow ls?"}))), None); // threadUnseenAt({"status":"completed","asking":"Allow ls?"})
        assert_eq!(thread_unread(&tv(json!({"status":"completed","asking":"Allow ls?"}))), false); // threadUnread({"status":"completed","asking":"Allow ls?"})
        assert_eq!(thread_needs_you(&tv(json!({"status":"completed","asking":"Allow ls?"}))), true); // threadNeedsYou({"status":"completed","asking":"Allow ls?"})
        assert_eq!(state_word(thread_state(&tv(json!({"status":"completed","asking":"Allow ls?"})))), "waiting"); // threadState({"status":"completed","asking":"Allow ls?"})
        assert_eq!(waiting_line(&tv(json!({"status":"completed","asking":"Allow ls?"}))), Some("Allow ls?".to_string())); // waitingLine({"status":"completed","asking":"Allow ls?"})
        assert_eq!(thread_word_of(&tv(json!({"status":"completed","asking":"Allow ls?"}))), "Needs you"); // threadWordOf({"status":"completed","asking":"Allow ls?"})
        assert_eq!(thread_unseen_at(&tv(json!({"status":"running","waitingOn":{"threadId":"t2","workspaceId":"w","sessionId":"s","title":"Lead","prompt":{"askId":"a1","toolName":"Bash","input":"{\"command\":\"ls\"}","options":[],"workspaceId":"w","sessionId":"s"},"placeId":"here","place":"mac"}}))), None); // threadUnseenAt({"status":"running","waitingOn":{"threadId":"t2","workspaceId":"w","sessionId":"s","title":"Lead","prompt":{"askId":"a1","toolName":"Bash","input":"{\"command\":\"ls\"}","options":[],"workspaceId":"w","sessionId":"s"},"placeId":"here","place":"mac"}})
        assert_eq!(thread_unread(&tv(json!({"status":"running","waitingOn":{"threadId":"t2","workspaceId":"w","sessionId":"s","title":"Lead","prompt":{"askId":"a1","toolName":"Bash","input":"{\"command\":\"ls\"}","options":[],"workspaceId":"w","sessionId":"s"},"placeId":"here","place":"mac"}}))), false); // threadUnread({"status":"running","waitingOn":{"threadId":"t2","workspaceId":"w","sessionId":"s","title":"Lead","prompt":{"askId":"a1","toolName":"Bash","input":"{\"command\":\"ls\"}","options":[],"workspaceId":"w","sessionId":"s"},"placeId":"here","place":"mac"}})
        assert_eq!(thread_needs_you(&tv(json!({"status":"running","waitingOn":{"threadId":"t2","workspaceId":"w","sessionId":"s","title":"Lead","prompt":{"askId":"a1","toolName":"Bash","input":"{\"command\":\"ls\"}","options":[],"workspaceId":"w","sessionId":"s"},"placeId":"here","place":"mac"}}))), true); // threadNeedsYou({"status":"running","waitingOn":{"threadId":"t2","workspaceId":"w","sessionId":"s","title":"Lead","prompt":{"askId":"a1","toolName":"Bash","input":"{\"command\":\"ls\"}","options":[],"workspaceId":"w","sessionId":"s"},"placeId":"here","place":"mac"}})
        assert_eq!(state_word(thread_state(&tv(json!({"status":"running","waitingOn":{"threadId":"t2","workspaceId":"w","sessionId":"s","title":"Lead","prompt":{"askId":"a1","toolName":"Bash","input":"{\"command\":\"ls\"}","options":[],"workspaceId":"w","sessionId":"s"},"placeId":"here","place":"mac"}})))), "waiting"); // threadState({"status":"running","waitingOn":{"threadId":"t2","workspaceId":"w","sessionId":"s","title":"Lead","prompt":{"askId":"a1","toolName":"Bash","input":"{\"command\":\"ls\"}","options":[],"workspaceId":"w","sessionId":"s"},"placeId":"here","place":"mac"}})
        assert_eq!(waiting_line(&tv(json!({"status":"running","waitingOn":{"threadId":"t2","workspaceId":"w","sessionId":"s","title":"Lead","prompt":{"askId":"a1","toolName":"Bash","input":"{\"command\":\"ls\"}","options":[],"workspaceId":"w","sessionId":"s"},"placeId":"here","place":"mac"}}))), Some("Run a command: ls needs an answer".to_string())); // waitingLine({"status":"running","waitingOn":{"threadId":"t2","workspaceId":"w","sessionId":"s","title":"Lead","prompt":{"askId":"a1","toolName":"Bash","input":"{\"command\":\"ls\"}","options":[],"workspaceId":"w","sessionId":"s"},"placeId":"here","place":"mac"}})
        assert_eq!(thread_word_of(&tv(json!({"status":"running","waitingOn":{"threadId":"t2","workspaceId":"w","sessionId":"s","title":"Lead","prompt":{"askId":"a1","toolName":"Bash","input":"{\"command\":\"ls\"}","options":[],"workspaceId":"w","sessionId":"s"},"placeId":"here","place":"mac"}}))), "Needs you"); // threadWordOf({"status":"running","waitingOn":{"threadId":"t2","workspaceId":"w","sessionId":"s","title":"Lead","prompt":{"askId":"a1","toolName":"Bash","input":"{\"command\":\"ls\"}","options":[],"workspaceId":"w","sessionId":"s"},"placeId":"here","place":"mac"}})
        assert_eq!(thread_unseen_at(&tv(json!({"status":"completed","sub":{"state":"running","on":"boat","machine":"m","size":{"cpu":2,"memMb":4096},"branch":"b","held":{"files":1,"commits":0}}}))), None); // threadUnseenAt({"status":"completed","sub":{"state":"running","on":"boat","machine":"m","size":{"cpu":2,"memMb":4096},"branch":"b","held":{"files":1,"commits":0}}})
        assert_eq!(thread_unread(&tv(json!({"status":"completed","sub":{"state":"running","on":"boat","machine":"m","size":{"cpu":2,"memMb":4096},"branch":"b","held":{"files":1,"commits":0}}}))), false); // threadUnread({"status":"completed","sub":{"state":"running","on":"boat","machine":"m","size":{"cpu":2,"memMb":4096},"branch":"b","held":{"files":1,"commits":0}}})
        assert_eq!(thread_needs_you(&tv(json!({"status":"completed","sub":{"state":"running","on":"boat","machine":"m","size":{"cpu":2,"memMb":4096},"branch":"b","held":{"files":1,"commits":0}}}))), true); // threadNeedsYou({"status":"completed","sub":{"state":"running","on":"boat","machine":"m","size":{"cpu":2,"memMb":4096},"branch":"b","held":{"files":1,"commits":0}}})
        assert_eq!(state_word(thread_state(&tv(json!({"status":"completed","sub":{"state":"running","on":"boat","machine":"m","size":{"cpu":2,"memMb":4096},"branch":"b","held":{"files":1,"commits":0}}})))), "completed"); // threadState({"status":"completed","sub":{"state":"running","on":"boat","machine":"m","size":{"cpu":2,"memMb":4096},"branch":"b","held":{"files":1,"commits":0}}})
        assert_eq!(waiting_line(&tv(json!({"status":"completed","sub":{"state":"running","on":"boat","machine":"m","size":{"cpu":2,"memMb":4096},"branch":"b","held":{"files":1,"commits":0}}}))), None); // waitingLine({"status":"completed","sub":{"state":"running","on":"boat","machine":"m","size":{"cpu":2,"memMb":4096},"branch":"b","held":{"files":1,"commits":0}}})
        assert_eq!(thread_word_of(&tv(json!({"status":"completed","sub":{"state":"running","on":"boat","machine":"m","size":{"cpu":2,"memMb":4096},"branch":"b","held":{"files":1,"commits":0}}}))), "Idle"); // threadWordOf({"status":"completed","sub":{"state":"running","on":"boat","machine":"m","size":{"cpu":2,"memMb":4096},"branch":"b","held":{"files":1,"commits":0}}})
        assert_eq!(thread_unseen_at(&tv(json!({"status":"running","sub":{"state":"running","on":"boat","machine":"m","size":{"cpu":2,"memMb":4096},"branch":"b","held":{"files":1,"commits":0}}}))), None); // threadUnseenAt({"status":"running","sub":{"state":"running","on":"boat","machine":"m","size":{"cpu":2,"memMb":4096},"branch":"b","held":{"files":1,"commits":0}}})
        assert_eq!(thread_unread(&tv(json!({"status":"running","sub":{"state":"running","on":"boat","machine":"m","size":{"cpu":2,"memMb":4096},"branch":"b","held":{"files":1,"commits":0}}}))), false); // threadUnread({"status":"running","sub":{"state":"running","on":"boat","machine":"m","size":{"cpu":2,"memMb":4096},"branch":"b","held":{"files":1,"commits":0}}})
        assert_eq!(thread_needs_you(&tv(json!({"status":"running","sub":{"state":"running","on":"boat","machine":"m","size":{"cpu":2,"memMb":4096},"branch":"b","held":{"files":1,"commits":0}}}))), false); // threadNeedsYou({"status":"running","sub":{"state":"running","on":"boat","machine":"m","size":{"cpu":2,"memMb":4096},"branch":"b","held":{"files":1,"commits":0}}})
        assert_eq!(state_word(thread_state(&tv(json!({"status":"running","sub":{"state":"running","on":"boat","machine":"m","size":{"cpu":2,"memMb":4096},"branch":"b","held":{"files":1,"commits":0}}})))), "running"); // threadState({"status":"running","sub":{"state":"running","on":"boat","machine":"m","size":{"cpu":2,"memMb":4096},"branch":"b","held":{"files":1,"commits":0}}})
        assert_eq!(waiting_line(&tv(json!({"status":"running","sub":{"state":"running","on":"boat","machine":"m","size":{"cpu":2,"memMb":4096},"branch":"b","held":{"files":1,"commits":0}}}))), None); // waitingLine({"status":"running","sub":{"state":"running","on":"boat","machine":"m","size":{"cpu":2,"memMb":4096},"branch":"b","held":{"files":1,"commits":0}}})
        assert_eq!(thread_word_of(&tv(json!({"status":"running","sub":{"state":"running","on":"boat","machine":"m","size":{"cpu":2,"memMb":4096},"branch":"b","held":{"files":1,"commits":0}}}))), "Working"); // threadWordOf({"status":"running","sub":{"state":"running","on":"boat","machine":"m","size":{"cpu":2,"memMb":4096},"branch":"b","held":{"files":1,"commits":0}}})
        assert_eq!(subagent_state_word(SubagentState::Running), "Working"); // subagentStateWord("running")
        assert_eq!(subagent_state_word(SubagentState::Done), "Done"); // subagentStateWord("done")
        assert_eq!(subagent_state_word(SubagentState::Failed), "Failed"); // subagentStateWord("failed")
        assert_eq!(subagent_state_word(SubagentState::Stopped), "Stopped"); // subagentStateWord("stopped")
        assert_eq!(needs_you_line(&serde_json::from_value(json!({"askId":"a1","toolName":"Bash","input":"{\"command\":\"ls\"}","options":[],"workspaceId":"w","sessionId":"s"})).unwrap()), "needs you: Run a command: ls"); // needsYouLine({"askId":"a1","toolName":"Bash","input":"{\"command\":\"ls\"}","options":[],"workspaceId":"w","sessionId":"s"})
    }

    #[test]
    fn a_read_thread_settles_after_the_window_unless_held_failed_or_unseen() {
        assert_eq!(thread_seen(&SettleFacts { working: false, asking: false, failed: false, started_at: None, ended_at: None, read_at: None, settled_at: None }), true); // threadSeen({"working":false,"asking":false,"failed":false,"startedAt":null,"endedAt":null,"readAt":null,"settledAt":null})
        assert_eq!(moment(settle_moment(&SettleFacts { working: false, asking: false, failed: false, started_at: None, ended_at: None, read_at: None, settled_at: None }, Some(1000.0), false)), "null"); // JSON.stringify(settleMoment({"working":false,"asking":false,"failed":false,"startedAt":null,"endedAt":null,"readAt":null,"settledAt":null}, 1000))
        assert_eq!(moment(settle_moment(&SettleFacts { working: false, asking: false, failed: false, started_at: None, ended_at: None, read_at: None, settled_at: None }, None, false)), "null"); // JSON.stringify(settleMoment({"working":false,"asking":false,"failed":false,"startedAt":null,"endedAt":null,"readAt":null,"settledAt":null}, null))
        assert_eq!(moment(settle_moment(&SettleFacts { working: false, asking: false, failed: false, started_at: None, ended_at: None, read_at: None, settled_at: None }, Some(1000.0), true)), "null"); // JSON.stringify(settleMoment({"working":false,"asking":false,"failed":false,"startedAt":null,"endedAt":null,"readAt":null,"settledAt":null}, 1000, true))
        assert_eq!(thread_settled(&SettleFacts { working: false, asking: false, failed: false, started_at: None, ended_at: None, read_at: None, settled_at: None }, 1006.0, Some(1000.0), false), false); // threadSettled({"working":false,"asking":false,"failed":false,"startedAt":null,"endedAt":null,"readAt":null,"settledAt":null}, 1006, 1000)
        assert_eq!(thread_settled(&SettleFacts { working: false, asking: false, failed: false, started_at: None, ended_at: None, read_at: None, settled_at: None }, 1005.0, Some(1000.0), false), false); // threadSettled({"working":false,"asking":false,"failed":false,"startedAt":null,"endedAt":null,"readAt":null,"settledAt":null}, 1005, 1000)
        assert_eq!(thread_seen(&SettleFacts { working: true, asking: false, failed: false, started_at: None, ended_at: Some(1.0), read_at: None, settled_at: None }), false); // threadSeen({"working":true,"asking":false,"failed":false,"startedAt":null,"endedAt":1,"readAt":null,"settledAt":null})
        assert_eq!(moment(settle_moment(&SettleFacts { working: true, asking: false, failed: false, started_at: None, ended_at: Some(1.0), read_at: None, settled_at: None }, Some(1000.0), false)), "null"); // JSON.stringify(settleMoment({"working":true,"asking":false,"failed":false,"startedAt":null,"endedAt":1,"readAt":null,"settledAt":null}, 1000))
        assert_eq!(moment(settle_moment(&SettleFacts { working: true, asking: false, failed: false, started_at: None, ended_at: Some(1.0), read_at: None, settled_at: None }, None, false)), "null"); // JSON.stringify(settleMoment({"working":true,"asking":false,"failed":false,"startedAt":null,"endedAt":1,"readAt":null,"settledAt":null}, null))
        assert_eq!(moment(settle_moment(&SettleFacts { working: true, asking: false, failed: false, started_at: None, ended_at: Some(1.0), read_at: None, settled_at: None }, Some(1000.0), true)), "null"); // JSON.stringify(settleMoment({"working":true,"asking":false,"failed":false,"startedAt":null,"endedAt":1,"readAt":null,"settledAt":null}, 1000, true))
        assert_eq!(thread_settled(&SettleFacts { working: true, asking: false, failed: false, started_at: None, ended_at: Some(1.0), read_at: None, settled_at: None }, 1006.0, Some(1000.0), false), false); // threadSettled({"working":true,"asking":false,"failed":false,"startedAt":null,"endedAt":1,"readAt":null,"settledAt":null}, 1006, 1000)
        assert_eq!(thread_settled(&SettleFacts { working: true, asking: false, failed: false, started_at: None, ended_at: Some(1.0), read_at: None, settled_at: None }, 1005.0, Some(1000.0), false), false); // threadSettled({"working":true,"asking":false,"failed":false,"startedAt":null,"endedAt":1,"readAt":null,"settledAt":null}, 1005, 1000)
        assert_eq!(thread_seen(&SettleFacts { working: false, asking: true, failed: false, started_at: None, ended_at: Some(1.0), read_at: None, settled_at: None }), false); // threadSeen({"working":false,"asking":true,"failed":false,"startedAt":null,"endedAt":1,"readAt":null,"settledAt":null})
        assert_eq!(moment(settle_moment(&SettleFacts { working: false, asking: true, failed: false, started_at: None, ended_at: Some(1.0), read_at: None, settled_at: None }, Some(1000.0), false)), "null"); // JSON.stringify(settleMoment({"working":false,"asking":true,"failed":false,"startedAt":null,"endedAt":1,"readAt":null,"settledAt":null}, 1000))
        assert_eq!(moment(settle_moment(&SettleFacts { working: false, asking: true, failed: false, started_at: None, ended_at: Some(1.0), read_at: None, settled_at: None }, None, false)), "null"); // JSON.stringify(settleMoment({"working":false,"asking":true,"failed":false,"startedAt":null,"endedAt":1,"readAt":null,"settledAt":null}, null))
        assert_eq!(moment(settle_moment(&SettleFacts { working: false, asking: true, failed: false, started_at: None, ended_at: Some(1.0), read_at: None, settled_at: None }, Some(1000.0), true)), "null"); // JSON.stringify(settleMoment({"working":false,"asking":true,"failed":false,"startedAt":null,"endedAt":1,"readAt":null,"settledAt":null}, 1000, true))
        assert_eq!(thread_settled(&SettleFacts { working: false, asking: true, failed: false, started_at: None, ended_at: Some(1.0), read_at: None, settled_at: None }, 1006.0, Some(1000.0), false), false); // threadSettled({"working":false,"asking":true,"failed":false,"startedAt":null,"endedAt":1,"readAt":null,"settledAt":null}, 1006, 1000)
        assert_eq!(thread_settled(&SettleFacts { working: false, asking: true, failed: false, started_at: None, ended_at: Some(1.0), read_at: None, settled_at: None }, 1005.0, Some(1000.0), false), false); // threadSettled({"working":false,"asking":true,"failed":false,"startedAt":null,"endedAt":1,"readAt":null,"settledAt":null}, 1005, 1000)
        assert_eq!(thread_seen(&SettleFacts { working: false, asking: false, failed: false, started_at: Some(1.0), ended_at: Some(5.0), read_at: Some(6.0), settled_at: None }), true); // threadSeen({"working":false,"asking":false,"failed":false,"startedAt":1,"endedAt":5,"readAt":6,"settledAt":null})
        assert_eq!(moment(settle_moment(&SettleFacts { working: false, asking: false, failed: false, started_at: Some(1.0), ended_at: Some(5.0), read_at: Some(6.0), settled_at: None }, Some(1000.0), false)), "{\"ms\":1006,\"byHand\":false}"); // JSON.stringify(settleMoment({"working":false,"asking":false,"failed":false,"startedAt":1,"endedAt":5,"readAt":6,"settledAt":null}, 1000))
        assert_eq!(moment(settle_moment(&SettleFacts { working: false, asking: false, failed: false, started_at: Some(1.0), ended_at: Some(5.0), read_at: Some(6.0), settled_at: None }, None, false)), "null"); // JSON.stringify(settleMoment({"working":false,"asking":false,"failed":false,"startedAt":1,"endedAt":5,"readAt":6,"settledAt":null}, null))
        assert_eq!(moment(settle_moment(&SettleFacts { working: false, asking: false, failed: false, started_at: Some(1.0), ended_at: Some(5.0), read_at: Some(6.0), settled_at: None }, Some(1000.0), true)), "null"); // JSON.stringify(settleMoment({"working":false,"asking":false,"failed":false,"startedAt":1,"endedAt":5,"readAt":6,"settledAt":null}, 1000, true))
        assert_eq!(thread_settled(&SettleFacts { working: false, asking: false, failed: false, started_at: Some(1.0), ended_at: Some(5.0), read_at: Some(6.0), settled_at: None }, 1006.0, Some(1000.0), false), true); // threadSettled({"working":false,"asking":false,"failed":false,"startedAt":1,"endedAt":5,"readAt":6,"settledAt":null}, 1006, 1000)
        assert_eq!(thread_settled(&SettleFacts { working: false, asking: false, failed: false, started_at: Some(1.0), ended_at: Some(5.0), read_at: Some(6.0), settled_at: None }, 1005.0, Some(1000.0), false), false); // threadSettled({"working":false,"asking":false,"failed":false,"startedAt":1,"endedAt":5,"readAt":6,"settledAt":null}, 1005, 1000)
        assert_eq!(thread_seen(&SettleFacts { working: false, asking: false, failed: false, started_at: Some(1.0), ended_at: Some(5.0), read_at: Some(4.0), settled_at: None }), false); // threadSeen({"working":false,"asking":false,"failed":false,"startedAt":1,"endedAt":5,"readAt":4,"settledAt":null})
        assert_eq!(moment(settle_moment(&SettleFacts { working: false, asking: false, failed: false, started_at: Some(1.0), ended_at: Some(5.0), read_at: Some(4.0), settled_at: None }, Some(1000.0), false)), "null"); // JSON.stringify(settleMoment({"working":false,"asking":false,"failed":false,"startedAt":1,"endedAt":5,"readAt":4,"settledAt":null}, 1000))
        assert_eq!(moment(settle_moment(&SettleFacts { working: false, asking: false, failed: false, started_at: Some(1.0), ended_at: Some(5.0), read_at: Some(4.0), settled_at: None }, None, false)), "null"); // JSON.stringify(settleMoment({"working":false,"asking":false,"failed":false,"startedAt":1,"endedAt":5,"readAt":4,"settledAt":null}, null))
        assert_eq!(moment(settle_moment(&SettleFacts { working: false, asking: false, failed: false, started_at: Some(1.0), ended_at: Some(5.0), read_at: Some(4.0), settled_at: None }, Some(1000.0), true)), "null"); // JSON.stringify(settleMoment({"working":false,"asking":false,"failed":false,"startedAt":1,"endedAt":5,"readAt":4,"settledAt":null}, 1000, true))
        assert_eq!(thread_settled(&SettleFacts { working: false, asking: false, failed: false, started_at: Some(1.0), ended_at: Some(5.0), read_at: Some(4.0), settled_at: None }, 1006.0, Some(1000.0), false), false); // threadSettled({"working":false,"asking":false,"failed":false,"startedAt":1,"endedAt":5,"readAt":4,"settledAt":null}, 1006, 1000)
        assert_eq!(thread_settled(&SettleFacts { working: false, asking: false, failed: false, started_at: Some(1.0), ended_at: Some(5.0), read_at: Some(4.0), settled_at: None }, 1005.0, Some(1000.0), false), false); // threadSettled({"working":false,"asking":false,"failed":false,"startedAt":1,"endedAt":5,"readAt":4,"settledAt":null}, 1005, 1000)
        assert_eq!(thread_seen(&SettleFacts { working: false, asking: false, failed: false, started_at: Some(1.0), ended_at: Some(5.0), read_at: None, settled_at: None }), false); // threadSeen({"working":false,"asking":false,"failed":false,"startedAt":1,"endedAt":5,"readAt":null,"settledAt":null})
        assert_eq!(moment(settle_moment(&SettleFacts { working: false, asking: false, failed: false, started_at: Some(1.0), ended_at: Some(5.0), read_at: None, settled_at: None }, Some(1000.0), false)), "null"); // JSON.stringify(settleMoment({"working":false,"asking":false,"failed":false,"startedAt":1,"endedAt":5,"readAt":null,"settledAt":null}, 1000))
        assert_eq!(moment(settle_moment(&SettleFacts { working: false, asking: false, failed: false, started_at: Some(1.0), ended_at: Some(5.0), read_at: None, settled_at: None }, None, false)), "null"); // JSON.stringify(settleMoment({"working":false,"asking":false,"failed":false,"startedAt":1,"endedAt":5,"readAt":null,"settledAt":null}, null))
        assert_eq!(moment(settle_moment(&SettleFacts { working: false, asking: false, failed: false, started_at: Some(1.0), ended_at: Some(5.0), read_at: None, settled_at: None }, Some(1000.0), true)), "null"); // JSON.stringify(settleMoment({"working":false,"asking":false,"failed":false,"startedAt":1,"endedAt":5,"readAt":null,"settledAt":null}, 1000, true))
        assert_eq!(thread_settled(&SettleFacts { working: false, asking: false, failed: false, started_at: Some(1.0), ended_at: Some(5.0), read_at: None, settled_at: None }, 1006.0, Some(1000.0), false), false); // threadSettled({"working":false,"asking":false,"failed":false,"startedAt":1,"endedAt":5,"readAt":null,"settledAt":null}, 1006, 1000)
        assert_eq!(thread_settled(&SettleFacts { working: false, asking: false, failed: false, started_at: Some(1.0), ended_at: Some(5.0), read_at: None, settled_at: None }, 1005.0, Some(1000.0), false), false); // threadSettled({"working":false,"asking":false,"failed":false,"startedAt":1,"endedAt":5,"readAt":null,"settledAt":null}, 1005, 1000)
        assert_eq!(thread_seen(&SettleFacts { working: false, asking: false, failed: true, started_at: Some(1.0), ended_at: Some(5.0), read_at: Some(6.0), settled_at: None }), true); // threadSeen({"working":false,"asking":false,"failed":true,"startedAt":1,"endedAt":5,"readAt":6,"settledAt":null})
        assert_eq!(moment(settle_moment(&SettleFacts { working: false, asking: false, failed: true, started_at: Some(1.0), ended_at: Some(5.0), read_at: Some(6.0), settled_at: None }, Some(1000.0), false)), "null"); // JSON.stringify(settleMoment({"working":false,"asking":false,"failed":true,"startedAt":1,"endedAt":5,"readAt":6,"settledAt":null}, 1000))
        assert_eq!(moment(settle_moment(&SettleFacts { working: false, asking: false, failed: true, started_at: Some(1.0), ended_at: Some(5.0), read_at: Some(6.0), settled_at: None }, None, false)), "null"); // JSON.stringify(settleMoment({"working":false,"asking":false,"failed":true,"startedAt":1,"endedAt":5,"readAt":6,"settledAt":null}, null))
        assert_eq!(moment(settle_moment(&SettleFacts { working: false, asking: false, failed: true, started_at: Some(1.0), ended_at: Some(5.0), read_at: Some(6.0), settled_at: None }, Some(1000.0), true)), "null"); // JSON.stringify(settleMoment({"working":false,"asking":false,"failed":true,"startedAt":1,"endedAt":5,"readAt":6,"settledAt":null}, 1000, true))
        assert_eq!(thread_settled(&SettleFacts { working: false, asking: false, failed: true, started_at: Some(1.0), ended_at: Some(5.0), read_at: Some(6.0), settled_at: None }, 1006.0, Some(1000.0), false), false); // threadSettled({"working":false,"asking":false,"failed":true,"startedAt":1,"endedAt":5,"readAt":6,"settledAt":null}, 1006, 1000)
        assert_eq!(thread_settled(&SettleFacts { working: false, asking: false, failed: true, started_at: Some(1.0), ended_at: Some(5.0), read_at: Some(6.0), settled_at: None }, 1005.0, Some(1000.0), false), false); // threadSettled({"working":false,"asking":false,"failed":true,"startedAt":1,"endedAt":5,"readAt":6,"settledAt":null}, 1005, 1000)
        assert_eq!(thread_seen(&SettleFacts { working: false, asking: false, failed: false, started_at: Some(1.0), ended_at: Some(5.0), read_at: None, settled_at: Some(7.0) }), false); // threadSeen({"working":false,"asking":false,"failed":false,"startedAt":1,"endedAt":5,"readAt":null,"settledAt":7})
        assert_eq!(moment(settle_moment(&SettleFacts { working: false, asking: false, failed: false, started_at: Some(1.0), ended_at: Some(5.0), read_at: None, settled_at: Some(7.0) }, Some(1000.0), false)), "{\"ms\":7,\"byHand\":true}"); // JSON.stringify(settleMoment({"working":false,"asking":false,"failed":false,"startedAt":1,"endedAt":5,"readAt":null,"settledAt":7}, 1000))
        assert_eq!(moment(settle_moment(&SettleFacts { working: false, asking: false, failed: false, started_at: Some(1.0), ended_at: Some(5.0), read_at: None, settled_at: Some(7.0) }, None, false)), "{\"ms\":7,\"byHand\":true}"); // JSON.stringify(settleMoment({"working":false,"asking":false,"failed":false,"startedAt":1,"endedAt":5,"readAt":null,"settledAt":7}, null))
        assert_eq!(moment(settle_moment(&SettleFacts { working: false, asking: false, failed: false, started_at: Some(1.0), ended_at: Some(5.0), read_at: None, settled_at: Some(7.0) }, Some(1000.0), true)), "{\"ms\":7,\"byHand\":true}"); // JSON.stringify(settleMoment({"working":false,"asking":false,"failed":false,"startedAt":1,"endedAt":5,"readAt":null,"settledAt":7}, 1000, true))
        assert_eq!(thread_settled(&SettleFacts { working: false, asking: false, failed: false, started_at: Some(1.0), ended_at: Some(5.0), read_at: None, settled_at: Some(7.0) }, 1006.0, Some(1000.0), false), true); // threadSettled({"working":false,"asking":false,"failed":false,"startedAt":1,"endedAt":5,"readAt":null,"settledAt":7}, 1006, 1000)
        assert_eq!(thread_settled(&SettleFacts { working: false, asking: false, failed: false, started_at: Some(1.0), ended_at: Some(5.0), read_at: None, settled_at: Some(7.0) }, 1005.0, Some(1000.0), false), true); // threadSettled({"working":false,"asking":false,"failed":false,"startedAt":1,"endedAt":5,"readAt":null,"settledAt":7}, 1005, 1000)
        assert_eq!(thread_seen(&SettleFacts { working: false, asking: false, failed: false, started_at: Some(1.0), ended_at: Some(5.0), read_at: Some(6.0), settled_at: Some(3.0) }), true); // threadSeen({"working":false,"asking":false,"failed":false,"startedAt":1,"endedAt":5,"readAt":6,"settledAt":3})
        assert_eq!(moment(settle_moment(&SettleFacts { working: false, asking: false, failed: false, started_at: Some(1.0), ended_at: Some(5.0), read_at: Some(6.0), settled_at: Some(3.0) }, Some(1000.0), false)), "{\"ms\":1006,\"byHand\":false}"); // JSON.stringify(settleMoment({"working":false,"asking":false,"failed":false,"startedAt":1,"endedAt":5,"readAt":6,"settledAt":3}, 1000))
        assert_eq!(moment(settle_moment(&SettleFacts { working: false, asking: false, failed: false, started_at: Some(1.0), ended_at: Some(5.0), read_at: Some(6.0), settled_at: Some(3.0) }, None, false)), "null"); // JSON.stringify(settleMoment({"working":false,"asking":false,"failed":false,"startedAt":1,"endedAt":5,"readAt":6,"settledAt":3}, null))
        assert_eq!(moment(settle_moment(&SettleFacts { working: false, asking: false, failed: false, started_at: Some(1.0), ended_at: Some(5.0), read_at: Some(6.0), settled_at: Some(3.0) }, Some(1000.0), true)), "null"); // JSON.stringify(settleMoment({"working":false,"asking":false,"failed":false,"startedAt":1,"endedAt":5,"readAt":6,"settledAt":3}, 1000, true))
        assert_eq!(thread_settled(&SettleFacts { working: false, asking: false, failed: false, started_at: Some(1.0), ended_at: Some(5.0), read_at: Some(6.0), settled_at: Some(3.0) }, 1006.0, Some(1000.0), false), true); // threadSettled({"working":false,"asking":false,"failed":false,"startedAt":1,"endedAt":5,"readAt":6,"settledAt":3}, 1006, 1000)
        assert_eq!(thread_settled(&SettleFacts { working: false, asking: false, failed: false, started_at: Some(1.0), ended_at: Some(5.0), read_at: Some(6.0), settled_at: Some(3.0) }, 1005.0, Some(1000.0), false), false); // threadSettled({"working":false,"asking":false,"failed":false,"startedAt":1,"endedAt":5,"readAt":6,"settledAt":3}, 1005, 1000)
        assert_eq!(thread_seen(&SettleFacts { working: false, asking: false, failed: false, started_at: Some(9.0), ended_at: None, read_at: Some(10.0), settled_at: None }), true); // threadSeen({"working":false,"asking":false,"failed":false,"startedAt":9,"endedAt":null,"readAt":10,"settledAt":null})
        assert_eq!(moment(settle_moment(&SettleFacts { working: false, asking: false, failed: false, started_at: Some(9.0), ended_at: None, read_at: Some(10.0), settled_at: None }, Some(1000.0), false)), "{\"ms\":1010,\"byHand\":false}"); // JSON.stringify(settleMoment({"working":false,"asking":false,"failed":false,"startedAt":9,"endedAt":null,"readAt":10,"settledAt":null}, 1000))
        assert_eq!(moment(settle_moment(&SettleFacts { working: false, asking: false, failed: false, started_at: Some(9.0), ended_at: None, read_at: Some(10.0), settled_at: None }, None, false)), "null"); // JSON.stringify(settleMoment({"working":false,"asking":false,"failed":false,"startedAt":9,"endedAt":null,"readAt":10,"settledAt":null}, null))
        assert_eq!(moment(settle_moment(&SettleFacts { working: false, asking: false, failed: false, started_at: Some(9.0), ended_at: None, read_at: Some(10.0), settled_at: None }, Some(1000.0), true)), "null"); // JSON.stringify(settleMoment({"working":false,"asking":false,"failed":false,"startedAt":9,"endedAt":null,"readAt":10,"settledAt":null}, 1000, true))
        assert_eq!(thread_settled(&SettleFacts { working: false, asking: false, failed: false, started_at: Some(9.0), ended_at: None, read_at: Some(10.0), settled_at: None }, 1006.0, Some(1000.0), false), false); // threadSettled({"working":false,"asking":false,"failed":false,"startedAt":9,"endedAt":null,"readAt":10,"settledAt":null}, 1006, 1000)
        assert_eq!(thread_settled(&SettleFacts { working: false, asking: false, failed: false, started_at: Some(9.0), ended_at: None, read_at: Some(10.0), settled_at: None }, 1005.0, Some(1000.0), false), false); // threadSettled({"working":false,"asking":false,"failed":false,"startedAt":9,"endedAt":null,"readAt":10,"settledAt":null}, 1005, 1000)
        assert_eq!(thread_seen(&SettleFacts { working: false, asking: false, failed: false, started_at: None, ended_at: None, read_at: None, settled_at: Some(2.0) }), true); // threadSeen({"working":false,"asking":false,"failed":false,"startedAt":null,"endedAt":null,"readAt":null,"settledAt":2})
        assert_eq!(moment(settle_moment(&SettleFacts { working: false, asking: false, failed: false, started_at: None, ended_at: None, read_at: None, settled_at: Some(2.0) }, Some(1000.0), false)), "{\"ms\":2,\"byHand\":true}"); // JSON.stringify(settleMoment({"working":false,"asking":false,"failed":false,"startedAt":null,"endedAt":null,"readAt":null,"settledAt":2}, 1000))
        assert_eq!(moment(settle_moment(&SettleFacts { working: false, asking: false, failed: false, started_at: None, ended_at: None, read_at: None, settled_at: Some(2.0) }, None, false)), "{\"ms\":2,\"byHand\":true}"); // JSON.stringify(settleMoment({"working":false,"asking":false,"failed":false,"startedAt":null,"endedAt":null,"readAt":null,"settledAt":2}, null))
        assert_eq!(moment(settle_moment(&SettleFacts { working: false, asking: false, failed: false, started_at: None, ended_at: None, read_at: None, settled_at: Some(2.0) }, Some(1000.0), true)), "{\"ms\":2,\"byHand\":true}"); // JSON.stringify(settleMoment({"working":false,"asking":false,"failed":false,"startedAt":null,"endedAt":null,"readAt":null,"settledAt":2}, 1000, true))
        assert_eq!(thread_settled(&SettleFacts { working: false, asking: false, failed: false, started_at: None, ended_at: None, read_at: None, settled_at: Some(2.0) }, 1006.0, Some(1000.0), false), true); // threadSettled({"working":false,"asking":false,"failed":false,"startedAt":null,"endedAt":null,"readAt":null,"settledAt":2}, 1006, 1000)
        assert_eq!(thread_settled(&SettleFacts { working: false, asking: false, failed: false, started_at: None, ended_at: None, read_at: None, settled_at: Some(2.0) }, 1005.0, Some(1000.0), false), true); // threadSettled({"working":false,"asking":false,"failed":false,"startedAt":null,"endedAt":null,"readAt":null,"settledAt":2}, 1005, 1000)
    }
}

mod wsp_tool_calls {
    use wsp_words::wsp_tools::*;

    fn servers(s: serde_json::Value) -> indexmap::IndexMap<String, wsp_wire::views::harness::McpServerSpec> {
        serde_json::from_value(s).unwrap()
    }

    fn followed(f: Option<Followed>) -> String {
        match f {
            None => "null".into(),
            Some(Followed::Opened) => "{\"opened\":true}".into(),
            Some(Followed::Named(n)) => serde_json::to_string(&serde_json::json!({ "named": n })).unwrap(),
        }
    }

    #[test]
    fn a_spawn_is_wsps_run_or_fork_and_a_slate_rides_unless_turned_off() {
        assert_eq!(spawns_thread("mcp__wsp__run"), true); // spawnsThread("mcp__wsp__run")
        assert_eq!(spawns_thread("mcp__wsp__fork"), true); // spawnsThread("mcp__wsp__fork")
        assert_eq!(spawns_thread("mcp__wsp__send"), false); // spawnsThread("mcp__wsp__send")
        assert_eq!(spawns_thread("mcp__other__run"), false); // spawnsThread("mcp__other__run")
        assert_eq!(spawns_thread("run"), false); // spawnsThread("run")
        assert_eq!(launch_has_slate(None), false); // launchHasSlate(undefined)
        assert_eq!(launch_has_slate(Some(&servers(serde_json::json!({})))), false); // launchHasSlate({})
        assert_eq!(launch_has_slate(Some(&servers(serde_json::json!({ "wsp": { "command": "wsp", "args": [] }, "other": { "command": "x", "args": [], "noSlate": true } })))), true); // launchHasSlate({wsp:{...}, other:{noSlate:true}})
        assert_eq!(launch_has_slate(Some(&servers(serde_json::json!({ "other": { "command": "x", "args": [] } })))), false); // launchHasSlate({other:{...}})
        assert_eq!(launch_has_slate(Some(&servers(serde_json::json!({ "wsp": { "command": "wsp", "args": [], "noSlate": true } })))), false); // launchHasSlate({wsp:{noSlate:true}})
        // launchHasSlate({wsp:{noSlate:false}}) is outside McpServerSpec, whose noSlate is `true` or absent.
    }

    #[test]
    fn a_call_follows_the_threads_it_names_or_opens_unless_detached() {
        assert_eq!(followed(threads_followed("mcp__wsp__send", "{\"thread\":\"t1\"}")), "{\"named\":[\"t1\"]}"); // JSON.stringify(threadsFollowed({toolName:"mcp__wsp__send",input:"{\"thread\":\"t1\"}"}) ?? null)
        assert_eq!(followed(threads_followed("mcp__wsp__send", "{\"thread\":\"\"}")), "null"); // JSON.stringify(threadsFollowed({toolName:"mcp__wsp__send",input:"{\"thread\":\"\"}"}) ?? null)
        assert_eq!(followed(threads_followed("mcp__wsp__send", "{\"thread\":\"t1\",\"detach\":true}")), "null"); // JSON.stringify(threadsFollowed({toolName:"mcp__wsp__send",input:"{\"thread\":\"t1\",\"detach\":true}"}) ?? null)
        assert_eq!(followed(threads_followed("mcp__wsp__send", "{\"thread\":\"t1\",\"detach\":\"yes\"}")), "{\"named\":[\"t1\"]}"); // JSON.stringify(threadsFollowed({toolName:"mcp__wsp__send",input:"{\"thread\":\"t1\",\"detach\":\"yes\"}"}) ?? null)
        assert_eq!(followed(threads_followed("mcp__wsp__run", "{}")), "{\"opened\":true}"); // JSON.stringify(threadsFollowed({toolName:"mcp__wsp__run",input:"{}"}) ?? null)
        assert_eq!(followed(threads_followed("mcp__wsp__run", "{\"detach\":true}")), "null"); // JSON.stringify(threadsFollowed({toolName:"mcp__wsp__run",input:"{\"detach\":true}"}) ?? null)
        assert_eq!(followed(threads_followed("mcp__wsp__threads_wait", "{\"threads\":[\"a\",\"\",3,\"b\"]}")), "{\"named\":[\"a\",\"b\"]}"); // JSON.stringify(threadsFollowed({toolName:"mcp__wsp__threads_wait",input:"{\"threads\":[\"a\",\"\",3,\"b\"]}"}) ?? null)
        assert_eq!(followed(threads_followed("mcp__wsp__threads_wait", "{\"threads\":[]}")), "null"); // JSON.stringify(threadsFollowed({toolName:"mcp__wsp__threads_wait",input:"{\"threads\":[]}"}) ?? null)
        assert_eq!(followed(threads_followed("mcp__wsp__threads_wait", "{\"threads\":\"a\"}")), "{\"named\":[\"a\"]}"); // JSON.stringify(threadsFollowed({toolName:"mcp__wsp__threads_wait",input:"{\"threads\":\"a\"}"}) ?? null)
        assert_eq!(followed(threads_followed("mcp__wsp__send", "{bad")), "null"); // JSON.stringify(threadsFollowed({toolName:"mcp__wsp__send",input:"{bad"}) ?? null)
        assert_eq!(followed(threads_followed("mcp__wsp__send", "null")), "null"); // JSON.stringify(threadsFollowed({toolName:"mcp__wsp__send",input:"null"}) ?? null)
        assert_eq!(followed(threads_followed("mcp__wsp__send", "[1]")), "null"); // JSON.stringify(threadsFollowed({toolName:"mcp__wsp__send",input:"[1]"}) ?? null)
        assert_eq!(followed(threads_followed("mcp__other__send", "{\"thread\":\"t\"}")), "null"); // JSON.stringify(threadsFollowed({toolName:"mcp__other__send",input:"{\"thread\":\"t\"}"}) ?? null)
        assert_eq!(followed(threads_followed("mcp__wsp__fork", "{}")), "null"); // JSON.stringify(threadsFollowed({toolName:"mcp__wsp__fork",input:"{}"}) ?? null)
    }
}
