//! OS mode: a client for the bore tunneling protocol, <https://github.com/ekzhang/bore>.
//!
//! bore speaks null-delimited JSON on a control connection to port 7835. The
//! client says which port it wants (`Hello`), the server binds a public port
//! and heartbeats, and every visitor to that port arrives as a `Connection`
//! carrying an id. The client answers each one by opening a *second*
//! connection to the server, sending `Accept(id)` on it, and from then on that
//! connection is the visitor's raw bytes, piped to the local port.
//!
//! Everything here is blocking std I/O on plain threads: the one tokio runtime
//! in wasmrun belongs to the wasmnet proxy, and a tunnel carrying a dev
//! server's traffic does not need another.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{self, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// The server's control port, fixed by the protocol.
pub const CONTROL_PORT: u16 = 7835;

/// The public server `bore` itself defaults to.
pub const DEFAULT_SERVER: &str = "bore.pub";

/// Longest frame the protocol allows. A longer one is a peer that is not bore.
const MAX_FRAME_LENGTH: usize = 256;

/// bore's own timeout for connecting and for the first message on a stream.
const NETWORK_TIMEOUT: Duration = Duration::from_secs(3);

/// The server heartbeats every 500ms, so this much silence is a dead link.
const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(10);

const MAX_BACKOFF: Duration = Duration::from_secs(30);

/// Visitors forwarded at once. Each costs two threads; past this one is left
/// unaccepted and the server drops it after its own ten-second grace.
const MAX_FORWARDED: usize = 64;

#[derive(Debug, Serialize)]
enum ClientMessage {
    Authenticate(String),
    Hello(u16),
    Accept(String),
}

#[derive(Debug, Deserialize)]
enum ServerMessage {
    Challenge(String),
    Hello(u16),
    Heartbeat,
    Connection(String),
    Error(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TunnelStatus {
    Connecting,
    Connected,
    /// The link dropped after it had been up; retrying.
    Reconnecting,
    /// The last attempt failed; retrying with backoff. `last_error` says why.
    Failed,
    /// Stopped by the caller.
    Disconnected,
}

impl TunnelStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            TunnelStatus::Connecting => "Connecting",
            TunnelStatus::Connected => "Connected",
            TunnelStatus::Reconnecting => "Reconnecting",
            TunnelStatus::Failed => "Failed",
            TunnelStatus::Disconnected => "Disconnected",
        }
    }
}

/// A bore server address: `host` or `host:port`, with the port defaulting to
/// [`CONTROL_PORT`]. IPv6 literals go in brackets, `[::1]:7835`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoreServer {
    pub host: String,
    pub port: u16,
}

impl BoreServer {
    pub fn parse(spec: &str) -> Result<Self, String> {
        let spec = spec.trim();
        let (host, port) = if let Some(rest) = spec.strip_prefix('[') {
            let (host, tail) = rest
                .split_once(']')
                .ok_or_else(|| format!("unclosed '[' in tunnel server '{spec}'"))?;
            match tail.strip_prefix(':') {
                Some(port) => (host, Some(port)),
                None if tail.is_empty() => (host, None),
                None => return Err(format!("unexpected '{tail}' in tunnel server '{spec}'")),
            }
        } else {
            match spec.rsplit_once(':') {
                Some((host, _)) if host.contains(':') => {
                    return Err(format!(
                        "IPv6 tunnel server '{spec}' needs brackets, like [::1]:{CONTROL_PORT}"
                    ))
                }
                Some((host, port)) => (host, Some(port)),
                None => (spec, None),
            }
        };

        if host.is_empty() {
            return Err(format!("tunnel server '{spec}' has no host"));
        }
        let port = match port {
            Some(p) => p
                .parse::<u16>()
                .ok()
                .filter(|p| *p != 0)
                .ok_or_else(|| format!("invalid port '{p}' in tunnel server '{spec}'"))?,
            None => CONTROL_PORT,
        };

        Ok(Self {
            host: host.to_string(),
            port,
        })
    }

    fn url_host(&self) -> String {
        if self.host.contains(':') {
            format!("[{}]", self.host)
        } else {
            self.host.clone()
        }
    }

    fn connect(&self) -> io::Result<TcpStream> {
        let mut last = io::Error::new(io::ErrorKind::NotFound, "no address");
        for addr in (self.host.as_str(), self.port).to_socket_addrs()? {
            match TcpStream::connect_timeout(&addr, NETWORK_TIMEOUT) {
                Ok(stream) => return Ok(stream),
                Err(e) => last = e,
            }
        }
        Err(last)
    }
}

impl std::fmt::Display for BoreServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.url_host(), self.port)
    }
}

struct State {
    status: TunnelStatus,
    public_port: Option<u16>,
    last_error: Option<String>,
}

struct Shared {
    server: BoreServer,
    /// sha256 of the secret, which bore uses as the HMAC key.
    auth_key: Option<[u8; 32]>,
    /// Where visitors go. 0 until a program is listening, and they are closed.
    target: AtomicU16,
    state: Mutex<State>,
    stop: AtomicBool,
    /// A handle on the live control connection, so `stop` can shut it and
    /// wake the thread blocked reading it.
    control: Mutex<Option<TcpStream>>,
    forwarded: AtomicUsize,
}

impl Shared {
    fn set(&self, status: TunnelStatus, error: Option<String>) {
        let mut state = self.state.lock().unwrap();
        state.status = status;
        if error.is_some() || status == TunnelStatus::Connected {
            state.last_error = error;
        }
    }

    fn stopped(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }
}

/// A tunnel from a bore server's public port to a local one.
///
/// The public port is held for as long as the client runs, and the local
/// port it forwards to can change underneath it with [`set_target`], so a
/// program that stops and starts again keeps its public URL. Dropping the
/// client closes the tunnel.
///
/// [`set_target`]: BoreClient::set_target
pub struct BoreClient {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl BoreClient {
    /// Start connecting in the background. Returns at once; poll [`status`]
    /// or use [`wait_connected`] to learn how it went.
    ///
    /// [`status`]: BoreClient::status
    /// [`wait_connected`]: BoreClient::wait_connected
    pub fn start(
        server: BoreServer,
        secret: Option<&str>,
        target: Option<u16>,
    ) -> io::Result<Self> {
        let shared = Arc::new(Shared {
            server,
            auth_key: secret.map(|s| Sha256::digest(s.as_bytes()).into()),
            target: AtomicU16::new(target.unwrap_or(0)),
            state: Mutex::new(State {
                status: TunnelStatus::Connecting,
                public_port: None,
                last_error: None,
            }),
            stop: AtomicBool::new(false),
            control: Mutex::new(None),
            forwarded: AtomicUsize::new(0),
        });

        let thread_shared = Arc::clone(&shared);
        let thread = thread::Builder::new()
            .name("bore".to_string())
            .spawn(move || run(thread_shared))?;

        Ok(Self {
            shared,
            thread: Some(thread),
        })
    }

    /// Point the tunnel at a local port on 127.0.0.1, or at nothing.
    /// Connections already forwarded keep going where they were going.
    pub fn set_target(&self, port: Option<u16>) {
        self.shared
            .target
            .store(port.unwrap_or(0), Ordering::SeqCst);
    }

    pub fn server(&self) -> &BoreServer {
        &self.shared.server
    }

    pub fn status(&self) -> TunnelStatus {
        self.shared.state.lock().unwrap().status
    }

    pub fn public_port(&self) -> Option<u16> {
        self.shared.state.lock().unwrap().public_port
    }

    pub fn public_url(&self) -> Option<String> {
        self.public_port()
            .map(|port| format!("http://{}:{port}", self.shared.server.url_host()))
    }

    pub fn last_error(&self) -> Option<String> {
        self.shared.state.lock().unwrap().last_error.clone()
    }

    /// Wait until the tunnel is up or has failed once, at most `timeout`.
    pub fn wait_connected(&self, timeout: Duration) -> TunnelStatus {
        let deadline = Instant::now() + timeout;
        loop {
            let status = self.status();
            if status != TunnelStatus::Connecting || Instant::now() >= deadline {
                return status;
            }
            thread::sleep(Duration::from_millis(50));
        }
    }

    pub fn stop(&mut self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        if let Some(control) = self.shared.control.lock().unwrap().take() {
            let _ = control.shutdown(Shutdown::Both);
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        self.shared.set(TunnelStatus::Disconnected, None);
    }
}

impl Drop for BoreClient {
    fn drop(&mut self) {
        self.stop();
    }
}

/// The control loop: connect, serve until the link drops, reconnect. A
/// reconnect asks for the public port it had, so a blip does not change the
/// URL unless someone else took the port meanwhile.
fn run(shared: Arc<Shared>) {
    let mut want_port = 0;
    let mut backoff = Duration::from_secs(1);
    let mut was_connected = false;

    while !shared.stopped() {
        match open_control(&shared, want_port) {
            Ok((stream, public_port)) => {
                want_port = public_port;
                backoff = Duration::from_secs(1);
                was_connected = true;
                shared.state.lock().unwrap().public_port = Some(public_port);
                shared.set(TunnelStatus::Connected, None);

                let reason = serve_control(&shared, stream);
                *shared.control.lock().unwrap() = None;
                if shared.stopped() {
                    break;
                }
                shared.set(TunnelStatus::Reconnecting, Some(reason));
            }
            Err(ControlError::Server(message)) if want_port != 0 => {
                // Most likely the old public port is taken. Take any.
                shared.set(TunnelStatus::Reconnecting, Some(message));
                want_port = 0;
            }
            Err(e) => {
                let status = if was_connected {
                    TunnelStatus::Reconnecting
                } else {
                    TunnelStatus::Failed
                };
                shared.set(status, Some(e.to_string()));
                sleep_unless_stopped(&shared, backoff);
                backoff = (backoff * 2).min(MAX_BACKOFF);
            }
        }
    }
}

fn sleep_unless_stopped(shared: &Shared, duration: Duration) {
    let deadline = Instant::now() + duration;
    while !shared.stopped() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(100));
    }
}

#[derive(Debug)]
enum ControlError {
    Io(String),
    /// The server answered, and the answer was no.
    Server(String),
}

impl std::fmt::Display for ControlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ControlError::Io(e) => write!(f, "{e}"),
            ControlError::Server(e) => write!(f, "server refused: {e}"),
        }
    }
}

impl From<io::Error> for ControlError {
    fn from(e: io::Error) -> Self {
        ControlError::Io(e.to_string())
    }
}

/// Connect, authenticate, and ask for `port` (0 for any). Returns the
/// control stream and the public port the server bound.
fn open_control(shared: &Shared, port: u16) -> Result<(TcpStream, u16), ControlError> {
    let mut stream = shared
        .server
        .connect()
        .map_err(|e| ControlError::Io(format!("could not reach {}: {e}", shared.server)))?;
    stream.set_read_timeout(Some(NETWORK_TIMEOUT))?;
    stream.set_write_timeout(Some(NETWORK_TIMEOUT))?;
    // Published before the handshake so `stop` can interrupt a slow one.
    *shared.control.lock().unwrap() = Some(stream.try_clone()?);

    authenticate(&mut stream, shared.auth_key.as_ref())?;
    send(&mut stream, &ClientMessage::Hello(port))?;

    match recv(&mut stream)? {
        Some(ServerMessage::Hello(public_port)) => {
            stream.set_read_timeout(Some(HEARTBEAT_TIMEOUT))?;
            Ok((stream, public_port))
        }
        Some(ServerMessage::Error(message)) => Err(ControlError::Server(message)),
        Some(ServerMessage::Challenge(_)) => Err(ControlError::Server(
            "the server requires a secret and none was given".to_string(),
        )),
        Some(other) => Err(ControlError::Io(format!(
            "unexpected {other:?} before Hello"
        ))),
        None => Err(ControlError::Io(
            "the server closed the connection during the handshake".to_string(),
        )),
    }
}

/// Answer the server's challenge if we hold a secret. Without one there is
/// nothing to do: a server that wanted one sends a `Challenge` where the
/// `Hello` should be, which `open_control` reports.
fn authenticate(stream: &mut TcpStream, key: Option<&[u8; 32]>) -> Result<(), ControlError> {
    let Some(key) = key else {
        return Ok(());
    };
    match recv(stream)? {
        Some(ServerMessage::Challenge(challenge)) => {
            let tag = answer_challenge(key, &challenge)
                .ok_or_else(|| ControlError::Io(format!("malformed challenge '{challenge}'")))?;
            send(stream, &ClientMessage::Authenticate(tag))?;
            Ok(())
        }
        Some(ServerMessage::Error(message)) => Err(ControlError::Server(message)),
        _ => Err(ControlError::Server(
            "a secret was given but the server does not ask for one".to_string(),
        )),
    }
}

/// Read the control connection until it ends, forwarding each visitor.
/// Returns why it ended.
fn serve_control(shared: &Arc<Shared>, mut stream: TcpStream) -> String {
    loop {
        match recv(&mut stream) {
            Ok(Some(ServerMessage::Heartbeat)) => {}
            Ok(Some(ServerMessage::Connection(id))) => {
                if shared.forwarded.fetch_add(1, Ordering::SeqCst) >= MAX_FORWARDED {
                    shared.forwarded.fetch_sub(1, Ordering::SeqCst);
                    continue;
                }
                let conn_shared = Arc::clone(shared);
                let spawned =
                    thread::Builder::new()
                        .name("bore-conn".to_string())
                        .spawn(move || {
                            let _ = forward(&conn_shared, &id);
                            conn_shared.forwarded.fetch_sub(1, Ordering::SeqCst);
                        });
                if spawned.is_err() {
                    shared.forwarded.fetch_sub(1, Ordering::SeqCst);
                }
            }
            Ok(Some(ServerMessage::Error(message))) => return format!("server error: {message}"),
            Ok(Some(_)) => {}
            Ok(None) => return "the server closed the tunnel".to_string(),
            Err(e)
                if e.kind() == io::ErrorKind::WouldBlock || e.kind() == io::ErrorKind::TimedOut =>
            {
                return "no heartbeat from the server".to_string()
            }
            Err(e) => return format!("tunnel connection lost: {e}"),
        }
    }
}

/// Carry one visitor: claim it from the server on a fresh connection, then
/// pipe it to the target. With no target the claim still happens, so the
/// visitor is closed now rather than left hanging until the server gives up.
fn forward(shared: &Shared, id: &str) -> io::Result<()> {
    let mut remote = shared.server.connect()?;
    remote.set_read_timeout(Some(NETWORK_TIMEOUT))?;
    remote.set_write_timeout(Some(NETWORK_TIMEOUT))?;
    authenticate(&mut remote, shared.auth_key.as_ref())
        .map_err(|e| io::Error::other(e.to_string()))?;
    send(&mut remote, &ClientMessage::Accept(id.to_string()))?;

    let port = shared.target.load(Ordering::SeqCst);
    if port == 0 {
        return Ok(());
    }
    let local =
        TcpStream::connect_timeout(&SocketAddr::from(([127, 0, 0, 1], port)), NETWORK_TIMEOUT)?;

    remote.set_read_timeout(None)?;
    remote.set_write_timeout(None)?;
    pipe(remote, local)
}

/// Copy both ways until both directions finish, half-closing each side as
/// its source runs dry so request/response protocols see a clean EOF.
fn pipe(a: TcpStream, b: TcpStream) -> io::Result<()> {
    let (mut a_read, mut b_write) = (a.try_clone()?, b.try_clone()?);
    let upstream = thread::spawn(move || {
        let _ = io::copy(&mut a_read, &mut b_write);
        let _ = b_write.shutdown(Shutdown::Write);
    });

    let (mut b_read, mut a_write) = (b, a);
    let _ = io::copy(&mut b_read, &mut a_write);
    let _ = a_write.shutdown(Shutdown::Write);
    let _ = upstream.join();
    Ok(())
}

fn send(stream: &mut TcpStream, message: &ClientMessage) -> io::Result<()> {
    let mut frame = serde_json::to_vec(message).map_err(io::Error::other)?;
    frame.push(0);
    stream.write_all(&frame)?;
    stream.flush()
}

/// Read one frame, a byte at a time. Frames are tiny, and reading no further
/// than the delimiter matters on a data connection: whatever follows the
/// handshake is the visitor's bytes, not ours to buffer.
fn recv(stream: &mut impl Read) -> io::Result<Option<ServerMessage>> {
    let mut frame = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        if stream.read(&mut byte)? == 0 {
            return if frame.is_empty() {
                Ok(None)
            } else {
                Err(io::ErrorKind::UnexpectedEof.into())
            };
        }
        if byte[0] == 0 {
            break;
        }
        if frame.len() == MAX_FRAME_LENGTH {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "frame longer than the bore protocol allows",
            ));
        }
        frame.push(byte[0]);
    }
    serde_json::from_slice(&frame)
        .map(Some)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// bore's challenge response: hex(HMAC-SHA256(sha256(secret), uuid bytes)).
fn answer_challenge(key: &[u8; 32], challenge: &str) -> Option<String> {
    let uuid = uuid_bytes(challenge)?;
    Some(hex(&hmac_sha256(key, &uuid)))
}

fn uuid_bytes(uuid: &str) -> Option<[u8; 16]> {
    let digits: Vec<u8> = uuid.bytes().filter(|b| *b != b'-').collect();
    if digits.len() != 32 {
        return None;
    }
    let mut out = [0u8; 16];
    for (i, pair) in digits.chunks(2).enumerate() {
        let hi = (pair[0] as char).to_digit(16)?;
        let lo = (pair[1] as char).to_digit(16)?;
        out[i] = (hi * 16 + lo) as u8;
    }
    Some(out)
}

fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    const BLOCK: usize = 64;
    let mut block = [0u8; BLOCK];
    if key.len() > BLOCK {
        block[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        block[..key.len()].copy_from_slice(key);
    }

    let ipad: Vec<u8> = block.iter().map(|b| b ^ 0x36).collect();
    let opad: Vec<u8> = block.iter().map(|b| b ^ 0x5c).collect();

    let mut inner = Sha256::new();
    inner.update(&ipad);
    inner.update(message);
    let inner = inner.finalize();

    let mut outer = Sha256::new();
    outer.update(&opad);
    outer.update(inner);
    outer.finalize().into()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufRead;
    use std::net::TcpListener;

    #[test]
    fn parses_server_addresses() {
        assert_eq!(
            BoreServer::parse("bore.pub").unwrap(),
            BoreServer {
                host: "bore.pub".into(),
                port: CONTROL_PORT
            }
        );
        assert_eq!(
            BoreServer::parse("tunnel.example.com:9000").unwrap().port,
            9000
        );
        assert_eq!(BoreServer::parse("[::1]:7000").unwrap().host, "::1");
        assert_eq!(BoreServer::parse("[::1]").unwrap().port, CONTROL_PORT);
        assert_eq!(
            BoreServer::parse("[::1]:7000").unwrap().to_string(),
            "[::1]:7000"
        );

        assert!(BoreServer::parse("").is_err());
        assert!(BoreServer::parse(":7835").is_err());
        assert!(BoreServer::parse("host:0").is_err());
        assert!(BoreServer::parse("host:port").is_err());
        assert!(BoreServer::parse("::1").is_err());
    }

    #[test]
    fn messages_match_the_wire_format() {
        // What bore's serde derives produce; see `shared.rs` upstream.
        assert_eq!(
            serde_json::to_string(&ClientMessage::Hello(0)).unwrap(),
            r#"{"Hello":0}"#
        );
        assert_eq!(
            serde_json::to_string(&ClientMessage::Accept("a-b".into())).unwrap(),
            r#"{"Accept":"a-b"}"#
        );
        assert!(matches!(
            serde_json::from_str(r#""Heartbeat""#).unwrap(),
            ServerMessage::Heartbeat
        ));
        assert!(matches!(
            serde_json::from_str(r#"{"Hello":41234}"#).unwrap(),
            ServerMessage::Hello(41234)
        ));
    }

    #[test]
    fn hmac_matches_rfc_4231() {
        // Test case 2
        let tag = hmac_sha256(b"Jefe", b"what do ya want for nothing?");
        assert_eq!(
            hex(&tag),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
        // Test case 6: a key longer than the block is hashed first
        let tag = hmac_sha256(
            &[0xaa; 131],
            b"Test Using Larger Than Block-Size Key - Hash Key First",
        );
        assert_eq!(
            hex(&tag),
            "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54"
        );
    }

    #[test]
    fn parses_uuids() {
        let bytes = uuid_bytes("67e55044-10b1-426f-9247-bb680e5fe0c8").unwrap();
        assert_eq!(bytes[0], 0x67);
        assert_eq!(bytes[15], 0xc8);
        assert!(uuid_bytes("not-a-uuid").is_none());
        assert!(uuid_bytes("67e55044-10b1-426f-9247-bb680e5fe0cz").is_none());
    }

    #[test]
    fn recv_stops_at_the_delimiter() {
        let mut input: &[u8] = b"\"Heartbeat\"\0visitor bytes";
        assert!(matches!(
            recv(&mut input).unwrap(),
            Some(ServerMessage::Heartbeat)
        ));
        assert_eq!(input, b"visitor bytes");

        let mut empty: &[u8] = b"";
        assert!(recv(&mut empty).unwrap().is_none());

        let long = vec![b'x'; MAX_FRAME_LENGTH + 1];
        assert!(recv(&mut long.as_slice()).is_err());
    }

    /// A minimal bore server: enough of `server.rs` upstream to drive the
    /// client through a handshake, an optional challenge, and one visitor.
    struct FakeBore {
        listener: TcpListener,
        public: TcpListener,
    }

    fn read_frame(reader: &mut impl BufRead) -> String {
        let mut buf = Vec::new();
        reader.read_until(0, &mut buf).unwrap();
        buf.pop();
        String::from_utf8(buf).unwrap()
    }

    fn write_frame(stream: &mut TcpStream, json: &str) {
        stream.write_all(json.as_bytes()).unwrap();
        stream.write_all(&[0]).unwrap();
    }

    impl FakeBore {
        fn new() -> Self {
            Self {
                listener: TcpListener::bind("127.0.0.1:0").unwrap(),
                public: TcpListener::bind("127.0.0.1:0").unwrap(),
            }
        }

        fn server(&self) -> BoreServer {
            BoreServer {
                host: "127.0.0.1".into(),
                port: self.listener.local_addr().unwrap().port(),
            }
        }

        fn public_port(&self) -> u16 {
            self.public.local_addr().unwrap().port()
        }

        /// Accept a connection and, with a secret, check its answer.
        fn accept(&self, secret: Option<&str>) -> (TcpStream, io::BufReader<TcpStream>) {
            let (mut stream, _) = self.listener.accept().unwrap();
            let mut reader = io::BufReader::new(stream.try_clone().unwrap());
            if let Some(secret) = secret {
                let challenge = "67e55044-10b1-426f-9247-bb680e5fe0c8";
                write_frame(&mut stream, &format!(r#"{{"Challenge":"{challenge}"}}"#));
                let key: [u8; 32] = Sha256::digest(secret.as_bytes()).into();
                let expected = answer_challenge(&key, challenge).unwrap();
                assert_eq!(
                    read_frame(&mut reader),
                    format!(r#"{{"Authenticate":"{expected}"}}"#)
                );
            }
            (stream, reader)
        }
    }

    /// Drive one visitor through the client: handshake, heartbeat, a
    /// `Connection`, the `Accept` on a second connection, then raw bytes to
    /// and from a local app that answers "pong:" plus what it read.
    fn visitor_round_trip(secret: Option<&'static str>) {
        let app = TcpListener::bind("127.0.0.1:0").unwrap();
        let app_port = app.local_addr().unwrap().port();
        thread::spawn(move || {
            let (mut conn, _) = app.accept().unwrap();
            let mut request = String::new();
            conn.read_to_string(&mut request).unwrap();
            conn.write_all(format!("pong:{request}").as_bytes())
                .unwrap();
        });

        let bore = FakeBore::new();
        let server = bore.server();
        let public_port = bore.public_port();

        let (checked_tx, checked_rx) = std::sync::mpsc::channel::<()>();
        let fake = thread::spawn(move || {
            let (mut control, mut control_reader) = bore.accept(secret);
            assert_eq!(read_frame(&mut control_reader), r#"{"Hello":0}"#);
            write_frame(&mut control, &format!(r#"{{"Hello":{public_port}}}"#));
            write_frame(&mut control, r#""Heartbeat""#);

            let visitor = thread::spawn(move || {
                let mut visitor = TcpStream::connect(("127.0.0.1", public_port)).unwrap();
                visitor.write_all(b"ping").unwrap();
                visitor.shutdown(Shutdown::Write).unwrap();
                let mut reply = String::new();
                visitor.read_to_string(&mut reply).unwrap();
                reply
            });
            let (visitor_side, _) = bore.public.accept().unwrap();
            let id = "11111111-2222-3333-4444-555555555555";
            write_frame(&mut control, &format!(r#"{{"Connection":"{id}"}}"#));

            let (data, mut data_reader) = bore.accept(secret);
            assert_eq!(
                read_frame(&mut data_reader),
                format!(r#"{{"Accept":"{id}"}}"#)
            );
            let (mut visitor_read, mut data_write) =
                (visitor_side.try_clone().unwrap(), data.try_clone().unwrap());
            let up = thread::spawn(move || {
                io::copy(&mut visitor_read, &mut data_write).unwrap();
                data_write.shutdown(Shutdown::Write).unwrap();
            });
            let mut visitor_write = visitor_side;
            io::copy(&mut data_reader, &mut visitor_write).unwrap();
            visitor_write.shutdown(Shutdown::Write).unwrap();
            up.join().unwrap();
            let reply = visitor.join().unwrap();
            // Held until the test has read the status, which closing the
            // control connection would turn into Reconnecting
            let _ = checked_rx.recv_timeout(Duration::from_secs(5));
            drop(control);
            reply
        });

        let client = BoreClient::start(server, secret, Some(app_port)).unwrap();
        assert_eq!(
            client.wait_connected(Duration::from_secs(5)),
            TunnelStatus::Connected
        );
        assert_eq!(client.public_port(), Some(public_port));
        assert_eq!(
            client.public_url(),
            Some(format!("http://127.0.0.1:{public_port}"))
        );
        checked_tx.send(()).unwrap();

        assert_eq!(fake.join().unwrap(), "pong:ping");
    }

    #[test]
    fn forwards_a_visitor_to_the_target() {
        visitor_round_trip(None);
    }

    #[test]
    fn answers_the_challenge_on_every_connection() {
        visitor_round_trip(Some("mysecret123"));
    }

    #[test]
    fn reports_a_server_that_wants_a_secret() {
        let bore = FakeBore::new();
        let server = bore.server();
        let fake = thread::spawn(move || {
            let (mut control, _) = bore.accept(None);
            write_frame(
                &mut control,
                r#"{"Challenge":"67e55044-10b1-426f-9247-bb680e5fe0c8"}"#,
            );
            // Keep the listener alive until the client has read the refusal
            thread::sleep(Duration::from_millis(500));
        });

        let client = BoreClient::start(server, None, None).unwrap();
        assert_eq!(
            client.wait_connected(Duration::from_secs(5)),
            TunnelStatus::Failed
        );
        assert!(client.last_error().unwrap().contains("requires a secret"));
        fake.join().unwrap();
    }

    #[test]
    fn stop_interrupts_a_live_tunnel() {
        let bore = FakeBore::new();
        let server = bore.server();
        let public_port = bore.public_port();
        thread::spawn(move || {
            let (mut control, mut reader) = bore.accept(None);
            read_frame(&mut reader);
            write_frame(&mut control, &format!(r#"{{"Hello":{public_port}}}"#));
            // Never heartbeat again: the client is parked in a read
            thread::sleep(Duration::from_secs(30));
        });

        let mut client = BoreClient::start(server, None, None).unwrap();
        assert_eq!(
            client.wait_connected(Duration::from_secs(5)),
            TunnelStatus::Connected
        );
        let started = Instant::now();
        client.stop();
        assert!(started.elapsed() < Duration::from_secs(2));
        assert_eq!(client.status(), TunnelStatus::Disconnected);
    }

    #[test]
    fn target_can_move() {
        let client = BoreClient::start(
            BoreServer {
                host: "127.0.0.1".into(),
                port: 1,
            },
            None,
            None,
        )
        .unwrap();
        let target = || client.shared.target.load(Ordering::SeqCst);
        assert_eq!(target(), 0);
        client.set_target(Some(3000));
        assert_eq!(target(), 3000);
        client.set_target(None);
        assert_eq!(target(), 0);
    }
}
