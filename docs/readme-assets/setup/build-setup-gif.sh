#!/bin/sh
set -eu

src=${1:?usage: build-setup-gif.sh <setup.mov> <output.gif> <wallpaper> <start:duration[,start:duration...]> [speed] [hold]}
out=${2:?usage: build-setup-gif.sh <setup.mov> <output.gif> <wallpaper> <start:duration[,start:duration...]> [speed] [hold]}
wall=${3:?usage: build-setup-gif.sh <setup.mov> <output.gif> <wallpaper> <start:duration[,start:duration...]> [speed] [hold]}
cuts=${4:?usage: build-setup-gif.sh <setup.mov> <output.gif> <wallpaper> <start:duration[,start:duration...]> [speed] [hold]}
speed=${5:-3}
hold=${6:-1.5}

dimensions=$(ffprobe -v error -select_streams v:0 -show_entries stream=width,height -of csv=s=x:p=0 "$src")
input_width=${dimensions%x*}
input_height=${dimensions#*x}
if [ "$input_width" -le 0 ] || [ "$input_height" -le 0 ]; then
  echo "invalid source dimensions: $dimensions" >&2
  exit 2
fi
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
mask_file="${TMPDIR:-/tmp}/wsp-readme-setup-window-mask-${dimensions}.png"
node "$script_dir/../window-mask.mjs" "$input_width" "$input_height" "$mask_file"

graph=
labels=
mask_outputs=
count=0
old_ifs=$IFS
IFS=,
for cut in $cuts; do
  start=${cut%%:*}
  duration=${cut#*:}
  label="seg$count"
  raw="raw$count"
  mask="mask$count"
  graph="${graph}[0:v]trim=start=$start:duration=$duration,setpts=(PTS-STARTPTS)/$speed,format=rgba[$raw];[$raw][$mask]alphamerge=shortest=1,scale=1600:1104:force_original_aspect_ratio=decrease:flags=lanczos,setsar=1[$label];"
  labels="${labels}[$label]"
  mask_outputs="${mask_outputs}[$mask]"
  count=$((count + 1))
done
IFS=$old_ifs
if [ "$count" -eq 0 ]; then
  echo "no trim segments" >&2
  exit 2
fi
graph="[2:v]alphaextract,split=$count${mask_outputs};${graph}${labels}concat=n=$count:v=1:a=0,tpad=stop_mode=clone:stop_duration=$hold[fg];[1:v]scale=1600:1104:force_original_aspect_ratio=increase:flags=lanczos,crop=1600:1104[bg];[bg][fg]overlay=(W-w)/2:(H-h)/2:shortest=1:format=auto,fps=10,split[a][b];[a]palettegen=max_colors=128:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=3[out]"

# setup.mov is a full app-window recording. Preserve the whole app and setup
# dialog, fit it inside the fixed frame over the theme's wallpaper, and hold
# the final ready view.
ffmpeg -loglevel error -y -i "$src" -loop 1 -framerate 30 -i "$wall" -loop 1 -framerate 30 -i "$mask_file" \
  -filter_complex "$graph" \
  -map "[out]" -loop 0 "$out"
