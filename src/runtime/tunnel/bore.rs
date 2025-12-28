use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq)]
pub enum TunnelStatus {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Failed,
}

pub struct BoreClient {
    server: String,
    secret: Option<String>,
    local_port: u16,
    status: Arc<Mutex<TunnelStatus>>,
    public_port: Arc<Mutex<Option<u16>>>,
    stop_signal: Arc<Mutex<bool>>,
}

impl BoreClient {
    pub fn new(server: String, secret: Option<String>, local_port: u16) -> Self {
        Self {
            server,
            secret,
            local_port,
            status: Arc::new(Mutex::new(TunnelStatus::Disconnected)),
            public_port: Arc::new(Mutex::new(None)),
            stop_signal: Arc::new(Mutex::new(false)),
        }
    }

    pub fn connect(&mut self) -> Result<String, String> {
        *self.status.lock().unwrap() = TunnelStatus::Connecting;

        let stream = self.establish_connection()?;
        let public_port = self.perform_handshake(&stream)?;

        *self.public_port.lock().unwrap() = Some(public_port);
        *self.status.lock().unwrap() = TunnelStatus::Connected;

        let public_url = self.get_public_url();

        self.start_keepalive_thread(stream);

        Ok(public_url)
    }

    fn establish_connection(&self) -> Result<TcpStream, String> {
        let server = &self.server;
        let stream = TcpStream::connect(server)
            .map_err(|e| format!("Failed to connect to {server}: {e}"))?;

        stream
            .set_read_timeout(Some(Duration::from_secs(30)))
            .map_err(|e| format!("Failed to set read timeout: {e}"))?;

        stream
            .set_write_timeout(Some(Duration::from_secs(10)))
            .map_err(|e| format!("Failed to set write timeout: {e}"))?;

        Ok(stream)
    }

    fn perform_handshake(&self, stream: &TcpStream) -> Result<u16, String> {
        let mut writer = stream
            .try_clone()
            .map_err(|e| format!("Failed to clone stream: {e}"))?;
        let reader = stream
            .try_clone()
            .map_err(|e| format!("Failed to clone stream: {e}"))?;
        let mut reader = BufReader::new(reader);

        writer
            .write_all(b"HELLO 1\n")
            .map_err(|e| format!("Failed to send HELLO: {e}"))?;
        writer
            .flush()
            .map_err(|e| format!("Failed to flush: {e}"))?;

        if let Some(ref secret) = self.secret {
            let auth_msg = format!("AUTH {secret}\n");
            writer
                .write_all(auth_msg.as_bytes())
                .map_err(|e| format!("Failed to send AUTH: {e}"))?;
            writer
                .flush()
                .map_err(|e| format!("Failed to flush: {e}"))?;
        }

        let local_port = self.local_port;
        let port_msg = format!("{local_port}\n");
        writer
            .write_all(port_msg.as_bytes())
            .map_err(|e| format!("Failed to send port: {e}"))?;
        writer
            .flush()
            .map_err(|e| format!("Failed to flush: {e}"))?;

        let mut response = String::new();
        reader
            .read_line(&mut response)
            .map_err(|e| format!("Failed to read response: {e}"))?;

        let response = response.trim();

        if response.starts_with("OK ") {
            let public_port_str = response.trim_start_matches("OK ").trim();
            public_port_str
                .parse::<u16>()
                .map_err(|e| format!("Invalid public port '{public_port_str}': {e}"))
        } else {
            Err(format!("Unexpected response from bore server: {response}"))
        }
    }

    fn start_keepalive_thread(&self, stream: TcpStream) {
        let status = Arc::clone(&self.status);
        let stop_signal = Arc::clone(&self.stop_signal);
        let server = self.server.clone();
        let secret = self.secret.clone();
        let local_port = self.local_port;
        let public_port = Arc::clone(&self.public_port);

        thread::spawn(move || {
            let mut current_stream = stream;

            loop {
                if *stop_signal.lock().unwrap() {
                    break;
                }

                let mut buf = vec![0u8; 8192];
                match current_stream.peek(&mut buf) {
                    Ok(_) => {
                        thread::sleep(Duration::from_secs(5));
                    }
                    Err(_) => {
                        *status.lock().unwrap() = TunnelStatus::Reconnecting;

                        match Self::reconnect(&server, &secret, local_port) {
                            Ok((new_stream, new_public_port)) => {
                                current_stream = new_stream;
                                *public_port.lock().unwrap() = Some(new_public_port);
                                *status.lock().unwrap() = TunnelStatus::Connected;
                            }
                            Err(_) => {
                                *status.lock().unwrap() = TunnelStatus::Failed;
                                thread::sleep(Duration::from_secs(10));
                            }
                        }
                    }
                }
            }
        });
    }

    fn reconnect(
        server: &str,
        secret: &Option<String>,
        local_port: u16,
    ) -> Result<(TcpStream, u16), String> {
        let stream = TcpStream::connect(server)
            .map_err(|e| format!("Failed to reconnect to {server}: {e}"))?;

        stream
            .set_read_timeout(Some(Duration::from_secs(30)))
            .map_err(|e| format!("Failed to set read timeout: {e}"))?;

        stream
            .set_write_timeout(Some(Duration::from_secs(10)))
            .map_err(|e| format!("Failed to set write timeout: {e}"))?;

        let mut writer = stream
            .try_clone()
            .map_err(|e| format!("Failed to clone stream: {e}"))?;
        let reader = stream
            .try_clone()
            .map_err(|e| format!("Failed to clone stream: {e}"))?;
        let mut reader = BufReader::new(reader);

        writer
            .write_all(b"HELLO 1\n")
            .map_err(|e| format!("Failed to send HELLO: {e}"))?;
        writer
            .flush()
            .map_err(|e| format!("Failed to flush: {e}"))?;

        if let Some(ref s) = secret {
            let auth_msg = format!("AUTH {s}\n");
            writer
                .write_all(auth_msg.as_bytes())
                .map_err(|e| format!("Failed to send AUTH: {e}"))?;
            writer
                .flush()
                .map_err(|e| format!("Failed to flush: {e}"))?;
        }

        let port_msg = format!("{local_port}\n");
        writer
            .write_all(port_msg.as_bytes())
            .map_err(|e| format!("Failed to send port: {e}"))?;
        writer
            .flush()
            .map_err(|e| format!("Failed to flush: {e}"))?;

        let mut response = String::new();
        reader
            .read_line(&mut response)
            .map_err(|e| format!("Failed to read response: {e}"))?;

        let response = response.trim();

        if response.starts_with("OK ") {
            let public_port_str = response.trim_start_matches("OK ").trim();
            let public_port = public_port_str
                .parse::<u16>()
                .map_err(|e| format!("Invalid public port '{public_port_str}': {e}"))?;
            Ok((stream, public_port))
        } else {
            Err(format!("Unexpected response from bore server: {response}"))
        }
    }

    pub fn get_public_url(&self) -> String {
        let port = self.public_port.lock().unwrap();
        if let Some(p) = *port {
            let host = self.server.split(':').next().unwrap_or(&self.server);
            format!("http://{host}:{p}")
        } else {
            String::from("No public URL available")
        }
    }

    pub fn get_status(&self) -> TunnelStatus {
        self.status.lock().unwrap().clone()
    }

    pub fn get_public_port(&self) -> Option<u16> {
        *self.public_port.lock().unwrap()
    }

    pub fn stop(&self) {
        *self.stop_signal.lock().unwrap() = true;
        *self.status.lock().unwrap() = TunnelStatus::Disconnected;
    }
}

impl Drop for BoreClient {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bore_client_creation() {
        let client = BoreClient::new("bore.pub:7835".to_string(), None, 3000);
        assert_eq!(client.server, "bore.pub:7835");
        assert_eq!(client.local_port, 3000);
        assert_eq!(client.get_status(), TunnelStatus::Disconnected);
    }

    #[test]
    fn test_bore_client_with_secret() {
        let client = BoreClient::new(
            "tunnel.mydomain.com:7835".to_string(),
            Some("mysecret123".to_string()),
            3000,
        );
        assert_eq!(client.secret, Some("mysecret123".to_string()));
    }

    #[test]
    fn test_public_url_format() {
        let client = BoreClient::new("bore.pub:7835".to_string(), None, 3000);
        *client.public_port.lock().unwrap() = Some(12345);
        assert_eq!(client.get_public_url(), "http://bore.pub:12345");
    }
}
