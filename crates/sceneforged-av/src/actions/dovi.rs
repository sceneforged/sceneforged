//! Dolby Vision profile conversion.
//!
//! This module provides functionality for converting Dolby Vision metadata
//! between profiles, particularly Profile 7 to Profile 8.1 conversion which
//! enables playback on devices that don't support dual-layer Dolby Vision.
//!
//! Uses the native `dolby_vision` crate for RPU parsing and conversion,
//! combined with ffmpeg for stream extraction and mkvmerge for remuxing.

use crate::{Error, Result, Workspace};
use dolby_vision::rpu::ConversionMode;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;
use std::process::Command;

/// Target Dolby Vision profiles for conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DvProfile {
    /// Profile 8.1 - Single layer with static metadata
    /// Most compatible with consumer devices
    Profile8,
}

impl DvProfile {
    fn conversion_mode(&self) -> ConversionMode {
        match self {
            DvProfile::Profile8 => ConversionMode::To81,
        }
    }
}

/// Convert Dolby Vision Profile 7 to Profile 8.1.
///
/// This process:
/// 1. Extracts the HEVC stream from the container
/// 2. Extracts RPU (Reference Processing Unit) metadata
/// 3. Converts RPU from Profile 7 to Profile 8.1
/// 4. Injects the converted RPU back into the HEVC stream
/// 5. Remuxes with original audio and subtitles
///
/// # Requirements
///
/// - ffmpeg: For stream extraction
/// - mkvmerge: For final remuxing
/// - dovi_tool: For RPU extraction, injection, and conversion fallback
///
/// # Example
///
/// ```no_run
/// use sceneforged_av::{Workspace, actions::{convert_dv_profile, DvProfile}};
///
/// let workspace = Workspace::new("/path/to/dv_profile7.mkv")?;
/// convert_dv_profile(&workspace, DvProfile::Profile8)?;
/// workspace.finalize(None)?;
/// # Ok::<(), sceneforged_av::Error>(())
/// ```
pub fn convert_dv_profile(workspace: &Workspace, target: DvProfile) -> Result<()> {
    let input = workspace.input();
    let output = workspace.output();

    #[cfg(feature = "tracing")]
    tracing::info!(
        "Converting Dolby Vision to profile {:?} for: {:?}",
        target,
        input
    );

    // Step 1: Extract HEVC elementary stream
    let hevc_file = workspace.temp_file("video.hevc");
    extract_hevc(input, &hevc_file)?;

    // Step 2: Extract RPU data
    let rpu_file = workspace.temp_file("RPU.bin");
    extract_rpu(&hevc_file, &rpu_file)?;

    // Step 3: Convert RPU to target profile
    let converted_rpu = workspace.temp_file("RPU_converted.bin");
    convert_rpu(&rpu_file, &converted_rpu, target)?;

    // Step 4: Inject converted RPU back into HEVC
    let hevc_with_rpu = workspace.temp_file("video_converted.hevc");
    inject_rpu(&hevc_file, &converted_rpu, &hevc_with_rpu)?;

    // Step 5: Remux with original audio and subtitles
    remux_with_audio_subs(input, &hevc_with_rpu, output)?;

    #[cfg(feature = "tracing")]
    tracing::info!("Dolby Vision conversion complete: {:?}", output);

    Ok(())
}

/// Convert RPU data using the native dolby_vision crate.
///
/// This reads RPU data from a binary file, converts each RPU to the target
/// profile, and writes the converted data to the output file.
fn convert_rpu_native(input: &Path, output: &Path, target: DvProfile) -> Result<()> {
    use dolby_vision::rpu::dovi_rpu::DoviRpu;

    let input_file = File::open(input)?;
    let mut reader = BufReader::new(input_file);

    let output_file = File::create(output)?;
    let mut writer = BufWriter::new(output_file);

    let mut buffer = Vec::new();
    reader.read_to_end(&mut buffer)?;

    // RPU data is stored as a series of NAL units
    // Each starts with a start code (0x00 0x00 0x00 0x01 or 0x00 0x00 0x01)
    let mut converted_data = Vec::new();
    let mut offset = 0;

    while offset < buffer.len() {
        // Find start code
        let start_code_len = if offset + 4 <= buffer.len()
            && buffer[offset..offset + 4] == [0x00, 0x00, 0x00, 0x01]
        {
            4
        } else if offset + 3 <= buffer.len() && buffer[offset..offset + 3] == [0x00, 0x00, 0x01] {
            3
        } else {
            offset += 1;
            continue;
        };

        let nalu_start = offset + start_code_len;

        // Find next start code or end
        let mut nalu_end = buffer.len();
        for i in nalu_start + 1..buffer.len() - 2 {
            if buffer[i..i + 3] == [0x00, 0x00, 0x01]
                || (i + 3 < buffer.len() && buffer[i..i + 4] == [0x00, 0x00, 0x00, 0x01])
            {
                nalu_end = i;
                break;
            }
        }

        let nalu_data = &buffer[nalu_start..nalu_end];

        // Try to parse as RPU NAL unit
        if let Ok(mut rpu) = DoviRpu::parse_unspec62_nalu(nalu_data) {
            // Convert to target profile
            if let Err(e) = rpu.convert_with_mode(target.conversion_mode()) {
                #[cfg(feature = "tracing")]
                tracing::warn!("Failed to convert RPU: {}", e);
            }

            // Write converted RPU
            if let Ok(converted_nalu) = rpu.write_hevc_unspec62_nalu() {
                converted_data.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);
                converted_data.extend_from_slice(&converted_nalu);
            }
        } else {
            // Not an RPU NAL, copy as-is
            converted_data.extend_from_slice(&buffer[offset..nalu_end]);
        }

        offset = nalu_end;
    }

    writer.write_all(&converted_data)?;

    Ok(())
}

// NOTE: Native HEVC extraction using BSF is not available in ffmpeg-the-third v4.
// The BSF API (av_bsf_get_by_name, AVBSFContext, av_bsf_alloc, av_bsf_init,
// av_bsf_send_packet, av_bsf_receive_packet) is not exposed in the FFI bindings.
// We rely on the CLI fallback implementation instead.

/// Extract HEVC elementary stream using ffmpeg CLI (fallback).
fn extract_hevc_cli(input: &Path, output: &Path) -> Result<()> {
    #[cfg(feature = "tracing")]
    tracing::debug!("Extracting HEVC stream via CLI from {:?}", input);

    let status = Command::new("ffmpeg")
        .args(["-y", "-i"])
        .arg(input)
        .args([
            "-c:v",
            "copy",
            "-bsf:v",
            "hevc_mp4toannexb",
            "-an",
            "-sn",
            "-f",
            "hevc",
        ])
        .arg(output)
        .status()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Error::tool_not_found("ffmpeg")
            } else {
                Error::Io(e)
            }
        })?;

    if !status.success() {
        return Err(Error::tool_failed("ffmpeg", "HEVC extraction failed"));
    }

    Ok(())
}

fn extract_hevc(input: &Path, output: &Path) -> Result<()> {
    #[cfg(feature = "tracing")]
    tracing::debug!("Extracting HEVC stream from {:?}", input);

    // Use CLI implementation (native BSF API not available in ffmpeg-the-third v4)
    extract_hevc_cli(input, output)
}

fn extract_rpu(hevc_file: &Path, rpu_output: &Path) -> Result<()> {
    #[cfg(feature = "tracing")]
    tracing::debug!("Extracting RPU from {:?}", hevc_file);

    let status = Command::new("dovi_tool")
        .args(["extract-rpu", "-i"])
        .arg(hevc_file)
        .arg("-o")
        .arg(rpu_output)
        .status()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Error::tool_not_found("dovi_tool")
            } else {
                Error::Io(e)
            }
        })?;

    if !status.success() {
        return Err(Error::tool_failed("dovi_tool", "RPU extraction failed"));
    }

    Ok(())
}

fn convert_rpu(rpu_input: &Path, rpu_output: &Path, target: DvProfile) -> Result<()> {
    #[cfg(feature = "tracing")]
    tracing::debug!("Converting RPU to profile {:?}", target);

    // Try native conversion first
    match convert_rpu_native(rpu_input, rpu_output, target) {
        Ok(()) => return Ok(()),
        Err(e) => {
            #[cfg(feature = "tracing")]
            tracing::warn!("Native RPU conversion failed, falling back to CLI: {}", e);
        }
    }

    // Fallback to dovi_tool CLI
    let mode = match target {
        DvProfile::Profile8 => "2", // Mode 2 = Convert to P8.1
    };

    let status = Command::new("dovi_tool")
        .args(["convert", "--mode", mode, "-i"])
        .arg(rpu_input)
        .arg("-o")
        .arg(rpu_output)
        .status()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Error::tool_not_found("dovi_tool")
            } else {
                Error::Io(e)
            }
        })?;

    if !status.success() {
        return Err(Error::tool_failed("dovi_tool", "RPU conversion failed"));
    }

    Ok(())
}

fn inject_rpu(hevc_input: &Path, rpu_file: &Path, output: &Path) -> Result<()> {
    #[cfg(feature = "tracing")]
    tracing::debug!("Injecting converted RPU into HEVC");

    let status = Command::new("dovi_tool")
        .args(["inject-rpu", "-i"])
        .arg(hevc_input)
        .arg("--rpu-in")
        .arg(rpu_file)
        .arg("-o")
        .arg(output)
        .status()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Error::tool_not_found("dovi_tool")
            } else {
                Error::Io(e)
            }
        })?;

    if !status.success() {
        return Err(Error::tool_failed("dovi_tool", "RPU injection failed"));
    }

    Ok(())
}

fn remux_with_audio_subs(original: &Path, hevc_file: &Path, output: &Path) -> Result<()> {
    #[cfg(feature = "tracing")]
    tracing::debug!("Remuxing with audio and subtitles from original");

    let status = Command::new("mkvmerge")
        .arg("-o")
        .arg(output)
        // New video track
        .arg(hevc_file)
        // Audio and subs from original (no video)
        .arg("--no-video")
        .arg(original)
        .status()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Error::tool_not_found("mkvmerge")
            } else {
                Error::Io(e)
            }
        })?;

    // mkvmerge returns 0 for success, 1 for warnings (still OK), 2 for errors
    if !status.success() && status.code() != Some(1) {
        return Err(Error::tool_failed("mkvmerge", "remux failed"));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dv_profile_conversion_mode() {
        assert!(matches!(
            DvProfile::Profile8.conversion_mode(),
            ConversionMode::To81
        ));
    }
}
