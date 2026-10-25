{"meta":{"door":"ws","source":"packages/runtime/test/places-update.test.ts > moving a place onto the daemon this host deploys > picks up no byte for a place already running this daemon, and refuses one this host does not hold","outs":"recorded"}}
{"in":{"token":"<<host-token>>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"placeId":"p_nothing","id":2,"op":"places.update"}}
{"out":{"id":2,"ok":false,"error":"no place named p_nothing; you have old-macbook"}}
