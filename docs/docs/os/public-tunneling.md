# Public Tunneling

Expose your local WASM applications to the internet using the Bore tunneling protocol.

## Overview

The public tunneling feature allows you to make your WASM applications running in OS mode accessible from the internet. This is useful for:

- **Demos and testing**: Share your work-in-progress with others
- **Webhooks**: Receive callbacks from external services
- **Remote access**: Access your local development environment from anywhere
- **API development**: Test integrations with external systems

wasmrun supports the [bore](https://github.com/ekzhang/bore) tunneling protocol, providing a simple TCP-based tunnel without requiring TLS/HTTPS setup.

## Quick Start

### Using Public bore.pub Server

The simplest way to expose your app is using the public bore.pub server:

```bash
# Start your OS mode application
wasmrun os -p ./my-app

# In the UI or via API, start the tunnel
# POST /api/tunnel/start
```

Your app will be assigned a public URL like: `http://bore.pub:12345`

### Using a Custom Bore Server

For production use or custom domains, you can run your own bore server:

```bash
# On your VPS (one-time setup)
cargo install bore-cli
bore server --secret mysecret123

# Configure wasmrun to use your server
# Update OsRunConfig with:
# - tunnel_server: "tunnel.mydomain.com:7835"
# - tunnel_secret: "mysecret123"
```

## Configuration

### OsRunConfig Options

```rust
pub struct OsRunConfig {
    // ... other fields
    pub expose: bool,                    // Enable public tunneling
    pub tunnel_server: Option<String>,   // Custom bore server (default: "bore.pub:7835")
    pub tunnel_secret: Option<String>,   // Authentication secret for private servers
}
```

### Example Configuration

```rust
let config = OsRunConfig {
    project_path: "./my-app".to_string(),
    expose: true,
    tunnel_server: Some("tunnel.mydomain.com:7835".to_string()),
    tunnel_secret: Some("mysecret123".to_string()),
    // ... other fields
};
```

## REST API

### Start Tunnel

Start a public tunnel connection.

```http
POST /api/tunnel/start
```

**Response:**
```json
{
  "success": true,
  "public_url": "http://bore.pub:12345",
  "status": "Connected"
}
```

### Get Tunnel Status

Check the current tunnel status and public URL.

```http
GET /api/tunnel/status
```

**Response:**
```json
{
  "success": true,
  "status": "Connected",
  "public_url": "http://bore.pub:12345",
  "public_port": 12345
}
```

**Status Values:**
- `Not started` - No tunnel has been initiated
- `Disconnected` - Tunnel was stopped
- `Connecting` - Establishing connection to bore server
- `Connected` - Tunnel is active
- `Reconnecting` - Attempting to restore connection
- `Failed` - Connection failed

### Stop Tunnel

Stop the active tunnel connection.

```http
POST /api/tunnel/stop
```

**Response:**
```json
{
  "success": true,
  "message": "Tunnel stopped"
}
```

## Bore Protocol

wasmrun implements the bore protocol as follows:

1. **Handshake**: Send `HELLO 1\n` to identify protocol version
2. **Authentication** (optional): Send `AUTH <secret>\n` for private servers
3. **Port Registration**: Send `<local_port>\n` to register the port to expose
4. **Response**: Server responds with `OK <public_port>\n`
5. **Keepalive**: Background thread maintains connection and handles reconnection

## Connection Management

### Automatic Reconnection

The bore client includes automatic reconnection logic:

- Background keepalive thread monitors connection health
- On disconnect, status changes to `Reconnecting`
- Automatic retry with exponential backoff
- Status changes to `Connected` on successful reconnection
- Status changes to `Failed` if reconnection attempts fail

### Connection Lifecycle

```sh
┌─────────────┐
│ Disconnected│
└──────┬──────┘
       │ start()
       ▼
┌─────────────┐
│ Connecting  │
└──────┬──────┘
       │ success
       ▼
┌─────────────┐    disconnect    ┌──────────────┐
│  Connected  │ ───────────────> │ Reconnecting │
└─────────────┘                  └──────┬───────┘
       │                                │
       │ stop()                   retry │
       ▼                                │
┌─────────────┐    failed        ┌─────▼────────┐
│ Disconnected│ <─────────────── │    Failed    │
└─────────────┘                  └──────────────┘
```

## Security Considerations

### Public bore.pub Server

- **Plain TCP**: No TLS encryption (HTTPS support planned for future releases)
- **Public server**: Anyone can see traffic if they know your port
- **Random ports**: Assigned port is unpredictable but not secret
- **Best for**: Development, testing, demos

### Private Bore Server

- **Authentication**: Use `tunnel_secret` to restrict access
- **Custom domain**: Control your own infrastructure
- **Network policy**: Restrict which destinations WASM apps can connect to
- **Best for**: Production use, sensitive data

### Recommendations

1. **Never expose sensitive data** through public tunnels without TLS
2. **Use authentication** for private bore servers
3. **Implement application-level security** (authentication, authorization)
4. **Monitor connection logs** for suspicious activity
5. **Use temporary tunnels** for development only

## Troubleshooting

### Connection Failed

**Problem**: Tunnel fails to connect

**Solutions**:
- Check bore server is running and accessible
- Verify network connectivity
- Check firewall rules allow outbound connections to bore server port
- Ensure DNS resolution works for custom servers

### Reconnection Loop

**Problem**: Tunnel continuously reconnects

**Solutions**:
- Check bore server logs for errors
- Verify authentication secret matches server configuration
- Check if port is already in use
- Monitor network stability

### Wrong Public URL

**Problem**: Public URL doesn't work

**Solutions**:
- Verify local application is running on specified port
- Check port forwarding configuration
- Ensure bore server has correct public IP/domain
- Test with `curl http://bore.pub:<port>` directly

## Advanced Usage

### Self-Hosted Setup with nginx

```nginx
# /etc/nginx/sites-available/bore-tunnel
server {
    listen 80;
    server_name tunnel.mydomain.com;

    location / {
        proxy_pass http://localhost:7835;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
}
```

### Docker Compose

```yaml
version: '3.8'
services:
  bore-server:
    image: ekzhang/bore
    command: server --secret mysecret123
    ports:
      - "7835:7835"
    restart: unless-stopped
```

### Dynamic DNS with DuckDNS

```bash
# Update DuckDNS every 5 minutes (cron)
*/5 * * * * curl "https://www.duckdns.org/update?domains=myapp&token=YOUR_TOKEN&ip="
```

## Next Steps

- [Network Isolation](./network-isolation.md) - Understanding WASM network security
- [Port Forwarding](./port-forwarding.md) - Forward ports to WASM processes
- [OS Mode](./index.md) - Complete OS mode documentation

## References

- [bore GitHub Repository](https://github.com/ekzhang/bore)
- [bore Protocol Specification](https://github.com/ekzhang/bore#how-it-works)
- [wasmrun OS Mode Architecture](../../OS_IMPLEMENTATION.md)
