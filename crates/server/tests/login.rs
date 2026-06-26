//! Integration test for the offline login flow: a tiny Rust client performs
//! handshake -> Login Start -> Login Success -> Login Acknowledged ->
//! Configuration Disconnect, and checks each server response.

use std::sync::Arc;

use leather_protocol::{PacketWriter, read_frame, write_frame, write_varint};
use leather_server::config::ServerConfig;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};

/// Handshake body: id 0x00, protocol, address, port, next_state.
fn handshake_body(protocol: i32, addr: &str, port: u16, next_state: i32) -> Vec<u8> {
    let mut body = PacketWriter::new(0x00).into_body();
    write_varint(&mut body, protocol);
    write_varint(&mut body, addr.len() as i32);
    body.extend_from_slice(addr.as_bytes());
    body.extend_from_slice(&port.to_be_bytes());
    write_varint(&mut body, next_state);
    body
}

async fn spawn_server() -> std::net::SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = leather_server::serve(listener, Arc::new(ServerConfig::default())).await;
    });
    addr
}

#[tokio::test]
async fn offline_login_succeeds_then_configuration_disconnect() {
    const NAME: &str = "Steve";
    const UUID: u128 = 0x1234_5678_9abc_def0_1122_3344_5566_7788;

    let addr = spawn_server().await;
    let mut stream = TcpStream::connect(addr).await.unwrap();

    // 1. Handshake -> next_state = 2 (login).
    write_frame(
        &mut stream,
        &handshake_body(776, "127.0.0.1", addr.port(), 2),
    )
    .await
    .unwrap();

    // 2. Login Start (0x00): name + UUID.
    let mut start = PacketWriter::new(0x00);
    start.write_string(NAME).write_uuid(UUID);
    write_frame(&mut stream, &start.into_body()).await.unwrap();

    // 3. Login Success (0x02): UUID + name + properties count (0).
    let mut success = read_frame(&mut stream).await.unwrap();
    assert_eq!(success.read_varint().unwrap(), 0x02, "login success id");
    assert_eq!(success.read_uuid().unwrap(), UUID, "uuid echoed");
    assert_eq!(success.read_string().unwrap(), NAME, "name echoed");
    assert_eq!(success.read_varint().unwrap(), 0, "no properties");
    // Minecraft 26.2 added a trailing session_id UUID; we reuse the player UUID.
    assert_eq!(success.read_uuid().unwrap(), UUID, "session_id (26.2)");

    // 4. Login Acknowledged (0x03), no fields -> enters Configuration.
    write_frame(&mut stream, &PacketWriter::new(0x03).into_body())
        .await
        .unwrap();

    // 5. Configuration begins: the server announces its brand via Custom Payload
    //    (id 0x01), on the "minecraft:brand" channel.
    let mut brand = read_frame(&mut stream).await.unwrap();
    assert_eq!(
        brand.read_varint().unwrap(),
        0x01,
        "config custom_payload id"
    );
    assert_eq!(
        brand.read_string().unwrap(),
        "minecraft:brand",
        "brand channel"
    );
    assert_eq!(brand.read_string().unwrap(), "LeatherMC", "brand value");

    stream.shutdown().await.ok();
}
