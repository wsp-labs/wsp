{"meta":{"door":"ws","source":"packages/runtime/test/places-door.test.ts > the door a computer you own dials > is refused on a host that serves none, and answered on one that does","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"id":2,"op":"places.door"}}
{"out":{"id":2,"ok":false,"error":"this host opens no door for computers you own; wsp up serves one"}}
