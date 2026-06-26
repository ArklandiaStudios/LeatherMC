//! Configuration state: send our brand and the registries, then finish.
//!
//! Sequence (Minecraft 26.2):
//!   1. send Custom Payload (brand) and an empty Known Packs list,
//!   2. wait for the client's Known Packs reply,
//!   3. stream every registry via Registry Data, then Finish Configuration,
//!   4. wait for the client's Acknowledge Finish Configuration -> Play.

use leather_protocol::{PacketWriter, Result, read_frame, write_frame};
use tokio::net::TcpStream;

use crate::registries::Registries;

// Clientbound configuration packet ids (protocol 776).
const C_CUSTOM_PAYLOAD: i32 = 1;
const C_KNOWN_PACKS: i32 = 14;
const C_REGISTRY_DATA: i32 = 7;
const C_FINISH_CONFIG: i32 = 3;

// Serverbound configuration packet ids.
const S_KNOWN_PACKS: i32 = 7;
const S_FINISH_CONFIG: i32 = 3;

pub async fn handle(stream: &mut TcpStream, registries: &Registries) -> Result<()> {
    // Announce our brand.
    let mut brand = PacketWriter::new(C_CUSTOM_PAYLOAD);
    brand
        .write_string("minecraft:brand")
        .write_string("LeatherMC");
    write_frame(stream, &brand.into_body()).await?;

    // Send an empty Known Packs list: we share no datapacks with the client, so
    // the client expects (and we send) every registry entry in full.
    let mut known = PacketWriter::new(C_KNOWN_PACKS);
    known.write_varint(0);
    write_frame(stream, &known.into_body()).await?;

    let mut sent_registries = false;
    loop {
        let mut frame = read_frame(stream).await?;
        match frame.read_varint()? {
            S_KNOWN_PACKS if !sent_registries => {
                send_registries(stream, registries).await?;
                let finish = PacketWriter::new(C_FINISH_CONFIG);
                write_frame(stream, &finish.into_body()).await?;
                sent_registries = true;
            }
            S_FINISH_CONFIG => return Ok(()), // ack -> caller switches to Play
            _ => {}                           // client_information, brand, etc. — ignored for now
        }
    }
}

async fn send_registries(stream: &mut TcpStream, registries: &Registries) -> Result<()> {
    for registry in &registries.list {
        let mut w = PacketWriter::new(C_REGISTRY_DATA);
        w.write_string(&registry.id);
        w.write_varint(registry.entries.len() as i32);
        for entry in &registry.entries {
            w.write_string(&entry.id);
            w.write_bool(true); // has data
            w.write_bytes(&entry.nbt);
        }
        write_frame(stream, &w.into_body()).await?;
    }
    Ok(())
}
