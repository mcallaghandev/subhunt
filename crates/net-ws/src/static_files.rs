//! A deliberately tiny static file server: just enough to deliver the web client.

use std::io::Write;
use std::net::TcpStream;
use std::path::Path;

/// Answers one plain HTTP request and closes the connection.
pub fn serve(mut stream: TcpStream, headers: &str, site: Option<&Path>) {
    // We only peeked at the request, so read it off the socket now. GET
    // requests have no body, so the headers are all there is.
    let mut discard = vec![0u8; headers.len()];
    let _ = std::io::Read::read_exact(&mut stream, &mut discard);

    let response = match (request_path(headers), site) {
        (Some(path), Some(site)) => {
            // Prefer a pre-compressed `.gz` copy if the browser understands gzip.
            let gzipped = accepts_gzip(headers).then(|| load(site, &format!("{path}.gz"))).flatten();
            match (gzipped, load(site, &path)) {
                (Some((body, _)), Some((_, content_type))) => response(200, "OK", content_type, Some("gzip"), &body),
                (_, Some((body, content_type))) => response(200, "OK", content_type, None, &body),
                _ => response(404, "Not Found", "text/plain", None, b"Not found"),
            }
        }
        (Some(_), None) => response(404, "Not Found", "text/plain", None, b"This server has no web client configured."),
        (None, _) => response(400, "Bad Request", "text/plain", None, b"Bad request"),
    };
    let _ = stream.write_all(&response);
}

fn accepts_gzip(headers: &str) -> bool {
    headers.lines().any(|line| {
        let line = line.to_ascii_lowercase();
        line.starts_with("accept-encoding:") && line.contains("gzip")
    })
}

/// Pulls the path out of a request line like `GET /index.html HTTP/1.1`.
fn request_path(headers: &str) -> Option<String> {
    let mut parts = headers.lines().next()?.split_whitespace();
    if parts.next()? != "GET" {
        return None;
    }
    let path = parts.next()?.split(['?', '#']).next()?;
    let path = if path == "/" { "/index.html" } else { path };
    Some(path.to_string())
}

/// Reads a file from inside `site`, refusing anything that tries to escape it.
fn load(site: &Path, path: &str) -> Option<(Vec<u8>, &'static str)> {
    let relative = path.trim_start_matches('/');
    // Never let a request like `/../../etc/passwd` climb out of the site folder.
    if relative.split('/').any(|part| part == ".." || part.is_empty()) || relative.contains('\\') {
        return None;
    }
    let file = site.join(relative);
    let body = std::fs::read(&file).ok()?;
    Some((body, content_type(&file)))
}

fn content_type(file: &Path) -> &'static str {
    match file.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript",
        Some("wasm") => "application/wasm",
        Some("css") => "text/css",
        Some("png") => "image/png",
        Some("ico") => "image/x-icon",
        Some("json") => "application/json",
        _ => "application/octet-stream",
    }
}

fn response(status: u16, reason: &str, content_type: &str, encoding: Option<&str>, body: &[u8]) -> Vec<u8> {
    let encoding = encoding.map(|e| format!("Content-Encoding: {e}\r\n")).unwrap_or_default();
    let mut out = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\n{encoding}Content-Length: {}\r\n\
         Cache-Control: no-cache\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    out.extend_from_slice(body);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_maps_to_index_and_queries_are_ignored() {
        assert_eq!(request_path("GET / HTTP/1.1\r\n\r\n").as_deref(), Some("/index.html"));
        assert_eq!(request_path("GET /pkg/a.js?v=2 HTTP/1.1\r\n\r\n").as_deref(), Some("/pkg/a.js"));
        assert_eq!(request_path("POST / HTTP/1.1\r\n\r\n"), None);
    }

    #[test]
    fn detects_gzip_support() {
        assert!(accepts_gzip("GET / HTTP/1.1\r\nAccept-Encoding: gzip, br\r\n\r\n"));
        assert!(!accepts_gzip("GET / HTTP/1.1\r\nHost: x\r\n\r\n"));
    }

    #[test]
    fn cannot_escape_the_site_folder() {
        let site = Path::new(env!("CARGO_MANIFEST_DIR"));
        assert!(load(site, "/Cargo.toml").is_some());
        assert!(load(site, "/../Cargo.toml").is_none());
        assert!(load(site, "/src/../Cargo.toml").is_none());
        assert!(load(site, "//etc/passwd").is_none());
    }
}
