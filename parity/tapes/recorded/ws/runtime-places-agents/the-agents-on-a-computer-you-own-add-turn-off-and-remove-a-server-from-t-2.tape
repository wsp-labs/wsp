{"meta":{"door":"ws","source":"packages/runtime/test/places-agents.test.ts > the agents on a computer you own > add, turn off and remove a server from the host's own socket alone, each on the computer named, and never log the values an add carried","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"id":2,"op":"events.subscribe"}}
{"out":{"id":2,"ok":true,"seq":2,"stream":"00000000-0000-4000-8000-000000000001"}}
{"in":{"target":{"placeId":"here"},"agent":"claude","name":"acme","url":"https://mcp.acme.example/mcp","headers":{"Authorization":"<redacted>"},"id":3,"op":"servers.add"}}
{"out":{"type":"agents.changed","target":{"placeId":"here"},"seq":3}}
{"out":{"id":3,"ok":true,"file":"~/.claude.json"}}
{"in":{"target":{"placeId":"p_00000001"},"agent":"codex","name":"acme","on":false,"id":4,"op":"servers.toggle"}}
{"out":{"type":"agents.changed","target":{"placeId":"p_00000001"},"seq":4}}
{"out":{"id":4,"ok":true,"file":"~/.codex/config.toml"}}
{"in":{"target":{"placeId":"p_00000001"},"agent":"claude","name":"acme","scope":"home","id":5,"op":"servers.remove"}}
{"out":{"type":"agents.changed","target":{"placeId":"p_00000001"},"seq":5}}
{"out":{"id":5,"ok":true,"file":"~/.claude.json"}}
{"in":{"target":{"placeId":"p_00000001"},"agent":"claude","name":"echo","command":"npx","env":{"ACME_KEY":"<redacted>"},"headers":{"Authorization":"<redacted>"},"id":6,"op":"servers.add"}}
{"out":{"type":"agents.changed","target":{"placeId":"p_00000001"},"seq":6}}
{"out":{"id":6,"ok":false,"error":"refused {\"Authorization\":\"<redacted>\"} {\"ACME_KEY\":\"<redacted>\"}","kind":"usage"}}
{"in":{"purpose":"connect","id":7,"op":"ticket.issue"}}
{"out":{"id":7,"ok":true,"ticket":"<redacted>","expiresAt":1792941993515}}
