use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};

use crate::runtime::microkernel::Pid;

pub type SocketFd = i32;
pub type HostPort = u16;
pub type GuestPort = u16;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketProtocol {
    Tcp,
    Udp,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct PortMapping {
    pub guest_port: GuestPort,
    pub host_port: HostPort,
    pub protocol: SocketProtocol,
    pub created_at: std::time::SystemTime,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub socket_fd: SocketFd,
    pub local_addr: SocketAddr,
    pub peer_addr: Option<SocketAddr>,
    pub protocol: SocketProtocol,
    pub state: ConnectionState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Listening,
    Connected,
}

pub struct NetworkNamespace {
    pid: Pid,
    base_port: u16,
    port_mappings: Arc<RwLock<HashMap<GuestPort, PortMapping>>>,
    connections: Arc<RwLock<HashMap<SocketFd, ConnectionInfo>>>,
    #[allow(dead_code)]
    next_host_port: Arc<RwLock<u16>>,
}

impl NetworkNamespace {
    pub fn new(pid: Pid) -> Self {
        let base_port = Self::calculate_base_port(pid);

        Self {
            pid,
            base_port,
            port_mappings: Arc::new(RwLock::new(HashMap::new())),
            connections: Arc::new(RwLock::new(HashMap::new())),
            next_host_port: Arc::new(RwLock::new(base_port)),
        }
    }

    pub fn calculate_base_port(pid: Pid) -> u16 {
        let base = 10000_u32 + (pid * 1000);
        if base > 65535 {
            ((base % 55535) + 10000) as u16
        } else {
            base as u16
        }
    }

    #[allow(dead_code)]
    pub fn allocate_port(&self, guest_port: GuestPort, protocol: SocketProtocol) -> Result<u16> {
        let mut mappings = self.port_mappings.write().unwrap();

        if mappings.contains_key(&guest_port) {
            anyhow::bail!("Port {guest_port} already allocated");
        }

        let mut next_port = self.next_host_port.write().unwrap();
        let host_port = *next_port;
        *next_port += 1;

        if *next_port >= self.base_port + 1000 {
            *next_port = self.base_port;
        }

        let mapping = PortMapping {
            guest_port,
            host_port,
            protocol,
            created_at: std::time::SystemTime::now(),
        };

        mappings.insert(guest_port, mapping);
        Ok(host_port)
    }

    #[allow(dead_code)]
    pub fn deallocate_port(&self, guest_port: GuestPort) -> Result<()> {
        let mut mappings = self.port_mappings.write().unwrap();

        if mappings.remove(&guest_port).is_none() {
            anyhow::bail!("Port {guest_port} not allocated");
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub fn get_host_port(&self, guest_port: GuestPort) -> Option<HostPort> {
        let mappings = self.port_mappings.read().unwrap();
        mappings.get(&guest_port).map(|m| m.host_port)
    }

    #[allow(dead_code)]
    pub fn register_connection(
        &self,
        socket_fd: SocketFd,
        local_addr: SocketAddr,
        peer_addr: Option<SocketAddr>,
        protocol: SocketProtocol,
        state: ConnectionState,
    ) -> Result<()> {
        let mut connections = self.connections.write().unwrap();

        let info = ConnectionInfo {
            socket_fd,
            local_addr,
            peer_addr,
            protocol,
            state,
        };

        connections.insert(socket_fd, info);
        Ok(())
    }

    #[allow(dead_code)]
    pub fn update_connection_state(
        &self,
        socket_fd: SocketFd,
        state: ConnectionState,
    ) -> Result<()> {
        let mut connections = self.connections.write().unwrap();

        if let Some(conn) = connections.get_mut(&socket_fd) {
            conn.state = state;
            Ok(())
        } else {
            anyhow::bail!("Connection with fd {socket_fd} not found")
        }
    }

    #[allow(dead_code)]
    pub fn unregister_connection(&self, socket_fd: SocketFd) -> Result<()> {
        let mut connections = self.connections.write().unwrap();

        if connections.remove(&socket_fd).is_none() {
            anyhow::bail!("Connection with fd {socket_fd} not found");
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub fn get_connection(&self, socket_fd: SocketFd) -> Option<ConnectionInfo> {
        let connections = self.connections.read().unwrap();
        connections.get(&socket_fd).cloned()
    }

    #[allow(dead_code)]
    pub fn list_port_mappings(&self) -> Vec<PortMapping> {
        let mappings = self.port_mappings.read().unwrap();
        mappings.values().cloned().collect()
    }

    #[allow(dead_code)]
    pub fn list_connections(&self) -> Vec<ConnectionInfo> {
        let connections = self.connections.read().unwrap();
        connections.values().cloned().collect()
    }

    pub fn get_stats(&self) -> NetworkStats {
        let mappings = self.port_mappings.read().unwrap();
        let connections = self.connections.read().unwrap();

        let active_connections = connections
            .values()
            .filter(|c| c.state == ConnectionState::Connected)
            .count();

        let listening_sockets = connections
            .values()
            .filter(|c| c.state == ConnectionState::Listening)
            .count();

        NetworkStats {
            pid: self.pid,
            base_port: self.base_port,
            allocated_ports: mappings.len(),
            total_connections: connections.len(),
            active_connections,
            listening_sockets,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkStats {
    pub pid: Pid,
    pub base_port: u16,
    pub allocated_ports: usize,
    pub total_connections: usize,
    pub active_connections: usize,
    pub listening_sockets: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_port_calculation() {
        assert_eq!(NetworkNamespace::calculate_base_port(0), 10000);
        assert_eq!(NetworkNamespace::calculate_base_port(1), 11000);
        assert_eq!(NetworkNamespace::calculate_base_port(10), 20000);
    }

    #[test]
    fn test_port_allocation() {
        let ns = NetworkNamespace::new(1);

        let port1 = ns.allocate_port(8080, SocketProtocol::Tcp).unwrap();
        assert_eq!(port1, 11000);

        let port2 = ns.allocate_port(8081, SocketProtocol::Tcp).unwrap();
        assert_eq!(port2, 11001);

        assert!(ns.allocate_port(8080, SocketProtocol::Tcp).is_err());
    }

    #[test]
    fn test_port_deallocation() {
        let ns = NetworkNamespace::new(1);

        ns.allocate_port(8080, SocketProtocol::Tcp).unwrap();
        assert!(ns.deallocate_port(8080).is_ok());
        assert!(ns.deallocate_port(8080).is_err());
    }

    #[test]
    fn test_connection_tracking() {
        let ns = NetworkNamespace::new(1);
        let addr = "127.0.0.1:8080".parse().unwrap();

        ns.register_connection(
            3,
            addr,
            None,
            SocketProtocol::Tcp,
            ConnectionState::Listening,
        )
        .unwrap();

        let conn = ns.get_connection(3).unwrap();
        assert_eq!(conn.socket_fd, 3);
        assert_eq!(conn.state, ConnectionState::Listening);

        ns.update_connection_state(3, ConnectionState::Connected)
            .unwrap();
        let conn = ns.get_connection(3).unwrap();
        assert_eq!(conn.state, ConnectionState::Connected);

        ns.unregister_connection(3).unwrap();
        assert!(ns.get_connection(3).is_none());
    }

    #[test]
    fn test_network_stats() {
        let ns = NetworkNamespace::new(5);
        let addr = "127.0.0.1:8080".parse().unwrap();

        ns.allocate_port(8080, SocketProtocol::Tcp).unwrap();
        ns.allocate_port(8081, SocketProtocol::Udp).unwrap();

        ns.register_connection(
            3,
            addr,
            None,
            SocketProtocol::Tcp,
            ConnectionState::Listening,
        )
        .unwrap();
        ns.register_connection(
            4,
            addr,
            Some(addr),
            SocketProtocol::Tcp,
            ConnectionState::Connected,
        )
        .unwrap();

        let stats = ns.get_stats();
        assert_eq!(stats.pid, 5);
        assert_eq!(stats.base_port, 15000);
        assert_eq!(stats.allocated_ports, 2);
        assert_eq!(stats.total_connections, 2);
        assert_eq!(stats.active_connections, 1);
        assert_eq!(stats.listening_sockets, 1);
    }
}
