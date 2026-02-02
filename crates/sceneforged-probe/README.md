# video-probe

Pure Rust video file probing with HDR/Dolby Vision detection.

## Features

- **Container parsing**: MKV/WebM (via `matroska`), MP4/MOV (via `mp4parse`)
- **Video codec analysis**: HEVC/H.265 with NAL unit parsing
- **HDR detection**: HDR10, HDR10+, HLG, Dolby Vision
- **No external tool dependencies**: Pure Rust implementation

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
video-probe = "0.1"
```

## Usage

```rust
use video_probe::{probe_file, HdrFormat};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let info = probe_file("movie.mkv")?;

    println!("Container: {}", info.container);
    println!("Duration: {:?}ms", info.duration_ms);

    for video in &info.video_tracks {
        println!("Video: {} {}x{}", video.codec, video.width, video.height);

        if let Some(ref hdr) = video.hdr_format {
            match hdr {
                HdrFormat::DolbyVision { profile, .. } => {
                    println!("  Dolby Vision Profile {}", profile);
                }
                HdrFormat::Hdr10 { .. } => println!("  HDR10"),
                HdrFormat::Hdr10Plus { .. } => println!("  HDR10+"),
                HdrFormat::Hlg => println!("  HLG"),
                HdrFormat::Sdr => println!("  SDR"),
            }
        }
    }

    for audio in &info.audio_tracks {
        println!("Audio: {} {}ch {}Hz",
            audio.codec, audio.channels, audio.sample_rate);
    }

    Ok(())
}
```

## HDR Detection

The crate detects HDR formats through multiple methods:

1. **Container metadata**: MKV Colour element, MP4 colr box
2. **HEVC bitstream parsing**: SPS/VUI for transfer characteristics
3. **SEI messages**: HDR10 static metadata (SMPTE ST.2086), HDR10+ dynamic metadata
4. **Dolby Vision**: RPU NAL unit detection via `dolby_vision` crate

### Supported HDR Formats

| Format | Detection Method |
|--------|------------------|
| HDR10 | Transfer characteristics (PQ), mastering display SEI |
| HDR10+ | User data registered SEI with Samsung signature |
| HLG | Transfer characteristics (ARIB STD-B67) |
| Dolby Vision | RPU NAL units, dvcC/dvvC configuration boxes |

## Supported Containers

| Container | Extension | Notes |
|-----------|-----------|-------|
| Matroska | .mkv, .webm | Full metadata support |
| MP4 | .mp4, .m4v, .mov | Basic support (limited HEVC) |

## Supported Codecs

### Video
- HEVC/H.265 (with full NAL parsing)
- AVC/H.264
- AV1
- VP8/VP9
- MPEG-1/2/4

### Audio
- AAC (LC, HE, HEv2)
- AC-3, E-AC-3
- DTS, DTS-HD MA
- TrueHD
- FLAC, Opus, Vorbis
- PCM

### Subtitles
- SRT, ASS/SSA
- PGS (HDMV)
- VobSub
- WebVTT

## Example CLI

```bash
cargo run --example probe -- /path/to/video.mkv
```

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.
