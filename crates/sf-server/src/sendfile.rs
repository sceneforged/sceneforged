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

use dashmap::mapref::entry::Entry;

use crate::context::AppContext;
use crate::middleware::auth::validate_auth_headers;

// ---------------------------------------------------------------------------
// Peek classification
// ---------------------------------------------------------------------------

/// Pre-parsed routing information extracted from a TCP peek buffer.
///
/// Avoids double-parsing by extracting IDs and indices during the initial
/// peek classification, then passing them through to the sendfile handler.
#[derive(Debug)]
pub enum PeekRoute {
    /// HLS segment: `/api/stream/{mf_id}/segment_{index}.m4s`
    Segment {
        mf_id: sf_core::MediaFileId,
        index: usize,
    },
    /// Direct play: `/api/stream/{mf_id}/direct`
    Direct {
        mf_id: sf_core::MediaFileId,
    },
    /// Jellyfin stream: `/Videos/{item_id}/stream[?...]`
    JellyfinStream {
        item_id: sf_core::ItemId,
    },
}

/// Classify a peeked HTTP request buffer into a sendfile route.
///
/// Returns `Some(route)` if the request can be served via sendfile, `None`
/// if it should fall through to the normal Axum/hyper pipeline.
pub fn classify_peek(peek_buf: &[u8]) -> Option<PeekRoute> {
    let path = extract_get_path(peek_buf)?;
    classify_path(path)
}

/// Classify a request path into a sendfile route.
///
/// Shared by both the peek classifier (initial connection) and the keep-alive
/// loop (subsequent requests on the same connection).
fn classify_path(path: &str) -> Option<PeekRoute> {
    // Try /api/stream/{uuid}/...
    if let Some(rest) = path.strip_prefix("/api/stream/") {
        let slash = rest.find('/')?;
        let uuid_part = &rest[..slash];
        let suffix = &rest[slash + 1..];

        if uuid_part.len() != 36 {
            return None;
        }

        if suffix == "direct" {
            let mf_id = uuid_part.parse().ok()?;
            return Some(PeekRoute::Direct { mf_id });
        }

        if let Some(inner) = suffix.strip_prefix("segment_") {
            if let Some(num_str) = inner.strip_suffix(".m4s") {
                let mf_id = uuid_part.parse().ok()?;
                let index = num_str.parse().ok()?;
                return Some(PeekRoute::Segment { mf_id, index });
            }
        }

        return None;
    }

    // Try /Videos/{uuid}/stream (Jellyfin direct stream).
    let path_no_query = path.split('?').next().unwrap_or(path);
    if let Some(rest) = path_no_query.strip_prefix("/Videos/") {
        let slash = rest.find('/')?;
        let uuid_part = &rest[..slash];
        let suffix = &rest[slash + 1..];

        if uuid_part.len() == 36 && suffix == "stream" {
            let item_id = uuid_part.parse().ok()?;
            return Some(PeekRoute::JellyfinStream { item_id });
        }
    }

    None
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
    let mut tmp = [0u8; 4096];

    // Read until we find \r\n\r\n (end of headers).
    loop {
        let n = match stream.read(&mut tmp) {
            Ok(0) => return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "connection closed")),
            Ok(n) => n,
            Err(e) => return Err(e),
        };
        buf.extend_from_slice(&tmp[..n]);
        if buf.len() >= 4 {
            let scan_start = buf.len().saturating_sub(n + 3);
            if buf[scan_start..].windows(4).any(|w| w == b"\r\n\r\n") {
                break;
            }
        }
        // Safety limit: 8KB of headers.
        if buf.len() > 8192 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "headers too large",
            ));
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
            if err.kind() == io::ErrorKind::WouldBlock {
                // Send buffer full — adjust for any partial send and retry.
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
                if bytes_sent == 0 {
                    // No progress — back off briefly to let the client drain.
                    std::thread::sleep(std::time::Duration::from_millis(1));
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
            if err.kind() == io::ErrorKind::WouldBlock {
                // Send buffer full — back off briefly and retry.
                std::thread::sleep(std::time::Duration::from_millis(1));
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

/// Handle a sendfile-routed request with HTTP keep-alive support.
///
/// Called from `spawn_blocking` with a std `TcpStream` and the pre-parsed
/// [`PeekRoute`]. Reads the full HTTP headers, validates auth once, then
/// dispatches to the appropriate serve function. After the first response,
/// loops to serve additional requests on the same connection (keep-alive),
/// saving TCP setup, auth, and file-open overhead per subsequent request.
pub fn handle_sendfile(mut stream: TcpStream, ctx: &AppContext, route: PeekRoute) -> io::Result<()> {
    let req = read_request_headers(&mut stream)?;

    // Authenticate once for the entire connection.
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

    // Serve the initial request.
    dispatch_route(&mut stream, ctx, &route, &req)?;

    // Keep-alive loop: try to serve more requests on the same connection.
    // Use a 15-second idle timeout between requests.
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(15)));

    while let Ok(next_req) = read_request_headers(&mut stream) {
        if let Some(next_route) = classify_path(&next_req.path) {
            dispatch_route(&mut stream, ctx, &next_route, &next_req)?;
        } else {
            break; // Non-sendfile route — can't handle it, close.
        }
    }

    Ok(())
}

/// Dispatch a classified request to the appropriate serve function.
fn dispatch_route(
    stream: &mut TcpStream,
    ctx: &AppContext,
    route: &PeekRoute,
    req: &ParsedRequest,
) -> io::Result<()> {
    tracing::debug!(route = ?route, range = ?req.range, "sendfile request");
    match *route {
        PeekRoute::Segment { mf_id, index } => serve_segment(stream, ctx, mf_id, index),
        PeekRoute::Direct { mf_id } => serve_direct(stream, ctx, mf_id, req.range),
        PeekRoute::JellyfinStream { item_id } => {
            serve_jellyfin_stream(stream, ctx, item_id, &req.path, req.range)
        }
    }
}

/// Serve an HLS segment via sendfile(2).
fn serve_segment(
    stream: &mut TcpStream,
    ctx: &AppContext,
    mf_id: sf_core::MediaFileId,
    seg_index: usize,
) -> io::Result<()> {
    // Look up prepared media in HLS cache, populating on demand if missing.
    // Uses request coalescing so concurrent segment requests for the same
    // uncached file don't trigger redundant moov parses.
    let prepared = match populate_hls_cache_blocking(ctx, mf_id) {
        Ok(p) => p,
        Err(_) => {
            let response = b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
            stream.write_all(response)?;
            return Ok(());
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
         Connection: keep-alive\r\n\
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

/// Serve a file directly via sendfile(2) with Range support.
fn serve_direct(
    stream: &mut TcpStream,
    ctx: &AppContext,
    mf_id: sf_core::MediaFileId,
    range: Option<(u64, Option<u64>)>,
) -> io::Result<()> {
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
    serve_file_sendfile(stream, path, range)
}

/// Serve a Jellyfin `/Videos/{id}/stream` request via sendfile(2).
fn serve_jellyfin_stream(
    stream: &mut TcpStream,
    ctx: &AppContext,
    item_id: sf_core::ItemId,
    request_path: &str,
    range: Option<(u64, Option<u64>)>,
) -> io::Result<()> {
    let media_source_id = extract_media_source_id(request_path);

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
    serve_file_sendfile(stream, path, range)
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

    let start_time = std::time::Instant::now();

    let bytes_sent = match range {
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
                 Connection: keep-alive\r\n\
                 \r\n"
            );
            stream.write_all(response_headers.as_bytes())?;
            sendfile_range(sock_fd, file_fd, start, length, None)?;
            length
        }
        None => {
            let response_headers = format!(
                "HTTP/1.1 200 OK\r\n\
                 Content-Type: {content_type}\r\n\
                 Content-Length: {file_size}\r\n\
                 Accept-Ranges: bytes\r\n\
                 Connection: keep-alive\r\n\
                 \r\n"
            );
            stream.write_all(response_headers.as_bytes())?;
            sendfile_range(sock_fd, file_fd, 0, file_size, None)?;
            file_size
        }
    };

    let _ = set_tcp_nopush(sock_fd, false);

    let elapsed = start_time.elapsed();
    let mbps = if elapsed.as_secs_f64() > 0.0 {
        (bytes_sent as f64 * 8.0) / (elapsed.as_secs_f64() * 1_000_000.0)
    } else {
        0.0
    };
    tracing::debug!(
        bytes = bytes_sent,
        elapsed_ms = elapsed.as_millis() as u64,
        mbps = format_args!("{mbps:.1}"),
        path = %path.display(),
        "sendfile transfer complete"
    );

    Ok(())
}

/// Extract `mediaSourceId` query parameter from a request path.
fn extract_media_source_id(path: &str) -> Option<sf_core::MediaFileId> {
    let (_, query) = path.split_once('?')?;
    for param in query.split('&') {
        if let Some(value) = param
            .strip_prefix("mediaSourceId=")
            .or_else(|| param.strip_prefix("MediaSourceId="))
        {
            return value.parse().ok();
        }
    }
    None
}

/// Blocking populate of HLS cache for the sendfile path with request coalescing.
///
/// Uses `ctx.hls_loading` DashMap to ensure only one thread performs the
/// moov parse for a given media file. Other threads poll until the cache
/// is populated or the loader finishes (then retry).
fn populate_hls_cache_blocking(
    ctx: &AppContext,
    mf_id: sf_core::MediaFileId,
) -> io::Result<std::sync::Arc<sf_media::PreparedMedia>> {
    // Fast path: already cached — touch timestamp for LRU.
    if let Some(mut entry) = ctx.hls_cache.get_mut(&mf_id) {
        entry.1 = std::time::Instant::now();
        return Ok(entry.0.clone());
    }

    loop {
        match ctx.hls_loading.entry(mf_id) {
            Entry::Vacant(e) => {
                // We're the loader.
                let notify = std::sync::Arc::new(tokio::sync::Notify::new());
                e.insert(notify.clone());

                let result = do_populate_blocking(ctx, mf_id);

                // Always cleanup + wake waiters.
                ctx.hls_loading.remove(&mf_id);
                notify.notify_waiters();

                return result;
            }
            Entry::Occupied(e) => {
                // Another task is loading — drop the entry ref and poll.
                drop(e);
            }
        }

        // Poll: wait for cache to be populated or loader to finish.
        for _ in 0..400 {
            std::thread::sleep(std::time::Duration::from_millis(1));
            if let Some(mut entry) = ctx.hls_cache.get_mut(&mf_id) {
                entry.1 = std::time::Instant::now();
                return Ok(entry.0.clone());
            }
            if !ctx.hls_loading.contains_key(&mf_id) {
                break; // Loader finished (possibly with error) — retry the loop.
            }
        }
        // If we exit the poll loop, either the loader failed or timed out.
        // Re-enter the outer loop to try becoming the loader ourselves.
    }
}

/// Three-tier DB lookup + moov parse + cache insert (blocking path).
fn do_populate_blocking(
    ctx: &AppContext,
    mf_id: sf_core::MediaFileId,
) -> io::Result<std::sync::Arc<sf_media::PreparedMedia>> {
    let conn = sf_db::pool::get_conn(&ctx.db)
        .map_err(|e| io::Error::other(e.to_string()))?;
    let mf = sf_db::queries::media_files::get_media_file(&conn, mf_id)
        .map_err(|e| io::Error::other(e.to_string()))?
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "media file not found"))?;

    // --- Tier 2: DB blob ---
    if let Ok(Some(blob)) = sf_db::queries::media_files::get_hls_prepared(&conn, mf_id) {
        drop(conn);
        if let Ok(mut prepared) = sf_media::PreparedMedia::from_bincode(&blob) {
            prepared.file_path = std::path::PathBuf::from(&mf.file_path);
            let prepared = std::sync::Arc::new(prepared);
            ctx.hls_cache.insert(mf_id, (prepared.clone(), std::time::Instant::now()));
            tracing::debug!(media_file_id = %mf_id, "HLS cache loaded from DB (sendfile)");
            return Ok(prepared);
        }
    } else {
        drop(conn);
    }

    // --- Tier 3: moov parse ---
    let path = std::path::Path::new(&mf.file_path);
    let file = std::fs::File::open(path)?;
    let mut reader = std::io::BufReader::new(file);

    let metadata = sf_media::parse_moov(&mut reader)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
    let prepared = sf_media::build_prepared_media(&metadata, path)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

    // Persist to DB for next time.
    if let Ok(blob) = prepared.to_bincode() {
        if let Ok(conn) = sf_db::pool::get_conn(&ctx.db) {
            let _ = sf_db::queries::media_files::set_hls_prepared(&conn, mf_id, &blob);
        }
    }

    let prepared = std::sync::Arc::new(prepared);
    ctx.hls_cache.insert(mf_id, (prepared.clone(), std::time::Instant::now()));
    Ok(prepared)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Segment classification --

    #[test]
    fn peek_classifies_segment() {
        let buf = b"GET /api/stream/550e8400-e29b-41d4-a716-446655440000/segment_0.m4s HTTP/1.1\r\nHost: localhost\r\n\r\n";
        match classify_peek(buf) {
            Some(PeekRoute::Segment { mf_id, index }) => {
                assert_eq!(mf_id.to_string(), "550e8400-e29b-41d4-a716-446655440000");
                assert_eq!(index, 0);
            }
            other => panic!("Expected Segment, got {other:?}"),
        }
    }

    #[test]
    fn peek_classifies_segment_high_index() {
        let buf = b"GET /api/stream/550e8400-e29b-41d4-a716-446655440000/segment_42.m4s HTTP/1.1\r\n";
        match classify_peek(buf) {
            Some(PeekRoute::Segment { index, .. }) => assert_eq!(index, 42),
            other => panic!("Expected Segment, got {other:?}"),
        }
    }

    // -- Direct classification --

    #[test]
    fn peek_classifies_direct() {
        let buf = b"GET /api/stream/550e8400-e29b-41d4-a716-446655440000/direct HTTP/1.1\r\nHost: localhost\r\n\r\n";
        match classify_peek(buf) {
            Some(PeekRoute::Direct { mf_id }) => {
                assert_eq!(mf_id.to_string(), "550e8400-e29b-41d4-a716-446655440000");
            }
            other => panic!("Expected Direct, got {other:?}"),
        }
    }

    // -- Jellyfin classification --

    #[test]
    fn peek_classifies_jellyfin_stream() {
        let buf = b"GET /Videos/550e8400-e29b-41d4-a716-446655440000/stream HTTP/1.1\r\nHost: localhost\r\n\r\n";
        match classify_peek(buf) {
            Some(PeekRoute::JellyfinStream { item_id }) => {
                assert_eq!(item_id.to_string(), "550e8400-e29b-41d4-a716-446655440000");
            }
            other => panic!("Expected JellyfinStream, got {other:?}"),
        }
    }

    #[test]
    fn peek_classifies_jellyfin_stream_with_query() {
        let buf = b"GET /Videos/550e8400-e29b-41d4-a716-446655440000/stream?mediaSourceId=abc HTTP/1.1\r\n";
        assert!(matches!(classify_peek(buf), Some(PeekRoute::JellyfinStream { .. })));
    }

    // -- Rejection tests (must return None) --

    #[test]
    fn peek_rejects_init_mp4() {
        let buf = b"GET /api/stream/550e8400-e29b-41d4-a716-446655440000/init.mp4 HTTP/1.1\r\n";
        assert!(classify_peek(buf).is_none());
    }

    #[test]
    fn peek_rejects_playlist() {
        let buf = b"GET /api/stream/550e8400-e29b-41d4-a716-446655440000/index.m3u8 HTTP/1.1\r\n";
        assert!(classify_peek(buf).is_none());
    }

    #[test]
    fn peek_rejects_post() {
        let buf = b"POST /api/stream/550e8400-e29b-41d4-a716-446655440000/segment_0.m4s HTTP/1.1\r\n";
        assert!(classify_peek(buf).is_none());
    }

    #[test]
    fn peek_rejects_other_path() {
        let buf = b"GET /api/items HTTP/1.1\r\n";
        assert!(classify_peek(buf).is_none());
    }

    #[test]
    fn peek_rejects_short_buffer() {
        let buf = b"GET /";
        assert!(classify_peek(buf).is_none());
    }

    #[test]
    fn peek_rejects_bad_uuid_length() {
        let buf = b"GET /api/stream/not-a-uuid/segment_0.m4s HTTP/1.1\r\n";
        assert!(classify_peek(buf).is_none());
    }

    #[test]
    fn peek_rejects_bad_segment_index() {
        let buf = b"GET /api/stream/550e8400-e29b-41d4-a716-446655440000/segment_abc.m4s HTTP/1.1\r\n";
        assert!(classify_peek(buf).is_none());
    }

    #[test]
    fn peek_rejects_jellyfin_master_playlist() {
        let buf = b"GET /Videos/550e8400-e29b-41d4-a716-446655440000/master.m3u8 HTTP/1.1\r\n";
        assert!(classify_peek(buf).is_none());
    }

    #[test]
    fn peek_rejects_jellyfin_bad_uuid() {
        let buf = b"GET /Videos/not-a-uuid/stream HTTP/1.1\r\n";
        assert!(classify_peek(buf).is_none());
    }

    // -- extract_media_source_id --

    #[test]
    fn extract_media_source_id_present() {
        let id = extract_media_source_id(
            "/Videos/550e8400-e29b-41d4-a716-446655440000/stream?mediaSourceId=660e8400-e29b-41d4-a716-446655440001",
        ).unwrap();
        assert_eq!(id.to_string(), "660e8400-e29b-41d4-a716-446655440001");
    }

    #[test]
    fn extract_media_source_id_absent() {
        assert!(extract_media_source_id("/Videos/550e8400-e29b-41d4-a716-446655440000/stream").is_none());
    }

    // -- Range value parser --

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

    // -- sendfile EAGAIN regression tests --
    //
    // These verify that sendfile_range completes successfully even when the
    // socket is non-blocking (which triggers EAGAIN when the send buffer
    // fills up).  Before the fix, EAGAIN with bytes_sent==0 caused an
    // immediate error return and a silent connection drop.

    /// Helper: set SO_SNDBUF to a small value to make EAGAIN more likely.
    #[cfg(unix)]
    fn set_small_sndbuf(fd: RawFd) {
        let size: libc::c_int = 4096;
        unsafe {
            libc::setsockopt(
                fd,
                libc::SOL_SOCKET,
                libc::SO_SNDBUF,
                &size as *const _ as *const libc::c_void,
                std::mem::size_of::<libc::c_int>() as libc::socklen_t,
            );
        }
    }

    /// Regression: sendfile_range must succeed on a non-blocking socket by
    /// retrying on EAGAIN instead of returning an error.
    #[cfg(unix)]
    #[test]
    fn sendfile_range_handles_eagain_on_nonblocking_socket() {
        use std::os::unix::net::UnixStream;

        // 256KB of deterministic data — larger than any socket buffer.
        let data_len: usize = 256 * 1024;
        let data: Vec<u8> = (0..data_len).map(|i| (i % 251) as u8).collect();

        // Write to a temp file.
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), &data).unwrap();
        let file = std::fs::File::open(tmp.path()).unwrap();
        let file_fd = file.as_raw_fd();

        // Create a connected Unix socket pair.
        let (sender, receiver) = UnixStream::pair().unwrap();

        // Make the sender non-blocking + small buffer → guarantees EAGAIN.
        sender.set_nonblocking(true).unwrap();
        set_small_sndbuf(sender.as_raw_fd());

        let sock_fd = sender.as_raw_fd();

        // Reader thread: slowly drain from the other end.
        let handle = std::thread::spawn(move || {
            let mut received = Vec::with_capacity(data_len);
            let mut buf = [0u8; 8192];
            loop {
                match std::io::Read::read(&mut &receiver, &mut buf) {
                    Ok(0) => break,
                    Ok(n) => received.extend_from_slice(&buf[..n]),
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(e) => panic!("reader error: {e}"),
                }
            }
            received
        });

        // This is the call under test — before the fix it would fail with
        // "Resource temporarily unavailable" on the first EAGAIN.
        sendfile_range(sock_fd, file_fd, 0, data_len as u64, None)
            .expect("sendfile_range must handle EAGAIN and complete");

        // Close the sender so the reader sees EOF.
        drop(sender);
        drop(file);

        let received = handle.join().expect("reader thread panicked");
        assert_eq!(received.len(), data_len, "all bytes must be received");
        assert_eq!(received, data, "data must be intact");
    }

    /// Regression: sendfile_range with headers on a non-blocking socket.
    #[cfg(unix)]
    #[test]
    fn sendfile_range_handles_eagain_with_headers() {
        use std::os::unix::net::UnixStream;

        let data_len: usize = 128 * 1024;
        let data: Vec<u8> = (0..data_len).map(|i| (i % 199) as u8).collect();
        let header = b"HTTP/1.1 200 OK\r\nContent-Length: 131072\r\n\r\n";

        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), &data).unwrap();
        let file = std::fs::File::open(tmp.path()).unwrap();
        let file_fd = file.as_raw_fd();

        let (sender, receiver) = UnixStream::pair().unwrap();
        sender.set_nonblocking(true).unwrap();
        set_small_sndbuf(sender.as_raw_fd());
        let sock_fd = sender.as_raw_fd();

        let expected_total = header.len() + data_len;
        let handle = std::thread::spawn(move || {
            let mut received = Vec::with_capacity(expected_total);
            let mut buf = [0u8; 8192];
            loop {
                match std::io::Read::read(&mut &receiver, &mut buf) {
                    Ok(0) => break,
                    Ok(n) => received.extend_from_slice(&buf[..n]),
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(e) => panic!("reader error: {e}"),
                }
            }
            received
        });

        let hdr_slice: &[u8] = header;
        sendfile_range(sock_fd, file_fd, 0, data_len as u64, Some(&[hdr_slice]))
            .expect("sendfile_range with headers must handle EAGAIN");

        drop(sender);
        drop(file);

        let received = handle.join().expect("reader thread panicked");
        assert_eq!(received.len(), expected_total, "header + body bytes");
        assert_eq!(&received[..header.len()], &header[..]);
        assert_eq!(&received[header.len()..], &data[..]);
    }
}
