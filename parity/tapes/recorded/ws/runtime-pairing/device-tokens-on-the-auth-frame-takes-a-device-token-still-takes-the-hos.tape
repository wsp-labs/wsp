{"meta":{"door":"ws","source":"packages/runtime/test/pairing.test.ts > device tokens on the auth frame > takes a device token, still takes the host's own, and refuses a revoked one with 4401","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true}}
{"in":{"deviceId":"d_00000001","id":2,"op":"devices.revoke"}}
{"out":{"id":2,"ok":true,"revoked":true}}
