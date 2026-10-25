// SPDX-License-Identifier: AGPL-3.0-only
// The cloud setup a client opens on: which provider keys this host holds, the price of the machine a build boots and
// where it would boot. Saving the provider key is the one write, and a provider that refuses the key is told before
// anything lands in the wsp home.
import { keyCheckLine, type KeyCheck, type MachineBackend } from "@wsp/engine";
import { KEY_REFUSED, KEY_UNCHECKED, type InitKeys, type InitSetup, type PlaceKind } from "@wsp/protocol";
import type { InitDoor, Runtime } from "@wsp/runtime";

export interface CloudSetupDeps {
  rt: Runtime;
  home: string;
  /** The wsp home's .env as it stands, read at each ask: the one place the setup reads a key from, so a key in the
   * process environment or a checkout's .env never reads as saved on a screen. */
  saved(): Readonly<Record<string, string>>;
  /** Writes the keys given into the wsp home's .env, the one writer every road uses. */
  saveKeys(set: Record<string, string>): void;
  /** Wires the provider module the keys saved here name into the runtime, so a host that started with none forks
   * once one is saved. */
  provider(saved: Readonly<Record<string, string>>): void;
  /** Every provider a key can be saved for and whether the file holds one, by the word WSP_PROVIDER holds. */
  keysHeld(saved: Readonly<Record<string, string>>): InitKeys;
  /** What saving this key for this provider writes into the wsp home's .env, under the variables that provider's own
   * module reads; nothing where no provider of that name takes a key. A key with no provider named is the wired
   * provider's. */
  keySet(key: string, provider?: string): Record<string, string> | undefined;
  /** The provider whose key the setup's own key step asks for, by the word WSP_PROVIDER holds; nothing where this
   * host's provider reads no key at all. */
  keyProvider(): string | undefined;
  /** The provider this host forks on, under the word its machines are stamped with, read at each ask. */
  forksOn(): string;
  /** Whether the provider takes this key, asked before it is saved: a refusal is the person's to fix on the step that
   * typed the key. */
  checkKey(key: string, provider?: string): Promise<KeyCheck>;
}

export class CloudSetup implements InitDoor {
  constructor(private readonly deps: CloudSetupDeps) {}

  async get(o: { on?: string } = {}): Promise<InitSetup> {
    const keyProvider = this.deps.keyProvider();
    // Nothing to price where no build can start: the client reads the runtime's sentence for it, so the sheet and the
    // command line say the same one. A place the ask named and this host cannot build at is that place's own
    // refusal, not a null price.
    let built: { place: string; name: string; kind: PlaceKind; backend: MachineBackend } | undefined;
    let buildRefusal: string | undefined;
    try {
      built = await this.deps.rt.golden.buildPlace(o.on);
    } catch (e) {
      if (o.on !== undefined) throw e;
      buildRefusal = e instanceof Error ? e.message : String(e);
    }
    const pricing = built?.backend.pricing;
    return {
      keys: this.deps.keysHeld(this.deps.saved()),
      ...(keyProvider !== undefined ? { keyProvider } : {}),
      home: this.deps.home,
      pricing: pricing === undefined ? null : { size: pricing.defaultSize, rateUsdPerHour: pricing.rateUsdPerHour(pricing.defaultSize), ...(pricing.builderDiskGb !== undefined ? { builderDiskGb: pricing.builderDiskGb } : {}) },
      ...(built !== undefined ? { place: { id: built.place, name: built.name, kind: built.kind } } : {}),
      forksOn: this.deps.forksOn(),
      ...(buildRefusal !== undefined ? { buildRefusal } : {}),
    };
  }

  /** Saves the provider key. It is put to the provider before anything is written, so a key it refuses is never saved;
   * the refusal carries its kind, since a road that never answered is worth pressing again and a refused key is
   * not. */
  async keys(k: { provider?: string; key?: string }): Promise<InitSetup> {
    const key = k.key?.trim();
    if (key !== undefined && key !== "") {
      const saves = this.deps.keySet(key, k.provider);
      const provider = k.provider ?? this.deps.keyProvider();
      if (saves === undefined || provider === undefined) throw new Error(k.provider === undefined ? "this host forks machines on a provider that reads no API key, so there is none to save" : `${k.provider} is not a provider this wsp takes a key for`);
      const check = await this.deps.checkKey(key, k.provider);
      const line = keyCheckLine(check, provider);
      if (line !== undefined) throw Object.assign(new Error(line), { kind: check.state === "refused" ? KEY_REFUSED : KEY_UNCHECKED });
      this.deps.saveKeys(saves);
      this.deps.provider(this.deps.saved());
    }
    return this.get();
  }
}
