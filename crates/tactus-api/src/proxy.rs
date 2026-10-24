use base64::Engine;
use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(15);
const HEADER_LIMIT: usize = 16 * 1024;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProxyKind {
    #[default]
    Http,
    Socks5,
}

impl ProxyKind {
    pub const ALL: [ProxyKind; 2] = [ProxyKind::Http, ProxyKind::Socks5];

    pub fn name(self) -> &'static str {
        match self {
            ProxyKind::Http => "HTTP",
            ProxyKind::Socks5 => "SOCKS5",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Proxy {
    pub enabled: bool,
    pub kind: ProxyKind,
    pub host: String,
    pub port: u16,
    pub login: String,
    pub password: String,
}

impl Default for Proxy {
    fn default() -> Self {
        Self {
            enabled: false,
            kind: ProxyKind::Http,
            host: String::new(),
            port: 8080,
            login: String::new(),
            password: String::new(),
        }
    }
}

pub fn encode_component(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => char::from(byte).to_string(),
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

fn failure(message: impl Into<String>) -> io::Error {
    io::Error::other(message.into())
}

impl Proxy {
    pub fn active(&self) -> bool {
        self.enabled && !self.host.trim().is_empty() && self.port != 0
    }

    pub fn url(&self) -> String {
        let scheme = match self.kind {
            ProxyKind::Http => "http",
            ProxyKind::Socks5 => "socks5h",
        };
        let credentials = if self.login.is_empty() {
            String::new()
        } else {
            format!("{}:{}@", encode_component(&self.login), encode_component(&self.password))
        };
        format!("{scheme}://{credentials}{}:{}", self.host.trim(), self.port)
    }

    pub fn tunnel(&self, host: &str, port: u16) -> io::Result<TcpStream> {
        let mut stream = connect(self.host.trim(), self.port)?;
        stream.set_read_timeout(Some(TIMEOUT))?;
        stream.set_write_timeout(Some(TIMEOUT))?;
        match self.kind {
            ProxyKind::Http => self.http_connect(&mut stream, host, port)?,
            ProxyKind::Socks5 => self.socks_connect(&mut stream, host, port)?,
        }
        stream.set_read_timeout(None)?;
        stream.set_write_timeout(None)?;
        Ok(stream)
    }

    fn http_connect(&self, stream: &mut TcpStream, host: &str, port: u16) -> io::Result<()> {
        let mut request = format!("CONNECT {host}:{port} HTTP/1.1\r\nHost: {host}:{port}\r\n");
        if !self.login.is_empty() {
            let pair = base64::engine::general_purpose::STANDARD.encode(format!("{}:{}", self.login, self.password));
            request.push_str(&format!("Proxy-Authorization: Basic {pair}\r\n"));
        }
        request.push_str("\r\n");
        stream.write_all(request.as_bytes())?;
        let mut response = Vec::new();
        let mut byte = [0];
        while !response.ends_with(b"\r\n\r\n") {
            if response.len() >= HEADER_LIMIT || stream.read(&mut byte)? == 0 {
                return Err(failure("the proxy closed the connection"));
            }
            response.push(byte[0]);
        }
        let response = String::from_utf8_lossy(&response);
        let status = response.lines().next().unwrap_or_default();
        match status.split_whitespace().nth(1) {
            Some("200") => Ok(()),
            _ => Err(failure(format!("the proxy refused the connection: {status}"))),
        }
    }

    fn socks_connect(&self, stream: &mut TcpStream, host: &str, port: u16) -> io::Result<()> {
        let login = !self.login.is_empty();
        stream.write_all(if login { &[5, 2, 0, 2] } else { &[5, 1, 0] })?;
        let mut choice = [0; 2];
        stream.read_exact(&mut choice)?;
        match choice {
            [5, 0] => {}
            [5, 2] if login => {
                let (user, password) = (self.login.as_bytes(), self.password.as_bytes());
                let too_long = || failure("the proxy login or password is too long");
                let mut request = vec![1, u8::try_from(user.len()).map_err(|_| too_long())?];
                request.extend_from_slice(user);
                request.push(u8::try_from(password.len()).map_err(|_| too_long())?);
                request.extend_from_slice(password);
                stream.write_all(&request)?;
                let mut status = [0; 2];
                stream.read_exact(&mut status)?;
                if status[1] != 0 {
                    return Err(failure("the proxy rejected the login or password"));
                }
            }
            _ => return Err(failure("the proxy needs a login and password")),
        }
        let name = host.as_bytes();
        let mut request = vec![5, 1, 0, 3, u8::try_from(name.len()).map_err(|_| failure("the host name is too long"))?];
        request.extend_from_slice(name);
        request.extend_from_slice(&port.to_be_bytes());
        stream.write_all(&request)?;
        let mut reply = [0; 4];
        stream.read_exact(&mut reply)?;
        if reply[1] != 0 {
            return Err(failure(format!("the proxy could not connect (SOCKS error {})", reply[1])));
        }
        let address = match reply[3] {
            1 => 4,
            4 => 16,
            3 => {
                let mut length = [0];
                stream.read_exact(&mut length)?;
                usize::from(length[0])
            }
            _ => return Err(failure("the proxy sent an unknown address type")),
        };
        let mut rest = vec![0; address + 2];
        stream.read_exact(&mut rest)
    }
}

pub fn connect(host: &str, port: u16) -> io::Result<TcpStream> {
    let mut last = failure(format!("{host} has no address"));
    for address in (host, port).to_socket_addrs()? {
        match TcpStream::connect_timeout(&address, TIMEOUT) {
            Ok(stream) => return Ok(stream),
            Err(error) => last = error,
        }
    }
    Err(last)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, TcpListener};

    fn proxy(kind: ProxyKind, port: u16, login: &str) -> Proxy {
        Proxy { enabled: true, kind, host: "127.0.0.1".into(), port, login: login.into(), password: "p@ss word".into() }
    }

    fn serve(reply: impl FnOnce(&mut TcpStream) + Send + 'static) -> u16 {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            reply(&mut stream);
        });
        port
    }

    #[test]
    fn builds_proxy_urls() {
        assert_eq!(proxy(ProxyKind::Http, 3128, "").url(), "http://127.0.0.1:3128");
        assert_eq!(proxy(ProxyKind::Socks5, 1080, "me").url(), "socks5h://me:p%40ss%20word@127.0.0.1:1080");
        assert!(!Proxy { host: " ".into(), ..proxy(ProxyKind::Http, 1, "") }.active());
    }

    #[test]
    fn tunnels_through_http_connect() {
        let port = serve(|stream| {
            let mut request = [0; 512];
            let read = stream.read(&mut request).unwrap();
            let request = String::from_utf8_lossy(&request[..read]).into_owned();
            assert!(request.starts_with("CONNECT ynison.music.yandex.ru:443 HTTP/1.1\r\n"));
            assert!(request.contains("Proxy-Authorization: Basic bWU6cEBzcyB3b3Jk\r\n"));
            stream.write_all(b"HTTP/1.1 200 Connection established\r\n\r\nhello").unwrap();
        });
        let mut stream = proxy(ProxyKind::Http, port, "me").tunnel("ynison.music.yandex.ru", 443).unwrap();
        let mut greeting = [0; 5];
        stream.read_exact(&mut greeting).unwrap();
        assert_eq!(&greeting, b"hello");
    }

    #[test]
    fn reports_http_refusals() {
        let port = serve(|stream| {
            let mut request = [0; 512];
            let _ = stream.read(&mut request).unwrap();
            stream.write_all(b"HTTP/1.1 407 Proxy Authentication Required\r\n\r\n").unwrap();
        });
        let error = proxy(ProxyKind::Http, port, "").tunnel("example.com", 443).unwrap_err();
        assert!(error.to_string().contains("407"));
    }

    #[test]
    fn tunnels_through_socks5_with_login() {
        let port = serve(|stream| {
            let mut greeting = [0; 4];
            stream.read_exact(&mut greeting).unwrap();
            assert_eq!(greeting, [5, 2, 0, 2]);
            stream.write_all(&[5, 2]).unwrap();
            let mut auth = [0; 14];
            stream.read_exact(&mut auth).unwrap();
            assert_eq!(&auth[..4], &[1, 2, b'm', b'e']);
            assert_eq!(&auth[4..], b"\x09p@ss word");
            stream.write_all(&[1, 0]).unwrap();
            let mut connect = [0; 5 + 11 + 2];
            stream.read_exact(&mut connect).unwrap();
            assert_eq!(&connect[..5], &[5, 1, 0, 3, 11]);
            assert_eq!(&connect[5..16], b"example.com");
            assert_eq!(&connect[16..], &443u16.to_be_bytes());
            stream.write_all(&[5, 0, 0, 1, 10, 0, 0, 1, 0, 80, b'o', b'k']).unwrap();
        });
        let mut stream = proxy(ProxyKind::Socks5, port, "me").tunnel("example.com", 443).unwrap();
        let mut rest = [0; 2];
        stream.read_exact(&mut rest).unwrap();
        assert_eq!(&rest, b"ok");
    }

    #[test]
    fn reports_socks_failures() {
        let port = serve(|stream| {
            let mut greeting = [0; 3];
            stream.read_exact(&mut greeting).unwrap();
            stream.write_all(&[5, 0]).unwrap();
            let mut connect = [0; 5 + 11 + 2];
            stream.read_exact(&mut connect).unwrap();
            stream.write_all(&[5, 5, 0, 1, 0, 0, 0, 0, 0, 0]).unwrap();
        });
        let error = proxy(ProxyKind::Socks5, port, "").tunnel("example.com", 443).unwrap_err();
        assert!(error.to_string().contains("SOCKS error 5"));
    }
}
