{"meta":{"door":"ws","source":"packages/host/test/verbs-folders-settings.test.ts > wsp verbs over the host: folders, export and what a person sets > what a person sets on one of their computers > refuses a count that is not a whole number of one or more, a word reset does not take, and a line that sets nothing, as usage","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8","road":"init"}}
{"in":{"id":2,"op":"places.list"}}
{"out":{"id":2,"ok":true,"places":[{"id":"here","kind":"computer","name":"ip-172-31-35-82","default":true,"os":"Linux 7.0.0-1014-aws","shape":{"cpu":8,"memMb":31744},"diskFreeBytes":108972564480,"engine":"docker","home":"<run>/wsp-verbs-000001/user","present":true,"takesForks":false,"buildsImages":false,"cap":{"threads":16},"capDefault":{"threads":16},"turnLimitMs":null,"turnLimitDefault":null,"spawn":{"spawn":true,"maxMachines":3,"maxDepth":2},"spawnDefault":{"spawn":true,"maxMachines":3,"maxDepth":2},"running":0}],"adds":[],"pending":[]}}
{"in":{"placeId":"here","id":3,"op":"places.set"}}
{"out":{"id":3,"ok":false,"error":"nothing to set on ip-172-31-35-82: it takes threads at once, turn limit, agents may start agents and levels deep","kind":"usage"}}
