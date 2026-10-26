{"meta":{"door":"ws","source":"packages/runtime/test/local-workspace.test.ts > local workspace > a computer the person paired is refused every op outside the device list at the door, on every kind of workspace, and nothing runs","outs":"recorded"}}
{"in":{"token":"<redacted-1>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"workspaceId":"ws_00000001","prompt":"hi","id":2,"op":"sessions.start"}}
{"out":{"id":2,"ok":false,"error":"sessions.start is not a paired computer's to ask for until the owner gives this device a role; run it on the computer the host runs on"}}
{"in":{"sessionId":"s_none","prompt":"and this","id":3,"op":"sessions.steer"}}
{"out":{"id":3,"ok":false,"error":"sessions.steer is not a paired computer's to ask for until the owner gives this device a role; run it on the computer the host runs on"}}
{"in":{"workspaceId":"ws_00000001","argv":["true"],"id":4,"op":"workspaces.exec"}}
{"out":{"id":4,"ok":false,"error":"workspaces.exec is not a paired computer's to ask for until the owner gives this device a role; run it on the computer the host runs on"}}
{"in":{"workspaceId":"ws_00000001","id":5,"op":"workspaces.bringBack"}}
{"out":{"id":5,"ok":false,"error":"workspaces.bringBack is not a paired computer's to ask for until the owner gives this device a role; run it on the computer the host runs on"}}
{"in":{"workspaceId":"ws_00000001","id":6,"op":"daemon.open"}}
{"out":{"id":6,"ok":false,"error":"daemon.open is not a paired computer's to ask for until the owner gives this device a role; run it on the computer the host runs on"}}
{"in":{"workspaceId":"ws_00000002","prompt":"hi","id":7,"op":"sessions.start"}}
{"out":{"id":7,"ok":false,"error":"sessions.start is not a paired computer's to ask for until the owner gives this device a role; run it on the computer the host runs on"}}
{"in":{"sessionId":"s_none","prompt":"and this","id":8,"op":"sessions.steer"}}
{"out":{"id":8,"ok":false,"error":"sessions.steer is not a paired computer's to ask for until the owner gives this device a role; run it on the computer the host runs on"}}
{"in":{"workspaceId":"ws_00000002","argv":["true"],"id":9,"op":"workspaces.exec"}}
{"out":{"id":9,"ok":false,"error":"workspaces.exec is not a paired computer's to ask for until the owner gives this device a role; run it on the computer the host runs on"}}
{"in":{"workspaceId":"ws_00000002","id":10,"op":"workspaces.bringBack"}}
{"out":{"id":10,"ok":false,"error":"workspaces.bringBack is not a paired computer's to ask for until the owner gives this device a role; run it on the computer the host runs on"}}
{"in":{"workspaceId":"ws_00000002","id":11,"op":"daemon.open"}}
{"out":{"id":11,"ok":false,"error":"daemon.open is not a paired computer's to ask for until the owner gives this device a role; run it on the computer the host runs on"}}
{"in":{"placeId":"pl_box","id":12,"op":"daemon.open"}}
{"out":{"id":12,"ok":false,"error":"daemon.open is not a paired computer's to ask for until the owner gives this device a role; run it on the computer the host runs on"}}
{"in":{"sessionId":"s_none","askId":"a1","optionId":"yes","id":13,"op":"sessions.answer"}}
{"out":{"id":13,"ok":false,"error":"sessions.answer is not a paired computer's to ask for until the owner gives this device a role; run it on the computer the host runs on"}}
{"in":{"sessionId":"s_none","permissionMode":"bypassPermissions","id":14,"op":"sessions.access"}}
{"out":{"id":14,"ok":false,"error":"sessions.access is not a paired computer's to ask for until the owner gives this device a role; run it on the computer the host runs on"}}
{"in":{"prompt":"hi","id":15,"op":"sessions.start"}}
{"out":{"id":15,"ok":false,"error":"sessions.start is not a paired computer's to ask for until the owner gives this device a role; run it on the computer the host runs on"}}
{"in":{"id":16,"op":"workspaces.list"}}
{"out":{"id":16,"ok":true,"workspaces":[{"id":"ws_00000001","name":"mac","machineId":"local","phase":"running","kind":"local","golden":"","createdAt":"2026-10-25T17:46:18.051Z","project":{"id":"pr_00000003","name":"mac","path":"<run>/wsp-project-000001","computer":"here"},"folder":"<run>/wsp-project-000001","home":"<run>/wsp-localws-000002","agents":{"spawn":true,"maxMachines":3,"maxDepth":2}},{"id":"ws_00000002","name":"b1","machineId":"m1","phase":"running","kind":"cloud","golden":"snap_g","createdAt":"2026-10-25T17:46:18.057Z","project":{"id":"pr_00000004","name":"stub-4","path":"/root/stub-4","computer":"default"},"home":"/root","agents":{"spawn":true,"maxMachines":3,"maxDepth":2},"provider":"default","pauseMode":"memory"}]}}
{"in":{"id":17,"op":"status.list"}}
{"out":{"id":17,"ok":true,"statuses":[{"id":"ws_00000001","name":"mac","machineId":"local","phase":"running","kind":"local","golden":"","createdAt":"2026-10-25T17:46:18.051Z","project":{"id":"pr_00000003","name":"mac","path":"<run>/wsp-project-000001","computer":"here"},"folder":"<run>/wsp-project-000001","home":"<run>/wsp-localws-000002","agents":{"spawn":true,"maxMachines":3,"maxDepth":2},"machineState":"running","size":{"cpu":8,"memMb":31744},"rateUsdPerHour":0,"facts":{"os":"Ubuntu 24.04.5 LTS","uptimeMs":16193680,"folder":"<run>/wsp-localws-000002"},"reach":{"state":"unsupported"}},{"id":"ws_00000002","name":"b1","machineId":"m1","phase":"running","kind":"cloud","golden":"snap_g","createdAt":"2026-10-25T17:46:18.057Z","project":{"id":"pr_00000004","name":"stub-4","path":"/root/stub-4","computer":"default"},"home":"/root","agents":{"spawn":true,"maxMachines":3,"maxDepth":2},"provider":"default","pauseMode":"memory","machineState":"running","size":{"cpu":2,"memMb":4096},"rateUsdPerHour":0.11000000000000001,"idleAt":1792951578062,"reach":{"state":"unsupported"}}]}}
{"in":{"id":18,"op":"sessions.list"}}
{"out":{"id":18,"ok":true,"sessions":[]}}
{"in":{"id":19,"op":"devices.list"}}
{"out":{"id":19,"ok":true,"devices":[{"id":"d_00000005","name":"laptop","createdAt":"2026-10-25T17:46:18.068Z","lastSeenAt":"2026-10-25T17:46:18.070Z"}]}}
{"in":{"project":"pr_00000004","name":"b2","golden":"snap_g","id":20,"op":"workspaces.create"}}
{"out":{"id":20,"ok":true,"workspace":{"id":"ws_00000006","name":"b2","machineId":"m2","phase":"running","kind":"cloud","golden":"snap_g","createdAt":"2026-10-25T17:46:18.091Z","project":{"id":"pr_00000004","name":"stub-4","path":"/root/stub-4","computer":"default"},"home":"/root","agents":{"spawn":true,"maxMachines":3,"maxDepth":2},"provider":"default","pauseMode":"memory"}}}
