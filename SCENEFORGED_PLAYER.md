# sceneforged-player — Zero-Copy Serving Implementation

Claude Code reference. The player never transcodes. It parses MP4/MKV containers at scan time, precomputes HLS segment maps, and serves media via `sendfile(2)` zero-copy I/O.

---

## Profiles

**Profile A** — MKV, up to 4K HDR/DV, H.265 or AV1, lossless/spatial audio. Direct stream only via HTTP range requests.

**Profile B** — MP4 faststart, 1080p/720p SDR, H.264 High, AAC-LC stereo 256kbps. Keyframes every 2 seconds. Served as HLS fMP4 segments via precomputed segment maps.

The player determines which profile a file is by its container: `.mkv` → Profile A, `.mp4` → Profile B.

---

## sf-media Crate Structure

```
crates/sf-media/
└── src/
    ├── lib.rs
    ├── mp4/
    │   ├── mod.rs
    │   ├── atoms.rs            # MP4 box header parsing
    │   ├── sample_table.rs     # stts/stsz/stss/stsc/stco/ctts → ResolvedSampleTable
    │   └── avc_config.rs       # avcC → SPS/PPS, esds → AudioSpecificConfig
    ├── mkv/
    │   ├── mod.rs
    │   └── cues.rs             # EBML parsing, Tracks + Cues elements
    ├── segment_map/
    │   ├── mod.rs
    │   ├── builder.rs          # SegmentMap construction from resolved samples
    │   └── types.rs            # SegmentMap, PrecomputedSegment, DataRange
    ├── fmp4/
    │   ├── mod.rs
    │   ├── init_segment.rs     # ftyp + moov (empty stbl, mvex) builder
    │   ├── moof_builder.rs     # Per-segment moof (mfhd + traf + trun) serialisation
    │   └── boxes.rs            # Box header helpers, size patching, mdat header
    ├── hls/
    │   ├── mod.rs
    │   ├── playlist.rs         # Master + variant m3u8 generation
    │   └── handlers.rs         # Axum handlers for playlists, init, segments
    ├── direct_stream.rs        # HTTP range request handler (Profile A + fallback)
    └── zero_copy.rs            # sendfile(2) wrapper, TCP_CORK batching
```

### Dependencies

```toml
[dependencies]
tokio = { version = "1", features = ["fs", "io-util", "net"] }
bytes = "1"
nix = { version = "0.29", features = ["fs", "socket"] }
dashmap = "6"
axum = "0.8"
tokio-util = { version = "0.7", features = ["io"] }
```

No FFmpeg. No codec libraries. No C dependencies.

---

## MP4 Moov Parser

### Box Header

```rust
pub struct BoxHeader {
    pub box_type: [u8; 4],
    pub size: u64,              // Total size including header
    pub header_size: u8,        // 8 or 16
}

pub fn read_box_header(reader: &mut impl Read) -> Result<BoxHeader> {
    let mut buf = [0u8; 8];
    reader.read_exact(&mut buf)?;
    let size = u32::from_be_bytes(buf[0..4].try_into().unwrap());
    let box_type: [u8; 4] = buf[4..8].try_into().unwrap();

    if size == 1 {
        let mut ext = [0u8; 8];
        reader.read_exact(&mut ext)?;
        Ok(BoxHeader { box_type, size: u64::from_be_bytes(ext), header_size: 16 })
    } else {
        Ok(BoxHeader { box_type, size: size as u64, header_size: 8 })
    }
}
```

### Boxes to Parse

Navigate `moov` → `trak` → `mdia` → `minf` → `stbl`. Parse these from stbl:

| Box | Contains | Struct |
|-----|----------|--------|
| `stts` | Sample durations as run-length `(count, delta)` | `Vec<(u32, u32)>` |
| `ctts` | Composition time offsets (B-frames) as run-length `(count, offset)` | `Vec<(u32, i32)>` |
| `stss` | Sync sample numbers (1-indexed keyframe list) | `Vec<u32>` |
| `stsz` | Per-sample byte sizes (or `default_size` if uniform) | `default_size: u32, sizes: Vec<u32>` |
| `stsc` | Sample-to-chunk mapping `(first_chunk, samples_per_chunk, desc_index)` | `Vec<(u32, u32, u32)>` |
| `stco` | 32-bit chunk byte offsets | `Vec<u64>` |
| `co64` | 64-bit chunk byte offsets | `Vec<u64>` |

From `stsd` → `avc1` → `avcC`: extract SPS and PPS NAL units (needed for init segment).
From `stsd` → `mp4a` → `esds`: extract AudioSpecificConfig bytes (needed for init segment).
From `mdhd`: extract track timescale.
From `tkhd`: extract track dimensions.

### Sample Table Resolution

Flatten stsc + stco + stsz into per-sample byte offsets:

```rust
pub struct ResolvedSample {
    pub index: u32,
    pub file_offset: u64,           // Byte position in MP4 file
    pub size: u32,                  // Byte count
    pub duration: u32,              // In timescale units
    pub composition_offset: i32,
    pub is_sync: bool,
    pub decode_timestamp: u64,      // DTS in timescale units
    pub composition_timestamp: u64, // CTS = DTS + composition_offset
}

pub struct ResolvedSampleTable {
    pub samples: Vec<ResolvedSample>,
    pub timescale: u32,
}
```

**Algorithm:**

1. Iterate chunks (0..stco.len()). For each chunk, look up samples_per_chunk from stsc (stsc entries apply from `first_chunk` until the next entry).
2. Within a chunk, samples are contiguous. First sample starts at the chunk offset. Each subsequent sample starts at previous offset + previous size.
3. Accumulate DTS from stts durations. Mark sync from stss membership (1-indexed). Apply ctts offsets.
4. If no stss box exists, all samples are sync (typical for audio tracks).

---

## Segment Map

### Types

```rust
pub struct SegmentMap {
    pub item_id: String,
    pub file_path: PathBuf,
    pub video_codec: VideoCodecInfo,
    pub audio_codec: AudioCodecInfo,
    pub width: u32,
    pub height: u32,
    pub duration_secs: f64,
    pub duration_ticks: i64,            // Jellyfin 100ns ticks
    pub init_segment: Vec<u8>,          // ftyp + moov, ~1-2KB
    pub master_playlist: String,
    pub variant_playlist: String,
    pub segments: Vec<PrecomputedSegment>,
}

pub struct PrecomputedSegment {
    pub index: u32,
    pub start_time_secs: f64,
    pub duration_secs: f64,
    pub moof_bytes: Vec<u8>,            // Pre-serialised, ~200-500 bytes
    pub data_ranges: Vec<DataRange>,    // Byte ranges in source MP4
    pub data_length: u64,               // Sum of all range lengths
}

pub struct DataRange {
    pub file_offset: u64,
    pub length: u64,
}

pub struct VideoCodecInfo {
    pub profile: u8,                    // 100 = High
    pub level: u8,                      // 41 = 4.1
    pub sps: Vec<u8>,                   // Raw NAL bytes
    pub pps: Vec<u8>,
}

pub struct AudioCodecInfo {
    pub sample_rate: u32,
    pub channels: u32,
    pub audio_specific_config: Vec<u8>, // 2-5 bytes
}
```

### Building Segments

Target segment duration: **6 seconds**.

1. Collect keyframe indices from the video ResolvedSampleTable (where `is_sync == true`).
2. Walk keyframes. Start a new segment at each keyframe where `dts_seconds >= target_boundary - 1.0`. Update target to `keyframe_time + 6.0`.
3. For each segment (range of video samples between two boundaries):
   - Find corresponding audio samples by converting the video time range to the audio timescale.
   - Collect all sample `(file_offset, size)` pairs for video + audio.
   - Sort by file_offset, merge adjacent/overlapping into contiguous `DataRange`s. For well-interleaved faststart MP4, this produces 1 range.
   - Pre-build the moof box (see below).
4. Build the init segment and HLS playlists.

---

## fMP4 Serialisation

### Init Segment Structure

```
ftyp [isom, iso5, iso6, avc1, mp41]
moov
  mvhd (timescale=1000, duration=0)
  trak (video, track_id=1)
    tkhd (width, height)
    mdia
      mdhd (video timescale, duration=0)
      hdlr (vide)
      minf → stbl
        stsd → avc1 → avcC (SPS, PPS from source)
        stts (0 entries)
        stsc (0 entries)
        stsz (0 entries, sample_count=0)
        stco (0 entries)
  trak (audio, track_id=2)
    tkhd
    mdia
      mdhd (audio timescale, duration=0)
      hdlr (soun)
      minf → stbl
        stsd → mp4a → esds (AudioSpecificConfig from source)
        stts/stsc/stsz/stco (all empty)
  mvex
    trex (track_id=1, defaults)
    trex (track_id=2, defaults)
```

### Moof Structure (Per Segment)

```
moof
  mfhd
    sequence_number (1-indexed)
  traf (video)
    tfhd (track_id=1, default_sample_flags=0x01010000 for non-sync default)
    tfdt (base_media_decode_time = segment start DTS in video timescale)
    trun
      flags: 0x000301 (data_offset + sample_duration + sample_size) or
             0x000B01 (+ sample_flags + composition_time_offset if B-frames)
      sample_count
      data_offset = moof_total_size + 8 (mdat header)
      per-sample: duration, size [, flags, composition_offset]
      first sample flags: 0x02000000 (sync) if keyframe
  traf (audio)
    tfhd (track_id=2, default_sample_flags=0x02000000)
    tfdt (base_media_decode_time = segment start DTS in audio timescale)
    trun
      flags: 0x000301 (data_offset + sample_duration + sample_size)
      sample_count
      data_offset = (offset past video data in mdat)
      per-sample: duration, size
```

The `data_offset` in each trun is relative to the start of the enclosing moof's first byte. The first trun's data_offset = `moof_size + 8` (8 for mdat header). The second trun's data_offset accounts for the video data length preceding audio data in the mdat.

### Box Helpers

```rust
/// Write placeholder box header at current position. Returns offset for patching.
pub fn write_box_header_placeholder(buf: &mut Vec<u8>, box_type: &[u8; 4]) -> usize {
    let offset = buf.len();
    buf.extend_from_slice(&0u32.to_be_bytes());
    buf.extend_from_slice(box_type);
    offset
}

/// Patch box size at offset with actual size.
pub fn patch_box_size(buf: &mut Vec<u8>, offset: usize) {
    let size = (buf.len() - offset) as u32;
    buf[offset..offset + 4].copy_from_slice(&size.to_be_bytes());
}

/// Full box header (version byte + 3 flag bytes after box type).
pub fn write_full_box_header(buf: &mut Vec<u8>, box_type: &[u8; 4], version: u8, flags: u32) -> usize {
    let offset = write_box_header_placeholder(buf, box_type);
    buf.push(version);
    buf.extend_from_slice(&flags.to_be_bytes()[1..4]);
    offset
}

/// 8-byte mdat box header for given payload size.
pub fn mdat_header(payload_size: u64) -> [u8; 8] {
    let mut h = [0u8; 8];
    h[0..4].copy_from_slice(&((payload_size + 8) as u32).to_be_bytes());
    h[4..8].copy_from_slice(b"mdat");
    h
}
```

After building moof bytes, patch the trun `data_offset` fields: scan for trun boxes in the buffer and write `moof_total_size + 8` into the data_offset position.

---

## HLS Playlists

### Master Playlist

```
#EXTM3U
#EXT-X-VERSION:7
#EXT-X-STREAM-INF:BANDWIDTH={bitrate},RESOLUTION={w}x{h},CODECS="avc1.640029,mp4a.40.2"
variant.m3u8
```

Codec string `avc1.640029` = H.264 High 4.1. `mp4a.40.2` = AAC-LC. Bitrate estimated from file size / duration.

### Variant Playlist

```
#EXTM3U
#EXT-X-VERSION:7
#EXT-X-TARGETDURATION:{max_segment_ceil}
#EXT-X-MEDIA-SEQUENCE:0
#EXT-X-PLAYLIST-TYPE:VOD
#EXT-X-MAP:URI="init.mp4"

#EXTINF:{seg0_duration},
segment_0.m4s
#EXTINF:{seg1_duration},
segment_1.m4s
...
#EXT-X-ENDLIST
```

---

## Serving

### Routes

```rust
// Profile B — HLS
.route("/Videos/:item_id/hls/master.m3u8",    get(hls::serve_master_playlist))
.route("/Videos/:item_id/hls/variant.m3u8",    get(hls::serve_variant_playlist))
.route("/Videos/:item_id/hls/init.mp4",         get(hls::serve_init_segment))
.route("/Videos/:item_id/hls/segment_:index.m4s", get(hls::serve_segment))

// Both profiles — direct stream
.route("/Videos/:item_id/stream",               get(direct_stream::serve_file))
.route("/Items/:item_id/Download",              get(direct_stream::serve_file))
```

### Segment Handler (Hot Path)

For each segment request:

1. Look up `SegmentMap` from `AppState.segment_maps` (DashMap<String, SegmentMap>).
2. Look up `PrecomputedSegment` by index.
3. Response = `moof_bytes` (from RAM) + mdat header (8 bytes) + file data (sendfile).

```rust
async fn serve_segment(item_id: &str, index: u32, state: &AppState) -> Response {
    let map = state.segment_maps.get(item_id).unwrap();
    let seg = &map.segments[index as usize];

    let mdat_hdr = mdat_header(seg.data_length);
    let total = seg.moof_bytes.len() + 8 + seg.data_length as usize;

    // Stream: moof from RAM, mdat header from RAM, payload via sendfile/read
    let (mut tx, body) = Body::channel();
    let moof = seg.moof_bytes.clone();
    let ranges = seg.data_ranges.clone();
    let path = map.file_path.clone();

    tokio::spawn(async move {
        tx.send_data(Bytes::from(moof)).await.ok();
        tx.send_data(Bytes::from(mdat_hdr.to_vec())).await.ok();

        let mut file = tokio::fs::File::open(&path).await.unwrap();
        for r in &ranges {
            file.seek(SeekFrom::Start(r.file_offset)).await.unwrap();
            let mut buf = vec![0u8; r.length as usize];
            file.read_exact(&mut buf).await.unwrap();
            tx.send_data(Bytes::from(buf)).await.ok();
        }
    });

    Response::builder()
        .header("Content-Type", "video/mp4")
        .header("Content-Length", total)
        .header("Cache-Control", "public, max-age=31536000")
        .body(body).unwrap()
}
```

### Zero-Copy Upgrade (Phase 7)

Replace the tokio file read with `sendfile(2)`:

```rust
use nix::sys::sendfile::sendfile as nix_sendfile;
use nix::sys::socket::setsockopt;
use nix::sys::socket::sockopt::TcpCork;

fn serve_zero_copy(socket_fd: RawFd, file_fd: RawFd, seg: &PrecomputedSegment) -> io::Result<()> {
    setsockopt(socket_fd, TcpCork, &true)?;

    write_all_fd(socket_fd, &seg.moof_bytes)?;
    write_all_fd(socket_fd, &mdat_header(seg.data_length))?;

    for r in &seg.data_ranges {
        let mut off = r.file_offset as i64;
        let mut rem = r.length as usize;
        while rem > 0 {
            match nix_sendfile(socket_fd, file_fd, Some(&mut off), rem) {
                Ok(0) => break,
                Ok(n) => rem -= n,
                Err(nix::errno::Errno::EINTR) => continue,
                Err(e) => return Err(e.into()),
            }
        }
    }

    setsockopt(socket_fd, TcpCork, &false)?;
    Ok(())
}
```

Userspace bytes per segment: ~308. Payload goes disk → kernel page cache → NIC DMA.

### Direct Stream Handler (Profile A)

Standard HTTP range request handler. Parse `Range: bytes=X-Y` header, respond with `206 Partial Content` and the byte range from the file. Use `tokio::fs::File` seek + take, or sendfile for zero-copy.

Support `bytes=X-Y`, `bytes=X-`, and `bytes=-Y` (suffix) forms. Return `Accept-Ranges: bytes` on all responses. Return `416 Range Not Satisfiable` for invalid ranges.

---

## Playlist Handler (PlaybackInfo)

When Jellyfin clients request `/Items/{id}/PlaybackInfo`:

- Profile A: `supports_direct_play: true`, `supports_transcoding: false`, `direct_stream_url: /Videos/{id}/stream`
- Profile B: `supports_direct_play: true`, `supports_transcoding: false`, `transcoding_url: /Videos/{id}/hls/master.m3u8`, `transcoding_sub_protocol: "hls"`, `transcoding_container: "mp4"`

`supports_transcoding` is always `false`. The `transcoding_url` field for Profile B points to the precomputed HLS stream (clients treat it the same as a transcode URL even though no transcoding occurs).

---

## Scanner Integration

When the scanner finds a media file:

1. Detect profile by extension: `.mp4` → B, `.mkv` → A.
2. **Profile B**: Open file, parse moov atom, resolve sample tables, build SegmentMap, store in `AppState.segment_maps`. Extract codec info for `media_streams` table.
3. **Profile A**: Parse MKV EBML header, Info, Tracks, Cues. Extract codec ID, dimensions, duration for `media_streams` table. No segment map needed.
4. Store `media_profile` column (`'A'` or `'B'`) in `items` table.

### Database Addition

```sql
ALTER TABLE items ADD COLUMN media_profile TEXT CHECK (media_profile IN ('A', 'B'));
```

---

## Multi-File Support

An item may have both a Profile A and Profile B file. The schema changes:

**Replace** single `file_path`/`file_size`/`container` on `items` with a `media_files` table:

```sql
CREATE TABLE media_files (
    id              TEXT PRIMARY KEY,   -- UUID
    item_id         TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    media_profile   TEXT NOT NULL CHECK (media_profile IN ('A', 'B')),
    file_path       TEXT NOT NULL,
    file_size       INTEGER NOT NULL,
    container       TEXT NOT NULL,      -- 'mp4' or 'mkv'
    width           INTEGER,
    height          INTEGER,
    duration_ticks  INTEGER,
    UNIQUE(item_id, media_profile)      -- One file per profile per item
);

CREATE INDEX idx_media_files_item ON media_files(item_id);
```

**Remove** from `items`: `file_path`, `file_size`, `container`, `media_profile`. Keep `duration_ticks` on `items` (canonical duration from the source file).

**Update** `media_streams` to reference `media_files` instead of `items`:

```sql
CREATE TABLE media_streams (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    media_file_id   TEXT NOT NULL REFERENCES media_files(id) ON DELETE CASCADE,
    stream_index    INTEGER NOT NULL,
    stream_type     TEXT NOT NULL,
    codec           TEXT NOT NULL,
    width           INTEGER,
    height          INTEGER,
    bit_rate        INTEGER,
    channels        INTEGER,
    sample_rate     INTEGER,
    language        TEXT,
    title           TEXT,
    is_default      BOOLEAN DEFAULT 0,
    is_forced       BOOLEAN DEFAULT 0
);
```

### Profile Selection Logic

When a client requests playback:

1. Get all `media_files` for the item.
2. If client supports direct play of Profile A (detected via User-Agent or client capabilities), serve Profile A.
3. Otherwise, serve Profile B via HLS.
4. If only one profile exists, serve that.

`AppState.segment_maps` key changes from `item_id` to `media_file_id`.

---

## Thumbnail Generation (Optional)

The only place FFmpeg may appear. At scan time, optionally:

```bash
ffmpeg -ss {time} -i {file} -vframes 1 -f image2 pipe:1
```

One-shot subprocess, not a runtime dependency. Can be skipped entirely if TMDB artwork is sufficient.

---

## Implementation Phases

### Phase 3: Scanner + Container Parsing

1. MP4 moov parser: box traversal, sample table structs, avcC/esds extraction
2. Sample table resolution: stsc+stco+stsz flattening algorithm
3. MKV EBML parser: Info, Tracks, Cues
4. `media_files` table + scanner integration
5. Test: scanner populates correct codec info from container parsing alone

### Phase 4: Segment Map + HLS

1. Segment boundary computation (keyframe-aligned, ~6s target)
2. Data range merging
3. fMP4 init segment builder
4. fMP4 moof builder (mfhd + traf + trun serialisation)
5. HLS playlist generation
6. Segment serving handler (channel body: moof + mdat header + file read)
7. Init segment + playlist handlers
8. Test: hls.js or Safari plays a Profile B file

### Phase 4b: Direct Stream

1. HTTP range request parser
2. File serving with 206 Partial Content
3. Profile selection in PlaybackInfo
4. Test: Infuse direct-plays a Profile A MKV with seeking

### Phase 7: Zero-Copy

1. sendfile(2) wrapper via nix
2. TCP_CORK batching
3. Replace file read path with sendfile in segment handler
4. Benchmark userspace bytes per segment
5. Test: <500 bytes userspace per segment served