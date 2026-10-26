{"meta":{"door":"ws","source":"packages/runtime/test/serve-refusal.test.ts > a refused frame > on a socket let in on a ticket, carries kind ticket so a client tells not-yours-to-see from a fault","outs":"recorded"}}
{"in":{"id":1,"op":"places.list"}}
{"out":{"id":1,"ok":false,"error":"a socket let in on a ticket cannot see or change the places this host holds; run wsp computers on the computer the host runs on","kind":"ticket"}}
{"in":{"id":2,"op":"account.get"}}
{"out":{"id":2,"ok":false,"error":"a socket let in on a ticket cannot see the account this host is signed in to; run wsp login on the computer the host runs on","kind":"ticket"}}
{"in":{"id":3,"op":"devices.list"}}
{"out":{"id":3,"ok":false,"error":"a socket let in on a ticket cannot see or change the devices paired with this host; run wsp host devices on the computer the host runs on","kind":"ticket"}}
{"in":{"id":4,"op":"cost.spend"}}
{"out":{"id":4,"ok":false,"error":"a socket let in on a ticket cannot see or change the places this host holds; run wsp computers on the computer the host runs on","kind":"ticket"}}
{"in":{"purpose":"relay","id":5,"op":"ticket.issue"}}
{"out":{"id":5,"ok":false,"error":"a request relayed from a machine cannot mint a ticket into this host","kind":"ticket"}}
