#!/bin/bash
# Convert Dolby Vision Profile 7 to Profile 8.1 for Apple TV compatibility
set -e

INPUT="$1"
OUTPUT="${2:-${INPUT%.*}_p8.mkv}"

if [ -z "$INPUT" ]; then
    echo "Usage: $0 <input.mkv> [output.mkv]"
    exit 1
fi

DIR=$(dirname "$INPUT")
BASE=$(basename "$INPUT")
OUT_BASE=$(basename "$OUTPUT")

echo "Converting: $BASE"
echo "Output: $OUT_BASE"

docker run --rm -v "$DIR:/data" dv-tools sh -c "
    set -e
    cd /data

    echo '==> Extracting and converting HEVC (P7 -> P8.1)...'
    ffmpeg -y -i '$BASE' -c:v copy -bsf:v hevc_mp4toannexb -an -sn -f hevc - 2>/dev/null | \
        dovi_tool --mode 2 convert -i - -o video_p8.hevc

    echo '==> Remuxing with original audio/subs...'
    mkvmerge -o '$OUT_BASE' video_p8.hevc --no-video '$BASE'

    rm -f video_p8.hevc
    echo '==> Done!'
"

echo "Output: $OUTPUT"
