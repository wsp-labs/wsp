'use strict';
// The stand-in gh's answers for the review flow (tests/pr_review.rs): the pull request read as an issue for the
// reviewer's task. Any other line falls through to the stub's own answers.

const REPO = 'wsp-test/repo';
const NUMBER = 42;
const ISSUE_FIELDS = 'number,url,title,body,state,comments';

const same = (argv, words) => argv.length === words.length && words.every((w, i) => w === argv[i]);

function run(argv, _stdin, fx, { json, refuse }) {
  const pr = fx.pr;
  if (same(argv, ['issue', 'view', String(NUMBER), '-R', REPO, '--json', ISSUE_FIELDS])) {
    if (!pr) return refuse(`GraphQL: Could not resolve to an issue or pull request with the number of ${NUMBER}.`);
    return json({ number: NUMBER, url: `https://github.com/${REPO}/pull/${NUMBER}`, title: pr.title, body: pr.body || '', state: pr.state, comments: [] });
  }
  return null;
}

module.exports = { run };
