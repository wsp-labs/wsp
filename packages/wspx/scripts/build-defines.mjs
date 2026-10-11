// SPDX-License-Identifier: AGPL-3.0-only
// What a bundle bakes into the host at build time, read off the building
// shell: the PostHog project key the usage counts are sent under and the host
// they go to, and whether the build carries Solari. The release workflow
// sets them; every other build leaves them empty, and a host built with no key
// sends nothing. The names carry no WSP_ prefix because they are build inputs,
// never read by a running wsp.
export const buildDefines = (env = process.env) => ({
  __WSP_POSTHOG_KEY__: JSON.stringify(env.POSTHOG_KEY ?? ""),
  __WSP_POSTHOG_HOST__: JSON.stringify(env.POSTHOG_HOST ?? ""),
  ...cloudDefine(env),
});

/** Whether a build is the public one: PUBLIC_BUILD=1, which the release sets. A public build carries Boat and leaves
 * out Solari, the cloud's top-level roads and the skill text about them; every other build, the one wsp's own
 * development runs on included, keeps them all. */
export const publicBuild = (env = process.env) => env.PUBLIC_BUILD === "1";

/** The constant Solari's tables are gated on, for a bundler to fold: false in a public build, true in any other. */
export const cloudDefine = (env = process.env) => ({ __WSP_CLOUD__: JSON.stringify(!publicBuild(env)) });
