//! Workspace management for pipeline operations.
//!
//! A [`Workspace`] provides a temporary directory for intermediate files and
//! manages the input/output lifecycle with safe finalization (optional backup
//! of the original file).

use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// Workspace for pipeline execution.
///
/// Provides a temporary directory for intermediate files and manages the
/// input/output file paths with atomic finalization.
///
/// # Example
///
/// ```no_run
/// use sf_av::Workspace;
///
/// let workspace = Workspace::new(std::path::Path::new("/path/to/input.mkv")).unwrap();
/// // ... perform processing, writing output to workspace.output() ...
/// workspace.finalize(Some("bak")).unwrap();
/// ```
pub struct Workspace {
    temp_dir: TempDir,
    input_path: PathBuf,
}

impl Workspace {
    /// Create a new workspace for processing a file.
    ///
    /// Creates a temporary directory and records the input path. The output
    /// path will share the same filename as the input, located inside the
    /// temp directory.
    pub fn new(input: &Path) -> sf_core::Result<Self> {
        let temp_dir = TempDir::new().map_err(|e| sf_core::Error::Tool {
            tool: "workspace".to_string(),
            message: format!("failed to create temp dir: {e}"),
        })?;

        Ok(Self {
            temp_dir,
            input_path: input.to_path_buf(),
        })
    }

    /// The original input file path.
    pub fn input(&self) -> &Path {
        &self.input_path
    }

    /// The output file path (same filename as input, inside the temp dir).
    pub fn output(&self) -> PathBuf {
        let file_name = self
            .input_path
            .file_name()
            .unwrap_or_else(|| std::ffi::OsStr::new("output"));
        self.temp_dir.path().join(file_name)
    }

    /// Path to the temporary directory.
    pub fn temp_dir(&self) -> &Path {
        self.temp_dir.path()
    }

    /// Create a path for a named temporary file inside the workspace.
    pub fn temp_file(&self, name: &str) -> PathBuf {
        self.temp_dir.path().join(name)
    }

    /// Finalize the workspace: optionally back up the original, then move the
    /// output file to the input location.
    ///
    /// - If `backup_ext` is `Some("bak")` and the original exists, it will be
    ///   renamed to `<original>.bak` before the output replaces it.
    /// - Returns the final path (the original input location).
    ///
    /// # Errors
    ///
    /// Returns an error if the output file does not exist or if any rename
    /// operation fails.
    pub fn finalize(self, backup_ext: Option<&str>) -> sf_core::Result<PathBuf> {
        let output = self.output();
        let dest = &self.input_path;

        if !output.exists() {
            return Err(sf_core::Error::Tool {
                tool: "workspace".to_string(),
                message: format!("output file does not exist: {}", output.display()),
            });
        }

        // Backup original if requested and it exists.
        if let Some(ext) = backup_ext {
            if dest.exists() {
                let backup = dest.with_extension(ext);
                std::fs::rename(dest, &backup).map_err(|e| sf_core::Error::Tool {
                    tool: "workspace".to_string(),
                    message: format!("failed to create backup: {e}"),
                })?;
            }
        }

        // Move output -> original location.
        // Try rename first (same filesystem), fall back to copy+remove.
        if let Err(_rename_err) = std::fs::rename(&output, dest) {
            std::fs::copy(&output, dest).map_err(|e| sf_core::Error::Tool {
                tool: "workspace".to_string(),
                message: format!("failed to copy output to destination: {e}"),
            })?;
            let _ = std::fs::remove_file(&output);
        }

        Ok(dest.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn workspace_paths() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let ws = Workspace::new(tmp.path()).unwrap();

        assert_eq!(ws.input(), tmp.path());
        assert!(ws.output().starts_with(ws.temp_dir()));
        assert_eq!(ws.output().file_name(), tmp.path().file_name());
    }

    #[test]
    fn temp_file_inside_workspace() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let ws = Workspace::new(tmp.path()).unwrap();
        let tf = ws.temp_file("intermediate.hevc");
        assert!(tf.starts_with(ws.temp_dir()));
        assert_eq!(tf.file_name().unwrap(), "intermediate.hevc");
    }

    #[test]
    fn finalize_without_backup() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("movie.mkv");
        fs::write(&input, b"original").unwrap();

        let ws = Workspace::new(&input).unwrap();
        let output_path = ws.output();
        fs::write(&output_path, b"processed").unwrap();

        let final_path = ws.finalize(None).unwrap();
        assert_eq!(final_path, input);
        assert_eq!(fs::read_to_string(&input).unwrap(), "processed");
    }

    #[test]
    fn finalize_with_backup() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("movie.mkv");
        fs::write(&input, b"original").unwrap();

        let ws = Workspace::new(&input).unwrap();
        let output_path = ws.output();
        fs::write(&output_path, b"processed").unwrap();

        let final_path = ws.finalize(Some("bak")).unwrap();
        assert_eq!(final_path, input);
        assert_eq!(fs::read_to_string(&input).unwrap(), "processed");

        let backup = dir.path().join("movie.bak");
        assert!(backup.exists());
        assert_eq!(fs::read_to_string(&backup).unwrap(), "original");
    }

    #[test]
    fn finalize_fails_when_output_missing() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("movie.mkv");
        fs::write(&input, b"original").unwrap();

        let ws = Workspace::new(&input).unwrap();
        // Don't write anything to the output.
        let result = ws.finalize(None);
        assert!(result.is_err());
    }
}
