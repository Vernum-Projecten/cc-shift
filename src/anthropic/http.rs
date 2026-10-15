//! HTTP requests through the system `curl`.
//!
//! The whole request, the bearer token and the body included, goes to curl
//! as a config file on stdin (`curl -K -`,
//! <https://curl.se/docs/manpage.html#-K>), so no secret is ever a
//! command-line argument that `ps` could show. Every request goes through
//! [`Http`], so tests use a fake.

use std::io::Write;
use std::process::{Command, Stdio};

use crate::error::Error;

/// How long one request may take, in seconds.
const MAX_SECONDS: u32 = 15;

/// One HTTP request.
#[derive(Clone, PartialEq, Eq)]
pub struct Request {
    /// `GET` or `POST`.
    pub method: &'static str,
    /// The full URL.
    pub url: String,
    /// Header lines, `Name: value`.
    pub headers: Vec<String>,
    /// The body, if any.
    pub body: Option<String>,
}

impl std::fmt::Debug for Request {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Request")
            .field("method", &self.method)
            .field("url", &self.url)
            .finish_non_exhaustive()
    }
}

/// The status and body of a response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    /// The HTTP status code.
    pub status: u16,
    /// The response body.
    pub body: Vec<u8>,
}

/// Sends HTTP requests.
pub trait Http {
    /// Sends `request` and returns the response, whatever its status.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Http`] when no response arrives.
    fn send(&self, request: &Request) -> Result<Response, Error>;
}

/// The system `curl`.
#[derive(Debug, Clone, Copy, Default)]
pub struct Curl;

impl Http for Curl {
    fn send(&self, request: &Request) -> Result<Response, Error> {
        let fail =
            |what: String| Error::Http(format!("{} {}: {what}", request.method, request.url));
        let mut child = Command::new("/usr/bin/curl")
            .args(["--config", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| fail(format!("cannot run curl: {e}")))?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(config(request).as_bytes())
                .map_err(|e| fail(format!("cannot write to curl: {e}")))?;
        }
        let out = child
            .wait_with_output()
            .map_err(|e| fail(format!("curl did not finish: {e}")))?;
        if !out.status.success() {
            return Err(fail(String::from_utf8_lossy(&out.stderr).trim().to_owned()));
        }
        split_status(out.stdout).ok_or_else(|| fail("no status code".to_owned()))
    }
}

/// Returns the curl config for a request. The status code is written after
/// the body on a line of its own.
#[must_use]
pub fn config(request: &Request) -> String {
    let mut lines = vec![
        "silent".to_owned(),
        "show-error".to_owned(),
        format!("max-time = {MAX_SECONDS}"),
        format!("request = {}", quoted(request.method)),
        format!("url = {}", quoted(&request.url)),
        format!("write-out = {}", quoted("\n%{http_code}")),
    ];
    lines.extend(
        request
            .headers
            .iter()
            .map(|h| format!("header = {}", quoted(h))),
    );
    if let Some(body) = &request.body {
        lines.push(format!("data-raw = {}", quoted(body)));
    }
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

/// Quotes a value for a curl config file: double quotes, with `\` and `"`
/// escaped and line breaks written as `\n` and `\r`.
fn quoted(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

/// Splits curl's output into the body and the status code on its last line.
fn split_status(mut output: Vec<u8>) -> Option<Response> {
    let at = output.iter().rposition(|b| *b == b'\n')?;
    let status = std::str::from_utf8(output.get(at + 1..)?)
        .ok()?
        .trim()
        .parse()
        .ok()?;
    output.truncate(at);
    Some(Response {
        status,
        body: output,
    })
}

#[cfg(test)]
mod tests {
    use super::{Request, config, split_status};

    #[test]
    fn the_config_carries_the_whole_request() {
        let request = Request {
            method: "POST",
            url: "https://example.org/token".to_owned(),
            headers: vec!["Content-Type: application/json".to_owned()],
            body: Some(r#"{"a":"b\c"}"#.to_owned()),
        };
        assert_eq!(
            config(&request),
            "silent\nshow-error\nmax-time = 15\nrequest = \"POST\"\n\
             url = \"https://example.org/token\"\nwrite-out = \"\\n%{http_code}\"\n\
             header = \"Content-Type: application/json\"\n\
             data-raw = \"{\\\"a\\\":\\\"b\\\\c\\\"}\"\n"
        );
    }

    #[test]
    fn the_status_is_split_from_the_body() {
        let response = split_status(b"{\"ok\":true}\n200".to_vec());
        assert_eq!(
            response.map(|r| (r.status, r.body)),
            Some((200, b"{\"ok\":true}".to_vec()))
        );
    }

    #[test]
    fn debug_output_never_shows_headers_or_body() {
        let request = Request {
            method: "GET",
            url: "https://example.org".to_owned(),
            headers: vec!["Authorization: Bearer secret".to_owned()],
            body: Some("secret".to_owned()),
        };
        assert!(!format!("{request:?}").contains("secret"), "{request:?}");
    }
}
