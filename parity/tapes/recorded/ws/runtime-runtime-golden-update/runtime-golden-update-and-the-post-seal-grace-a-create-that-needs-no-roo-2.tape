{"meta":{"door":"ws","source":"packages/runtime/test/runtime-golden-update.test.ts > runtime golden update and the post-seal grace > a create that needs no room carries no notice, on the result and on the wire; one that made room carries it beside the workspace","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"project":"pr_00000001","golden":"snap_wsp-h1s1-default-v1","name":"room","id":2,"op":"workspaces.create"}}
{"out":{"id":2,"ok":true,"workspace":{"id":"ws_00000002","name":"room","machineId":"m6","phase":"running","kind":"cloud","golden":"snap_wsp-h1s1-default-v1","createdAt":"2026-10-25T15:21:22.842Z","project":{"id":"pr_00000001","name":"stub-13","path":"/root/stub-13","computer":"default"},"home":"/root","agents":{"spawn":true,"maxMachines":3,"maxDepth":2},"provider":"default","pauseMode":"memory"},"notice":"Stopped the builder kept from image v1 to make room at the machine cap."}}
