// SPDX-License-Identifier: AGPL-3.0-only
import { shellQuote } from "./shell-quote.js";

/** The folder a thread's compose notes go in, one file per thread named by its id, beside the folder the shim sits in. */
export const DOCKER_NOTES_FOLDER = "docker-notes";

/** The label a container a thread made carries, its value the thread's id. */
export const DOCKER_THREAD_LABEL = "wsp.thread";

/** A thread's notes file past this many bytes keeps only its newest lines before the next note goes on. */
export const DOCKER_NOTES_CAP_BYTES = 32_768;
const DOCKER_NOTES_KEPT_LINES = 100;

/** The docker a thread finds first on its PATH, in the folder that also holds its wsp: `thread-bin` on the computer
 * the host runs on, `place-bin` on a box. It runs the docker after it on PATH with every word as given, adding the
 * thread's label to `run`, `create`, `container run|create` and `compose run`. `compose up|create` gets no label,
 * since compose hashes labels into a service's config and a second thread's `up` would recreate every container
 * (measured 2026-10-12): the shim appends one note instead, `thread`, folder, project and epoch seconds, tab apart.
 * `WSP_DOCKER_PASSED` names the shim folders a call came through, so a second shim on PATH neither loops back nor
 * labels twice. The daemon's twin is `docker_shim`, held to this text by the contract fixture. */
export function dockerShim(bin: string, notes: string): string {
  return DOCKER_SHIM.replace("{bin}", shellQuote(bin)).replace("{notes}", shellQuote(notes));
}

const DOCKER_SHIM = `#!/bin/sh
bin={bin}
notes={notes}
passed=\${WSP_DOCKER_PASSED-}
WSP_DOCKER_PASSED=$bin\${passed:+:$passed}
export WSP_DOCKER_PASSED
real=
set -f
IFS=:
for d in $PATH; do
  case :$WSP_DOCKER_PASSED: in *:"$d":*) continue ;; esac
  if [ -f "$d/docker" ] && [ -x "$d/docker" ]; then real=$d/docker; break; fi
done
unset IFS
set +f
if [ -z "$real" ]; then
  echo "docker is not installed on this computer" >&2
  exit 127
fi
thread=\${WSP_THREAD-}
[ -z "$passed" ] && [ \${#thread} -le 64 ] || thread=
case $thread in ""|*[!A-Za-z0-9_-]*) exec "$real" "$@" ;; esac
label=${DOCKER_THREAD_LABEL}=$thread
at=global want= note= first=1 project=\${COMPOSE_PROJECT_NAME-}
for a do
  [ -z "$first" ] || { set --; first=; }
  set -- "$@" "$a"
  if [ -n "$want" ]; then
    [ "$want" = project ] && project=$a
    want=
    continue
  fi
  case $at in
  global)
    case $a in
    --config|-c|--context|-H|--host|-l|--log-level|--tlscacert|--tlscert|--tlskey|-D[cHl]) want=value ;;
    -*) ;;
    run|create) set -- "$@" --label "$label"; at=done ;;
    container) at=container ;;
    compose) at=compose ;;
    *) at=done ;;
    esac ;;
  container)
    case $a in
    run|create) set -- "$@" --label "$label"; at=done ;;
    -*) ;;
    *) at=done ;;
    esac ;;
  compose)
    case $a in
    -p|--project-name) want=project ;;
    -p=*) project=\${a#-p=} ;;
    --project-name=*) project=\${a#*=} ;;
    -p*) project=\${a#-p} ;;
    -f|--file|--project-directory|--workdir|--env-file|--profile|--progress|--ansi|--parallel) want=value ;;
    -*) ;;
    run) set -- "$@" --label "$label"; at=done ;;
    up|create) note=1; at=done ;;
    *) at=done ;;
    esac ;;
  esac
done
if [ -n "$note" ]; then
  tab=$(printf '\\t')
  case $PWD$project in
  *"$tab"*|*"
"*) ;;
  *)
    {
      mkdir -p "$notes"
      file=$notes/$thread
      [ ! -f "$file" ] || [ "$(wc -c < "$file")" -le ${DOCKER_NOTES_CAP_BYTES} ] || { tail -n ${DOCKER_NOTES_KEPT_LINES} "$file" > "$file.part" && mv "$file.part" "$file"; }
      printf '%s\\t%s\\t%s\\t%s\\n' "$thread" "$PWD" "$project" "$(date +%s)" >> "$file"
    } 2>/dev/null
    ;;
  esac
fi
exec "$real" "$@"
`;
