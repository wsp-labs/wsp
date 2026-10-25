{"meta":{"door":"ws","source":"packages/runtime/test/pairing.test.ts > the device listing > lets a paired device revoke another device and cuts that device's socket, as it lets it revoke itself"}}
{"in":{"code":"<redacted>","name":"one","id":1,"op":"pair.redeem"}}
{"in":{"deviceId":"d_00000002","id":2,"op":"devices.revoke"}}
{"in":{"deviceId":"d_nope","id":3,"op":"devices.revoke"}}
{"in":{"id":4,"op":"devices.list"}}
