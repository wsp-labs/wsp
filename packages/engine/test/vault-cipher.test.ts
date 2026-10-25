// SPDX-License-Identifier: AGPL-3.0-only
// The sealed container: a place's key opens it and no other key does.
import { generateKeyPairSync } from "node:crypto";
import { describe, expect, it } from "vitest";
import { SealedVaultHeader, type SealedImage } from "@wsp/protocol";
import { publicKeyCipher } from "../src/vault-cipher.js";

const IMAGE: SealedImage = {
  name: "default",
  version: 1,
  hash: "a".repeat(64),
  recipeHash: "rh",
  logins: [{ name: "codex", state: "signed-in" }],
  sealedAt: "2026-09-12T00:00:00.000Z",
  sealedFrom: "this-mac",
  vault: { sha256: "b".repeat(64), bytes: 128, paths: 2, takenAt: "2026-09-12T00:00:00.000Z" },
};
const PLAIN = Buffer.from("tar bytes that are the person's logins");

const headerOf = (sealed: Buffer): SealedVaultHeader => SealedVaultHeader.parse(JSON.parse(sealed.subarray(0, sealed.indexOf(0x0a)).toString("utf8")));

describe("vault cipher", () => {
  it("a place's key round trips, and another place's key does not open it", () => {
    const place = generateKeyPairSync("x25519");
    const other = generateKeyPairSync("x25519");
    const sealed = publicKeyCipher.seal(PLAIN, IMAGE, { publicKey: place.publicKey });
    expect(headerOf(sealed)).toMatchObject({ to: "key" });
    expect(headerOf(sealed).ephemeral).toBeDefined();
    expect(publicKeyCipher.open(sealed, { privateKey: place.privateKey }).plain.equals(PLAIN)).toBe(true);
    expect(() => publicKeyCipher.open(sealed, { privateKey: other.privateKey })).toThrow(/would not open/);
  });
});
