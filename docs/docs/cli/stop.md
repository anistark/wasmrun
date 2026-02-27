# stop

Stop any running Wasmrun server instance.

## Synopsis

```sh
wasmrun stop
```

**Aliases:** `kill`

## Description

The `stop` command gracefully terminates any running Wasmrun development server. This is useful when:

- You started a server in the background
- The server is running in a different terminal
- You want to free up the port
- You need to restart the server with different options

## Usage

### Stop Running Server

```sh
wasmrun stop
```

Output:

```
🛑 Stopping Wasmrun server...
   ✓ Server stopped (PID: 12345)
```

If no server is running:

```
ℹ️  No Wasmrun server is currently running
```

## How It Works

The stop command:

1. Checks for running Wasmrun server processes
2. Finds the process ID (PID)
3. Sends a graceful shutdown signal (SIGTERM)
4. Waits for the server to terminate
5. Confirms shutdown

## Examples

### Basic Usage

Stop the server:

```sh
wasmrun stop
```

### Check if Server is Running

```sh
wasmrun stop
# If output says "No server running", port is free
```

### Stop and Restart

```sh
wasmrun stop
wasmrun run --port 3000
```

### Stop Before Changing Projects

```sh
wasmrun stop
cd ../other-project
wasmrun run
```

### Force Stop with System Commands

If `wasmrun stop` doesn't work:

```sh
# Find process
ps aux | grep wasmrun

# Kill by PID
kill -9 <PID>

# Or kill all wasmrun processes
pkill -9 wasmrun
```

## Common Scenarios

### Port Already in Use

If you see "port already in use":

```sh
# Stop existing server
wasmrun stop

# Start on same port
wasmrun run --port 8420
```

Or use a different port:

```sh
wasmrun run --port 8421
```

### Server Started in Background

If you started a server and closed the terminal:

```sh
# Stop it from any terminal
wasmrun stop
```

### Multiple Projects Running

Only one server can run at a time. To switch projects:

```sh
# Stop current server
wasmrun stop

# Navigate and start new project
cd ../my-other-project
wasmrun run
```

### Before System Shutdown

Clean shutdown before logging off:

```sh
wasmrun stop
wasmrun clean --all
```

## Server State

Wasmrun tracks the server process using:

- Process ID file: `.wasmrun-server/pid`
- Port information: `.wasmrun-server/port`

These files are automatically created when the server starts and cleaned up when it stops.

## Troubleshooting

### Stop Command Hangs

If `wasmrun stop` doesn't respond:

```sh
# Force kill
pkill -9 wasmrun
```

### Process Still Running

Check if the process is actually running:

```sh
# List wasmrun processes
ps aux | grep wasmrun

# Check specific port
lsof -i :8420
```

### Server Restarts Immediately

If server keeps restarting, check for:

- System service auto-restart
- Process monitor (PM2, systemd)
- Development tool (nodemon, cargo-watch)

### Permission Denied

If you lack permission to stop the process:

```sh
# Check who owns the process
ps aux | grep wasmrun

# If it's yours but permission denied:
sudo wasmrun stop
```

### Cannot Find PID File

If the PID file is missing:

```sh
# Manually find and kill
ps aux | grep "wasmrun.*run"
kill <PID>
```

## Graceful vs Force Shutdown

### Graceful Shutdown (Default)

`wasmrun stop` sends SIGTERM:
- Allows cleanup
- Closes connections properly
- Saves state
- Removes temp files

### Force Shutdown

Only use if graceful fails:

```sh
pkill -9 wasmrun
```

Force shutdown (SIGKILL):
- Immediate termination
- No cleanup
- May leave temp files
- May cause port conflicts

## Integration with Other Commands

### Before Cleaning

```sh
wasmrun stop
wasmrun clean --all
```

### Before Compilation

Not necessary, but good practice:

```sh
wasmrun stop
wasmrun compile --optimization release
```

### Before Plugin Updates

```sh
wasmrun stop
wasmrun plugin update all
wasmrun run
```

## Monitoring Server Status

### Check if Server is Running

```sh
# Method 1: Try to stop
wasmrun stop

# Method 2: Check process
ps aux | grep "wasmrun.*run"

# Method 3: Check port
lsof -i :8420
```

### Get Server PID

```sh
# From PID file
cat .wasmrun-server/pid

# From process list
pgrep -f "wasmrun.*run"
```

### Get Server Port

```sh
# From port file
cat .wasmrun-server/port

# From process
lsof -i -P | grep wasmrun
```

## Automation

### Stop All Wasmrun Processes

```sh
#!/bin/bash
# stop-all.sh
wasmrun stop
pkill wasmrun
echo "All Wasmrun processes stopped"
```

### Stop Before Sleep/Shutdown

Add to system shutdown hooks:

```sh
# ~/.bash_logout or shutdown script
wasmrun stop
```

### CI/CD Cleanup

```sh
# In CI script after tests
wasmrun stop || true
wasmrun clean --all
```

## Exit Codes

- `0` - Server stopped successfully (or wasn't running)
- `1` - Error occurred during stop

## See Also

- [run](./run.md) - Start development server
- [clean](./clean.md) - Clean build artifacts
- [os](./os.md) - OS mode server management
