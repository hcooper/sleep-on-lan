# sol - Sleep-on-LAN

A daemon that listens for Wake-on-LAN (WoL) packets and triggers system suspend instead of wake.

## Overview

This tool receives standard WoL magic packets over UDP and uses them to trigger system suspend via `systemctl suspend`. It's the inverse of Wake-on-LAN - instead of waking a sleeping machine, it puts an awake machine to sleep.

## Packet Format

Uses the standard Wake-on-LAN packet format:
- 6 bytes: `0xFF` (magic packet header)
- 96 bytes: Target MAC address repeated 16 times
- Total: 102 bytes

## Usage

### Running the daemon

```bash
# Build and run
cargo run

# Run on default port (10)
cargo run --release

# Run on custom port
cargo run --release -- --port 9999
```

### Command-line options

```
Options:
  -p, --port <PORT>  Port to listen on [default: 10]
  -h, --help         Print help
  -V, --version      Print version
```

### Sending sleep packets

You can use any standard Wake-on-LAN tool to send packets to port 10:

```bash
# Using wakeonlan tool
wakeonlan -i <target_ip> -p 10 AA:BB:CC:DD:EE:FF

# Using etherwake (requires root)
sudo etherwake -i eth0 AA:BB:CC:DD:EE:FF
```

Replace `AA:BB:CC:DD:EE:FF` with the MAC address of the target machine.

## Installation

### From source

```bash
cargo build --release
sudo cp target/release/sol /usr/local/bin/
```

### Systemd service

Create `/etc/systemd/system/sol.service`:

```ini
[Unit]
Description=Sleep-on-LAN daemon
After=network.target

[Service]
Type=simple
ExecStart=/usr/local/bin/sol --port 10
Restart=on-failure
User=root

[Install]
WantedBy=multi-user.target
```

Enable and start:

```bash
sudo systemctl enable sol
sudo systemctl start sol
```

## Security Considerations

**Warning**: This daemon has no authentication. Any device on your network that can send UDP packets to the listening port can trigger system suspend.

Recommendations:
- Use firewall rules to restrict access to trusted IP addresses
- Run on a non-standard port
- Only deploy on trusted networks

## Testing

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture
```

## Requirements

- Rust 1.70+
- Linux with systemd
- Appropriate permissions to run `systemctl suspend`

## License

MIT
