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
crop_width=1120
crop_height=1280
crop_x=$(((input_width - crop_width) / 2))
crop_y=$(((input_height - crop_height) / 2))
if [ "$crop_x" -lt 0 ] || [ "$crop_y" -lt 0 ]; then
  echo "source ${input_width}x${input_height} is smaller than setup crop ${crop_width}x${crop_height}" >&2
  exit 2
fi

graph=
labels=
count=0
old_ifs=$IFS
IFS=,
for cut in $cuts; do
  start=${cut%%:*}
  duration=${cut#*:}
  label="seg$count"
  graph="${graph}[0:v]trim=start=$start:duration=$duration,setpts=(PTS-STARTPTS)/$speed,crop=${crop_width}:${crop_height}:${crop_x}:${crop_y},scale=1472:976:force_original_aspect_ratio=decrease:flags=lanczos[$label];"
  labels="${labels}[$label]"
  count=$((count + 1))
done
IFS=$old_ifs
if [ "$count" -eq 0 ]; then
  echo "no trim segments" >&2
  exit 2
fi
graph="${graph}${labels}concat=n=$count:v=1:a=0,tpad=stop_mode=clone:stop_duration=$hold[fg];[1:v]scale=1600:1104:force_original_aspect_ratio=increase:flags=lanczos,crop=1600:1104[bg];[bg][fg]overlay=(W-w)/2:(H-h)/2:shortest=1:format=auto,fps=10,split[a][b];[a]palettegen=max_colors=128:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=3[out]"

# setup.mov is a full-window recording. Select useful action segments,
# crop the setup dialog, preserve its aspect and pixels, then hold the final
# ready frame over the same owner wallpaper in both theme variants.
ffmpeg -loglevel error -y -i "$src" -loop 1 -framerate 30 -i "$wall" \
  -filter_complex "$graph" \
  -map "[out]" -loop 0 "$out"
