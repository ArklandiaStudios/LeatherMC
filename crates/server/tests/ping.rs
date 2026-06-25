//! Integration test: a tiny Rust "client" that performs the full Server-List
//! Ping handshake against an in-process server and checks the responses.
//!
//! This is the automated half of our validation strategy — it lets CI verify
//! the ping brick without a real Minecraft client.

use std::sync::Arc;

use leather_protocol::{PacketWriter, read_frame, write_frame, write_varint};
use leather_server::config::ServerConfig;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};

/// Builds a handshake packet body: id 0x00, protocol, address, port, next_state.
///
/// PacketWriter only exposes the writers the server itself needs, so we mirror
/// the remaining fields (VarInt protocol, string, u16 port) by hand.
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
    let config = Arc::new(ServerConfig::default());
    tokio::spawn(async move {
        let _ = leather_server::serve(listener, config).await;
    });
    addr
}

#[tokio::test]
async fn server_answers_status_and_ping() {
    const CLIENT_PROTOCOL: i32 = 9999;
    let addr = spawn_server().await;

    let mut stream = TcpStream::connect(addr).await.unwrap();

    // 1. Handshake -> next_state = 1 (status).
    let body = handshake_body(CLIENT_PROTOCOL, "127.0.0.1", addr.port(), 1);
    write_frame(&mut stream, &body).await.unwrap();

    // 2. Status Request (empty body, id 0x00).
    write_frame(&mut stream, &PacketWriter::new(0x00).into_body())
        .await
        .unwrap();

    // 3. Read Status Response: id 0x00 + JSON string.
    let mut resp = read_frame(&mut stream).await.unwrap();
    assert_eq!(resp.read_varint().unwrap(), 0x00, "status response id");
    let json = resp.read_string().unwrap();
    assert!(json.contains("LeatherMC"), "MOTD/version present: {json}");
    // The server echoes the client's protocol so it shows as compatible.
    assert!(
        json.contains(&CLIENT_PROTOCOL.to_string()),
        "echoes client protocol: {json}"
    );

    // 4. Ping Request (id 0x01 + i64 payload) -> expect identical Pong.
    const PAYLOAD: i64 = 0x0123_4567_89AB_CDEF;
    let mut ping = PacketWriter::new(0x01);
    ping.write_i64(PAYLOAD);
    write_frame(&mut stream, &ping.into_body()).await.unwrap();

    let mut pong = read_frame(&mut stream).await.unwrap();
    assert_eq!(pong.read_varint().unwrap(), 0x01, "pong id");
    assert_eq!(pong.read_i64().unwrap(), PAYLOAD, "pong echoes payload");

    stream.shutdown().await.ok();
}
