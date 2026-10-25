{"meta":{"door":"ws","source":"packages/runtime/test/local-workspace.test.ts > local workspace > a computer the person paired is refused a thread's rename on a workspace here, since a rename runs a shell on the machine and writes the person's agent session file","outs":"recorded"}}
{"in":{"token":"<redacted-1>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"sessionId":"00000000-0000-4000-8000-000000000001","title":"renamed","id":2,"op":"sessions.rename"}}
{"out":{"id":2,"ok":false,"error":"sessions.rename is not a paired computer's to ask for until the owner gives this device a role; run it on the computer the host runs on"}}
{"in":{"workspaceId":"ws_00000001","name":"renamed","id":3,"op":"workspaces.rename"}}
{"out":{"id":3,"ok":true,"workspace":{"id":"ws_00000001","name":"renamed","machineId":"local","phase":"running","kind":"local","golden":"","createdAt":"2026-10-25T17:46:18.221Z","project":{"id":"pr_00000002","name":"mac","path":"<run>/wsp-project-000001","computer":"here"},"folder":"<run>/wsp-project-000001","home":"<run>/wsp-localws-000002","claudeSessionId":"00000000-0000-4000-8000-000000000001","agents":{"spawn":true,"maxMachines":3,"maxDepth":2}}}}
