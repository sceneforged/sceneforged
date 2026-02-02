//! Variable substitution for command templates.

use std::collections::HashMap;
use std::path::Path;

/// Variable substitution context for command templates.
///
/// Supports variable substitution in strings using the `{varname}` syntax.
///
/// # Example
///
/// ```
/// use sceneforged_av::TemplateContext;
/// use std::path::Path;
///
/// let ctx = TemplateContext::new()
///     .with_workspace(
///         Path::new("/input/movie.mkv"),
///         Path::new("/tmp/movie.mkv"),
///         Path::new("/tmp"),
///     )
///     .with_var("quality", "high");
///
/// assert_eq!(ctx.substitute("{filestem}.mp4"), "movie.mp4");
/// assert_eq!(ctx.substitute("{workspace}/output.hevc"), "/tmp/output.hevc");
/// ```
#[derive(Debug, Clone)]
pub struct TemplateContext {
    vars: HashMap<String, String>,
}

impl TemplateContext {
    /// Create a new empty template context.
    pub fn new() -> Self {
        Self {
            vars: HashMap::new(),
        }
    }

    /// Set workspace-related variables.
    ///
    /// This adds the following variables:
    /// - `{input}` - Full path to the input file
    /// - `{output}` - Full path to the output file
    /// - `{workspace}` - Path to the temp directory
    /// - `{filename}` - Input file name with extension
    /// - `{filestem}` - Input file name without extension
    /// - `{extension}` - Input file extension
    /// - `{dirname}` - Input file parent directory
    pub fn with_workspace(mut self, input: &Path, output: &Path, temp_dir: &Path) -> Self {
        self.vars
            .insert("input".to_string(), input.display().to_string());
        self.vars
            .insert("output".to_string(), output.display().to_string());
        self.vars
            .insert("workspace".to_string(), temp_dir.display().to_string());

        // File components
        if let Some(name) = input.file_name() {
            self.vars
                .insert("filename".to_string(), name.to_string_lossy().to_string());
        }
        if let Some(stem) = input.file_stem() {
            self.vars
                .insert("filestem".to_string(), stem.to_string_lossy().to_string());
        }
        if let Some(ext) = input.extension() {
            self.vars
                .insert("extension".to_string(), ext.to_string_lossy().to_string());
        }
        if let Some(parent) = input.parent() {
            self.vars
                .insert("dirname".to_string(), parent.display().to_string());
        }

        self
    }

    /// Add a custom variable.
    pub fn with_var(mut self, key: &str, value: &str) -> Self {
        self.vars.insert(key.to_string(), value.to_string());
        self
    }

    /// Set a variable.
    pub fn set(&mut self, key: &str, value: &str) {
        self.vars.insert(key.to_string(), value.to_string());
    }

    /// Get a variable value.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.vars.get(key).map(|s| s.as_str())
    }

    /// Substitute variables in a string.
    ///
    /// Variables are in the form `{varname}`.
    pub fn substitute(&self, template: &str) -> String {
        let mut result = template.to_string();
        for (key, value) in &self.vars {
            result = result.replace(&format!("{{{}}}", key), value);
        }
        result
    }

    /// Substitute variables in a list of strings.
    pub fn substitute_all(&self, templates: &[String]) -> Vec<String> {
        templates.iter().map(|t| self.substitute(t)).collect()
    }
}

impl Default for TemplateContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_substitute() {
        let ctx = TemplateContext::new().with_workspace(
            &PathBuf::from("/input/movie.mkv"),
            &PathBuf::from("/tmp/movie.mkv"),
            &PathBuf::from("/tmp"),
        );

        assert_eq!(ctx.substitute("{input}"), "/input/movie.mkv");
        assert_eq!(ctx.substitute("{filestem}.mp4"), "movie.mp4");
        assert_eq!(
            ctx.substitute("{workspace}/intermediate.hevc"),
            "/tmp/intermediate.hevc"
        );
    }

    #[test]
    fn test_custom_var() {
        let ctx = TemplateContext::new()
            .with_var("quality", "high")
            .with_var("codec", "hevc");

        assert_eq!(
            ctx.substitute("output_{quality}_{codec}.mkv"),
            "output_high_hevc.mkv"
        );
    }

    #[test]
    fn test_substitute_all() {
        let ctx = TemplateContext::new().with_var("name", "test");

        let templates = vec!["{name}.txt".to_string(), "{name}.log".to_string()];
        let results = ctx.substitute_all(&templates);

        assert_eq!(results, vec!["test.txt", "test.log"]);
    }
}
