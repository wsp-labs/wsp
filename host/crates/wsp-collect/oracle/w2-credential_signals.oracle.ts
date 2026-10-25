// SPDX-License-Identifier: AGPL-3.0-only
// Fixtures for spec/credential_signals.md on the edge inputs of the wave-2 audit (group c). `extension` is not exported by the TS.
// file_signals cases: args [name, {bytes, mode}, text (null when read answers nothing), shallow]; out {signals, reads}.
import { it } from "vitest";
import { bareUrls, fileSignals, keysSignal, modeSignal, nameSignal, pemSignal, topLevelKeys, urlSignal } from "../../../../packages/collect/src/credential-signals.js";
import { plain, write } from "./w2-host.js";

const c = (args: unknown[], out: unknown, name = JSON.stringify(args[0])) => ({ name, args, out: plain(out) });

it("credential signal fixtures", async () => {
  const names = ["\u017Fecret", "to\u212Aen.txt", "token.EXAMPLE", "SECRET.TS", "Secret.Md", "token_history", "secrets.zip", "tokens.png", ".token.ts", ".token", "Auth.json", "Hosts.yml", ".ENV", ".envrc", "id_rsa.PUB", "id_rsa", "mysecret", "secret-", "_secret", "token2", "accesstoken", "my-access_token-x", "a-token", ".env.local.bak", "credentialsfoo", ".CREDENTIALS", "X.KEY", "a.PEM", "key.json", "keys", "auth.json.bak", "hosts.yml", "netrc.bak", ".netrc.bak", ".npmrc.bak", "tokens.lock", "tokens"];
  const keyNames = ["token", "password", "passwd", "secret", "bearer", "auths", "credentials", "_authtoken", "apikey", "api_key", "private_key", "secret_key", "access_key", "secret_access_key", "_token", "_secret", "_api_key", "_apikey", "_private_key", "_secret_access_key", "myToken", "my-token", "MY_SECRET", "x_api_key", "xApiKey", "myAPIKey", "mySecretAccessKey", "accessKey", "AccessKey", "AWS_ACCESS_KEY_ID", "aws_secret_access_key", "token_", "tokens", "mytoken", "db_password", "password_hash", "secrets", "bearer_token", "_authToken", "authToken", "NPM_TOKEN", "github.token", "oauth.token", "x_passwd", "PASSWD", "Password", "to\u212Aen", "TO\u212AEN", "secret.key", "key", "apiKeyId", "api_key_id", "private-key", "privateKey", "SECRET_KEY_BASE", "client_secret", "refresh_token", "auth_token", "_apikey2"];
  const texts = [
    "export API_KEY=sk-x\n", "'token': x\n", "\"token\": \"x\"\n", "\ufeff{\n  \"token\": \"x\"\n}\n", "{\"users\":[{\"password\":\"x\"}],\"a\":1}", "{\"a\":{\"x\":1},\"b\":2,\"c\":{\"d\":{\"e\":{\"f\":1}}}}",
    "token=#abc\n", "token: #abc\n", "token = \" \"\n", "token: ' '\n", "token: \"\"\n", "token: x,,\n", "token: \"x\",\n", "token: \"${X}\",\n", "token: '${X}'\n", "token: \"a\"b\"\n", "token: ${X}${Y}\n", "token: prefix${X}\n", "token: ${{ secrets.X }}x\n", "token: $\n", "token: <>\n", "token: <a\nb>\n",
    "a:b:c\n", "key == x\n", "token:= v\n", "github.token: abc\n", "  token : x\n", "token\t=\tx\n", "TOKEN=x\r\nPASSWORD=\r\n", "token: x # a # b\n", "token: x\t# c\n", "token: x\u00a0# c\n",
    "{\"token\":\"x\",\"n\":1e999}", "{\"token\":\"\\ud800\"}", `${"{\"a\":".repeat(150)}1${"}".repeat(150)}`, "", "   ", "{\"k\":1,\"k\":{\"token\":\"x\"},\"k2\":1}", "{\"a\":1,\"a\":2}",
    "{\"TO\u212AEN\":\"x\"}", "{\"token\":\"${X}\\n\"}", "{\"token\":\"<a\\nb>\"}", "{\"1\":{\"token\":\"x\"},\"b\":1,\"0\":1}", "{\"token\":\" \"}",
    "[registry]\napi_key = \"x\"\n[a.b]\nsecret = 'y'", "//reg/:_authToken=x\n//reg2/:_authToken=${T}\n", "\ufefftoken: x\n", "token: x\n\ufeffpassword: y",
  ];
  const urls = [
    "https://u:p@ss@h", "https://u:p@h?next=https://a:b@c", "http://u:$P@h?next=https://a:b@c", "https://u:p\ufeffq@h", "https://u:p\u00a0q@h", "https://u:p\u0085q@h", "https://u:p@h\ufeffx", "https://u:p@h\u0085x", "a\u212A://u:p@h", "\u017Ftp://u:p@h", "pre-https://u:p@h", "xhttps://u:p@h/x", "https://u:p@h\"x\"", "https://u:p@h'x'", "https://u:p@h)", "https://u:p@h\nhttps://v:q@i", "https://:@h", "https://u:@h", "https://u::p@h", "https://u:p@", "https://u:p@/x", "https://a:b@c https://a:b@c", "ssh://git:pw@h:22/x", "a+b.c-d://u:p@h", "1http://u:p@h", "http://[::1]:80@h", "http://u:p/q@h", "http://u:p@h http://u:$X@i http://w:z@j", "https://u:${A}@h https://u:b@h", "https://u:$@h", "https://u:$1@h", "https://u:p@h\u2028x", "https://user:pa%40ss@h",
  ];
  const pems = ["\ufeff-----BEGIN PRIVATE KEY-----", "-----BEGIN-----PRIVATE KEY", "-----BEGIN CERTIFICATE----- PRIVATE KEY", "-----BEGIN ENCRYPTED PRIVATE KEY-----", "-----BEGIN PGP PRIVATE KEY BLOCK-----", "-----BEGIN PRIVATEKEY-----", "-----begin private key-----", "-----BEGIN\nPRIVATE KEY", "\t -----BEGIN PRIVATE KEY", "x -----BEGIN PRIVATE KEY", "-----BEGIN  PRIVATE  KEY", "-----BEGIN PRIVATE", "-----BEGIN.PRIVATE KEY", ""];
  const modes: [number, number][] = [[0o600, 102400], [0o600, 102401], [0o600, 1], [0o600, 0], [0o070, 5], [0o007, 5], [0o700, 5], [0o100, 5], [0o100600, 5], [0o4600, 5], [0o640, 5], [0o604, 5], [0o000, 5]];
  const files: [string, number, number, string | null, boolean][] = [
    [".env.local", 20, 0o644, "API_KEY=x", false], [".env.local", 20, 0o600, "API_KEY=x", true], ["prod.env", 20, 0o644, "API_KEY=x", false], ["x.ENV", 20, 0o644, "API_KEY=x", false], ["id_rsa", 200 * 1024, 0o600, "-----BEGIN PRIVATE KEY-----", false],
    [".env", 20, 0o644, "\ufeff{}", false], [".env", 20, 0o600, null, false], ["settings.json", 50, 0o600, "{\"token\":\"${X}\"}", true], ["settings.json", 50, 0o600, "[1]", true], ["settings.json", 50, 0o600, "[]", true], ["a.json", 50, 0o644, "{\"password\":\"x\"}", false],
    ["token.json", 50, 0o600, "{\"password\":\"x\"}", false], ["cert.pem", 50, 0o600, "-----BEGIN PRIVATE KEY-----\nhttps://u:p@h", false], ["token.txt", 0, 0o600, "x", false], ["token.example", 50, 0o600, "token: x", true], ["id_rsa.pub", 50, 0o600, "ssh-rsa", true],
    ["x.json", 102400, 0o600, "not json", true], ["x.json", 102401, 0o600, "not json", true], ["x.json", 102400, 0o644, "{\"token\":\"x\"}", true], ["x.json", 102401, 0o644, "{\"token\":\"x\"}", true], ["token.json", 102401, 0o644, "{\"token\":\"x\"}", true],
    ["x.toml", 50, 0o600, "[a]\nb = 1", true], ["x.toml", 50, 0o600, "plain words", true], ["x.toml", 50, 0o600, "   \n", true], ["x.toml", 50, 0o600, "{ \n }", true], ["x.toml", 50, 0o600, "{ \n } x", true], ["x.yaml", 50, 0o600, "- a", false],
    ["config", 50, 0o644, "https://u:p@h", false], ["config", 50, 0o644, "token: ${X}\nurl: https://u:${P}@h", false], ["Secret.json", 50, 0o700, "{}", false], ["secret.json", 5, 0o644, "x", false], ["a.json", 50, 0o600, "not json", false],
    ["a.pem", 50, 0o644, "ssh-rsa", false], ["x.key", 50, 0o644, "-----BEGIN RSA PRIVATE KEY-----", false], ["x.key", 50, 0o644, "TOKEN=a", false],
  ];
  const fileCases = [];
  for (const [name, bytes, mode, text, shallow] of files) {
    let reads = 0;
    const signals = await fileSignals(name, { bytes, mode }, async () => { reads += 1; return text ?? undefined; }, shallow);
    fileCases.push({ name: `${name} ${bytes} bytes mode ${mode.toString(8)} shallow ${shallow}`, args: [name, { bytes, mode }, text, shallow], out: { signals: signals ?? null, reads } });
  }
  write("credential_signals", {
    name_signal: names.map(n => c([n], nameSignal(n))),
    key_name_signal: keyNames.map(k => ({ name: `JSON key ${JSON.stringify(k)}`, args: [JSON.stringify({ [k]: "x" })], out: keysSignal(JSON.stringify({ [k]: "x" })) })),
    mode_signal: modes.map(([mode, bytes]) => ({ name: `mode ${mode.toString(8)}, ${bytes} bytes`, args: [{ mode, bytes }], out: modeSignal({ mode, bytes }) })),
    top_level_keys: texts.map(t => c([t], topLevelKeys(t), JSON.stringify(t).slice(0, 60))),
    keys_signal: texts.map(t => c([t], keysSignal(t), JSON.stringify(t).slice(0, 60))),
    pem_signal: pems.map(t => c([t], pemSignal(t))),
    url_signal: urls.map(t => c([t], urlSignal(t))),
    bare_urls: urls.map(t => c([t], bareUrls(t))),
    file_signals: fileCases,
  });
});
