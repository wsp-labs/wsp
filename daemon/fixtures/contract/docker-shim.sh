#!/bin/sh
bin='{bin}'
notes='{notes}'
passed=${WSP_DOCKER_PASSED-}
WSP_DOCKER_PASSED=$bin${passed:+:$passed}
export WSP_DOCKER_PASSED
real=
set -f
IFS=:
for d in $PATH; do
  for p in $WSP_DOCKER_PASSED; do [ "$d" -ef "$p" ] && continue 2; done
  if [ -f "$d/docker" ] && [ -x "$d/docker" ]; then real=$d/docker; break; fi
done
unset IFS
set +f
if [ -z "$real" ]; then
  echo "docker is not installed on this computer" >&2
  exit 127
fi
thread=${WSP_THREAD-}
[ -z "$passed" ] && [ ${#thread} -le 64 ] || thread=
case $thread in ""|*[!A-Za-z0-9_-]*) exec "$real" "$@" ;; esac
label=wsp.thread=$thread
at=global want= note= first=1 project=${COMPOSE_PROJECT_NAME-} cfile= cdir= efile=
for a do
  [ -z "$first" ] || { set --; first=; }
  set -- "$@" "$a"
  if [ -n "$want" ]; then
    case $want in
    project) project=$a ;;
    file) [ -n "$cfile" ] || cfile=$a ;;
    dir) cdir=$a ;;
    envfile) efile=$a ;;
    esac
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
    -p=*) project=${a#-p=} ;;
    --project-name=*) project=${a#*=} ;;
    -p*) project=${a#-p} ;;
    -f|--file) want=file ;;
    -f=*|--file=*) [ -n "$cfile" ] || cfile=${a#*=} ;;
    -f*) [ -n "$cfile" ] || cfile=${a#-f} ;;
    --project-directory|--workdir) want=dir ;;
    --project-directory=*|--workdir=*) cdir=${a#*=} ;;
    --env-file) want=envfile ;;
    --env-file=*) efile=${a#*=} ;;
    --profile|--progress|--ansi|--parallel) want=value ;;
    -*) ;;
    run) set -- "$@" --label "$label"; at=done ;;
    up|create) note=1; at=done ;;
    *) at=done ;;
    esac ;;
  esac
done
if [ -n "$note" ]; then
  tab=$(printf '\t')
  folder=$PWD
  if [ -z "$cdir$cfile" ] && [ -z "${COMPOSE_FILE+set}" ]; then
    case ${efile:=.env} in /*) ;; *) efile=$PWD/$efile ;; esac
    [ ! -f "$efile" ] || cfile=$(sed -n 's/^[[:space:]]*\(export[[:space:]][[:space:]]*\)\{0,1\}COMPOSE_FILE[[:space:]]*=[[:space:]]*//p' "$efile" 2>/dev/null | tail -n 1 | tr -d '\r' | sed -e 's/[[:space:]]*$//' -e "s/^\"\(.*\)\"$/\1/" -e "s/^'\(.*\)'$/\1/")
    cfile=${cfile%%:*}
  fi
  [ -n "$cdir" ] || { cfile=${cfile:-${COMPOSE_FILE%%:*}}; case $cfile in ""|-) ;; */*) cdir=${cfile%/*} cdir=${cdir:-/} ;; *) cdir=. ;; esac; }
  if [ -n "$cdir" ]; then
    case $cdir in /*) ;; *) cdir=$PWD/$cdir ;; esac
    cdir=$(cd -- "$cdir" 2>/dev/null && pwd) && [ -n "$cdir" ] && folder=$cdir
  fi
  case $folder$project in
  *"$tab"*|*"
"*) ;;
  *)
    {
      mkdir -p "$notes"
      file=$notes/$thread
      [ ! -f "$file" ] || [ "$(wc -c < "$file")" -le 32768 ] || { tail -n 100 "$file" > "$file.part" && mv "$file.part" "$file"; }
      printf '%s\t%s\t%s\t%s\n' "$thread" "$folder" "$project" "$(date +%s)" >> "$file"
    } 2>/dev/null
    ;;
  esac
fi
exec "$real" "$@"
