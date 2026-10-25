# conversations
Source: the functions of `packages/protocol/src/conversations.ts` (183 lines) at a3b870a6e; its zod shapes (`OutsideConversation`, `ConversationsAnswer`, `ResumeAsk`, `ConversationRead`, ...) and flat constants (`EARLIER_MESSAGES`, `PREVIEW_MESSAGES`, `CONVERSATION_OPEN_KIND`, the `*_FIX` and `RESUME_*` lines) are `wsp_wire::types::conversations`. Tests: no TS test file of its own; `tests/conversations.rs` pins 33 values printed by the TS under node.

## What it does
The words of a conversation a thread picks up from an agent's own store: the shapes an adapter reads one into, the command that stamps the store's folders, and the lines and refusals of a resume.

## Behaviour
1. `ConversationLine`, `StoredConversation`, `ConversationEarlier` are the adapter-side shapes of :48-74 (not on the wire).
2. `lasting_store_error(message)`: an error marked lasting; `is_lasting_store_error` reads the mark (:98-100).
3. `folders_stamp_command(folders)`: the folders joined by " " as given (the caller quotes them) into the `for d in ...; do printf ...; done` line of :103-104, GNU stat then BSD stat.
4. `earlier_line(count, agent)`: "message stays" for exactly 1, else "messages stay", the count written by `number_string` (:159).
5. The resume lines and fixes (:161-183): `conversation_open_line`, `conversation_open_fix` (the agent's own lets-go sentence after one space where it has one), `conversation_written_elsewhere_line` (a refusal line), `conversation_gone_line`, `conversation_is_thread_line` and `_fix` (the thread id cut to 8 UTF-16 units), `no_conversations_line`, `conversations_unread_line`, `conversations_failed_line`, `conversations_update_fix`.
6. `conversation_words::*`: the picker's fixed words and its sentences (:126-157).

## Edge cases
- `folders_stamp_command(&[])` writes `for d in ; do ...`: an empty loop, kept.
- `earlier_line(1.5, ..)` reads "1.5 earlier messages stay".
- `conversation_is_thread_line("c1", "abc")` keeps a short id whole.

## Behaviour nobody wrote down
- Rule 8.11: counts in a sentence are `String(n)`, so `number_string`.

## Tapes
`wsp conversations`, `wsp run --resume` refusals, the app's Resume a conversation picker.

## Decisions
1. The TS `lasting: true` property is the mark "lasting" on a `HostError`.
2. `ConversationRoad` and `ConversationStore` are the adapters' async contract, ported with the adapters (wsp-adapters), not here.
3. CONVERSATION_WORDS is a module: fixed words are constants, sentences functions.

## After the port
- none.

## Rules pinned
| Rule | Test |
|---|---|
| 2 | `pinned::a_lasting_store_error_is_marked` |
| 3 | `pinned::the_folder_stamp_command_joins_folders_as_given` |
| 4, 5 | `pinned::resume_lines_and_refusals` |
| 6 | `pinned::the_picker_words` |
| 1 | shapes only; read by the adapters' tests |
