# project_look
Source: the functions of `packages/protocol/src/project-look.ts` (28 lines) at a3b870a6e; its zod shapes (`ProjectIcon`, `ProjectHue`, `ProjectIconHash`) and flat constants (`PROJECT_ICON_PX`, `PROJECT_ICON_MAX_BYTES`, `PROJECT_ICON_SOURCE_PX`, `PROJECT_ICON_TYPES`, `PROJECT_ICON_AGAIN`) are `wsp_wire::types::project_look`. Tests: none; the TS has no test file for this module and there is no `tests/project_look.rs`.

## What it does
The words of a project's look that are not wire shapes: the one refusal the host gives when it will not keep an image a person picked for a project, and what that refusal tells the person to do.

## Behaviour
1. `project_icon_refusal(why)`: `said` is "The host did not take this image: <why>." and `fix` is PROJECT_ICON_AGAIN, "Pick it again; if it is refused again, pick another file." (project-look.ts:28). `why` is put in as given, with nothing trimmed or checked.

## Edge cases
- A `why` that ends in "." reads with two dots: the sentence always adds its own (read off project-look.ts:28). The one caller passes reasons with no period (packages/runtime/src/projects/images.ts, for example "it is not a PNG").

## Behaviour nobody wrote down
- None of the port rules: no length, cut, number or time is read here.

## Tapes
The refusal of a `projects.icon` request, which the runtime raises as a usage refusal from `said` and `fix` (packages/runtime/src/projects/images.ts:26-29).

## Decisions
1. The TS answer `{ said: string; fix: string }` is the struct `SaidFix { said, fix }`, Serialize under the same keys.

## After the port
- none.

## Rules pinned
| Rule | Test |
|---|---|
| 1 | none: no test pins it, in TS or Rust |
