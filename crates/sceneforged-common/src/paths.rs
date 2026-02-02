//! Path utilities for detecting file types by extension.
//!
//! This module provides functions to check if files are videos, subtitles, or images
//! based on their file extensions. These are used throughout the scanner and media
//! processing pipelines.

use std::path::Path;

/// List of supported video file extensions.
const VIDEO_EXTENSIONS: &[&str] = &[
    "mkv", "mp4", "avi", "m4v", "ts", "webm", "mov", "wmv", "flv",
];

/// List of supported subtitle file extensions.
const SUBTITLE_EXTENSIONS: &[&str] = &["srt", "ass", "ssa", "sub", "vtt", "idx"];

/// List of supported image file extensions.
const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "gif", "webp", "bmp"];

/// Check if a path has a video file extension.
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// use sceneforged_common::paths::is_video_file;
///
/// assert!(is_video_file(Path::new("movie.mkv")));
/// assert!(is_video_file(Path::new("/path/to/video.mp4")));
/// assert!(!is_video_file(Path::new("subtitle.srt")));
/// ```
pub fn is_video_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| VIDEO_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// Check if a path has a subtitle file extension.
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// use sceneforged_common::paths::is_subtitle_file;
///
/// assert!(is_subtitle_file(Path::new("movie.srt")));
/// assert!(is_subtitle_file(Path::new("/path/to/subtitle.ass")));
/// assert!(!is_subtitle_file(Path::new("video.mkv")));
/// ```
pub fn is_subtitle_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| SUBTITLE_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// Check if a path has an image file extension.
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// use sceneforged_common::paths::is_image_file;
///
/// assert!(is_image_file(Path::new("poster.jpg")));
/// assert!(is_image_file(Path::new("/path/to/image.png")));
/// assert!(!is_image_file(Path::new("video.mkv")));
/// ```
pub fn is_image_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| IMAGE_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// Get the list of video file extensions.
///
/// # Examples
///
/// ```
/// use sceneforged_common::paths::video_extensions;
///
/// let extensions = video_extensions();
/// assert!(extensions.contains(&"mkv"));
/// assert!(extensions.contains(&"mp4"));
/// ```
#[must_use]
pub fn video_extensions() -> &'static [&'static str] {
    VIDEO_EXTENSIONS
}

/// Get the list of subtitle file extensions.
///
/// # Examples
///
/// ```
/// use sceneforged_common::paths::subtitle_extensions;
///
/// let extensions = subtitle_extensions();
/// assert!(extensions.contains(&"srt"));
/// assert!(extensions.contains(&"ass"));
/// ```
#[must_use]
pub fn subtitle_extensions() -> &'static [&'static str] {
    SUBTITLE_EXTENSIONS
}

/// Get the list of image file extensions.
///
/// # Examples
///
/// ```
/// use sceneforged_common::paths::image_extensions;
///
/// let extensions = image_extensions();
/// assert!(extensions.contains(&"jpg"));
/// assert!(extensions.contains(&"png"));
/// ```
#[must_use]
pub fn image_extensions() -> &'static [&'static str] {
    IMAGE_EXTENSIONS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_video_file() {
        assert!(is_video_file(Path::new("movie.mkv")));
        assert!(is_video_file(Path::new("movie.mp4")));
        assert!(is_video_file(Path::new("movie.avi")));
        assert!(is_video_file(Path::new("movie.m4v")));
        assert!(is_video_file(Path::new("movie.ts")));
        assert!(is_video_file(Path::new("movie.webm")));
        assert!(is_video_file(Path::new("movie.mov")));
        assert!(is_video_file(Path::new("movie.wmv")));
        assert!(is_video_file(Path::new("movie.flv")));

        // Case insensitive
        assert!(is_video_file(Path::new("movie.MKV")));
        assert!(is_video_file(Path::new("movie.Mp4")));

        // With paths
        assert!(is_video_file(Path::new("/path/to/movie.mkv")));
        assert!(is_video_file(Path::new("relative/path/movie.mp4")));

        // Not video files
        assert!(!is_video_file(Path::new("subtitle.srt")));
        assert!(!is_video_file(Path::new("image.jpg")));
        assert!(!is_video_file(Path::new("document.txt")));
        assert!(!is_video_file(Path::new("no_extension")));
    }

    #[test]
    fn test_is_subtitle_file() {
        assert!(is_subtitle_file(Path::new("movie.srt")));
        assert!(is_subtitle_file(Path::new("movie.ass")));
        assert!(is_subtitle_file(Path::new("movie.ssa")));
        assert!(is_subtitle_file(Path::new("movie.sub")));
        assert!(is_subtitle_file(Path::new("movie.vtt")));
        assert!(is_subtitle_file(Path::new("movie.idx")));

        // Case insensitive
        assert!(is_subtitle_file(Path::new("movie.SRT")));
        assert!(is_subtitle_file(Path::new("movie.Ass")));

        // With paths
        assert!(is_subtitle_file(Path::new("/path/to/subtitle.srt")));

        // Not subtitle files
        assert!(!is_subtitle_file(Path::new("movie.mkv")));
        assert!(!is_subtitle_file(Path::new("image.jpg")));
        assert!(!is_subtitle_file(Path::new("no_extension")));
    }

    #[test]
    fn test_is_image_file() {
        assert!(is_image_file(Path::new("poster.jpg")));
        assert!(is_image_file(Path::new("poster.jpeg")));
        assert!(is_image_file(Path::new("poster.png")));
        assert!(is_image_file(Path::new("poster.gif")));
        assert!(is_image_file(Path::new("poster.webp")));
        assert!(is_image_file(Path::new("poster.bmp")));

        // Case insensitive
        assert!(is_image_file(Path::new("poster.JPG")));
        assert!(is_image_file(Path::new("poster.Png")));

        // With paths
        assert!(is_image_file(Path::new("/path/to/image.jpg")));

        // Not image files
        assert!(!is_image_file(Path::new("movie.mkv")));
        assert!(!is_image_file(Path::new("subtitle.srt")));
        assert!(!is_image_file(Path::new("no_extension")));
    }

    #[test]
    fn test_video_extensions() {
        let exts = video_extensions();
        assert_eq!(exts.len(), 9);
        assert!(exts.contains(&"mkv"));
        assert!(exts.contains(&"mp4"));
        assert!(exts.contains(&"avi"));
        assert!(exts.contains(&"m4v"));
        assert!(exts.contains(&"ts"));
        assert!(exts.contains(&"webm"));
        assert!(exts.contains(&"mov"));
        assert!(exts.contains(&"wmv"));
        assert!(exts.contains(&"flv"));
    }

    #[test]
    fn test_subtitle_extensions() {
        let exts = subtitle_extensions();
        assert_eq!(exts.len(), 6);
        assert!(exts.contains(&"srt"));
        assert!(exts.contains(&"ass"));
        assert!(exts.contains(&"ssa"));
        assert!(exts.contains(&"sub"));
        assert!(exts.contains(&"vtt"));
        assert!(exts.contains(&"idx"));
    }

    #[test]
    fn test_image_extensions() {
        let exts = image_extensions();
        assert_eq!(exts.len(), 6);
        assert!(exts.contains(&"jpg"));
        assert!(exts.contains(&"jpeg"));
        assert!(exts.contains(&"png"));
        assert!(exts.contains(&"gif"));
        assert!(exts.contains(&"webp"));
        assert!(exts.contains(&"bmp"));
    }

    #[test]
    fn test_edge_cases() {
        // Empty path
        assert!(!is_video_file(Path::new("")));
        assert!(!is_subtitle_file(Path::new("")));
        assert!(!is_image_file(Path::new("")));

        // Path with no extension
        assert!(!is_video_file(Path::new("filename")));
        assert!(!is_subtitle_file(Path::new("filename")));
        assert!(!is_image_file(Path::new("filename")));

        // Hidden files
        assert!(is_video_file(Path::new(".hidden.mkv")));
        assert!(is_subtitle_file(Path::new(".hidden.srt")));
        assert!(is_image_file(Path::new(".hidden.jpg")));

        // Multiple dots
        assert!(is_video_file(Path::new("movie.1080p.mkv")));
        assert!(is_subtitle_file(Path::new("movie.en.srt")));
        assert!(is_image_file(Path::new("poster.thumb.jpg")));
    }
}
