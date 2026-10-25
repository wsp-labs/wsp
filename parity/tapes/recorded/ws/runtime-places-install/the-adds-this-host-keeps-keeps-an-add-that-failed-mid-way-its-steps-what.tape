{"meta":{"door":"ws","source":"packages/runtime/test/places-install.test.ts > the adds this host keeps > keeps an add that failed mid-way, its steps, what it said and the fix, for a second client to read off places.list","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"addId":"a_spoo","address":"root@spoo","sshPort":2222,"id":2,"op":"places.add"}}
{"out":{"id":2,"ok":false,"error":"spoo has no curl or wget on its PATH. Install one of them there, then add again.","fix":"Install one of them there, then add again."}}
