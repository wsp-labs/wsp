{"meta":{"door":"ws","source":"packages/runtime/test/places-settings.test.ts > a place's cap and what runs there > refuses a number the row's kind does not take, a place this host does not hold and a set with no number, as usage, and writes nothing","outs":"recorded"}}
{"in":{"token":"<<host-token>>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"placeId":"here","machines":2,"id":2,"op":"places.set"}}
{"out":{"id":2,"ok":false,"error":"zingzys-mac takes threads at once, turn limit, agents may start agents and levels deep, not machines at once","kind":"usage"}}
{"in":{"placeId":"solari","threads":2,"id":3,"op":"places.set"}}
{"out":{"id":3,"ok":false,"error":"Solari takes machines at once, spend per day, delete after, nap after, turn limit, agents may start agents and levels deep, not threads at once","kind":"usage"}}
{"in":{"placeId":"p_nothing","threads":2,"id":4,"op":"places.set"}}
{"out":{"id":4,"ok":false,"error":"no place named p_nothing; you have zingzys-mac, Solari","kind":"usage"}}
{"in":{"placeId":"solari","id":5,"op":"places.set"}}
{"out":{"id":5,"ok":false,"error":"nothing to set on Solari: it takes machines at once, spend per day, delete after, nap after, turn limit, agents may start agents and levels deep","kind":"usage"}}
