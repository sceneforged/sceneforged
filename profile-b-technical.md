# Profile B & Zero-Copy HLS

Technical reference for sceneforged's HLS serving architecture.

---

## Profile B Contract

A Profile B file is an MP4 that the player can serve over HLS without transcoding or buffering.

| Property | Requirement |
|----------|-------------|
| Container | MP4, moov before mdat |
| Video | H.264 High ≤ 4.1, yuv420p |
| Resolution | ≤ 1920×1080 |
| Keyframes | Every 2s, fixed interval (GOP = 2 × fps), no scene-cut insertion |
| Audio | AAC-LC stereo, 48 kHz |
| Tracks | 1 video + 1 audio |
| Interleaving | Samples ordered by DTS across tracks |

**Faststart**: moov must precede mdat. Without this, the player can't read the sample tables without downloading the entire file.

**Fixed keyframes**: HLS segments must start on keyframes. Regular intervals mean segment boundaries are predictable without re-encoding.

**Interleaving**: When video and audio samples are interleaved by time, each segment's data is contiguous in the file (1-2 byte ranges instead of many scattered ranges).

---

## Scan-Time Processing

When a Profile B file is added to the library, the player parses it once and builds a segment map. No processing happens at serve time.

### 1. Parse moov

Walk the MP4 box tree: `moov → trak → mdia → minf → stbl`

Extract from stbl:

| Box | Data |
|-----|------|
| `stts` | Sample durations (decode delta per sample) |
| `ctts` | Composition offsets (PTS - DTS for B-frames) |
| `stss` | Sync sample indices (keyframes) |
| `stsz` | Sample sizes in bytes |
| `stsc` | Sample-to-chunk mapping |
| `stco`/`co64` | Chunk byte offsets |
| `avcC` | H.264 SPS/PPS (in video stsd) |
| `esds` | AAC AudioSpecificConfig (in audio stsd) |

### 2. Resolve sample table

Flatten stsc + stco + stsz into per-sample records:

```rust
struct ResolvedSample {
    file_offset: u64,      // Byte position in file
    size: u32,             // Compressed size
    dts: i64,              // Decode timestamp (timescale units)
    pts: i64,              // Presentation timestamp
    is_sync: bool,         // Keyframe?
    track_id: u32,         // Video or audio
}
```

### 3. Build segment map

Target ~6 second segments, aligned to video keyframes:

```
for each video keyframe:
    if keyframe.pts >= segment_start + 5.0 seconds:
        close current segment
        start new segment at this keyframe

for each segment:
    collect video samples in [start_pts, end_pts)
    collect audio samples in [start_pts, end_pts)
    compute byte ranges from sample offsets/sizes
    serialize moof box
```

### 4. Serialize fMP4 structures

**Init segment** (~1-2 KB): `ftyp` + `moov` with:
- `mvhd` (movie header)
- `trak` per track with `tkhd`, `mdia`, `minf`, `stbl` (empty sample tables)
- `mvex` with `trex` per track

**Per-segment moof** (~200-500 bytes):
```
moof
├── mfhd (sequence_number)
├── traf (video)
│   ├── tfhd (track_id, flags)
│   ├── tfdt (base_media_decode_time)
│   └── trun (sample_count, data_offset, [duration, size, flags, cts_offset]...)
└── traf (audio)
    ├── tfhd
    ├── tfdt
    └── trun
```

The `data_offset` in trun points past the moof+mdat header to where media bytes begin.

### 5. Store segment map

```rust
struct SegmentMap {
    file_path: PathBuf,
    init_segment: Vec<u8>,           // ftyp + moov
    master_playlist: String,
    variant_playlist: String,
    segments: Vec<Segment>,
}

struct Segment {
    index: u32,
    start_time: f64,
    duration: f64,
    moof: Vec<u8>,                   // Pre-serialized
    ranges: Vec<(u64, u64)>,         // (offset, length) in source file
    mdat_size: u64,
}
```

RAM per item: ~10-20 KB depending on duration.

---

## Serve-Time Processing

### HLS endpoint structure

```
/Videos/{id}/hls/master.m3u8     → master_playlist string
/Videos/{id}/hls/variant.m3u8   → variant_playlist string  
/Videos/{id}/hls/init.mp4       → init_segment bytes
/Videos/{id}/hls/segment_{n}.m4s → moof + mdat header + sendfile
```

### Segment response

For `segment_5.m4s`:

```rust
let seg = &segment_map.segments[5];

// 1. Write moof (from RAM)
response.write(&seg.moof)?;                    // ~300 bytes

// 2. Write mdat header (8 bytes)
let mdat_header = [
    ((seg.mdat_size + 8) >> 24) as u8,
    ((seg.mdat_size + 8) >> 16) as u8,
    ((seg.mdat_size + 8) >> 8) as u8,
    (seg.mdat_size + 8) as u8,
    b'm', b'd', b'a', b't',
];
response.write(&mdat_header)?;                 // 8 bytes

// 3. sendfile media data (zero-copy)
for (offset, length) in &seg.ranges {
    sendfile(socket_fd, file_fd, *offset, *length)?;
}                                              // 2-10 MB
```

### sendfile mechanics

```
User space:  sendfile(socket, file, offset, len)
                         │
                         ▼
Kernel:      Page cache ────────────► NIC DMA buffer
             (file data)  (no copy)   (to network)
```

Media bytes never enter userspace. The kernel transfers directly from page cache to network interface via DMA.

### Per-request overhead

| Component | Size | Source |
|-----------|------|--------|
| moof box | ~300 B | RAM |
| mdat header | 8 B | Computed |
| Media data | 2-10 MB | sendfile (zero-copy) |

Userspace handles ~308 bytes. Kernel handles megabytes.

---

## Data flow example

Client requests segment 5 of a 2-hour movie:

```
1. Lookup: segment_map.segments[5]
   - start_time: 30.0s
   - duration: 6.0s
   - moof: [pre-serialized 312 bytes]
   - ranges: [(45_234_176, 8_523_412)]
   - mdat_size: 8_523_412

2. Response assembly:
   [moof: 312 bytes from RAM]
   [mdat header: 8 bytes computed]
   [sendfile: offset=45234176, len=8523412]

3. Kernel handles:
   - Check if bytes 45234176..53757588 are in page cache
   - If not, read from disk into page cache
   - DMA from page cache to NIC

4. Total userspace work:
   - HashMap lookup
   - Write 320 bytes
   - One syscall
```

Latency: < 10ms to first byte.

---

## Segment boundary math

Given video track timescale `T` (usually 24000 for 24fps) and target duration 6s:

```
target_pts_delta = 6 * T = 144000

segment_boundaries = []
current_start = 0

for keyframe in sync_samples:
    kf_pts = compute_pts(keyframe)
    if kf_pts - current_start >= target_pts_delta - T:  # allow 1 frame early
        segment_boundaries.push(current_start, kf_pts)
        current_start = kf_pts

# Final segment to end of file
segment_boundaries.push(current_start, total_duration)
```

Audio samples are collected for the same PTS range. Since audio frames are small (~20ms), boundaries don't need to align exactly.

---

## fMP4 box layout

Init segment:
```
ftyp [isom, iso5, iso6, avc1, mp41]
moov
├── mvhd
├── trak (video)
│   ├── tkhd
│   └── mdia
│       ├── mdhd
│       ├── hdlr
│       └── minf
│           ├── vmhd
│           └── stbl
│               ├── stsd [avc1 + avcC]
│               ├── stts (empty)
│               ├── stsc (empty)
│               ├── stsz (empty)
│               └── stco (empty)
├── trak (audio) [similar structure]
└── mvex
    ├── trex (video)
    └── trex (audio)
```

Media segment:
```
moof
├── mfhd {sequence_number}
├── traf
│   ├── tfhd {track_id, default_sample_flags}
│   ├── tfdt {base_media_decode_time}
│   └── trun {sample_count, data_offset, per-sample entries}
└── traf [audio]
mdat
└── [compressed video NALUs + AAC frames, interleaved]
```

The `data_offset` in trun is relative to moof start. Set it to `moof_size + 8` (mdat header size).

---

## Key invariants

1. **moov before mdat** — required to read sample tables before streaming
2. **Fixed GOP** — segments align to keyframes without re-encoding  
3. **Interleaved samples** — segment data is contiguous (minimal byte ranges)
4. **Pre-serialized moof** — no computation at serve time
5. **sendfile for mdat** — zero-copy delivery of media bytes

Violation of any constraint forces fallback to direct stream (HTTP range requests, client handles seeking) or live transcode (not supported).
