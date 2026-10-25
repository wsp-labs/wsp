{"meta":{"door":"ws","source":"packages/host/test/verbs-folders-settings.test.ts > wsp verbs over the host: folders, export and what a person sets > folders --on reads the computer it names, this computer by its own name, and refuses a name nobody holds with the computers there are","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8","road":"init"}}
{"in":{"id":2,"op":"places.list"}}
{"out":{"id":2,"ok":true,"places":[{"id":"here","kind":"computer","name":"ip-172-31-47-246","default":true,"os":"Linux 7.0.0-1014-aws","shape":{"cpu":8,"memMb":31744},"diskFreeBytes":169168670720,"engine":"docker","home":"<run>/wsp-verbs-000001/user","present":true,"takesForks":false,"buildsImages":false,"cap":{"threads":16},"capDefault":{"threads":16},"turnLimitMs":null,"turnLimitDefault":null,"spawn":{"spawn":true,"maxMachines":3,"maxDepth":2},"spawnDefault":{"spawn":true,"maxMachines":3,"maxDepth":2},"running":0}],"adds":[],"pending":[]}}
{"in":{"id":3,"op":"host.folders"}}
{"out":{"id":3,"ok":true,"listing":{"dir":"<run>/wsp-verbs-000001/user","roots":["<run>/wsp-verbs-000001/user"],"folders":[{"path":"<run>/wsp-verbs-000001/user/code","repo":false}],"hidden":0}}}
