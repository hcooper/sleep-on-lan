use clap::Parser;
use pnet::datalink;
use std::process::Command;
use tokio::net::UdpSocket;

/// Sleep-on-LAN daemon - receives WoL-format UDP packets to trigger system suspend
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Port to listen on
    #[arg(short, long, default_value = "10")]
    port: u16,
}

const MAGIC_PACKET_HEADER: [u8; 6] = [0xFF; 6];
const EXPECTED_PACKET_SIZE: usize = 102; // 6 (header) + 16*6 (MAC repeated 16 times)

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Get local MAC addresses
    let local_macs = get_local_mac_addresses();
    if local_macs.is_empty() {
        eprintln!("Warning: No network interfaces with MAC addresses found");
    } else {
        println!("Monitoring for WoL packets targeting:");
        for mac in &local_macs {
            println!("  {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
                     mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]);
        }
    }

    // Bind to UDP socket
    let addr = format!("0.0.0.0:{}", args.port);
    let socket = UdpSocket::bind(&addr).await?;
    println!("Sleep-on-LAN daemon listening on {}", addr);

    let mut buf = [0u8; 1024];

    loop {
        let (len, peer) = socket.recv_from(&mut buf).await?;
        let packet = &buf[..len];

        match validate_wol_packet(packet, &local_macs) {
            Ok(mac) => {
                println!("Valid WoL packet received from {} for MAC {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
                         peer, mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]);

                match suspend_system() {
                    Ok(_) => println!("System suspend initiated"),
                    Err(e) => eprintln!("Failed to suspend system: {}", e),
                }
            }
            Err(e) => {
                eprintln!("Received invalid packet from {}: {}", peer, e);
            }
        }
    }
}

fn validate_wol_packet(packet: &[u8], local_macs: &[[u8; 6]]) -> Result<[u8; 6], String> {
    if packet.len() < EXPECTED_PACKET_SIZE {
        return Err(format!("Invalid size: {} (expected {})", packet.len(), EXPECTED_PACKET_SIZE));
    }

    // Verify magic packet header (6 bytes of 0xFF)
    if &packet[0..6] != MAGIC_PACKET_HEADER {
        return Err("Invalid header".to_string());
    }

    // Extract MAC address (should be repeated 16 times after header)
    let mac = &packet[6..12];

    // Verify MAC is repeated 16 times
    for i in 1..16 {
        if &packet[6 + i*6..6 + (i+1)*6] != mac {
            return Err("Invalid MAC repetition".to_string());
        }
    }

    let mut mac_array = [0u8; 6];
    mac_array.copy_from_slice(mac);

    // Verify MAC matches one of the local interfaces
    if !local_macs.contains(&mac_array) {
        return Err(format!(
            "MAC address {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x} does not match any local interface",
            mac_array[0], mac_array[1], mac_array[2], mac_array[3], mac_array[4], mac_array[5]
        ));
    }

    Ok(mac_array)
}

fn get_local_mac_addresses() -> Vec<[u8; 6]> {
    let mut macs = Vec::new();

    for iface in datalink::interfaces() {
        if let Some(mac) = iface.mac {
            macs.push(mac.octets());
        }
    }

    macs
}

fn suspend_system() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("systemctl")
        .arg("suspend")
        .output()?;

    if !output.status.success() {
        return Err(format!(
            "systemctl suspend failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ).into());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_valid_wol_packet(mac: &[u8; 6]) -> Vec<u8> {
        let mut packet = vec![0xFF; 6];
        for _ in 0..16 {
            packet.extend_from_slice(mac);
        }
        packet
    }

    #[test]
    fn test_valid_wol_packet() {
        let mac = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];
        let packet = create_valid_wol_packet(&mac);
        let local_macs = vec![mac];

        let result = validate_wol_packet(&packet, &local_macs);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), mac);
    }

    #[test]
    fn test_packet_too_short() {
        let packet = vec![0xFF; 50];
        let local_macs = vec![[0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]];
        let result = validate_wol_packet(&packet, &local_macs);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid size"));
    }

    #[test]
    fn test_invalid_header() {
        let mut packet = vec![0xAA; 6];
        let mac = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66];
        for _ in 0..16 {
            packet.extend_from_slice(&mac);
        }

        let local_macs = vec![mac];
        let result = validate_wol_packet(&packet, &local_macs);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid header"));
    }

    #[test]
    fn test_invalid_mac_repetition() {
        let mut packet = vec![0xFF; 6];
        let mac1 = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];
        let mac2 = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66];

        packet.extend_from_slice(&mac1);
        for _ in 1..16 {
            packet.extend_from_slice(&mac2);
        }

        let local_macs = vec![mac1, mac2];
        let result = validate_wol_packet(&packet, &local_macs);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid MAC repetition"));
    }

    #[test]
    fn test_exact_packet_size() {
        let mac = [0x00, 0x11, 0x22, 0x33, 0x44, 0x55];
        let packet = create_valid_wol_packet(&mac);
        assert_eq!(packet.len(), EXPECTED_PACKET_SIZE);

        let local_macs = vec![mac];
        let result = validate_wol_packet(&packet, &local_macs);
        assert!(result.is_ok());
    }

    #[test]
    fn test_different_mac_addresses() {
        let test_macs = [
            [0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
            [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
            [0x01, 0x23, 0x45, 0x67, 0x89, 0xAB],
        ];

        for mac in &test_macs {
            let packet = create_valid_wol_packet(mac);
            let local_macs = vec![*mac];
            let result = validate_wol_packet(&packet, &local_macs);
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), *mac);
        }
    }

    #[test]
    fn test_mac_not_in_local_interfaces() {
        let packet_mac = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];
        let local_mac = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66];
        let packet = create_valid_wol_packet(&packet_mac);
        let local_macs = vec![local_mac];

        let result = validate_wol_packet(&packet, &local_macs);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not match any local interface"));
    }
}
