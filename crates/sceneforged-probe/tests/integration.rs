//! Integration tests for sceneforged-probe

use sceneforged_probe::{
    container::Container,
    is_supported_format, probe_bytes,
    types::{ColorPrimaries, HdrFormat, TransferCharacteristics},
};

/// Test container detection from magic bytes
#[test]
fn test_container_detection_mkv_magic() {
    // MKV magic bytes (EBML header)
    let mkv_data = [
        0x1A, 0x45, 0xDF, 0xA3, // EBML header ID
        0x93, // EBML header size
        0x42, 0x82, // EBML version
        0x88, // Version size
        0x6D, 0x61, 0x74, 0x72, 0x6F, 0x73, 0x6B, 0x61, // "matroska"
    ];

    let result = probe_bytes(&mkv_data, None);
    // Will fail to fully parse but should detect as Matroska
    assert!(
        result.is_ok()
            || matches!(
                result,
                Err(sceneforged_probe::VideoProbeError::ContainerParse(_))
            )
    );
}

/// Test container detection from magic bytes - MP4
#[test]
fn test_container_detection_mp4_magic() {
    // MP4 ftyp box
    let mp4_data = [
        0x00, 0x00, 0x00, 0x14, // Box size (20 bytes)
        b'f', b't', b'y', b'p', // Box type
        b'i', b's', b'o', b'm', // Brand
        0x00, 0x00, 0x00, 0x01, // Minor version
        b'i', b's', b'o', b'm', // Compatible brand
    ];

    let result = probe_bytes(&mp4_data, Some(Container::Mp4));
    // Will fail to fully parse but should accept the hint
    assert!(
        result.is_ok()
            || matches!(
                result,
                Err(sceneforged_probe::VideoProbeError::ContainerParse(_))
            )
    );
}

/// Test HdrFormat display implementation
#[test]
fn test_hdr_format_display() {
    assert_eq!(format!("{}", HdrFormat::Sdr), "SDR");
    assert_eq!(
        format!(
            "{}",
            HdrFormat::Hdr10 {
                mastering_display: None,
                content_light_level: None,
            }
        ),
        "HDR10"
    );
    assert_eq!(
        format!(
            "{}",
            HdrFormat::Hdr10Plus {
                mastering_display: None,
                content_light_level: None,
            }
        ),
        "HDR10+"
    );
    assert_eq!(format!("{}", HdrFormat::Hlg), "HLG");
    assert_eq!(
        format!(
            "{}",
            HdrFormat::DolbyVision {
                profile: 8,
                level: Some(6),
                bl_compatibility_id: Some(4),
                rpu_present: true,
                el_present: false,
                bl_signal_compatibility: None,
            }
        ),
        "Dolby Vision Profile 8 Level 6"
    );
}

/// Test color primaries conversion
#[test]
fn test_color_primaries_from_u8() {
    assert_eq!(ColorPrimaries::from(1), ColorPrimaries::Bt709);
    assert_eq!(ColorPrimaries::from(9), ColorPrimaries::Bt2020);
    assert_eq!(ColorPrimaries::from(99), ColorPrimaries::Unknown(99));
}

/// Test transfer characteristics conversion
#[test]
fn test_transfer_characteristics_from_u8() {
    assert_eq!(
        TransferCharacteristics::from(1),
        TransferCharacteristics::Bt709
    );
    assert_eq!(
        TransferCharacteristics::from(16),
        TransferCharacteristics::SmpteSt2084
    );
    assert_eq!(
        TransferCharacteristics::from(18),
        TransferCharacteristics::AribStdB67
    );
    assert_eq!(
        TransferCharacteristics::from(99),
        TransferCharacteristics::Unknown(99)
    );
}

/// Test transfer characteristics HDR detection
#[test]
fn test_transfer_characteristics_is_hdr() {
    assert!(!TransferCharacteristics::Bt709.is_hdr());
    assert!(TransferCharacteristics::SmpteSt2084.is_hdr());
    assert!(TransferCharacteristics::AribStdB67.is_hdr());
}

/// Test that non-existent files return proper error
#[test]
fn test_probe_nonexistent_file() {
    let result = sceneforged_probe::probe_file("/this/file/does/not/exist.mkv");
    assert!(matches!(
        result,
        Err(sceneforged_probe::VideoProbeError::FileNotFound(_))
    ));
}

/// Test is_supported_format for non-existent files
#[test]
fn test_is_supported_format_nonexistent() {
    assert!(!is_supported_format("/nonexistent/file.mkv"));
}

/// Test Dolby Vision profile names
#[test]
fn test_dv_profile_names() {
    use sceneforged_probe::hdr::dolby_vision::get_dv_profile_name;

    assert!(get_dv_profile_name(5).contains("Single-layer"));
    assert!(get_dv_profile_name(7).contains("Dual-layer"));
    assert!(get_dv_profile_name(8).contains("Single-layer"));
    assert!(get_dv_profile_name(99).contains("Unknown"));
}
