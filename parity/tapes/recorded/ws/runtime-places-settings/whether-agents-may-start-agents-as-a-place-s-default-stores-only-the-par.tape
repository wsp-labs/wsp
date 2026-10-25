{"meta":{"door":"ws","source":"packages/runtime/test/places-settings.test.ts > whether agents may start agents, as a place's default > stores only the parts a person set, so a switch turned off and on again follows a later default, and levels deep resets alone"}}
{"in":{"token":"<redacted>","id":1,"op":"auth"}}
{"in":{"placeId":"here","spawn":{"spawn":false},"id":2,"op":"places.set"}}
{"in":{"placeId":"here","spawn":{"spawn":true},"id":3,"op":"places.set"}}
{"in":{"placeId":"here","spawn":{"maxDepth":4},"id":4,"op":"places.set"}}
{"in":{"placeId":"here","reset":["max-depth"],"id":5,"op":"places.set"}}
{"in":{"placeId":"here","reset":["spawn"],"id":6,"op":"places.set"}}
