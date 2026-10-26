{"meta":{"door":"ws","source":"packages/host/test/verbs-folders-settings.test.ts > wsp verbs over the host: folders, export and what a person sets > folders lists one level of this computer's folders with the repository marked and the hidden ones counted, and refuses a path outside the roots","outs":"recorded"}}
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8","road":"init"}}
{"in":{"dir":"<run>/wsp-verbs-000001/user/code","id":2,"op":"host.folders"}}
{"out":{"id":2,"ok":true,"listing":{"dir":"<run>/wsp-verbs-000001/user/code","roots":["<run>/wsp-verbs-000001/user"],"folders":[{"path":"<run>/wsp-verbs-000001/user/code/notes","repo":false},{"path":"<run>/wsp-verbs-000001/user/code/spoo","repo":true}],"hidden":1}}}
