{"meta":{"door":"ws","source":"packages/host/test/place-forks.test.ts > wsp add and the computer a project lives on > refuses a word that names no computer, and names the ones this host holds","outs":"recorded","computer":"ip-172-31-43-224"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8","road":"init"}}
{"in":{"id":2,"op":"places.list"}}
{"out":{"id":2,"ok":true,"places":[{"id":"here","kind":"computer","name":"ip-172-31-35-82","default":true,"os":"Linux 7.0.0-1014-aws","shape":{"cpu":8,"memMb":31744},"diskFreeBytes":120550121472,"engine":"none","home":"<run>/wsp-place-forks-000001/user","present":true,"takesForks":false,"buildsImages":false,"cap":{"threads":16},"capDefault":{"threads":16},"turnLimitMs":null,"turnLimitDefault":null,"spawn":{"spawn":true,"maxMachines":3,"maxDepth":2},"spawnDefault":{"spawn":true,"maxMachines":3,"maxDepth":2},"running":0}],"adds":[],"pending":[]}}
{"in":{"id":3,"op":"events.subscribe"}}
{"out":{"id":3,"ok":true,"seq":0,"stream":"00000000-0000-4000-8000-000000000001"}}
{"in":{"source":"https://github.com/dev/x.git","on":"nowhere","id":4,"op":"projects.add"}}
{"out":{"id":4,"ok":false,"error":"no place named nowhere; you have ip-172-31-35-82, default"}}
