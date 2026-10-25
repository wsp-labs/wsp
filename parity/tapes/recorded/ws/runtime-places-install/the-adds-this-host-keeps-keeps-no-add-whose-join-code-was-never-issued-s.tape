{"meta":{"door":"ws","source":"packages/runtime/test/places-install.test.ts > the adds this host keeps > keeps no add whose join code was never issued, since nothing would ever end it","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"addId":"a_ro","address":"root@10.0.0.9","id":2,"op":"places.add"}}
{"out":{"id":2,"ok":false,"error":"the state file is read-only"}}
{"in":{"id":3,"op":"places.list"}}
{"out":{"id":3,"ok":true,"places":[{"id":"here","kind":"computer","name":"zingzys-mac","mac":"mac-mini","default":true,"os":"macOS 15.0","shape":{"cpu":8,"memMb":16384},"engine":"docker","home":"/home/dev","present":true,"takesForks":false,"buildsImages":false,"cap":{"threads":8},"capDefault":{"threads":8},"turnLimitMs":null,"turnLimitDefault":null,"spawn":{"spawn":true,"maxMachines":3,"maxDepth":2},"spawnDefault":{"spawn":true,"maxMachines":3,"maxDepth":2},"running":0}],"adds":[],"pending":[{"id":"a_ro","address":"root@10.0.0.9","step":"connect","choices":{"name":"root@10.0.0.9","agents":{},"mcp":{},"clis":{},"skills":{},"plugins":{},"folders":{},"configs":{}},"startedAt":"2026-10-25T15:22:23.912Z"}]}}
