{"meta":{"door":"ws","source":"packages/runtime/test/serve.test.ts > serveRuntime the doctor's computer road > refuses a second road on one computer while the first is still going, and runs one after it has answered","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"placeId":"p_1","doctorId":"d_1","id":2,"op":"places.doctor"}}
{"in":{"placeId":"p_1","doctorId":"d_2","id":3,"op":"places.doctor"}}
{"out":{"id":3,"ok":false,"error":"the doctor is already proving spoo; wait for it","kind":"conflict"}}
{"out":{"id":2,"ok":true,"code":0}}
{"in":{"placeId":"p_1","doctorId":"d_3","id":4,"op":"places.doctor"}}
{"out":{"id":4,"ok":true,"code":0}}
