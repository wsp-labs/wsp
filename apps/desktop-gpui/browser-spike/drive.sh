#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only
# drive.sh <binary> <out-folder> [scenario...]: runs the spike's scenarios (popover keys resize scroll memory, all by
# default) under an X display that has a window manager (DISPLAY, default :99) and writes each scenario's logs and
# screenshots to the out folder, then one verdict line per check. The app gets a clean environment; SPIKE_FORWARD_KEYS
# and SPIKE_GTK_POLL_MS pass through when set. Needs xdotool, xwininfo and ImageMagick's import. Stops only its pids.
set -u

[ $# -ge 2 ] || { echo "usage: drive.sh <browser-spike binary> <out folder> [popover|keys|resize|scroll|memory]..." >&2; exit 2; }
bin=$1
out=$2
shift 2
scenarios=${*:-popover keys resize scroll memory}
mkdir -p "$out/home"
rm -f "$out"/keys.txt "$out"/resize.txt "$out"/scroll.txt "$out"/mem.txt
export DISPLAY=${DISPLAY:-:99}
here=$(cd "$(dirname "$0")" && pwd)
pid=
passed=()
[ -n "${SPIKE_FORWARD_KEYS:-}" ] && passed+=("SPIKE_FORWARD_KEYS=$SPIKE_FORWARD_KEYS")
[ -n "${SPIKE_GTK_POLL_MS:-}" ] && passed+=("SPIKE_GTK_POLL_MS=$SPIKE_GTK_POLL_MS")

stop() {
  if [ -n "$pid" ] && [ "$(ps -o ppid= -p "$pid" | tr -d ' ')" = "$$" ]; then
    kill "$pid" 2>/dev/null
    wait "$pid" 2>/dev/null
  fi
  pid=
}
trap stop EXIT
trap 'stop; exit 130' INT TERM HUP

# Starts the app and waits for its page to load (or, with --no-page, its window), then for the window manager to
# focus its 1280 px window. GPUI sets no _NET_WM_PID, and the last run's window is gone once stop() returns.
launch() {
  local name=$1 ready="page: load finished" focused
  shift
  case " $* " in *" --no-page "*) ready="window: opened" ;; esac
  env -i HOME="$out/home" PATH=/usr/bin:/bin LANG=C.UTF-8 DISPLAY="$DISPLAY" ${passed[@]+"${passed[@]}"} \
    "$bin" "$@" > "$out/$name.log" 2>&1 &
  pid=$!
  for _ in $(seq 60); do
    grep -q "$ready" "$out/$name.log" 2>/dev/null && break
    sleep 0.5
  done
  grep -q "$ready" "$out/$name.log" || { echo "$name: never said \"$ready\" in 30 s" >&2; exit 1; }
  sleep 3
  win=
  for _ in $(seq 20); do
    focused=$(xdotool getactivewindow 2>/dev/null) || focused=
    if [ -n "$focused" ] && xwininfo -id "$focused" 2>/dev/null | grep -q "Width: 1280"; then
      win=$focused
      break
    fi
    sleep 0.5
  done
  [ -n "$win" ] || { echo "$name: no focused 1280 px window; is a window manager running on $DISPLAY?" >&2; exit 1; }
}

# Window-relative coordinates; the pane's origin comes from the run's own log.
at() { xdotool mousemove --window "$win" "$1" "$2"; }
click() { at "$1" "$2"; sleep 0.2; xdotool click 1; sleep 0.5; }
pane() { grep "pane bounds" "$out/$1.log" | tail -1 | sed 's/.*pane bounds: \([0-9]*\),\([0-9]*\) \([0-9]*\)x\([0-9]*\).*/\1 \2 \3 \4/'; }
has() { if grep -q "$2" "$out/$1.log"; then echo "yes  $3"; else echo "no   $3"; fi; }
shot() { import -window root "$out/$1.png"; }
# The X11 windows directly under the app's window: the page's own, or the composition surface holding it.
page_windows() { xwininfo -id "$win" -children | grep -oE '0x[0-9a-f]+ [^:]*: \(\)  [0-9]+x[0-9]+\+-?[0-9]+\+-?[0-9]+' | tr '\n' ';'; }

# A pid and everything under it, as bench/mem.sh walks it.
tree() {
  ps -A -o pid= -o ppid= | awk -v root="$1" '
    { parent[$1] = $2; seen[NR] = $1 }
    END {
      keep[root] = 1; grew = 1
      while (grew) { grew = 0; for (i in seen) { p = seen[i]; if (!(p in keep) && (parent[p] in keep)) { keep[p] = 1; grew = 1 } } }
      for (p in keep) print p
    }'
}
# Proportional set size in MB: a page shared between processes is split between them, not counted in each.
pss() {
  local total=0 p kb
  for p in $(tree "$1"); do
    kb=$(awk '/^Pss:/{print $2}' "/proc/$p/smaps_rollup" 2>/dev/null)
    total=$((total + ${kb:-0}))
  done
  awk -v kb="$total" 'BEGIN { printf "%.1f", kb / 1024 }'
}
# User and system clock ticks so far.
ticks() {
  local total=0 p t
  for p in $(tree "$1"); do
    t=$(awk '{print $14 + $15}' "/proc/$p/stat" 2>/dev/null)
    total=$((total + ${t:-0}))
  done
  echo "$total"
}
# Share of one core over 10 s, read from 10 s after launch() returned.
cpu() {
  local before
  sleep 10
  before=$(ticks "$pid")
  sleep 10
  awk -v t="$(($(ticks "$pid") - before))" -v hz="$(getconf CLK_TCK)" 'BEGIN { printf "%.1f", t * 100 / hz / 10 }'
}

echo "binary: $bin${passed[*]+, with ${passed[*]}}"

# 1. The popover over the page, opened at launch, and the page alone.
popover() {
  launch popover --menu-open
  shot popover
  stop
  launch plain
  shot plain
  stop
}

# 2. Keys and focus, one launch per question so each log answers one thing.
keys() {
  launch keys-launch
  xdotool key ctrl+k
  sleep 1
  stop

  launch keys-page
  read -r px py pw ph <<< "$(pane keys-page)"
  click $((px + 100)) $((py + 28))
  xdotool type --delay 60 "hello"
  sleep 1
  xdotool key ctrl+k
  sleep 1
  shot keys-page-ctrl-k
  xdotool key ctrl+shift+p
  sleep 1
  xdotool key Escape
  sleep 1
  stop

  launch keys-menu-escape
  read -r px py pw ph <<< "$(pane keys-menu-escape)"
  click $((px + pw - 40)) 22
  shot keys-menu-open
  xdotool key Escape
  sleep 1
  stop

  launch keys-menu-press
  read -r px py pw ph <<< "$(pane keys-menu-press)"
  click $((px + pw - 40)) 22
  click $((px + 200)) $((py + 400))
  shot keys-menu-after-page-press
  stop

  launch keys-menu-item
  read -r px py pw ph <<< "$(pane keys-menu-item)"
  click $((px + pw - 40)) 22
  read -r mx my <<< "$(grep "menu bounds" "$out/keys-menu-item.log" | tail -1 | sed 's/.*menu bounds: \([0-9]*\),\([0-9]*\) .*/\1 \2/')"
  click $((mx + 60)) $((my + 130))
  shot keys-menu-after-item-press
  stop

  launch keys-address
  read -r px py pw ph <<< "$(pane keys-address)"
  click $((px + 150)) 22
  xdotool key ctrl+a
  xdotool type --delay 30 "https://example.com/"
  xdotool key Return
  sleep 6
  xdotool key ctrl+k
  sleep 1
  shot keys-address-example-com
  xdotool key Escape
  sleep 1
  stop

  {
    echo "-- keys"
    has keys-launch "chord: ctrl-k reached GPUI" "at launch, before any click, ctrl-k reaches GPUI"
    has keys-launch "page: key ctrl-k$" "at launch, before any click, ctrl-k reaches the page"
    has keys-page "page: field hello" "after a click in the page's field, typing reaches the field"
    has keys-page "chord: ctrl-k reached GPUI" "with the page focused, ctrl-k reaches GPUI"
    has keys-page "chord: ctrl-shift-p reached GPUI" "with the page focused, ctrl-shift-p reaches GPUI"
    has keys-page "escape: reached GPUI" "with the page focused, Escape reaches GPUI"
    has keys-page "page: key Escape$" "with the page focused, Escape reaches the page"
    has keys-menu-escape "menu: open (button)" "the Menu button opens the popover"
    has keys-menu-escape "menu: closed (escape)" "Escape closes the popover right after it opened"
    has keys-menu-press "menu: closed (press outside)" "a press on the page closes the popover"
    has keys-menu-item "menu: picked" "a press on a popover item that lies over the page picks the item"
    has keys-menu-item "page: press forwarded" "that press went to the page first (wry road only logs this)"
    has keys-address "address: enter https://example.com/" "typing in the address field and Enter reach GPUI"
    has keys-address "page: load finished https://example.com/" "the page loads example.com"
    has keys-address "chord: ctrl-k reached GPUI" "with the address field focused, ctrl-k reaches GPUI"
    has keys-address "escape: reached GPUI" "with the address field focused, Escape reaches GPUI"
  } > "$out/keys.txt"
}

# 3. Resize: the window, then the panel's divider, each followed by the windows X11 has under the app's.
resize() {
  launch resize
  read -r px py pw ph <<< "$(pane resize)"
  {
    echo "-- resize"
    echo "pane at launch: $px,$py ${pw}x$ph; X11 children: $(page_windows)"
  } > "$out/resize.txt"
  xdotool windowsize "$win" 1000 700
  sleep 2
  shot resize-window
  read -r px py pw ph <<< "$(pane resize)"
  echo "pane after windowsize 1000x700: $px,$py ${pw}x$ph; X11 children: $(page_windows)" >> "$out/resize.txt"
  at $((px - 3)) 300
  xdotool mousedown 1
  for step in $(seq 1 10); do
    at $((px - 3 + step * 15)) 300
    sleep 0.05
  done
  xdotool mouseup 1
  sleep 2
  shot resize-drag
  read -r px py pw ph <<< "$(pane resize)"
  {
    echo "pane after dragging the divider 150 px right: $px,$py ${pw}x$ph; X11 children: $(page_windows)"
    grep "panel drag" "$out/resize.log" | sed 's/^/app said: /'
    grep "page: viewport" "$out/resize.log" | tail -2 | sed 's/^/page saw: /'
  } >> "$out/resize.txt"
  stop
}

# 4. Scroll: 180 wheel notches at 60 a second over the page, which reports frames a second that moved.
scroll() {
  launch scroll
  read -r px py pw ph <<< "$(pane scroll)"
  click $((px + 300)) $((py + 300))
  xdotool click --repeat 180 --delay 16 5
  sleep 2
  stop
  {
    echo "-- scroll"
    grep "gtk poll" "$out/scroll.log"
    grep "scroll-fps" "$out/scroll.log"
  } >> "$out/scroll.txt"
}

# 5. At rest, with the page and with no page: CPU, then memory, every process under the app counted.
memory() {
  echo "-- memory" >> "$out/mem.txt"
  launch mem
  local load
  load=$(cpu)
  echo "with one page: $("$here/../bench/mem.sh" "$pid" 2>&1 | tr '\n' ' ')MB; proportional $(pss "$pid") MB; CPU at rest $load% of a core" >> "$out/mem.txt"
  ps -o pid=,rss=,args= --ppid "$pid" | sed 's/^/  child (rss KB): /' >> "$out/mem.txt"
  stop
  launch mem-nopage --no-page
  load=$(cpu)
  echo "no page: $("$here/../bench/mem.sh" "$pid" 2>&1 | tr '\n' ' ')MB; proportional $(pss "$pid") MB; CPU at rest $load% of a core" >> "$out/mem.txt"
  stop
}

for scenario in $scenarios; do
  case $scenario in
    popover | keys | resize | scroll | memory) "$scenario" ;;
    *) echo "unknown scenario $scenario" >&2; exit 2 ;;
  esac
done
for report in keys resize scroll mem; do
  if [ -f "$out/$report.txt" ]; then cat "$out/$report.txt"; fi
done
