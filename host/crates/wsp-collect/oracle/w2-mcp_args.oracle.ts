// SPDX-License-Identifier: AGPL-3.0-only
// Fixtures for spec/mcp_args.md: mcp.ts:1-290 on the edge inputs of the wave-2 audit (group d). Servers are written as the TS shape.
import { it } from "vitest";
import type { McpServer } from "../../../../packages/catalog/src/index.js";
import { hidesValue, homePaths, linuxFit, mcpRemoteHash, mcpRemoteOf, plainArg, readArgs, secretNamed, stdioLine } from "../../../../packages/collect/src/detect/mcp.js";
import { plain, write } from "./w2-host.js";

type Case = Record<string, unknown>;
const pure = (name: string, f: (...a: never[]) => unknown, ...args: unknown[]): Case => ({ name, args, out: plain((f as (...a: unknown[]) => unknown)(...args)) });
const H = "/Users/dev";
const U = "https://mcp.example.com/mcp";
const S = (command: string, args: string[] = [], env: Record<string, string> = {}, cwd?: string): McpServer => ({ name: "s", scope: "user", transport: { kind: "stdio", command, args, env, ...(cwd !== undefined ? { cwd } : {}) }, envRefs: [] });
const http = (url: string): McpServer => ({ name: "h", scope: "user", transport: { kind: "http", url, headers: {} }, envRefs: [] });

it("mcp_args fixtures", () => {
  const fixture: Record<string, Case[]> = {};

  fixture["linux_fit"] = ([
    ["command and an arg both mac-only: the command is named", S("/Applications/X.app/x", ["/System/y"])],
    ["arg --flag=path names the part after the equals sign", S("npx", ["--config=/Applications/x.json"])],
    ["arg -c=path (single dash) is stripped too", S("npx", ["-c=/Library/x"])],
    ["arg --dir=~/Library/x", S("npx", ["--dir=~/Library/x"])],
    ["a dotted flag name is not stripped", S("npx", ["--a.b=/Applications/x"])],
    ["cwd mac-only", S("npx", [], {}, "/private/tmp/x")],
    ["cwd empty is fine", S("npx", [], {}, "")],
    ["env: the first of two", S("npx", [], { A: "/System/a", B: "/Volumes/b" })],
    ["order is args then cwd then env", S("npx", ["/Volumes/v"], { A: "/System/a" }, "/Applications/c")],
    ["command ~/Library/x", S("~/Library/x")],
    ["another user's Library is not this home's", S("/Users/devops/Library/x")],
    ["home/Library without the slash", S("npx", [`${H}/Library`])],
    ["/Library without the slash", S("npx", ["/Library"])],
    ["/private/tmp", S("npx", ["/private/tmp/x"])],
    ["a Caskroom command", S("/opt/homebrew/Caskroom/x/y")],
    ["a Homebrew opt path is not a bin dir", S("/opt/homebrew/opt/foo/bin/x")],
    ["Homebrew sbin", S("/opt/homebrew/sbin/x")],
    ["/usr/local/bin/uvx", S("/usr/local/bin/uvx")],
    ["/usr/bin/env", S("/usr/bin/env", ["x"])],
    ["/bin/sh", S("/bin/sh")],
    ["/bin/x/y is named y", S("/bin/x/y")],
    ["./server", S("./server")],
    ["empty command: two spaces", S("")],
    ["a bin dir with nothing after it", S("/usr/local/bin/")],
    ["/usr/bin without the slash", S("/usr/bin")],
    ["a home bin", S(`${H}/bin/x`)],
    ["home .local/bin/npx", S(`${H}/.local/bin/npx`)],
    ["~/.local/bin/uv", S("~/.local/bin/uv")],
    ["~ alone", S("~")],
    ["another absolute path", S("/opt/x/y")],
    ["Homebrew uv", S("/opt/homebrew/bin/uv")],
    ["http url with a mac-looking path", http("https://x/Applications/")],
  ] as [string, McpServer][]).map(([n, s]) => pure(n, linuxFit, s, H));

  const remote = (name: string, ...args: string[]): Case => pure(name, mcpRemoteOf, args);
  fixture["mcp_remote_of"] = [
    remote("empty pin after the at sign", "mcp-remote@", U),
    remote("text between the first and second at sign: 0.3.1", "mcp-remote@0.3.1@latest", U),
    remote("text between the first and second at sign: latest", "mcp-remote@latest@0.1.0", U),
    remote("leading zeros", "mcp-remote@00.03.02", U),
    remote("v prefix", "mcp-remote@v1.0.0", U),
    remote("0.3.10", "mcp-remote@0.3.10", U),
    remote("0.10.0", "mcp-remote@0.10.0", U),
    remote("prerelease", "mcp-remote@1.0.0-beta", U),
    remote("a slash in the pin is no mcp-remote", "mcp-remote@a/b", U),
    remote("a url before mcp-remote is not seen", U, "mcp-remote"),
    remote("the first mcp-remote argument is the one", "mcp-remote@0.3.1", U, "mcp-remote@latest"),
    remote("a second mcp-remote after another word", "x", "mcp-remote@0.3.1", "mcp-remote", U),
    remote("the url is the first http argument even as a flag value", "mcp-remote", "--resource", "https://r.example", U),
    remote("an upper-case scheme is no url", "mcp-remote", "HTTP://x/mcp"),
    remote("a bare scheme is a url", "mcp-remote", "https://"),
    remote("header key trims to empty", "mcp-remote", U, "--header", " :x"),
    remote("header with an empty value", "mcp-remote", U, "--header", "A:"),
    remote("header with inner spaces", "mcp-remote", U, "--header", "  A b  :  v  w "),
    remote("header colon in the value", "mcp-remote", U, "--header", "Authorization: Bearer a:b"),
    remote("header without a colon", "mcp-remote", U, "--header", "nocolon"),
    remote("header named __proto__", "mcp-remote", U, "--header", "__proto__: x"),
    remote("header equals form", "mcp-remote", U, "--header=A:1"),
    remote("a header flag as the value of a header flag", "mcp-remote", U, "--header", "--header", "A: 1"),
    remote("lower-case -h is no header flag", "mcp-remote", U, "-h", "A: 1"),
    remote("authorize-param with a second equals sign", "mcp-remote", U, "--authorize-param", "a=1=2"),
    remote("authorize-param with the equals sign first", "mcp-remote", U, "--authorize-param", "=x"),
    remote("authorize-param with an empty value", "mcp-remote", U, "--authorize-param", "a="),
    remote("authorize-param trimmed", "mcp-remote", U, "--authorize-param", " a = b "),
    remote("empty resource is present", "mcp-remote", U, "--resource", ""),
    remote("blank resource is present", "mcp-remote", U, "--resource", "   "),
    remote("resource equals form", "mcp-remote", U, "--resource=https://r"),
    remote("resource trimmed", "mcp-remote", U, "--resource", "  https://r  "),
    remote("resource as the last argument", "mcp-remote", U, "--resource"),
    remote("resource twice: the first", "mcp-remote", U, "--resource", "a", "--resource", "b"),
    remote("resource whose value is a flag", "mcp-remote", U, "--resource", "--header", "A: 1"),
    remote("client metadata url alone", "mcp-remote", U, "--client-metadata-url", " https://m "),
    remote("client credentials alone", "mcp-remote", U, "--client-credentials"),
    remote("client credentials before the url", "mcp-remote", "--client-credentials", U),
    remote("json escaping of a header value", "mcp-remote", U, "--header", 'K: é"\\\u0001\u007f\u2028/<'),
    remote("integer-like header names", "mcp-remote", U, "--header", "10: a", "--header", "9: b", "--header", "B: c"),
    remote("key sort in UTF-16 order", "mcp-remote", U, "--header", "b: 1", "--header", "B: 2", "--header", "é: 3", "--header", "a: 4", "--header", "_: 5"),
    remote("every part", "mcp-remote", U, "--resource", "R", "--authorize-param", "z=1", "--header", "H: v", "--client-metadata-url", "M", "--client-credentials"),
    remote("a url with a space is hashed as written", "mcp-remote", "https://x.example/a b"),
  ];
  fixture["mcp_remote_hash"] = [["mcp-remote", U], ["mcp-remote", U, "--resource", ""], ["mcp-remote"], ["-y", "@scope/other", U], ["mcp-remote", "--header", "A: 1", U]].map(a => pure(`hash of ${JSON.stringify(a)}`, mcpRemoteHash, a));

  const names = ["API_KEY", "api_key", "MY_AUTH", "AUTHOR", "OAUTH", "AUTH", "NO_AUTH", "CERT", "CERTIFICATE", "SSL_CERT_FILE", "CREDENTIAL", "CREDENTIALS_PATH", "MY_CREDENTIAL_FILE", "credentials", "AUTHORIZATION", "PASSWORD", "TOKEN", "DB_PASS", "KEYTIMEOUT", "HOME", "X_SECRET", "x_secret", "SECRETARY", "", "_", "__AUTH__", "auth", "Auth_Token", "MONKEY", "PUBKEY", "PRIVATE_KEY", "GH_PAT", "gh_pat"];
  fixture["secret_named"] = names.map(n => pure(`secret_named ${JSON.stringify(n)}`, secretNamed, n));
  const flags = ["--api-key", "--API-KEY", "--token", "-H", "-h", "--header", "--static-oauth-client-info", "--auth", "--oauth", "--no-auth", "--auth-file", "--cert", "--certificate", "--cert-path", "--credentials", "--credential-file", "--password", "--pass", "-p", "-k", "--key", "--keyfile", "--key-file", "--secret", "--client-secret", "--verbose", "--port", "--authorization", "--api_key", "--authorize-param", "--header-name"];
  fixture["hides_value"] = flags.map(f => pure(`hides_value ${f}`, hidesValue, f));

  const K = (n: number) => "a".repeat(n - 1) + "1";
  const plains = ["https://x.example/a#token=abc", `https://x.example/${K(25)}`, `https://x.example/${K(24)}`, `https://x.example/${K(23)}`, `https://x.example/${K(22)}`, `https://x.example/${K(25)}.json`, `https://x.example/${"a".repeat(30)}`, `https://x.example/a/b/${K(25)}/c`, `https://x.example/${K(25).slice(0, 24)}=1`, "https://x.example/AAAAAAAAAAAAAAAAAAAAAAA-1", `https://x.example/%41${K(24)}`, `https://x.example/${K(25)}/..`, `https://x.example/a/../${K(25)}`, "HTTPS://X.EXAMPLE/", "https://@x.example/", "https://:@x.example/", "https://u@x.example/", "https://x.example:99999/", "https://", "http://[::1]/", "ftp://host/file", "file:///etc/x", "mailto:a@b", "a://b", "1a://b", "a+b.c-d://x/", "~", "~/", "/", "", "--x", "./x", "../x", "x/y", "C:/x", "https://x.example/?", "https://x.example/#", "https://x.example/?#", "https://x.example?x", "https://x.example\\a", "/Users/dev/x"];
  fixture["plain_arg"] = plains.map(a => pure(`plain_arg ${JSON.stringify(a)}`, plainArg, a));

  const ra = (name: string, command: string, args: string[]): Case => pure(name, (c: string, a: string[], h: string) => readArgs({ command: c, args: a }, h), command, args, "/h");
  fixture["read_args"] = [
    ra("an assignment does not use up the program word", "npx", ["API_KEY=x", "pkg", "other"]),
    ra("a plain assignment before the package", "npx", ["FOO=bar", "pkg", "other"]),
    ra("the program word survives a flag value", "npx", ["--package", "foo", "pkg", "x"]),
    ra("-y then paths", "npx", ["-y", "pkg", "/a", "/b", "/c", "/d", "/e"]),
    ra("secret flag followed by a flag", "npx", ["--api-key", "-x", "v"]),
    ra("secret flag followed by a dash-led value", "npx", ["--api-key", "-abc123"]),
    ra("secret flag with an empty value", "npx", ["--api-key", "", "pkg"]),
    ra("secret flag last", "npx", ["pkg", "--api-key"]),
    ra("dotted flag shows its value", "npx", ["--api.key", "sk-123"]),
    ra("triple dash flag", "npx", ["---x", "v"]),
    ra("-1 flag", "npx", ["-1", "sk-123"]),
    ra("flag=path under home is not tilde'd, a positional is", "/h/bin/x", ["--config=/h/x", "/h/x", "--dir", "/h/y"]),
    ra("--url= with a login is shown whole", "npx", ["--url=https://u:p@x.example/a?k=1"]),
    ra("--url= with a key segment is shown whole", "npx", ["--url=https://h.example/aaaaaaaaaaaaaaaaaaaaaaaa1"]),
    ra("--url value with a login shows host and path", "npx", ["--url", "https://u:p@h.example/p?q=1#f"]),
    ra("--url value with a key segment", "npx", ["--url", "https://h.example/aaaaaaaaaaaaaaaaaaaaaaaa1"]),
    ra("--url value on another scheme is shown raw", "npx", ["--url", "postgres://u:p@h/db"]),
    ra("--url value without a path", "npx", ["--url", "https://X.Example:443"]),
    ra("--url value with a port and odd characters", "npx", ["--url", "https://X.Example:8443/a b/ü"]),
    ra("--url value with a unicode host", "npx", ["--url", "https://Bücher.example/"]),
    ra("--url value that does not parse", "npx", ["--url", "https://"]),
    ra("--url value with an upper-case scheme", "npx", ["--url", "HTTPS://X.EXAMPLE/p"]),
    ra("positional postgres address, plain", "npx", ["x", "postgres://db/x"]),
    ra("positional custom scheme keeps its host case", "npx", ["x", "custom://Host/Path"]),
    ra("positional http address with a login under the program word", "npx", ["https://U:p@x.example/a"]),
    ra("non-secret assignment keeps its value as written", "npx", ["FOO=/h/x", "pkg"]),
    ra("empty secret assignment", "npx", ["API_KEY=", "x"]),
    ra("assignment value with equals signs", "npx", ["a=b=c", "x"]),
    ra("secret assignment value with equals signs", "npx", ["MY_TOKEN=a=b", "x"]),
    ra("lower-case secret assignment", "npx", ["my_token=abc"]),
    ra("digit-led name is no assignment", "npx", ["1A_KEY=x"]),
    ra("equals first is no assignment", "npx", ["=x"]),
    ra("AUTH and CERT words, not OAUTH or AUTHOR", "npx", ["MY_AUTH=x", "AUTHOR=y", "CERT=z", "OAUTH=w"]),
    ra("--env=NAME with no value", "npx", ["--env=NAME", "pkg"]),
    ra("--env=secret assignment", "docker", ["--env=my_token=abc"]),
    ra("--token=A=b", "npx", ["--token=A=b"]),
    ra("--header= is hidden", "npx", ["--header=Authorization: Bearer x"]),
    ra("-H=x", "npx", ["-H=x"]),
    ra("-H value", "npx", ["-H", "A: b"]),
    ra("--a.b=C_KEY=1", "npx", ["--a.b=C_KEY=1"]),
    ra("--=x", "npx", ["--=x"]),
    ra("--yes=true is no yes switch", "npx", ["--yes=true", "pkg"]),
    ra("-y last still reads {}", "npx", ["pkg", "-y"]),
    ra("-y and --yes alone", "npx", ["-y", "--yes"]),
    ra("-y after -- is a value; the first word after -- is still the program word", "npx", ["--", "-y", "--yes", "--"]),
    ra("-- twice", "npx", ["--", "--", "a"]),
    ra("the program word after --", "npx", ["--", "x", "y"]),
    ra("-- is not taken as a flag's value", "npx", ["--port", "--", "x"]),
    ra("non-secret assignment as a flag value", "npx", ["--metadata", "A=b"]),
    ra("hiding flag beats an assignment value", "npx", ["--api-key", "A_KEY=b"]),
    ra("secret assignment behind a plain flag", "npx", ["-e", "A_KEY=b"]),
    ra("uv runner", "uv", ["run", "--with", "x", "main.py"]),
    ra("uvx under .local/bin", "/h/.local/bin/uvx", ["pkg", "val"]),
    ra("npx under /usr/local/bin", "/usr/local/bin/npx", ["pkg", "val"]),
    ra("npx under ~/.local/bin", "~/.local/bin/npx", ["pkg", "val"]),
    ra("npx under a home bin is no runner", "/h/bin/npx", ["pkg"]),
    ra("./npx is no runner", "./npx", ["pkg"]),
    ra("uvx under ~/.local/bin", "~/.local/bin/uvx", ["pkg"]),
    ra("uv under Homebrew", "/opt/homebrew/bin/uv", ["pkg"]),
    ra("npx under /usr/bin", "/usr/bin/npx", ["pkg"]),
    ra("npx under /opt/x is no runner", "/opt/x/npx", ["pkg"]),
    ra("no arguments", "npx", []),
    ra("empty string argument under the program word", "npx", ["", "x"]),
    ra("empty string argument, no runner", "node", ["", "x"]),
    ra("home, tilde, tilde slash, home slash", "node", ["/h", "~", "~/", "/h/"]),
    ra("relative paths are values", "node", ["./x", "../y", "x/y"]),
    ra("UTF-8 sizes", "node", ["é", "😀"]),
    ra("flag values under home are tilde'd", "node", ["--out", "/h/x/y", "--out", "~/z"]),
    ra("unicode flag name", "node", ["--é", "v"]),
    ra("digits in flag names", "node", ["--a1", "v", "-9", "w"]),
    ra("an address that does not parse is hidden in an assignment", "node", ["X=https://", "Y=postgres://db/x", "Z=HTTPS://u:p@h/", "W=a://b"]),
    ra("address assignments with a fragment or a bare query", "node", ["X=https://h/a#f", "Y=https://h/?"]),
    ra("--env=DB=address with a login", "node", ["--env=DB=postgres://u:p@h/db", "--env=DB2=postgres://h/db"]),
    ra("flag then a program word", "npx", ["--registry", "https://r.example/", "pkg", "x"]),
    ra("an http(s) argument shown as host and path", "npx", ["--u", "http://[::1]:8080/a", "--u", "https://x.example:443/a/", "--u", "https://x.example/a/?q#f", "--u", "https://x.example//a", "--u", "http://x.example:80", "--u", "https://x.example/%7Efoo", "--u", "https://x.example/a%2Fb", "--u", "https://x.example/ü", "--u", "file:///a/b", "--u", "mailto:x@y"]),
  ];

  const sl = (name: string, command: string, args: string[]): Case => pure(name, (c: string, a: string[], h: string) => stdioLine({ kind: "stdio", command: c, args: a, env: {} }, h), command, args, "/h");
  fixture["stdio_line"] = [
    sl("-y then five paths: no cut", "npx", ["-y", "/a", "/b", "/c", "/d", "/e"]),
    sl("five paths", "npx", ["/a", "/b", "/c", "/d", "/e"]),
    sl("-y then six paths", "npx", ["-y", "/a", "/b", "/c", "/d", "/e", "/f"]),
    sl("an empty word is kept", "npx", ["", "/a"]),
    sl("-y in the middle", "npx", ["/a", "-y", "/b"]),
    sl("two --yes then six paths", "npx", ["--yes", "--yes", "/a", "/b", "/c", "/d", "/e", "/f"]),
    sl("empty command, no arguments", "", []),
    sl("empty command, one hidden word", "", ["a"]),
    sl("the command is the home itself", "/h", []),
    sl("only -y", "npx", ["-y"]),
    sl("hidden words count toward the cut", "node", ["a", "b", "c", "d", "e", "f"]),
    sl("a secret flag's value is the flag's word", "npx", ["p", "--api-key", "v", "--x", "y", "--z"]),
  ];

  const hp = (name: string, s: McpServer): Case => pure(name, homePaths, s, "/h");
  fixture["home_paths"] = [
    hp("tilde slash alone, home slash, tilde alone", S("x", ["~/", "/h/", "~"])),
    hp("a trailing slash is another path", S("x", ["/h/a", "/h/a/", "~/a"])),
    hp("flag forms", S("x", ["-d=/h/x", "--a.b=/h/y", "--a=b=/h/z", "--=/h/w"])),
    hp("order args, cwd, env and one entry each", S("x", ["/h/a"], { A: "/h/a", B: "~/b" }, "/h/a")),
    hp("empty cwd", S("x", [], {}, "")),
    hp("relative paths", S("x", ["./a", "a/b", ".local/bin"])),
    hp("local bin variants", S("x", ["/h/.local/bin", "/h/.local/bin/", "/h/.local/bin/x", "/h/.local/binx", "~/.local/bin/y"])),
    hp("home prefix lookalikes", S("x", ["/hx/a", "/h2/b"])),
    hp("empty env value", S("x", [], { A: "" })),
    hp("dots are not normalised", S("x", ["/h/a/../b", "/h/./c"])),
    hp("http has none", http("https://h/h/a")),
  ];

  write("mcp_args", fixture);
});
