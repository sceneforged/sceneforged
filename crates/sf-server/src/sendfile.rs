//! True zero-copy HLS segment serving via sendfile(2).
//!
//! Segment requests (`GET /api/stream/{id}/segment_{N}.m4s`) are intercepted
//! before reaching Axum and served directly on the raw TCP socket using
//! sendfile(2). This eliminates the userspace copy that would otherwise occur
//! when reading file data into a `Vec<u8>` buffer.
//!
//! All other requests fall through to the normal hyper/Axum pipeline.

use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::os::fd::{AsRawFd, RawFd};

use crate::context::AppContext;
use crate::middleware::auth::validate_auth_headers;

// ---------------------------------------------------------------------------
// Peek pattern matching
// ---------------------------------------------------------------------------

/// Check if a peeked buffer looks like a segment request.
///
/// Matches: `GET /api/stream/{uuid}/segment_{N}.m4s HTTP/1.x`
///
/// Conservative: any parse failure returns `false` (falls through to Axum).
pub fn is_segment_request(peek_buf: &[u8]) -> bool {
    // Find the request line (up to first \r\n or the whole buffer).
    let line_end = peek_buf
        .windows(2)
        .position(|w| w == b"\r\n")
        .unwrap_or(peek_buf.len());
    let line = &peek_buf[..line_end];

    // Must start with "GET ".
    if !line.starts_with(b"GET ") {
        return false;
    }

    // Find the path: between first space and second space.
    let after_method = &line[4..];
    let path_end = match after_method.iter().position(|&b| b == b' ') {
        Some(pos) => pos,
        None => return false,
    };
    let path = &after_method[..path_end];

    // Must match /api/stream/{uuid}/segment_{N}.m4s
    let path = match std::str::from_utf8(path) {
        Ok(s) => s,
        Err(_) => return false,
    };

    let rest = match path.strip_prefix("/api/stream/") {
        Some(r) => r,
        None => return false,
    };

    // Split on '/' to get uuid and segment filename.
    let slash_pos = match rest.find('/') {
        Some(p) => p,
        None => return false,
    };

    let uuid_part = &rest[..slash_pos];
    let segment_part = &rest[slash_pos + 1..];

    // Validate UUID format (36 chars, hex+dashes).
    if uuid_part.len() != 36 {
        return false;
    }

    // Validate segment filename.
    if let Some(inner) = segment_part.strip_prefix("segment_") {
        if let Some(num_str) = inner.strip_suffix(".m4s") {
            return num_str.parse::<u32>().is_ok();
        }
    }

    false
}

/// Check if a peeked buffer looks like a direct play request.
///
/// Matches: `GET /api/stream/{uuid}/direct HTTP/1.x`
pub fn is_direct_request(peek_buf: &[u8]) -> bool {
    let path = match extract_get_path(peek_buf) {
        Some(p) => p,
        None => return false,
    };

    let rest = match path.strip_prefix("/api/stream/") {
        Some(r) => r,
        None => return false,
    };

    let slash = match rest.find('/') {
        Some(p) => p,
        None => return false,
    };

    let uuid_part = &rest[..slash];
    let suffix = &rest[slash + 1..];

    uuid_part.len() == 36 && suffix == "direct"
}

/// Check if a peeked buffer looks like a Jellyfin video stream request.
///
/// Matches: `GET /Videos/{uuid}/stream` (with optional query string)
pub fn is_jellyfin_stream_request(peek_buf: &[u8]) -> bool {
    let path = match extract_get_path(peek_buf) {
        Some(p) => p,
        None => return false,
    };

    // Strip query string if present.
    let path_no_query = path.split('?').next().unwrap_or(path);

    let rest = match path_no_query.strip_prefix("/Videos/") {
        Some(r) => r,
        None => return false,
    };

    let slash = match rest.find('/') {
        Some(p) => p,
        None => return false,
    };

    let uuid_part = &rest[..slash];
    let suffix = &rest[slash + 1..];

    uuid_part.len() == 36 && suffix == "stream"
}

/// Extract the GET path from a peeked HTTP request buffer.
fn extract_get_path(peek_buf: &[u8]) -> Option<&str> {
    let line_end = peek_buf
        .windows(2)
        .position(|w| w == b"\r\n")
        .unwrap_or(peek_buf.len());
    let line = &peek_buf[..line_end];

    if !line.starts_with(b"GET ") {
        return None;
    }

    let after_method = &line[4..];
    let path_end = after_method.iter().position(|&b| b == b' ')?;
    let path_bytes = &after_method[..path_end];
    std::str::from_utf8(path_bytes).ok()
}

/// Parse a `Range: bytes=START-END` header value (blocking version).
fn parse_range_value(value: &str) -> Option<(u64, Option<u64>)> {
    let bytes_prefix = value.strip_prefix("bytes=")?;
    let mut parts = bytes_prefix.splitn(2, '-');
    let start_str = parts.next()?.trim();
    let end_str = parts.next()?.trim();

    let start: u64 = start_str.parse().ok()?;
    let end: Option<u64> = if end_str.is_empty() {
        None
    } else {
        Some(end_str.parse().ok()?)
    };

    Some((start, end))
}

/// Guess content type from a file path extension.
fn guess_content_type_from_path(path: &std::path::Path) -> &'static str {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    match ext {
        "mp4" | "m4v" => "video/mp4",
        "mkv" => "video/x-matroska",
        "avi" => "video/x-msvideo",
        "webm" => "video/webm",
        "ts" => "video/mp2t",
        "mov" => "video/quicktime",
        "wmv" => "video/x-ms-wmv",
        "flv" => "video/x-flv",
        _ => "application/octet-stream",
    }
}

// ---------------------------------------------------------------------------
// Minimal HTTP request parser
// ---------------------------------------------------------------------------

struct ParsedRequest {
    path: String,
    authorization: Option<String>,
    cookie: Option<String>,
    x_emby_token: Option<String>,
    range: Option<(u64, Option<u64>)>,
}

/// Read HTTP request headers from a blocking TCP stream.
///
/// Reads until `\r\n\r\n` delimiter. Only extracts the fields we need.
fn read_request_headers(stream: &mut TcpStream) -> io::Result<ParsedRequest> {
    let mut buf = Vec::with_capacity(2048);
    let mut byte = [0u8; 1];

    // Read until we find \r\n\r\n (end of headers).
    loop {
        match stream.read(&mut byte) {
            Ok(0) => return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "connection closed")),
            Ok(_) => {
                buf.push(byte[0]);
                if buf.len() >= 4 && &buf[buf.len() - 4..] == b"\r\n\r\n" {
                    break;
                }
                // Safety limit: 8KB of headers.
                if buf.len() > 8192 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "headers too large",
                    ));
                }
            }
            Err(e) => return Err(e),
        }
    }

    let header_str = std::str::from_utf8(&buf)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "non-UTF-8 headers"))?;

    let mut lines = header_str.lines();

    // Parse request line: "GET /path HTTP/1.x"
    let request_line = lines
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "empty request"))?;

    let mut parts = request_line.split_whitespace();
    let _method = parts.next(); // Already validated as GET by peek
    let path = parts
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing path"))?
        .to_owned();

    let mut authorization = None;
    let mut cookie = None;
    let mut x_emby_token = None;
    let mut range = None;

    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            let name_lower = name.trim().to_ascii_lowercase();
            let value = value.trim();
            match name_lower.as_str() {
                "authorization" => authorization = Some(value.to_owned()),
                "cookie" => cookie = Some(value.to_owned()),
                "x-emby-token" => x_emby_token = Some(value.to_owned()),
                "range" => range = parse_range_value(value),
                _ => {}
            }
        }
    }

    Ok(ParsedRequest {
        path,
        authorization,
        cookie,
        x_emby_token,
        range,
    })
}

// ---------------------------------------------------------------------------
// Platform-specific sendfile
// ---------------------------------------------------------------------------

/// Send a range of a file to a socket via sendfile(2).
///
/// Loops on partial sends until the entire range is sent.
#[cfg(target_os = "macos")]
fn sendfile_range(
    sock_fd: RawFd,
    file_fd: RawFd,
    mut offset: u64,
    mut remaining: u64,
    headers: Option<&[&[u8]]>,
) -> io::Result<()> {
    // On the first call, we may have headers to send via sf_hdtr.
    // After that, we just send the file data.
    let mut first_call = true;

    while remaining > 0 || first_call {
        let off = offset as libc::off_t;
        let mut len = remaining as libc::off_t;

        // Build sf_hdtr for headers on first call.
        let mut hdr_iovecs: Vec<libc::iovec> = Vec::new();
        let hdtr;
        let hdtr_ptr;

        if first_call {
            if let Some(hdrs) = headers {
                for h in hdrs {
                    hdr_iovecs.push(libc::iovec {
                        iov_base: h.as_ptr() as *mut _,
                        iov_len: h.len(),
                    });
                }
            }
        }

        if !hdr_iovecs.is_empty() {
            hdtr = libc::sf_hdtr {
                headers: hdr_iovecs.as_mut_ptr(),
                hdr_cnt: hdr_iovecs.len() as i32,
                trailers: std::ptr::null_mut(),
                trl_cnt: 0,
            };
            hdtr_ptr = &hdtr as *const libc::sf_hdtr;
        } else {
            hdtr_ptr = std::ptr::null();
        }

        // macOS sendfile: sendfile(fd, s, offset, &mut len, hdtr, flags)
        // On macOS, `len` is in/out: on input the number of bytes to send from
        // the file, on output the total bytes sent (including headers/trailers).
        let ret = unsafe {
            libc::sendfile(file_fd, sock_fd, off, &mut len, hdtr_ptr as *mut _, 0)
        };

        let bytes_sent = len as u64;

        if ret == -1 {
            let err = io::Error::last_os_error();
            // EAGAIN/EINTR: partial send, len tells us how much was sent.
            if err.kind() == io::ErrorKind::Interrupted {
                // Adjust for what was sent.
                if first_call && !hdr_iovecs.is_empty() {
                    let hdr_total: u64 = hdr_iovecs.iter().map(|v| v.iov_len as u64).sum();
                    if bytes_sent <= hdr_total {
                        // Only sent some headers, no file data yet. Retry.
                        // This is extremely unlikely but handle it.
                        first_call = false;
                        continue;
                    }
                    let file_sent = bytes_sent - hdr_total;
                    offset += file_sent;
                    remaining -= file_sent;
                } else {
                    offset += bytes_sent;
                    remaining -= bytes_sent;
                }
                first_call = false;
                continue;
            }
            if err.kind() == io::ErrorKind::WouldBlock && bytes_sent > 0 {
                // Partial send on non-blocking socket.
                if first_call && !hdr_iovecs.is_empty() {
                    let hdr_total: u64 = hdr_iovecs.iter().map(|v| v.iov_len as u64).sum();
                    if bytes_sent > hdr_total {
                        let file_sent = bytes_sent - hdr_total;
                        offset += file_sent;
                        remaining -= file_sent;
                    }
                } else {
                    offset += bytes_sent;
                    remaining -= bytes_sent;
                }
                first_call = false;
                continue;
            }
            return Err(err);
        }

        // Success: adjust offset/remaining.
        if first_call && !hdr_iovecs.is_empty() {
            let hdr_total: u64 = hdr_iovecs.iter().map(|v| v.iov_len as u64).sum();
            let file_sent = bytes_sent.saturating_sub(hdr_total);
            offset += file_sent;
            remaining -= file_sent;
        } else {
            offset += bytes_sent;
            remaining -= bytes_sent;
        }

        first_call = false;
    }

    Ok(())
}

/// Send a range of a file to a socket via sendfile(2). Linux variant.
#[cfg(target_os = "linux")]
fn sendfile_range(
    sock_fd: RawFd,
    file_fd: RawFd,
    mut offset: u64,
    mut remaining: u64,
    _headers: Option<&[&[u8]]>,
) -> io::Result<()> {
    while remaining > 0 {
        let mut off = offset as libc::off64_t;
        let count = remaining.min(0x7ffff000) as usize; // Max ~2GB per call
        let ret = unsafe { libc::sendfile64(sock_fd, file_fd, &mut off, count) };

        if ret == -1 {
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(err);
        }

        let sent = ret as u64;
        if sent == 0 {
            return Err(io::Error::new(
                io::ErrorKind::WriteZero,
                "sendfile returned 0",
            ));
        }

        offset += sent;
        remaining -= sent;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// TCP_NOPUSH / TCP_CORK
// ---------------------------------------------------------------------------

/// Set TCP_NOPUSH (macOS) or TCP_CORK (Linux) on a socket.
///
/// When enabled, the kernel buffers small writes until the flag is cleared,
/// allowing a single segment to be sent with the full payload.
fn set_tcp_nopush(fd: RawFd, enabled: bool) -> io::Result<()> {
    let val: libc::c_int = if enabled { 1 } else { 0 };

    #[cfg(target_os = "macos")]
    let (level, optname) = (libc::IPPROTO_TCP, libc::TCP_NOPUSH);

    #[cfg(target_os = "linux")]
    let (level, optname) = (libc::IPPROTO_TCP, libc::TCP_CORK);

    let ret = unsafe {
        libc::setsockopt(
            fd,
            level,
            optname,
            &val as *const libc::c_int as *const libc::c_void,
            std::mem::size_of::<libc::c_int>() as libc::socklen_t,
        )
    };

    if ret == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Main sendfile handler
// ---------------------------------------------------------------------------

/// Handle a segment request using sendfile(2).
///
/// Called from `spawn_blocking` with a std `TcpStream`. Reads the full HTTP
/// request, validates auth, looks up the segment in the HLS cache, then
/// sends the response using sendfile to avoid copying file data through
/// userspace.
pub fn handle_sendfile_segment(mut stream: TcpStream, ctx: &AppContext) -> io::Result<()> {
    // Parse the full HTTP request headers.
    let req = read_request_headers(&mut stream)?;

    // Authenticate.
    if validate_auth_headers(
        &ctx.config.auth,
        &ctx.db,
        req.authorization.as_deref(),
        req.cookie.as_deref(),
        req.x_emby_token.as_deref(),
    )
    .is_none()
    {
        let response = b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
        stream.write_all(response)?;
        return Ok(());
    }

    // Parse media_file_id and segment index from path.
    // Path: /api/stream/{uuid}/segment_{N}.m4s
    let (mf_id, seg_index) = match parse_segment_path(&req.path) {
        Some(v) => v,
        None => {
            let response = b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
            stream.write_all(response)?;
            return Ok(());
        }
    };

    // Look up prepared media in HLS cache, populating on demand if missing.
    let prepared = match ctx.hls_cache.get(&mf_id) {
        Some(entry) => entry.value().clone(),
        None => {
            // Cache miss — do a blocking populate (DB lookup + moov parse).
            match populate_hls_cache_blocking(ctx, mf_id) {
                Ok(p) => p,
                Err(_) => {
                    let response = b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                    stream.write_all(response)?;
                    return Ok(());
                }
            }
        }
    };

    let seg = match prepared.segments.get(seg_index) {
        Some(s) => s,
        None => {
            let response = b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
            stream.write_all(response)?;
            return Ok(());
        }
    };

    // Calculate total content length.
    let header_bytes_len = seg.moof_bytes.len() + seg.mdat_header.len();
    let content_length = header_bytes_len as u64 + seg.data_length;

    // Open the source MP4 file.
    let file = std::fs::File::open(&prepared.file_path).map_err(|e| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("Failed to open {}: {e}", prepared.file_path.display()),
        )
    })?;
    let file_fd = file.as_raw_fd();
    let sock_fd = stream.as_raw_fd();

    // Set TCP_NOPUSH to coalesce the HTTP headers + moof + mdat_header + data.
    let _ = set_tcp_nopush(sock_fd, true);

    // Write HTTP response headers.
    let response_headers = format!(
        "HTTP/1.1 200 OK\r\n\
         Content-Type: video/iso.segment\r\n\
         Content-Length: {content_length}\r\n\
         Connection: close\r\n\
         \r\n"
    );
    stream.write_all(response_headers.as_bytes())?;

    // Build the in-memory prefix: moof_bytes + mdat_header.
    // These are small (typically < 1KB combined).
    let mut prefix = Vec::with_capacity(header_bytes_len);
    prefix.extend_from_slice(&seg.moof_bytes);
    prefix.extend_from_slice(&seg.mdat_header);

    // Send prefix + first video range using sendfile with headers (macOS) or
    // write + sendfile (Linux).
    let video_ranges = &seg.video_data_ranges;
    let audio_ranges = &seg.audio_data_ranges;

    if let Some(first_video) = video_ranges.first() {
        // On macOS, use sf_hdtr to batch the prefix with the first sendfile call.
        // On Linux, write the prefix first, then sendfile.
        #[cfg(target_os = "macos")]
        {
            let prefix_slice: &[u8] = &prefix;
            sendfile_range(
                sock_fd,
                file_fd,
                first_video.file_offset,
                first_video.length,
                Some(&[prefix_slice]),
            )?;
        }

        #[cfg(target_os = "linux")]
        {
            stream.write_all(&prefix)?;
            sendfile_range(
                sock_fd,
                file_fd,
                first_video.file_offset,
                first_video.length,
                None,
            )?;
        }

        // Remaining video ranges.
        for range in &video_ranges[1..] {
            sendfile_range(sock_fd, file_fd, range.file_offset, range.length, None)?;
        }
    } else {
        // No video ranges — just write the prefix.
        stream.write_all(&prefix)?;
    }

    // Audio ranges.
    for range in audio_ranges {
        sendfile_range(sock_fd, file_fd, range.file_offset, range.length, None)?;
    }

    // Clear TCP_NOPUSH → flush.
    let _ = set_tcp_nopush(sock_fd, false);

    Ok(())
}

/// Handle a direct play request using sendfile(2).
///
/// Path: `/api/stream/{uuid}/direct`
///
/// Serves the source file directly with Range support and zero-copy transfer.
pub fn handle_sendfile_direct(mut stream: TcpStream, ctx: &AppContext) -> io::Result<()> {
    let req = read_request_headers(&mut stream)?;

    // Authenticate.
    if validate_auth_headers(
        &ctx.config.auth,
        &ctx.db,
        req.authorization.as_deref(),
        req.cookie.as_deref(),
        req.x_emby_token.as_deref(),
    )
    .is_none()
    {
        let response = b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
        stream.write_all(response)?;
        return Ok(());
    }

    // Parse media_file_id from path: /api/stream/{uuid}/direct
    let mf_id = match parse_direct_path(&req.path) {
        Some(id) => id,
        None => {
            let response = b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
            stream.write_all(response)?;
            return Ok(());
        }
    };

    // Look up file path from DB.
    let conn = sf_db::pool::get_conn(&ctx.db)
        .map_err(|e| io::Error::other(e.to_string()))?;
    let mf = match sf_db::queries::media_files::get_media_file(&conn, mf_id) {
        Ok(Some(mf)) => mf,
        _ => {
            let response = b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
            stream.write_all(response)?;
            return Ok(());
        }
    };
    drop(conn);

    let path = std::path::Path::new(&mf.file_path);
    serve_file_sendfile(&mut stream, path, req.range)
}

/// Handle a Jellyfin video stream request using sendfile(2).
///
/// Path: `/Videos/{uuid}/stream[?mediaSourceId=...]`
///
/// The UUID in the path is an item_id. If `mediaSourceId` query param is
/// present, use that directly; otherwise take the first media file for the item.
pub fn handle_sendfile_jellyfin_stream(mut stream: TcpStream, ctx: &AppContext) -> io::Result<()> {
    let req = read_request_headers(&mut stream)?;

    // Authenticate.
    if validate_auth_headers(
        &ctx.config.auth,
        &ctx.db,
        req.authorization.as_deref(),
        req.cookie.as_deref(),
        req.x_emby_token.as_deref(),
    )
    .is_none()
    {
        let response = b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
        stream.write_all(response)?;
        return Ok(());
    }

    // Parse item_id and optional mediaSourceId from path + query.
    let (item_id, media_source_id) = match parse_jellyfin_stream_path(&req.path) {
        Some(v) => v,
        None => {
            let response = b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
            stream.write_all(response)?;
            return Ok(());
        }
    };

    let conn = sf_db::pool::get_conn(&ctx.db)
        .map_err(|e| io::Error::other(e.to_string()))?;

    // Resolve the media file.
    let mf = if let Some(ms_id) = media_source_id {
        match sf_db::queries::media_files::get_media_file(&conn, ms_id) {
            Ok(Some(mf)) => mf,
            _ => {
                let response = b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                stream.write_all(response)?;
                return Ok(());
            }
        }
    } else {
        match sf_db::queries::media_files::list_media_files_by_item(&conn, item_id) {
            Ok(files) => match files.into_iter().next() {
                Some(mf) => mf,
                None => {
                    let response = b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                    stream.write_all(response)?;
                    return Ok(());
                }
            },
            Err(_) => {
                let response = b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                stream.write_all(response)?;
                return Ok(());
            }
        }
    };
    drop(conn);

    let path = std::path::Path::new(&mf.file_path);
    serve_file_sendfile(&mut stream, path, req.range)
}

/// Serve a file via sendfile(2) with optional Range support.
///
/// Shared logic for both direct play and Jellyfin stream handlers.
fn serve_file_sendfile(
    stream: &mut TcpStream,
    path: &std::path::Path,
    range: Option<(u64, Option<u64>)>,
) -> io::Result<()> {
    let file = std::fs::File::open(path).map_err(|e| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("Failed to open {}: {e}", path.display()),
        )
    })?;

    let file_size = file.metadata()?.len();
    let content_type = guess_content_type_from_path(path);
    let file_fd = file.as_raw_fd();
    let sock_fd = stream.as_raw_fd();

    let _ = set_tcp_nopush(sock_fd, true);

    match range {
        Some((start, end_opt)) => {
            let end = end_opt.unwrap_or(file_size - 1).min(file_size - 1);
            if start > end || start >= file_size {
                let response = format!(
                    "HTTP/1.1 416 Range Not Satisfiable\r\n\
                     Content-Range: bytes */{file_size}\r\n\
                     Content-Length: 0\r\n\
                     Connection: close\r\n\
                     \r\n"
                );
                stream.write_all(response.as_bytes())?;
                let _ = set_tcp_nopush(sock_fd, false);
                return Ok(());
            }

            let length = end - start + 1;
            let response_headers = format!(
                "HTTP/1.1 206 Partial Content\r\n\
                 Content-Type: {content_type}\r\n\
                 Content-Range: bytes {start}-{end}/{file_size}\r\n\
                 Content-Length: {length}\r\n\
                 Accept-Ranges: bytes\r\n\
                 Connection: close\r\n\
                 \r\n"
            );
            stream.write_all(response_headers.as_bytes())?;
            sendfile_range(sock_fd, file_fd, start, length, None)?;
        }
        None => {
            let response_headers = format!(
                "HTTP/1.1 200 OK\r\n\
                 Content-Type: {content_type}\r\n\
                 Content-Length: {file_size}\r\n\
                 Accept-Ranges: bytes\r\n\
                 Connection: close\r\n\
                 \r\n"
            );
            stream.write_all(response_headers.as_bytes())?;
            sendfile_range(sock_fd, file_fd, 0, file_size, None)?;
        }
    }

    let _ = set_tcp_nopush(sock_fd, false);
    Ok(())
}

/// Parse a direct play path like `/api/stream/{uuid}/direct`.
fn parse_direct_path(path: &str) -> Option<sf_core::MediaFileId> {
    let rest = path.strip_prefix("/api/stream/")?;
    let slash = rest.find('/')?;
    let uuid_str = &rest[..slash];
    let suffix = &rest[slash + 1..];

    if suffix != "direct" {
        return None;
    }

    uuid_str.parse().ok()
}

/// Parse a Jellyfin stream path like `/Videos/{uuid}/stream[?mediaSourceId=...]`.
///
/// Returns `(ItemId, Option<MediaFileId>)`.
fn parse_jellyfin_stream_path(
    path: &str,
) -> Option<(sf_core::ItemId, Option<sf_core::MediaFileId>)> {
    // Split path from query string.
    let (path_part, query) = match path.split_once('?') {
        Some((p, q)) => (p, Some(q)),
        None => (path, None),
    };

    let rest = path_part.strip_prefix("/Videos/")?;
    let slash = rest.find('/')?;
    let uuid_str = &rest[..slash];
    let suffix = &rest[slash + 1..];

    if suffix != "stream" {
        return None;
    }

    let item_id: sf_core::ItemId = uuid_str.parse().ok()?;

    // Parse mediaSourceId from query string if present.
    let media_source_id = query.and_then(|q| {
        for param in q.split('&') {
            if let Some(value) = param.strip_prefix("mediaSourceId=")
                .or_else(|| param.strip_prefix("MediaSourceId="))
            {
                return value.parse::<sf_core::MediaFileId>().ok();
            }
        }
        None
    });

    Some((item_id, media_source_id))
}

/// Blocking populate of HLS cache for the sendfile path.
///
/// Looks up the media file in DB, parses moov, builds PreparedMedia, and
/// inserts into the cache. Returns the cached Arc on success.
fn populate_hls_cache_blocking(
    ctx: &AppContext,
    mf_id: sf_core::MediaFileId,
) -> io::Result<std::sync::Arc<sf_media::PreparedMedia>> {
    let conn = sf_db::pool::get_conn(&ctx.db)
        .map_err(|e| io::Error::other(e.to_string()))?;
    let mf = sf_db::queries::media_files::get_media_file(&conn, mf_id)
        .map_err(|e| io::Error::other(e.to_string()))?
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "media file not found"))?;
    drop(conn);

    let path = std::path::Path::new(&mf.file_path);
    let file = std::fs::File::open(path)?;
    let mut reader = std::io::BufReader::new(file);

    let metadata = sf_media::parse_moov(&mut reader)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
    let prepared = sf_media::build_prepared_media(&metadata, path)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

    let prepared = std::sync::Arc::new(prepared);
    ctx.hls_cache.insert(mf_id, prepared.clone());
    Ok(prepared)
}

/// Parse a segment path like `/api/stream/{uuid}/segment_{N}.m4s`.
///
/// Returns `(MediaFileId, segment_index)` or `None` on failure.
fn parse_segment_path(path: &str) -> Option<(sf_core::MediaFileId, usize)> {
    let rest = path.strip_prefix("/api/stream/")?;
    let slash = rest.find('/')?;
    let uuid_str = &rest[..slash];
    let segment_part = &rest[slash + 1..];

    let mf_id: sf_core::MediaFileId = uuid_str.parse().ok()?;

    let inner = segment_part.strip_prefix("segment_")?;
    let num_str = inner.strip_suffix(".m4s")?;
    let index: usize = num_str.parse().ok()?;

    Some((mf_id, index))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peek_matches_valid_segment() {
        let buf = b"GET /api/stream/550e8400-e29b-41d4-a716-446655440000/segment_0.m4s HTTP/1.1\r\nHost: localhost\r\n\r\n";
        assert!(is_segment_request(buf));
    }

    #[test]
    fn peek_matches_high_index() {
        let buf = b"GET /api/stream/550e8400-e29b-41d4-a716-446655440000/segment_42.m4s HTTP/1.1\r\n";
        assert!(is_segment_request(buf));
    }

    #[test]
    fn peek_rejects_init_mp4() {
        let buf = b"GET /api/stream/550e8400-e29b-41d4-a716-446655440000/init.mp4 HTTP/1.1\r\n";
        assert!(!is_segment_request(buf));
    }

    #[test]
    fn peek_rejects_playlist() {
        let buf = b"GET /api/stream/550e8400-e29b-41d4-a716-446655440000/index.m3u8 HTTP/1.1\r\n";
        assert!(!is_segment_request(buf));
    }

    #[test]
    fn peek_rejects_direct() {
        let buf = b"GET /api/stream/550e8400-e29b-41d4-a716-446655440000/direct HTTP/1.1\r\n";
        assert!(!is_segment_request(buf));
    }

    #[test]
    fn peek_rejects_post() {
        let buf = b"POST /api/stream/550e8400-e29b-41d4-a716-446655440000/segment_0.m4s HTTP/1.1\r\n";
        assert!(!is_segment_request(buf));
    }

    #[test]
    fn peek_rejects_other_path() {
        let buf = b"GET /api/items HTTP/1.1\r\n";
        assert!(!is_segment_request(buf));
    }

    #[test]
    fn peek_rejects_short_buffer() {
        let buf = b"GET /";
        assert!(!is_segment_request(buf));
    }

    #[test]
    fn peek_rejects_bad_uuid_length() {
        let buf = b"GET /api/stream/not-a-uuid/segment_0.m4s HTTP/1.1\r\n";
        assert!(!is_segment_request(buf));
    }

    #[test]
    fn parse_segment_path_valid() {
        let (id, idx) = parse_segment_path(
            "/api/stream/550e8400-e29b-41d4-a716-446655440000/segment_5.m4s",
        )
        .unwrap();
        assert_eq!(id.to_string(), "550e8400-e29b-41d4-a716-446655440000");
        assert_eq!(idx, 5);
    }

    #[test]
    fn parse_segment_path_rejects_non_segment() {
        assert!(parse_segment_path("/api/stream/550e8400-e29b-41d4-a716-446655440000/init.mp4").is_none());
    }

    #[test]
    fn parse_segment_path_rejects_bad_index() {
        assert!(
            parse_segment_path("/api/stream/550e8400-e29b-41d4-a716-446655440000/segment_abc.m4s")
                .is_none()
        );
    }

    // -- Direct play peek tests --

    #[test]
    fn peek_matches_direct_request() {
        let buf = b"GET /api/stream/550e8400-e29b-41d4-a716-446655440000/direct HTTP/1.1\r\nHost: localhost\r\n\r\n";
        assert!(is_direct_request(buf));
    }

    #[test]
    fn peek_rejects_segment_as_direct() {
        let buf = b"GET /api/stream/550e8400-e29b-41d4-a716-446655440000/segment_0.m4s HTTP/1.1\r\n";
        assert!(!is_direct_request(buf));
    }

    #[test]
    fn peek_rejects_post_as_direct() {
        let buf = b"POST /api/stream/550e8400-e29b-41d4-a716-446655440000/direct HTTP/1.1\r\n";
        assert!(!is_direct_request(buf));
    }

    // -- Jellyfin stream peek tests --

    #[test]
    fn peek_matches_jellyfin_stream() {
        let buf = b"GET /Videos/550e8400-e29b-41d4-a716-446655440000/stream HTTP/1.1\r\nHost: localhost\r\n\r\n";
        assert!(is_jellyfin_stream_request(buf));
    }

    #[test]
    fn peek_matches_jellyfin_stream_with_query() {
        let buf = b"GET /Videos/550e8400-e29b-41d4-a716-446655440000/stream?mediaSourceId=abc HTTP/1.1\r\n";
        assert!(is_jellyfin_stream_request(buf));
    }

    #[test]
    fn peek_rejects_jellyfin_master_playlist() {
        let buf = b"GET /Videos/550e8400-e29b-41d4-a716-446655440000/master.m3u8 HTTP/1.1\r\n";
        assert!(!is_jellyfin_stream_request(buf));
    }

    #[test]
    fn peek_rejects_jellyfin_bad_uuid() {
        let buf = b"GET /Videos/not-a-uuid/stream HTTP/1.1\r\n";
        assert!(!is_jellyfin_stream_request(buf));
    }

    // -- Path parser tests --

    #[test]
    fn parse_direct_path_valid() {
        let id = parse_direct_path("/api/stream/550e8400-e29b-41d4-a716-446655440000/direct").unwrap();
        assert_eq!(id.to_string(), "550e8400-e29b-41d4-a716-446655440000");
    }

    #[test]
    fn parse_direct_path_rejects_segment() {
        assert!(parse_direct_path("/api/stream/550e8400-e29b-41d4-a716-446655440000/segment_0.m4s").is_none());
    }

    #[test]
    fn parse_jellyfin_stream_path_valid() {
        let (item_id, ms_id) = parse_jellyfin_stream_path(
            "/Videos/550e8400-e29b-41d4-a716-446655440000/stream",
        )
        .unwrap();
        assert_eq!(item_id.to_string(), "550e8400-e29b-41d4-a716-446655440000");
        assert!(ms_id.is_none());
    }

    #[test]
    fn parse_jellyfin_stream_path_with_media_source() {
        let (item_id, ms_id) = parse_jellyfin_stream_path(
            "/Videos/550e8400-e29b-41d4-a716-446655440000/stream?mediaSourceId=660e8400-e29b-41d4-a716-446655440001",
        )
        .unwrap();
        assert_eq!(item_id.to_string(), "550e8400-e29b-41d4-a716-446655440000");
        assert_eq!(ms_id.unwrap().to_string(), "660e8400-e29b-41d4-a716-446655440001");
    }

    #[test]
    fn parse_jellyfin_stream_path_rejects_master() {
        assert!(parse_jellyfin_stream_path("/Videos/550e8400-e29b-41d4-a716-446655440000/master.m3u8").is_none());
    }

    // -- Range value parser tests --

    #[test]
    fn parse_range_value_full() {
        let (start, end) = parse_range_value("bytes=0-999").unwrap();
        assert_eq!(start, 0);
        assert_eq!(end, Some(999));
    }

    #[test]
    fn parse_range_value_open_end() {
        let (start, end) = parse_range_value("bytes=500-").unwrap();
        assert_eq!(start, 500);
        assert_eq!(end, None);
    }

    #[test]
    fn parse_range_value_invalid() {
        assert!(parse_range_value("invalid").is_none());
    }
}
