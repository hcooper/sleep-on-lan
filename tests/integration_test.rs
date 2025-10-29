use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::time::timeout;

fn create_wol_packet(mac: &[u8; 6]) -> Vec<u8> {
    let mut packet = vec![0xFF; 6];
    for _ in 0..16 {
        packet.extend_from_slice(mac);
    }
    packet
}

#[tokio::test]
async fn test_udp_socket_receives_packet() {
    // Bind to a random port for testing
    let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let addr = socket.local_addr().unwrap();

    // Create a sender socket
    let sender = UdpSocket::bind("127.0.0.1:0").await.unwrap();

    // Send a WoL packet
    let mac = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];
    let packet = create_wol_packet(&mac);
    sender.send_to(&packet, addr).await.unwrap();

    // Receive the packet
    let mut buf = [0u8; 1024];
    let result = timeout(Duration::from_secs(1), socket.recv_from(&mut buf)).await;

    assert!(result.is_ok());
    let (len, _peer) = result.unwrap().unwrap();
    assert_eq!(len, 102);
    assert_eq!(&buf[..6], &[0xFF; 6]);
}

#[tokio::test]
async fn test_broadcast_packet_format() {
    // Test that we can create and parse a broadcast-style packet
    let mac = [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC];
    let packet = create_wol_packet(&mac);

    assert_eq!(packet.len(), 102);
    assert_eq!(&packet[0..6], &[0xFF; 6]);

    // Verify MAC is repeated 16 times
    for i in 0..16 {
        let start = 6 + i * 6;
        let end = start + 6;
        assert_eq!(&packet[start..end], &mac);
    }
}
