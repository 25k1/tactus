use super::RemoteHub;
use serde_json::{Value, json};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tungstenite::protocol::Role;
use tungstenite::{Message, WebSocket};

const MAX_HEAD: usize = 16 * 1024;
const MAX_BODY: usize = 64 * 1024;
const TICK: Duration = Duration::from_millis(100);

struct Request {
    method: String,
    path: String,
    query: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Request {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(key, _)| key.eq_ignore_ascii_case(name)).map(|(_, value)| value.as_str())
    }

    fn query(&self, name: &str) -> Option<&str> {
        self.query
            .split('&')
            .filter_map(|pair| pair.split_once('='))
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value)
    }
}

pub fn is_local(value: &str) -> bool {
    let rest = value.split_once("://").map_or(value, |(_, rest)| rest);
    let host = match rest.strip_prefix('[') {
        Some(ipv6) => ipv6.split(']').next().unwrap_or_default(),
        None => rest.split(['/', ':']).next().unwrap_or_default(),
    };
    matches!(host, "127.0.0.1" | "localhost" | "::1")
}

fn read_request(stream: &mut BufReader<TcpStream>) -> io::Result<Option<Request>> {
    let mut head = Vec::new();
    while !head.ends_with(b"\r\n\r\n") {
        let read = stream.read_until(b'\n', &mut head)?;
        if read == 0 || head.len() > MAX_HEAD {
            return Ok(None);
        }
    }
    let mut headers = [httparse::EMPTY_HEADER; 32];
    let mut parsed = httparse::Request::new(&mut headers);
    if !matches!(parsed.parse(&head), Ok(httparse::Status::Complete(_))) {
        return Ok(None);
    }
    let target = parsed.path.unwrap_or("/");
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let mut request = Request {
        method: parsed.method.unwrap_or_default().to_owned(),
        path: path.to_owned(),
        query: query.to_owned(),
        headers: parsed
            .headers
            .iter()
            .map(|header| (header.name.to_owned(), String::from_utf8_lossy(header.value).into_owned()))
            .collect(),
        body: Vec::new(),
    };
    let length: usize = request.header("Content-Length").and_then(|value| value.trim().parse().ok()).unwrap_or(0);
    if length > MAX_BODY {
        return Ok(None);
    }
    request.body.resize(length, 0);
    stream.read_exact(&mut request.body)?;
    Ok(Some(request))
}

fn respond(stream: &mut TcpStream, status: u16, body: &Value) -> io::Result<()> {
    let reason = match status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        _ => "Not Found",
    };
    let body = body.to_string();
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

fn error(message: &str) -> Value {
    json!({ "ok": false, "error": message })
}

fn command_result(result: Result<(), String>) -> (u16, Value) {
    match result {
        Ok(()) => (202, json!({ "ok": true })),
        Err(message) => (400, error(&message)),
    }
}

fn authorized(request: &Request, token: &str) -> bool {
    token.is_empty()
        || request.header("Authorization").and_then(|value| value.strip_prefix("Bearer ")) == Some(token)
        || request.query("token") == Some(token)
}

fn route(hub: &RemoteHub, request: &Request) -> (u16, Value) {
    let is_json = request.header("Content-Type").is_some_and(|value| value.starts_with("application/json"));
    let body = || -> Result<Value, String> {
        match std::str::from_utf8(&request.body).map(str::trim) {
            Ok("") => Ok(json!({})),
            Ok(text) => serde_json::from_str(text).map_err(|error| error.to_string()),
            Err(error) => Err(error.to_string()),
        }
    };
    match (request.method.as_str(), request.path.trim_end_matches('/')) {
        ("GET", path) => match path.strip_prefix("/api/").and_then(|what| hub.get(what)) {
            Some(value) => (200, value),
            None => (404, error("not found")),
        },
        ("POST", _) if !is_json => (400, error("Content-Type must be application/json")),
        ("POST", "/api/command") => command_result(body().and_then(|value| hub.submit(value))),
        ("POST", path) => match path.strip_prefix("/api/") {
            Some(command) => command_result(body().and_then(|mut value| {
                let object = value.as_object_mut().ok_or("the body must be a JSON object")?;
                object.insert("command".into(), Value::String(command.to_owned()));
                hub.submit(value)
            })),
            None => (404, error("not found")),
        },
        _ => (404, error("not found")),
    }
}

fn serve_socket(stream: TcpStream, request: &Request, hub: &RemoteHub, stop: &AtomicBool) -> io::Result<()> {
    let Some(key) = request.header("Sec-WebSocket-Key") else {
        return respond(&mut stream.try_clone()?, 400, &error("not a WebSocket request"));
    };
    let accept = tungstenite::handshake::derive_accept_key(key.trim().as_bytes());
    let mut raw = stream.try_clone()?;
    write!(
        raw,
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n"
    )?;
    stream.set_read_timeout(Some(TICK))?;
    let mut socket = WebSocket::from_raw_socket(stream, Role::Server, None);
    let events = hub.subscribe();
    while !stop.load(Ordering::Relaxed) {
        for event in events.try_iter() {
            if socket.send(Message::text(event)).is_err() {
                return Ok(());
            }
        }
        match socket.read() {
            Ok(Message::Text(text)) => {
                let reply = match serde_json::from_str(text.as_str())
                    .map_err(|error| error.to_string())
                    .and_then(|value| hub.submit(value))
                {
                    Ok(()) => json!({ "type": "ack", "ok": true }),
                    Err(message) => json!({ "type": "ack", "ok": false, "error": message }),
                };
                if socket.send(Message::text(reply.to_string())).is_err() {
                    return Ok(());
                }
            }
            Ok(Message::Close(_)) => return Ok(()),
            Ok(_) => {}
            Err(tungstenite::Error::Io(error))
                if matches!(error.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut) => {}
            Err(_) => return Ok(()),
        }
    }
    Ok(())
}

fn serve(stream: TcpStream, hub: &RemoteHub, token: &str, stop: &AtomicBool) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let Some(request) = read_request(&mut reader)? else {
        return Ok(());
    };
    let mut writer = stream.try_clone()?;
    let local = request.header("Host").is_some_and(is_local) && request.header("Origin").is_none_or(is_local);
    if !local {
        return respond(&mut writer, 403, &error("only local clients may use this API"));
    }
    if !authorized(&request, token) {
        return respond(&mut writer, 401, &error("a valid access token is required"));
    }
    if request.method == "GET" && request.path == "/ws" {
        return serve_socket(stream, &request, hub, stop);
    }
    let (status, body) = route(hub, &request);
    respond(&mut writer, status, &body)
}

pub struct RemoteServer {
    port: u16,
    stop: Arc<AtomicBool>,
}

impl RemoteServer {
    pub fn start(port: u16, token: String, hub: Arc<RemoteHub>) -> io::Result<RemoteServer> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))?;
        let port = listener.local_addr()?.port();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                if flag.load(Ordering::Relaxed) {
                    break;
                }
                let Ok(stream) = stream else { continue };
                let (hub, token, flag) = (hub.clone(), token.clone(), flag.clone());
                std::thread::spawn(move || {
                    let _ = serve(stream, &hub, &token, &flag);
                });
            }
        });
        Ok(RemoteServer { port, stop })
    }

    #[cfg(test)]
    pub fn port(&self) -> u16 {
        self.port
    }
}

impl Drop for RemoteServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect((Ipv4Addr::LOCALHOST, self.port));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::remote::RemoteCommand;
    use std::sync::mpsc::Receiver;

    fn start(token: &str) -> (RemoteServer, Arc<RemoteHub>, Receiver<RemoteCommand>) {
        let (hub, commands) = RemoteHub::new();
        let server = RemoteServer::start(0, token.to_owned(), hub.clone()).unwrap();
        (server, hub, commands)
    }

    fn call(port: u16, head: &str, body: &str) -> (u16, String) {
        let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
        write!(stream, "{head}\r\nHost: 127.0.0.1:{port}\r\nContent-Length: {}\r\n\r\n{body}", body.len()).unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        let status = response[9..12].parse().unwrap();
        let body = response.split_once("\r\n\r\n").map(|(_, body)| body.to_owned()).unwrap_or_default();
        (status, body)
    }

    #[test]
    fn serves_status_and_accepts_commands() {
        let (server, hub, commands) = start("");
        hub.publish_status(json!({ "title": "Song" }));
        let (status, body) = call(server.port(), "GET /api/status HTTP/1.1", "");
        assert_eq!(status, 200);
        assert!(body.contains("Song"));
        let json = "POST /api/seek HTTP/1.1\r\nContent-Type: application/json";
        assert_eq!(call(server.port(), json, r#"{"positionMs": 30000}"#).0, 202);
        assert_eq!(
            commands.recv_timeout(Duration::from_secs(2)).unwrap(),
            RemoteCommand::Seek { position_ms: Some(30000), delta_ms: None }
        );
        let command = "POST /api/command HTTP/1.1\r\nContent-Type: application/json";
        assert_eq!(call(server.port(), command, r#"{"command": "next"}"#).0, 202);
        assert_eq!(commands.recv_timeout(Duration::from_secs(2)).unwrap(), RemoteCommand::Next);
    }

    #[test]
    fn rejects_requests_a_web_page_could_forge() {
        let (server, _hub, commands) = start("");
        let port = server.port();
        assert_eq!(call(port, "POST /api/next HTTP/1.1", "{}").0, 400);
        let foreign = "POST /api/next HTTP/1.1\r\nContent-Type: application/json\r\nOrigin: https://example.com";
        assert_eq!(call(port, foreign, "{}").0, 403);
        let unknown = "POST /api/explode HTTP/1.1\r\nContent-Type: application/json";
        assert_eq!(call(port, unknown, "{}").0, 400);
        assert!(commands.try_recv().is_err());
    }

    #[test]
    fn requires_the_token_when_set() {
        let (server, _hub, _commands) = start("secret");
        let port = server.port();
        assert_eq!(call(port, "GET /api/status HTTP/1.1", "").0, 401);
        assert_eq!(call(port, "GET /api/status?token=secret HTTP/1.1", "").0, 200);
        assert_eq!(call(port, "GET /api/status HTTP/1.1\r\nAuthorization: Bearer secret", "").0, 200);
    }

    #[test]
    fn recognizes_local_origins() {
        assert!(is_local("127.0.0.1:9420"));
        assert!(is_local("http://localhost:3000"));
        assert!(is_local("[::1]:9420"));
        assert!(!is_local("https://example.com"));
        assert!(!is_local("http://127.0.0.1.evil.com"));
        assert!(!is_local("null"));
    }
}
