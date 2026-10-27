// SPDX-License-Identifier: AGPL-3.0-only
const speaks = (min: number, max: number): string => (min === max ? `protocol ${min}` : `protocols ${min} to ${max}`);

/** The two halves of the refusal a host gives an app whose protocol numbers leave out the host's own: what is true of
 * the two, and which one to update. A host above the app's highest is newer than the app; one below its lowest is
 * older. */
export function protocolMismatchHalves(host: number, range: readonly [number, number]): { happened: string; fix: string } {
  const [min, max] = range;
  const app = speaks(min, max);
  if (host > max) {
    return {
      happened: `This host is newer than the app: it speaks protocol ${host} and the app speaks ${app}.`,
      fix: "Update the app, and the wsp command with it, so they match this host.",
    };
  }
  return {
    happened: `This host is older than the app: it speaks protocol ${host} and the app speaks ${app}.`,
    fix: "Run wsp update on the computer this host runs on, then open the app again.",
  };
}
