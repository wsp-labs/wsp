// SPDX-License-Identifier: AGPL-3.0-only
//! conversations.ts has no test file of its own at a3b870a6e; every case below is pinned off the TS under node.

// Cases the spec pins; every expected value is what the TS function returned under node at a3b870a6e.
mod pinned {
    use wsp_words::conversations::*;

    #[test]
    fn the_folder_stamp_command_joins_folders_as_given() {
        assert_eq!(wsp_words::conversations::folders_stamp_command(&[] as &[&str]), "for d in ; do printf '%s %s\\n' \"$(stat -c %Y \"$d\" 2>/dev/null || stat -f %m \"$d\" 2>/dev/null)\" \"$(ls -A \"$d\" 2>/dev/null | wc -l)\"; done"); // foldersStampCommand([])
        assert_eq!(wsp_words::conversations::folders_stamp_command(&["/a"] as &[&str]), "for d in /a; do printf '%s %s\\n' \"$(stat -c %Y \"$d\" 2>/dev/null || stat -f %m \"$d\" 2>/dev/null)\" \"$(ls -A \"$d\" 2>/dev/null | wc -l)\"; done"); // foldersStampCommand(["/a"])
        assert_eq!(wsp_words::conversations::folders_stamp_command(&["/a","'/b c'"] as &[&str]), "for d in /a '/b c'; do printf '%s %s\\n' \"$(stat -c %Y \"$d\" 2>/dev/null || stat -f %m \"$d\" 2>/dev/null)\" \"$(ls -A \"$d\" 2>/dev/null | wc -l)\"; done"); // foldersStampCommand(["/a","'/b c'"])
    }

    #[test]
    fn resume_lines_and_refusals() {
        assert_eq!(wsp_words::conversations::earlier_line(0.0, "Codex"), "0 earlier messages stay in Codex's own history"); // earlierLine(0, "Codex")
        assert_eq!(wsp_words::conversations::earlier_line(1.0, "Claude Code"), "1 earlier message stays in Claude Code's own history"); // earlierLine(1, "Claude Code")
        assert_eq!(wsp_words::conversations::earlier_line(2.0, "Codex"), "2 earlier messages stay in Codex's own history"); // earlierLine(2, "Codex")
        assert_eq!(wsp_words::conversations::earlier_line(200.0, "Claude Code"), "200 earlier messages stay in Claude Code's own history"); // earlierLine(200, "Claude Code")
        assert_eq!(wsp_words::conversations::earlier_line(1.5, "Codex"), "1.5 earlier messages stay in Codex's own history"); // earlierLine(1.5, "Codex")
        assert_eq!(wsp_words::conversations::conversation_open_line("Fix the bug", "spoo"), "Fix the bug is open in another app on spoo, so continuing it here would split it"); // conversationOpenLine("Fix the bug", "spoo")
        assert_eq!(wsp_words::conversations::conversation_open_fix(None), "Continue in a copy with --copy, or close it there and try again."); // conversationOpenFix(undefined)
        assert_eq!(wsp_words::conversations::conversation_written_elsewhere_line("Codex", None), "Codex is writing this conversation in another app, so this turn did not run. Close it there and send again."); // conversationWrittenElsewhereLine("Codex", undefined)
        assert_eq!(wsp_words::conversations::conversation_open_fix(Some("Codex lets go of a closed one within a minute.")), "Continue in a copy with --copy, or close it there and try again. Codex lets go of a closed one within a minute."); // conversationOpenFix("Codex lets go of a closed one within a minute.")
        assert_eq!(wsp_words::conversations::conversation_written_elsewhere_line("Codex", Some("Codex lets go of a closed one within a minute.")), "Codex is writing this conversation in another app, so this turn did not run. Close it there and send again. Codex lets go of a closed one within a minute."); // conversationWrittenElsewhereLine("Codex", "Codex lets go of a closed one within a minute.")
        assert_eq!(wsp_words::conversations::conversation_gone_line("Codex", "abc", "spoo"), "Codex holds no conversation abc on spoo any more"); // conversationGoneLine("Codex", "abc", "spoo")
        assert_eq!(wsp_words::conversations::conversation_is_thread_line("c1", "01HABCDEFGHJKMNP"), "conversation c1 is thread 01HABCDE already"); // conversationIsThreadLine("c1", "01HABCDEFGHJKMNP")
        assert_eq!(wsp_words::conversations::conversation_is_thread_fix("01HABCDEFGHJKMNP"), "Send into it with wsp send 01HABCDE."); // conversationIsThreadFix("01HABCDEFGHJKMNP")
        assert_eq!(wsp_words::conversations::conversation_is_thread_line("c1", "abc"), "conversation c1 is thread abc already"); // conversationIsThreadLine("c1", "abc")
        assert_eq!(wsp_words::conversations::conversation_is_thread_fix("abc"), "Send into it with wsp send abc."); // conversationIsThreadFix("abc")
        assert_eq!(wsp_words::conversations::no_conversations_line("OpenCode"), "wsp cannot pick up OpenCode conversations"); // noConversationsLine("OpenCode")
        assert_eq!(wsp_words::conversations::conversations_unread_line("Codex", "spoo"), "Codex conversations on spoo were not read"); // conversationsUnreadLine("Codex", "spoo")
        assert_eq!(wsp_words::conversations::conversations_failed_line("Codex", "the store said no"), "Codex's conversations could not be read: the store said no"); // conversationsFailedLine("Codex", "the store said no")
        assert_eq!(wsp_words::conversations::conversations_update_fix("spoo"), "Update spoo with wsp add spoo --update to list them."); // conversationsUpdateFix("spoo")
    }

    #[test]
    fn the_picker_words() {
        assert_eq!(wsp_words::conversations::conversation_words::open_note(None), "Continuing it here too would split it. Continue in a copy, or close it there first."); // CONVERSATION_WORDS.openNote(undefined)
        assert_eq!(wsp_words::conversations::conversation_words::open_note(Some("Codex lets go of a closed one within a minute.")), "Continue in a copy, or close it there and try again. Codex lets go of a closed one within a minute."); // CONVERSATION_WORDS.openNote("Codex lets go of a closed one within a minute.")
        assert_eq!(wsp_words::conversations::conversation_words::on_branch("main"), "On main"); // CONVERSATION_WORDS.onBranch("main")
        assert_eq!(wsp_words::conversations::conversation_words::empty_in("wsp"), "No conversations in wsp yet"); // CONVERSATION_WORDS.emptyIn("wsp")
        assert_eq!(wsp_words::conversations::conversation_words::group("wsp"), "Conversations in wsp"); // CONVERSATION_WORDS.group("wsp")
        assert_eq!(wsp_words::conversations::conversation_words::reading("spoo"), "Reading conversations on spoo"); // CONVERSATION_WORDS.reading("spoo")
        assert_eq!(wsp_words::conversations::conversation_words::continues("Fix", true), "Continues a copy of Fix"); // CONVERSATION_WORDS.continues("Fix", true)
        assert_eq!(wsp_words::conversations::conversation_words::continues("Fix", false), "Continues Fix"); // CONVERSATION_WORDS.continues("Fix", false)
        assert_eq!(wsp_words::conversations::conversation_words::open_title("Fix", "spoo"), "Fix is open in another app on spoo"); // CONVERSATION_WORDS.openTitle("Fix", "spoo")
    }

    #[test]
    fn a_lasting_store_error_is_marked() {
        assert_eq!(wsp_words::conversations::is_lasting_store_error(&wsp_words::conversations::lasting_store_error("codex is not installed")), true); // isLastingStoreError(lastingStoreError("codex is not installed"))
        assert_eq!(wsp_words::conversations::lasting_store_error("codex is not installed").message, "codex is not installed"); // lastingStoreError("codex is not installed").message
    }
}
