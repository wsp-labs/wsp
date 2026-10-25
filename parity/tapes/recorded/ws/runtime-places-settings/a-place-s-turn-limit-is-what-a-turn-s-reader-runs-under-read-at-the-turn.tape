{"meta":{"door":"ws","source":"packages/runtime/test/places-settings.test.ts > a place's turn limit > is what a turn's reader runs under, read at the turn's launch, and a turn it stops says so and how to go on","outs":"recorded"}}
{"in":{"token":"<<host-token>>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"placeId":"here","turnLimitMs":7200000,"id":2,"op":"places.set"}}
{"out":{"id":2,"ok":true,"place":{"id":"here","kind":"computer","name":"zingzys-mac","mac":"mac-mini","default":true,"os":"macOS 15.0","shape":{"cpu":8,"memMb":16384},"engine":"docker","home":"/home/dev","present":true,"takesForks":false,"buildsImages":false,"cap":{"threads":8},"capDefault":{"threads":8},"settings":{"turnLimitMs":7200000},"turnLimitMs":7200000,"turnLimitDefault":null,"spawn":{"spawn":true,"maxMachines":3,"maxDepth":2},"spawnDefault":{"spawn":true,"maxMachines":3,"maxDepth":2},"running":0}}}
