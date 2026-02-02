//! Pure Rust Dolby Vision Profile 7 to 8.1 converter
//!
//! Usage: dv-convert <input.hevc> <output.hevc>

use std::env;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};

use dolby_vision::rpu::dovi_rpu::DoviRpu;
use dolby_vision::rpu::ConversionMode;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 3 {
        eprintln!("Usage: {} <input.hevc> <output.hevc>", args[0]);
        eprintln!();
        eprintln!("Converts Dolby Vision Profile 7 to Profile 8.1 (Apple TV compatible)");
        std::process::exit(1);
    }

    let input_path = &args[1];
    let output_path = &args[2];

    eprintln!("Reading: {}", input_path);

    let input_file = File::open(input_path)?;
    let mut reader = BufReader::new(input_file);
    let mut data = Vec::new();
    reader.read_to_end(&mut data)?;

    eprintln!("Processing {} bytes...", data.len());

    let output_data = convert_hevc_dv(&data)?;

    eprintln!("Writing: {}", output_path);
    let output_file = File::create(output_path)?;
    let mut writer = BufWriter::new(output_file);
    writer.write_all(&output_data)?;

    eprintln!("Done!");
    Ok(())
}

/// Convert HEVC stream with DV Profile 7 to Profile 8.1
fn convert_hevc_dv(data: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut output = Vec::with_capacity(data.len());
    let mut pos = 0;
    let mut rpu_count = 0;
    let mut converted_count = 0;

    while pos < data.len() {
        // Find next start code
        let start = find_start_code(data, pos);
        if start.is_none() {
            // Copy remaining data
            output.extend_from_slice(&data[pos..]);
            break;
        }
        let (nal_start, start_code_len) = start.unwrap();

        // Copy any data before the start code
        if nal_start > pos {
            output.extend_from_slice(&data[pos..nal_start]);
        }

        // Find the end of this NAL unit (next start code or end of data)
        let nal_data_start = nal_start + start_code_len;
        let nal_end = find_start_code(data, nal_data_start)
            .map(|(next_start, _)| next_start)
            .unwrap_or(data.len());

        let nal_data = &data[nal_data_start..nal_end];

        if nal_data.len() >= 2 {
            let nal_type = (nal_data[0] >> 1) & 0x3F;

            // NAL type 62 = UNSPEC62 = Dolby Vision RPU
            if nal_type == 62 {
                rpu_count += 1;

                // Try to convert the RPU
                match convert_rpu(nal_data) {
                    Ok(converted) => {
                        converted_count += 1;
                        // Write start code + converted NAL
                        output.extend_from_slice(&data[nal_start..nal_data_start]);
                        output.extend_from_slice(&converted);
                    }
                    Err(e) => {
                        eprintln!("Warning: Failed to convert RPU #{}: {}", rpu_count, e);
                        // Keep original NAL
                        output.extend_from_slice(&data[nal_start..nal_end]);
                    }
                }
            } else {
                // Copy non-RPU NAL unchanged
                output.extend_from_slice(&data[nal_start..nal_end]);
            }
        } else {
            // Copy tiny NAL unchanged
            output.extend_from_slice(&data[nal_start..nal_end]);
        }

        pos = nal_end;
    }

    eprintln!(
        "Processed {} RPU NAL units, converted {}",
        rpu_count, converted_count
    );
    Ok(output)
}

/// Find the next Annex B start code (0x000001 or 0x00000001)
fn find_start_code(data: &[u8], start: usize) -> Option<(usize, usize)> {
    let mut i = start;
    while i + 2 < data.len() {
        if data[i] == 0 && data[i + 1] == 0 {
            if data[i + 2] == 1 {
                return Some((i, 3));
            } else if i + 3 < data.len() && data[i + 2] == 0 && data[i + 3] == 1 {
                return Some((i, 4));
            }
        }
        i += 1;
    }
    None
}

/// Convert a single RPU NAL unit to Profile 8.1
fn convert_rpu(nal_data: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // Parse the UNSPEC62 NAL unit directly
    let mut rpu = DoviRpu::parse_unspec62_nalu(nal_data)?;

    // Convert to Profile 8.1
    rpu.convert_with_mode(ConversionMode::To81)?;

    // Write back as HEVC NAL unit
    let converted = rpu.write_hevc_unspec62_nalu()?;

    Ok(converted)
}
