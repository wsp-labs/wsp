{"meta":{"door":"ws","source":"packages/runtime/test/places-install.test.ts > putting the agent on a computer over ssh > answers a refused login over the wire with the kind the app reads its login fix off, and any other refusal without it","outs":"recorded"}}
{"in":{"token":"<<host-token>>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"address":"maya@box","id":2,"op":"places.add"}}
{"out":{"id":2,"ok":false,"error":"maya@box: Permission denied (publickey).","kind":"login"}}
{"in":{"address":"root@spoo","id":3,"op":"places.add"}}
{"out":{"id":3,"ok":false,"error":"root@spoo runs zsh as root's shell"}}
