#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::*;
use std::io::{Read, Write};
use std::net::TcpListener;

/// Serve one request on a local port with an empty 200 and hand back the
/// request's headers.
fn capture_one_request() -> (String, std::thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let mut request = Vec::new();
        let mut buf = [0u8; 1024];
        while !request.windows(4).any(|w| w == b"\r\n\r\n") {
            let n = socket.read(&mut buf).unwrap();
            if n == 0 {
                break;
            }
            request.extend_from_slice(&buf[..n]);
        }
        socket
            .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\nconnection: close\r\n\r\n")
            .unwrap();
        String::from_utf8_lossy(&request).into_owned()
    });
    (url, server)
}

fn user_agent_of(request: &str) -> Option<&str> {
    request.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.eq_ignore_ascii_case("user-agent")
            .then(|| value.trim())
    })
}

#[tokio::test]
async fn api_requests_send_the_given_user_agent() {
    let (url, server) = capture_one_request();
    let client = create_api_client(Some("some-app/1.2 (+https://example.com)")).unwrap();

    client.get(&url).send().await.unwrap();

    let request = server.join().unwrap();
    assert_eq!(
        user_agent_of(&request),
        Some("some-app/1.2 (+https://example.com)")
    );
}

#[tokio::test]
async fn api_requests_send_no_user_agent_unless_given_one() {
    let (url, server) = capture_one_request();
    let client = create_api_client(None).unwrap();

    client.get(&url).send().await.unwrap();

    assert_eq!(user_agent_of(&server.join().unwrap()), None);
}
