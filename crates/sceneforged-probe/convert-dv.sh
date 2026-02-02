#!/bin/bash
# Convert Dolby Vision Profile 7 to Profile 8.1 using pure Rust
# For Apple TV / Infuse / Sony Bravia compatibility
set -e

INPUT="$1"
OUTPUT="${2:-${INPUT%.*}_p8.mkv}"

if [ -z "$INPUT" ] || [ ! -f "$INPUT" ]; then
    echo "Usage: $0 <input.mkv> [output.mkv]"
    echo
    echo "Converts Dolby Vision Profile 7 -> Profile 8.1"
    echo "Compatible with: Apple TV, Infuse, Sony Bravia"
    exit 1
fi

DIR=$(cd "$(dirname "$INPUT")" && pwd)
BASE=$(basename "$INPUT")
OUT_BASE=$(basename "$OUTPUT")

echo "Converting: $BASE"
echo "Output:     $OUT_BASE"
echo

docker run --rm -v "$DIR:/data" video-probe -c "
set -e
cd /data

echo '==> Extracting and converting HEVC...'
ffmpeg -y -i '$BASE' -c:v copy -bsf:v hevc_mp4toannexb -an -sn -f hevc - 2>/dev/null | \
    dv-convert /dev/stdin video_p8.hevc

echo '==> Remuxing with original audio/subs...'
mkvmerge -q -o '$OUT_BASE' video_p8.hevc --no-video '$BASE'

rm -f video_p8.hevc
echo '==> Done!'
"

echo
echo "Output: $OUTPUT"
ls -lh "$OUTPUT" 2>/dev/null || ls -lh "$DIR/$OUT_BASE"
