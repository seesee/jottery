//! What the stub servers in the unit tests share.

use std::io::Read;
use std::net::TcpStream;

/// An HTTP request as a stub server received it.
pub struct Request {
    pub method: String,
    pub path: String,
    pub body: Vec<u8>,
    headers: Vec<(String, String)>,
}

impl Request {
    /// A header's value, whatever case it was sent in.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    /// The start of the request line, as far as a test needs it.
    pub fn line(&self) -> String {
        format!("{} {} HTTP/1.1", self.method, self.path)
    }
}

/// Read one whole request, or return None if the client went away first.
///
/// The body has to be read too. A client can send it after the headers, and a stub that
/// answers and closes with the body unread resets the connection, which the client can
/// then see instead of the answer. How often that happens depends on how busy the
/// machine is.
pub fn read_request(stream: &mut TcpStream) -> Option<Request> {
    let mut received = Vec::new();
    let mut buffer = [0u8; 8192];

    loop {
        let read = stream.read(&mut buffer).ok()?;
        if read == 0 {
            return None;
        }
        received.extend_from_slice(&buffer[..read]);

        let Some(header_end) = received.windows(4).position(|window| window == b"\r\n\r\n") else {
            continue;
        };

        let head = String::from_utf8_lossy(&received[..header_end]).to_string();
        let mut lines = head.lines();
        let mut request_line = lines.next().unwrap_or("").split_whitespace();
        let method = request_line.next().unwrap_or("").to_uppercase();
        let path = request_line.next().unwrap_or("").to_string();
        let headers: Vec<(String, String)> = lines
            .filter_map(|line| line.split_once(':'))
            .map(|(key, value)| (key.trim().to_string(), value.trim().to_string()))
            .collect();

        let mut request = Request { method, path, body: Vec::new(), headers };
        let length: usize = request.header("content-length").and_then(|value| value.parse().ok()).unwrap_or(0);

        if received.len() >= header_end + 4 + length {
            request.body = received[header_end + 4..header_end + 4 + length].to_vec();
            return Some(request);
        }
    }
}
