// SPDX-License-Identifier: AGPL-3.0-only
//! The docker a thread finds first on its PATH. Port of the function of packages/protocol/src/docker-shim.ts at
//! a3b870a6e; its constants are wsp_wire::types::docker_shim. See spec/shell_and_text.md.

// docker-shim.ts:5
// in wsp_wire::types::docker_shim: DOCKER_NOTES_FOLDER

// docker-shim.ts:8
// in wsp_wire::types::docker_shim: DOCKER_THREAD_LABEL

// docker-shim.ts:11
// in wsp_wire::types::docker_shim: DOCKER_NOTES_CAP_BYTES

// docker-shim.ts:12
const DOCKER_NOTES_KEPT_LINES: i64 = 100;

/// The docker a thread finds first on its PATH, in the folder that also holds its wsp: `thread-bin` on the computer
/// the host runs on, `place-bin` on a box. It runs the docker after it on PATH with every word as given, adding the
/// thread's label to `run`, `create`, `container run|create` and `compose run`. `compose up|create` gets no label,
/// since compose hashes labels into a service's config and a second thread's `up` would recreate every container
/// (measured 2026-10-12): the shim appends one note instead, `thread`, folder, project and epoch seconds, tab apart. The
/// folder is the one compose labels the project's containers with as `working_dir`: `--project-directory`, else the first
/// compose file's folder (`-f`, then `COMPOSE_FILE`, then the `COMPOSE_FILE` line of the env file compose reads, `.env`
/// or `--env-file`'s, which the shim reads for that line alone and only where it is a plain file), else this one, spelt
/// as the shell spells it, links kept.
/// `WSP_DOCKER_PASSED` names the shim folders a call came through, so a second shim on PATH, however PATH spells its
/// folder, neither loops back nor labels twice. The daemon's twin is `docker_shim`, held to this text by the contract fixture.
// docker-shim.ts:25
// export function dockerShim(bin: string, notes: string): string {
//   return DOCKER_SHIM.replace("{bin}", shellQuote(bin)).replace("{notes}", shellQuote(notes));
// }
pub fn docker_shim(bin: &str, notes: &str) -> String {
    todo!()
}

// docker-shim.ts:29
// private in the TS: its value with the two placeholders, copied by script from the TS:
const DOCKER_SHIM: &str = "#!/bin/sh\nbin={bin}\nnotes={notes}\npassed=${WSP_DOCKER_PASSED-}\nWSP_DOCKER_PASSED=$bin${passed:+:$passed}\nexport WSP_DOCKER_PASSED\nreal=\nset -f\nIFS=:\nfor d in $PATH; do\n  for p in $WSP_DOCKER_PASSED; do [ \"$d\" -ef \"$p\" ] && continue 2; done\n  if [ -f \"$d/docker\" ] && [ -x \"$d/docker\" ]; then real=$d/docker; break; fi\ndone\nunset IFS\nset +f\nif [ -z \"$real\" ]; then\n  echo \"docker is not installed on this computer\" >&2\n  exit 127\nfi\nthread=${WSP_THREAD-}\n[ -z \"$passed\" ] && [ ${#thread} -le 64 ] || thread=\ncase $thread in \"\"|*[!A-Za-z0-9_-]*) exec \"$real\" \"$@\" ;; esac\nlabel=wsp.thread=$thread\nat=global want= note= first=1 project=${COMPOSE_PROJECT_NAME-} cfile= cdir= efile=\nfor a do\n  [ -z \"$first\" ] || { set --; first=; }\n  set -- \"$@\" \"$a\"\n  if [ -n \"$want\" ]; then\n    case $want in\n    project) project=$a ;;\n    file) [ -n \"$cfile\" ] || cfile=$a ;;\n    dir) cdir=$a ;;\n    envfile) efile=$a ;;\n    esac\n    want=\n    continue\n  fi\n  case $at in\n  global)\n    case $a in\n    --config|-c|--context|-H|--host|-l|--log-level|--tlscacert|--tlscert|--tlskey|-D[cHl]) want=value ;;\n    -*) ;;\n    run|create) set -- \"$@\" --label \"$label\"; at=done ;;\n    container) at=container ;;\n    compose) at=compose ;;\n    *) at=done ;;\n    esac ;;\n  container)\n    case $a in\n    run|create) set -- \"$@\" --label \"$label\"; at=done ;;\n    -*) ;;\n    *) at=done ;;\n    esac ;;\n  compose)\n    case $a in\n    -p|--project-name) want=project ;;\n    -p=*) project=${a#-p=} ;;\n    --project-name=*) project=${a#*=} ;;\n    -p*) project=${a#-p} ;;\n    -f|--file) want=file ;;\n    -f=*|--file=*) [ -n \"$cfile\" ] || cfile=${a#*=} ;;\n    -f*) [ -n \"$cfile\" ] || cfile=${a#-f} ;;\n    --project-directory|--workdir) want=dir ;;\n    --project-directory=*|--workdir=*) cdir=${a#*=} ;;\n    --env-file) want=envfile ;;\n    --env-file=*) efile=${a#*=} ;;\n    --profile|--progress|--ansi|--parallel) want=value ;;\n    -*) ;;\n    run) set -- \"$@\" --label \"$label\"; at=done ;;\n    up|create) note=1; at=done ;;\n    *) at=done ;;\n    esac ;;\n  esac\ndone\nif [ -n \"$note\" ]; then\n  tab=$(printf '\\t')\n  folder=$PWD\n  if [ -z \"$cdir$cfile\" ] && [ -z \"${COMPOSE_FILE+set}\" ]; then\n    case ${efile:=.env} in /*) ;; *) efile=$PWD/$efile ;; esac\n    [ ! -f \"$efile\" ] || cfile=$(sed -n 's/^[[:space:]]*\\(export[[:space:]][[:space:]]*\\)\\{0,1\\}COMPOSE_FILE[[:space:]]*=[[:space:]]*//p' \"$efile\" 2>/dev/null | tail -n 1 | tr -d '\\r' | sed -e 's/[[:space:]]*$//' -e \"s/^\\\"\\(.*\\)\\\"$/\\1/\" -e \"s/^'\\(.*\\)'$/\\1/\")\n    cfile=${cfile%%:*}\n  fi\n  [ -n \"$cdir\" ] || { cfile=${cfile:-${COMPOSE_FILE%%:*}}; case $cfile in \"\"|-) ;; */*) cdir=${cfile%/*} cdir=${cdir:-/} ;; *) cdir=. ;; esac; }\n  if [ -n \"$cdir\" ]; then\n    case $cdir in /*) ;; *) cdir=$PWD/$cdir ;; esac\n    cdir=$(cd -- \"$cdir\" 2>/dev/null && pwd) && [ -n \"$cdir\" ] && folder=$cdir\n  fi\n  case $folder$project in\n  *\"$tab\"*|*\"\n\"*) ;;\n  *)\n    {\n      mkdir -p \"$notes\"\n      file=$notes/$thread\n      [ ! -f \"$file\" ] || [ \"$(wc -c < \"$file\")\" -le 32768 ] || { tail -n 100 \"$file\" > \"$file.part\" && mv \"$file.part\" \"$file\"; }\n      printf '%s\\t%s\\t%s\\t%s\\n' \"$thread\" \"$folder\" \"$project\" \"$(date +%s)\" >> \"$file\"\n    } 2>/dev/null\n    ;;\n  esac\nfi\nexec \"$real\" \"$@\"\n";
// const DOCKER_SHIM = `#!/bin/sh
// bin={bin}
// notes={notes}
// passed=\${WSP_DOCKER_PASSED-}
// WSP_DOCKER_PASSED=$bin\${passed:+:$passed}
// export WSP_DOCKER_PASSED
// real=
// set -f
// IFS=:
// for d in $PATH; do
//   for p in $WSP_DOCKER_PASSED; do [ "$d" -ef "$p" ] && continue 2; done
//   if [ -f "$d/docker" ] && [ -x "$d/docker" ]; then real=$d/docker; break; fi
// done
// unset IFS
// set +f
// if [ -z "$real" ]; then
//   echo "docker is not installed on this computer" >&2
//   exit 127
// fi
// thread=\${WSP_THREAD-}
// [ -z "$passed" ] && [ \${#thread} -le 64 ] || thread=
// case $thread in ""|*[!A-Za-z0-9_-]*) exec "$real" "$@" ;; esac
// label=${DOCKER_THREAD_LABEL}=$thread
// at=global want= note= first=1 project=\${COMPOSE_PROJECT_NAME-} cfile= cdir= efile=
// for a do
//   [ -z "$first" ] || { set --; first=; }
//   set -- "$@" "$a"
//   if [ -n "$want" ]; then
//     case $want in
//     project) project=$a ;;
//     file) [ -n "$cfile" ] || cfile=$a ;;
//     dir) cdir=$a ;;
//     envfile) efile=$a ;;
//     esac
//     want=
//     continue
//   fi
//   case $at in
//   global)
//     case $a in
//     --config|-c|--context|-H|--host|-l|--log-level|--tlscacert|--tlscert|--tlskey|-D[cHl]) want=value ;;
//     -*) ;;
//     run|create) set -- "$@" --label "$label"; at=done ;;
//     container) at=container ;;
//     compose) at=compose ;;
//     *) at=done ;;
//     esac ;;
//   container)
//     case $a in
//     run|create) set -- "$@" --label "$label"; at=done ;;
//     -*) ;;
//     *) at=done ;;
//     esac ;;
//   compose)
//     case $a in
//     -p|--project-name) want=project ;;
//     -p=*) project=\${a#-p=} ;;
//     --project-name=*) project=\${a#*=} ;;
//     -p*) project=\${a#-p} ;;
//     -f|--file) want=file ;;
//     -f=*|--file=*) [ -n "$cfile" ] || cfile=\${a#*=} ;;
//     -f*) [ -n "$cfile" ] || cfile=\${a#-f} ;;
//     --project-directory|--workdir) want=dir ;;
//     --project-directory=*|--workdir=*) cdir=\${a#*=} ;;
//     --env-file) want=envfile ;;
//     --env-file=*) efile=\${a#*=} ;;
//     --profile|--progress|--ansi|--parallel) want=value ;;
//     -*) ;;
//     run) set -- "$@" --label "$label"; at=done ;;
//     up|create) note=1; at=done ;;
//     *) at=done ;;
//     esac ;;
//   esac
// done
// if [ -n "$note" ]; then
//   tab=$(printf '\\t')
//   folder=$PWD
//   if [ -z "$cdir$cfile" ] && [ -z "\${COMPOSE_FILE+set}" ]; then
//     case \${efile:=.env} in /*) ;; *) efile=$PWD/$efile ;; esac
//     [ ! -f "$efile" ] || cfile=$(sed -n 's/^[[:space:]]*\\(export[[:space:]][[:space:]]*\\)\\{0,1\\}COMPOSE_FILE[[:space:]]*=[[:space:]]*//p' "$efile" 2>/dev/null | tail -n 1 | tr -d '\\r' | sed -e 's/[[:space:]]*$//' -e "s/^\\"\\(.*\\)\\"$/\\1/" -e "s/^'\\(.*\\)'$/\\1/")
//     cfile=\${cfile%%:*}
//   fi
//   [ -n "$cdir" ] || { cfile=\${cfile:-\${COMPOSE_FILE%%:*}}; case $cfile in ""|-) ;; */*) cdir=\${cfile%/*} cdir=\${cdir:-/} ;; *) cdir=. ;; esac; }
//   if [ -n "$cdir" ]; then
//     case $cdir in /*) ;; *) cdir=$PWD/$cdir ;; esac
//     cdir=$(cd -- "$cdir" 2>/dev/null && pwd) && [ -n "$cdir" ] && folder=$cdir
//   fi
//   case $folder$project in
//   *"$tab"*|*"
// "*) ;;
//   *)
//     {
//       mkdir -p "$notes"
//       file=$notes/$thread
//       [ ! -f "$file" ] || [ "$(wc -c < "$file")" -le ${DOCKER_NOTES_CAP_BYTES} ] || { tail -n ${DOCKER_NOTES_KEPT_LINES} "$file" > "$file.part" && mv "$file.part" "$file"; }
//       printf '%s\\t%s\\t%s\\t%s\\n' "$thread" "$folder" "$project" "$(date +%s)" >> "$file"
//     } 2>/dev/null
//     ;;
//   esac
// fi
// exec "$real" "$@"
// `;

