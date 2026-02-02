//! Pipeline integration tests
//!
//! Tests for workspace management, template substitution, and pipeline execution.

use sceneforged::pipeline::TemplateContext;
use std::path::PathBuf;
use tempfile::tempdir;

/// Test workspace-based template substitution
#[test]
fn test_template_substitution_with_workspace() {
    let ctx = TemplateContext::new().with_workspace(
        &PathBuf::from("/input/movies/My Movie (2023).mkv"),
        &PathBuf::from("/output/movies/My Movie (2023).mkv"),
        &PathBuf::from("/tmp/sceneforged-work-123"),
    );

    assert_eq!(
        ctx.substitute("{input}"),
        "/input/movies/My Movie (2023).mkv"
    );
    assert_eq!(
        ctx.substitute("{output}"),
        "/output/movies/My Movie (2023).mkv"
    );
    assert_eq!(ctx.substitute("{workspace}"), "/tmp/sceneforged-work-123");
    assert_eq!(ctx.substitute("{filestem}"), "My Movie (2023)");
    assert_eq!(ctx.substitute("{extension}"), "mkv");
    assert_eq!(ctx.substitute("{dirname}"), "/input/movies");
    assert_eq!(ctx.substitute("{filename}"), "My Movie (2023).mkv");
}

/// Test template substitution with custom variables
#[test]
fn test_template_substitution_custom_vars() {
    let ctx = TemplateContext::new()
        .with_workspace(
            &PathBuf::from("/input/movie.mkv"),
            &PathBuf::from("/output/movie.mkv"),
            &PathBuf::from("/tmp/work"),
        )
        .with_var("profile", "8")
        .with_var("target_codec", "hevc");

    assert_eq!(ctx.substitute("dovi_tool -m {profile}"), "dovi_tool -m 8");
    assert_eq!(ctx.substitute("-c:v {target_codec}"), "-c:v hevc");
}

/// Test template substitution for complex ffmpeg commands
#[test]
fn test_template_substitution_complex_command() {
    let ctx = TemplateContext::new().with_workspace(
        &PathBuf::from("/movies/test.mkv"),
        &PathBuf::from("/movies/test_out.mkv"),
        &PathBuf::from("/tmp/work"),
    );

    let template = "ffmpeg -i {input} -c:v copy -c:a aac {workspace}/intermediate.mkv";
    let result = ctx.substitute(template);

    assert_eq!(
        result,
        "ffmpeg -i /movies/test.mkv -c:v copy -c:a aac /tmp/work/intermediate.mkv"
    );
}

/// Test substitute_all for list of arguments
#[test]
fn test_template_substitute_all() {
    let ctx = TemplateContext::new().with_workspace(
        &PathBuf::from("/input/movie.mkv"),
        &PathBuf::from("/output/movie.mkv"),
        &PathBuf::from("/tmp/work"),
    );

    let templates = vec![
        "-i".to_string(),
        "{input}".to_string(),
        "-o".to_string(),
        "{output}".to_string(),
    ];

    let result = ctx.substitute_all(&templates);

    assert_eq!(
        result,
        vec!["-i", "/input/movie.mkv", "-o", "/output/movie.mkv"]
    );
}

/// Test that unrecognized variables are left as-is
#[test]
fn test_template_unrecognized_variables() {
    let ctx = TemplateContext::new().with_workspace(
        &PathBuf::from("/input/movie.mkv"),
        &PathBuf::from("/output/movie.mkv"),
        &PathBuf::from("/tmp/work"),
    );

    // {unknown} should remain unchanged
    let result = ctx.substitute("{input} to {unknown}");
    assert_eq!(result, "/input/movie.mkv to {unknown}");
}

/// Test temporary workspace creation
#[test]
fn test_workspace_temp_creation() {
    let temp = tempdir().unwrap();
    let workspace_dir = temp.path().join("workspace");

    std::fs::create_dir(&workspace_dir).unwrap();
    assert!(workspace_dir.exists());

    // Create some test files
    let input_file = workspace_dir.join("input.mkv");
    std::fs::write(&input_file, b"fake video content").unwrap();
    assert!(input_file.exists());

    // Simulate intermediate file
    let intermediate = workspace_dir.join("intermediate.hevc");
    std::fs::write(&intermediate, b"fake hevc stream").unwrap();
    assert!(intermediate.exists());

    // Cleanup happens automatically when temp goes out of scope
}

/// Test file operations in workspace
#[test]
fn test_workspace_file_operations() {
    let temp = tempdir().unwrap();
    let input = temp.path().join("source.mkv");
    let output = temp.path().join("output.mkv");
    let work = temp.path().join("work");

    // Create the work directory
    std::fs::create_dir(&work).unwrap();

    // Simulate input file
    std::fs::write(&input, b"test input content").unwrap();

    // Build template context
    let ctx = TemplateContext::new().with_workspace(&input, &output, &work);

    // Verify paths resolve correctly
    assert_eq!(ctx.substitute("{input}"), input.display().to_string());
    assert_eq!(ctx.substitute("{output}"), output.display().to_string());
    assert_eq!(ctx.substitute("{workspace}"), work.display().to_string());

    // Simulate output creation
    std::fs::write(&output, b"test output content").unwrap();
    assert!(output.exists());

    // Verify file sizes differ (simulating processing)
    let input_size = std::fs::metadata(&input).unwrap().len();
    let output_size = std::fs::metadata(&output).unwrap().len();
    assert!(output_size > 0);
    assert_eq!(input_size, 18); // "test input content"
    assert_eq!(output_size, 19); // "test output content"
}
