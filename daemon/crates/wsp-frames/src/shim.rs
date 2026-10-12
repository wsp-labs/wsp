// SPDX-License-Identifier: AGPL-3.0-only
//! The wsp a process inside a machine runs, as two lines of shell onto the binary that answers the word. The
//! host's deploy writes it on a fork and the workspace runtime writes it into a workspace's own upper on a
//! computer somebody owns; the text is here so the two roads cannot spell it apart, and the protocol's twin is
//! held to it by the contract fixture.

use crate::shell::shell_quote;

/// The shim at `numbers::GUEST_WSP_PATH`, handing the whole line to the binary named.
pub fn guest_wsp_shim(binary: &str) -> String {
    format!("#!/bin/sh\nexec {binary} wsp \"$@\"\n")
}

/// The wsp a thread on a computer somebody joined runs, which that computer's daemon writes in wsp's folder under
/// the login's home: the line goes to the door named and nowhere else. That computer's loopback port is anybody's
/// to bind and the token file beside it is its daemon's own, so a door that is missing is said, never routed around.
/// The daemon's unit rides along where it has one, so that sentence names the restart that opens the door again.
pub fn computer_wsp_shim(binary: &str, door: &str, unit: Option<&str>) -> String {
    let unit = unit.map(|unit| format!("{COMPUTER_WSP_UNIT_FLAG} {} ", shell_quote(unit))).unwrap_or_default();
    format!("#!/bin/sh\nexec {} {COMPUTER_WSP_VERB} {unit}{} \"$@\"\n", shell_quote(binary), shell_quote(door))
}

/// The docker a thread on a computer somebody joined finds first on its PATH, beside its wsp: it labels the thread's
/// `run` and `create` and notes its `compose up`, then runs the docker after it on PATH. The protocol's `dockerShim`
/// says why, and the contract fixture holds the two to one text.
pub fn docker_shim(bin: &str, notes: &str) -> String {
    DOCKER_SHIM.replacen("{bin}", &shell_quote(bin), 1).replacen("{notes}", &shell_quote(notes), 1)
}

const DOCKER_SHIM: &str = r#"#!/bin/sh
bin={bin}
notes={notes}
passed=${WSP_DOCKER_PASSED-}
WSP_DOCKER_PASSED=$bin${passed:+:$passed}
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
thread=${WSP_THREAD-}
[ -z "$passed" ] && [ ${#thread} -le 64 ] || thread=
case $thread in ""|*[!A-Za-z0-9_-]*) exec "$real" "$@" ;; esac
label=wsp.thread=$thread
at=global want= note= first=1 project=${COMPOSE_PROJECT_NAME-}
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
    -p=*) project=${a#-p=} ;;
    --project-name=*) project=${a#*=} ;;
    -p*) project=${a#-p} ;;
    -f|--file|--project-directory|--workdir|--env-file|--profile|--progress|--ansi|--parallel) want=value ;;
    -*) ;;
    run) set -- "$@" --label "$label"; at=done ;;
    up|create) note=1; at=done ;;
    *) at=done ;;
    esac ;;
  esac
done
if [ -n "$note" ]; then
  tab=$(printf '\t')
  case $PWD$project in
  *"$tab"*|*"
"*) ;;
  *)
    {
      mkdir -p "$notes"
      file=$notes/$thread
      [ ! -f "$file" ] || [ "$(wc -c < "$file")" -le 32768 ] || { tail -n 100 "$file" > "$file.part" && mv "$file.part" "$file"; }
      printf '%s\t%s\t%s\t%s\n' "$thread" "$PWD" "$project" "$(date +%s)" >> "$file"
    } 2>/dev/null
    ;;
  esac
fi
exec "$real" "$@"
"#;

/// The verb that shim runs, the door its first word after it.
pub const COMPUTER_WSP_VERB: &str = "wsp-door";
/// The flag ahead of the door naming the daemon's unit.
pub const COMPUTER_WSP_UNIT_FLAG: &str = "--unit";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_computer_shim_names_its_door_and_no_port() {
        assert_eq!(
            computer_wsp_shim("/usr/local/bin/wsp-daemon", "/home/o'b/.wsp/daemon.sock", None),
            "#!/bin/sh\nexec '/usr/local/bin/wsp-daemon' wsp-door '/home/o'\\''b/.wsp/daemon.sock' \"$@\"\n"
        );
        assert_eq!(
            computer_wsp_shim("/d", "/root/.wsp/daemon.sock", Some("wsp-place-ecffdb75.service")),
            "#!/bin/sh\nexec '/d' wsp-door --unit 'wsp-place-ecffdb75.service' '/root/.wsp/daemon.sock' \"$@\"\n"
        );
    }

    #[test]
    fn the_shim_hands_the_line_on_whole_to_the_binary_it_names() {
        assert_eq!(guest_wsp_shim("/sbin/wsp-init"), "#!/bin/sh\nexec /sbin/wsp-init wsp \"$@\"\n");
    }
}
