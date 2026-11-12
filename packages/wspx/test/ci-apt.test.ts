// SPDX-License-Identifier: AGPL-3.0-only
import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, it } from "vitest";
import { writeStub } from "../../protocol/test/stub-script.js";

it.each([{ failedDns: false, prepareOnly: false }, { failedDns: true, prepareOnly: false }, { failedDns: true, prepareOnly: true }])("repairs runner package access for %j", ({ failedDns, prepareOnly }) => {
  const root = mkdtempSync(join(tmpdir(), "wsp-ci-apt-"));
  try {
    const apt = join(root, "apt");
    const bin = join(root, "bin");
    mkdirSync(join(apt, "sources.list.d"), { recursive: true });
    mkdirSync(bin);
    const mirror = join(apt, "apt-mirrors.txt");
    const direct = join(apt, "sources.list");
    const indirect = join(apt, "sources.list.d", "ubuntu.sources");
    const ports = join(apt, "sources.list.d", "ports.sources");
    const arm = "URIs: http://ports.ubuntu.com/ubuntu-ports\n";
    writeFileSync(mirror, "http://azure.archive.ubuntu.com/ubuntu\nhttps://archive.ubuntu.com/ubuntu\n");
    writeFileSync(direct, "deb https://azure.archive.ubuntu.com/ubuntu noble main\n");
    writeFileSync(indirect, `URIs: mirror+file:${mirror}\nSuites: noble\n`);
    writeFileSync(ports, arm);
    writeStub(join(bin, "sudo"), '#!/bin/sh\nexec "$@"\n');
    writeStub(join(bin, "apt-get"), '#!/bin/sh\nprintf "%s\\n" "$*" >> calls\n');
    writeStub(join(bin, "timeout"), '#!/bin/sh\nshift\nexec "$@"\n');
    writeStub(join(bin, "getent"), failedDns ? '#!/bin/sh\ntest -f dns-ready\n' : '#!/bin/sh\nexit 0\n');
    writeStub(join(bin, "ip"), '#!/bin/sh\necho "default via 192.0.2.1 dev eth0"\n');
    writeStub(join(bin, "resolvectl"), '#!/bin/sh\nprintf "%s\\n" "$*" >> resolver-calls\ntouch dns-ready\n');
    const script = join(root, "install.sh");
    // Run the actual script against fixture configuration, with no sudo or network access.
    writeFileSync(script, readFileSync(new URL("../../../scripts/ci-apt.sh", import.meta.url), "utf8").replaceAll("/etc/apt", apt));
    const ran = spawnSync("bash", [script, ...(prepareOnly ? [] : ["musl-tools", "gperf"])], { cwd: root, env: { GITHUB_ACTIONS: "true", PATH: `${bin}:${process.env["PATH"] ?? ""}` }, encoding: "utf8" });
    expect(ran.status, ran.stderr).toBe(0);
    expect(readFileSync(mirror, "utf8")).not.toContain("azure.archive.ubuntu.com");
    expect(readFileSync(direct, "utf8")).toBe("deb https://archive.ubuntu.com/ubuntu noble main\n");
    expect(readFileSync(indirect, "utf8")).toContain(`mirror+file:${mirror}`);
    expect(readFileSync(ports, "utf8")).toBe(arm);
    expect(readFileSync(join(root, "calls"), "utf8").trim().split("\n")).toEqual([
      "-o Acquire::Retries=3 -o APT::Update::Error-Mode=any update",
      ...(prepareOnly ? [] : ["-o Acquire::Retries=3 install -y --no-install-recommends musl-tools gperf"]),
    ]);
    if (failedDns) expect(readFileSync(join(root, "resolver-calls"), "utf8").trim().split("\n")).toEqual(["dns eth0 1.1.1.1 8.8.8.8", "domain eth0 ~.", "flush-caches"]);
    else expect(existsSync(join(root, "resolver-calls"))).toBe(false);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
