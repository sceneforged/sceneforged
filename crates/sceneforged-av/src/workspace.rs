//! Workspace management for pipeline execution.

use crate::{Error, Result};
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// Workspace for pipeline execution.
///
/// Provides a temporary directory for intermediate files and manages
/// the input/output file paths with atomic finalization.
///
/// # Example
///
/// ```no_run
/// use sceneforged_av::Workspace;
///
/// let workspace = Workspace::new("/path/to/input.mkv")?;
/// // Process files using workspace.temp_file("intermediate.hevc")
/// // When done, finalize to replace the original or save to a new location
/// workspace.finalize(None)?;
/// # Ok::<(), sceneforged_av::Error>(())
/// ```
pub struct Workspace {
    temp_dir: TempDir,
    input_path: PathBuf,
    output_path: PathBuf,
}

impl Workspace {
    /// Create a new workspace for processing a file.
    pub fn new<P: AsRef<Path>>(input: P) -> Result<Self> {
        let input = input.as_ref();
        let temp_dir = TempDir::new().map_err(|e| Error::Workspace(e.to_string()))?;

        let input_path = input.to_path_buf();

        // Output will be named same as input, in temp dir initially
        let file_name = input
            .file_name()
            .ok_or_else(|| Error::InvalidInput("Invalid input file path".to_string()))?;
        let output_path = temp_dir.path().join(file_name);

        Ok(Self {
            temp_dir,
            input_path,
            output_path,
        })
    }

    /// Get the input file path.
    pub fn input(&self) -> &Path {
        &self.input_path
    }

    /// Get the output file path.
    pub fn output(&self) -> &Path {
        &self.output_path
    }

    /// Get the temp directory path.
    pub fn temp_dir(&self) -> &Path {
        self.temp_dir.path()
    }

    /// Create a temp file path with the given name.
    pub fn temp_file(&self, name: &str) -> PathBuf {
        self.temp_dir.path().join(name)
    }

    /// Move the final output to replace the original (or specified destination).
    ///
    /// This creates a backup of the original file and atomically replaces it.
    /// If the operation fails, the backup is restored.
    pub fn finalize(self, destination: Option<&Path>) -> Result<PathBuf> {
        let dest = destination.unwrap_or(&self.input_path);

        if !self.output_path.exists() {
            return Err(Error::Workspace(format!(
                "Output file does not exist: {:?}",
                self.output_path
            )));
        }

        // Create backup of original if it exists
        if dest.exists() {
            let backup = dest.with_extension("bak");
            std::fs::rename(dest, &backup).map_err(|e| {
                Error::Workspace(format!("Failed to create backup of original file: {}", e))
            })?;

            // Move output to destination
            if let Err(e) = std::fs::rename(&self.output_path, dest) {
                // Restore backup on failure
                let _ = std::fs::rename(&backup, dest);
                return Err(Error::Workspace(format!(
                    "Failed to move output to destination: {}",
                    e
                )));
            }

            // Remove backup on success
            let _ = std::fs::remove_file(&backup);
        } else {
            std::fs::rename(&self.output_path, dest).map_err(|e| {
                Error::Workspace(format!("Failed to move output to destination: {}", e))
            })?;
        }

        Ok(dest.to_path_buf())
    }

    /// Clean up without finalizing (discard output).
    pub fn cleanup(self) {
        // TempDir will clean up on drop
        drop(self.temp_dir);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_workspace_paths() {
        let temp_file = NamedTempFile::new().unwrap();
        let workspace = Workspace::new(temp_file.path()).unwrap();

        assert_eq!(workspace.input(), temp_file.path());
        assert!(workspace.output().starts_with(workspace.temp_dir()));
        assert_eq!(workspace.output().file_name(), temp_file.path().file_name());
    }

    #[test]
    fn test_temp_file() {
        let temp_file = NamedTempFile::new().unwrap();
        let workspace = Workspace::new(temp_file.path()).unwrap();

        let intermediate = workspace.temp_file("test.hevc");
        assert!(intermediate.starts_with(workspace.temp_dir()));
        assert_eq!(intermediate.file_name().unwrap(), "test.hevc");
    }
}
