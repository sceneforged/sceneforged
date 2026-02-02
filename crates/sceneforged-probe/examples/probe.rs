//! Example: Probe a video file and print metadata

use sceneforged_probe::{probe_file, HdrFormat};
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} <video_file>", args[0]);
        eprintln!();
        eprintln!("Example:");
        eprintln!("  {} movie.mkv", args[0]);
        std::process::exit(1);
    }

    let path = &args[1];

    match probe_file(path) {
        Ok(info) => {
            println!("File: {}", info.file_path);
            println!(
                "Size: {} bytes ({:.2} MB)",
                info.file_size,
                info.file_size as f64 / 1_000_000.0
            );
            println!("Container: {}", info.container);

            if let Some(duration) = info.duration_ms {
                let seconds = duration / 1000;
                let minutes = seconds / 60;
                let hours = minutes / 60;
                println!(
                    "Duration: {:02}:{:02}:{:02}",
                    hours,
                    minutes % 60,
                    seconds % 60
                );
            }

            println!();
            println!("Video Tracks ({}):", info.video_tracks.len());
            for (i, video) in info.video_tracks.iter().enumerate() {
                println!("  [{}] {} {}x{}", i, video.codec, video.width, video.height);

                if let Some(fps) = video.frame_rate {
                    println!("      Frame rate: {:.3} fps", fps);
                }

                if let Some(depth) = video.bit_depth {
                    println!("      Bit depth: {}-bit", depth);
                }

                if let Some(ref primaries) = video.color_primaries {
                    println!("      Color primaries: {:?}", primaries);
                }

                if let Some(ref transfer) = video.transfer_characteristics {
                    println!("      Transfer: {:?}", transfer);
                }

                if let Some(ref hdr) = video.hdr_format {
                    print!("      HDR Format: ");
                    match hdr {
                        HdrFormat::Sdr => println!("SDR"),
                        HdrFormat::Hdr10 {
                            mastering_display,
                            content_light_level,
                        } => {
                            println!("HDR10");
                            if let Some(ref md) = mastering_display {
                                println!("        Mastering Display:");
                                println!(
                                    "          Max luminance: {} cd/m²",
                                    md.max_luminance as f64 / 10000.0
                                );
                                println!(
                                    "          Min luminance: {} cd/m²",
                                    md.min_luminance as f64 / 10000.0
                                );
                            }
                            if let Some(ref cll) = content_light_level {
                                println!("        Content Light Level:");
                                println!("          MaxCLL: {} cd/m²", cll.max_cll);
                                println!("          MaxFALL: {} cd/m²", cll.max_fall);
                            }
                        }
                        HdrFormat::Hdr10Plus { .. } => println!("HDR10+"),
                        HdrFormat::Hlg => println!("HLG"),
                        HdrFormat::DolbyVision {
                            profile,
                            level,
                            bl_compatibility_id,
                            rpu_present,
                            el_present,
                            ..
                        } => {
                            println!("Dolby Vision");
                            println!("        Profile: {}", profile);
                            if let Some(lvl) = level {
                                println!("        Level: {}", lvl);
                            }
                            if let Some(compat) = bl_compatibility_id {
                                println!("        BL Compatibility: {}", compat);
                            }
                            println!("        RPU Present: {}", rpu_present);
                            println!("        EL Present: {}", el_present);
                        }
                    }
                }
            }

            println!();
            println!("Audio Tracks ({}):", info.audio_tracks.len());
            for (i, audio) in info.audio_tracks.iter().enumerate() {
                print!(
                    "  [{}] {} {}ch {}Hz",
                    i, audio.codec, audio.channels, audio.sample_rate
                );

                if let Some(depth) = audio.bit_depth {
                    print!(" {}-bit", depth);
                }

                if let Some(ref lang) = audio.language {
                    print!(" [{}]", lang);
                }

                if let Some(ref title) = audio.title {
                    print!(" \"{}\"", title);
                }

                if audio.default {
                    print!(" (default)");
                }

                println!();
            }

            if !info.subtitle_tracks.is_empty() {
                println!();
                println!("Subtitle Tracks ({}):", info.subtitle_tracks.len());
                for (i, sub) in info.subtitle_tracks.iter().enumerate() {
                    print!("  [{}] {}", i, sub.codec);

                    if let Some(ref lang) = sub.language {
                        print!(" [{}]", lang);
                    }

                    if let Some(ref title) = sub.title {
                        print!(" \"{}\"", title);
                    }

                    if sub.default {
                        print!(" (default)");
                    }

                    if sub.forced {
                        print!(" (forced)");
                    }

                    println!();
                }
            }
        }
        Err(e) => {
            eprintln!("Error probing file: {}", e);
            std::process::exit(1);
        }
    }
}
