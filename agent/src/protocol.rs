use anyhow::Context;
use serde_json::Value;
use std::io::Cursor;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const MAX_PACKET_SIZE: usize = 1024 * 1024;

#[derive(Debug, Clone)]
pub struct Handshake {
    pub protocol_version: i32,
    pub server_address: String,
    pub server_port: u16,
    pub next_state: i32,
}

pub fn encode_varint(mut value: i32, output: &mut Vec<u8>) {
    loop {
        let mut byte = (value as u32 & 0x7f) as u8;
        value = ((value as u32) >> 7) as i32;
        if value != 0 {
            byte |= 0x80;
        }
        output.push(byte);
        if value == 0 {
            break;
        }
    }
}

fn decode_varint(cursor: &mut Cursor<&[u8]>) -> anyhow::Result<i32> {
    let mut result = 0i32;
    for shift in (0..35).step_by(7) {
        let position = cursor.position() as usize;
        let byte = *cursor
            .get_ref()
            .get(position)
            .context("unexpected end of VarInt")?;
        cursor.set_position((position + 1) as u64);
        result |= ((byte & 0x7f) as i32) << shift;
        if byte & 0x80 == 0 {
            return Ok(result);
        }
    }
    anyhow::bail!("VarInt is too large")
}

async fn read_varint(stream: &mut TcpStream) -> anyhow::Result<i32> {
    let mut result = 0i32;
    for shift in (0..35).step_by(7) {
        let byte = stream.read_u8().await?;
        result |= ((byte & 0x7f) as i32) << shift;
        if byte & 0x80 == 0 {
            return Ok(result);
        }
    }
    anyhow::bail!("VarInt is too large")
}

fn read_string(cursor: &mut Cursor<&[u8]>) -> anyhow::Result<String> {
    let length = decode_varint(cursor)?;
    if !(0..=32767).contains(&length) {
        anyhow::bail!("invalid string length")
    }
    let start = cursor.position() as usize;
    let end = start + length as usize;
    let source = cursor.get_ref();
    let bytes = source.get(start..end).context("truncated string")?;
    cursor.set_position(end as u64);
    Ok(std::str::from_utf8(bytes)?.to_owned())
}

fn write_string(value: &str, output: &mut Vec<u8>) {
    encode_varint(value.len() as i32, output);
    output.extend_from_slice(value.as_bytes());
}

pub async fn read_packet(stream: &mut TcpStream) -> anyhow::Result<Vec<u8>> {
    let length = read_varint(stream).await?;
    if length < 0 || length as usize > MAX_PACKET_SIZE {
        anyhow::bail!("invalid packet size")
    }
    let mut packet = vec![0u8; length as usize];
    stream.read_exact(&mut packet).await?;
    Ok(packet)
}

pub fn parse_handshake(packet: &[u8]) -> anyhow::Result<Handshake> {
    let mut cursor = Cursor::new(packet);
    if decode_varint(&mut cursor)? != 0 {
        anyhow::bail!("expected handshake packet")
    }
    let protocol_version = decode_varint(&mut cursor)?;
    let server_address = read_string(&mut cursor)?;
    let position = cursor.position() as usize;
    let port_bytes: [u8; 2] = packet
        .get(position..position + 2)
        .context("truncated handshake port")?
        .try_into()?;
    cursor.set_position((position + 2) as u64);
    let server_port = u16::from_be_bytes(port_bytes);
    let next_state = decode_varint(&mut cursor)?;
    if !matches!(next_state, 1 | 2) {
        anyhow::bail!("invalid handshake next state")
    }
    Ok(Handshake {
        protocol_version,
        server_address,
        server_port,
        next_state,
    })
}

async fn write_packet(stream: &mut TcpStream, body: &[u8]) -> anyhow::Result<()> {
    let mut packet = Vec::with_capacity(body.len() + 5);
    encode_varint(body.len() as i32, &mut packet);
    packet.extend_from_slice(body);
    stream.write_all(&packet).await?;
    Ok(())
}

pub async fn serve_status(stream: &mut TcpStream, status: Value) -> anyhow::Result<()> {
    let request = read_packet(stream).await?;
    let mut cursor = Cursor::new(request.as_slice());
    if decode_varint(&mut cursor)? != 0 {
        anyhow::bail!("expected status request")
    }

    let json = serde_json::to_string(&status)?;
    let mut response = Vec::with_capacity(json.len() + 8);
    encode_varint(0, &mut response);
    write_string(&json, &mut response);
    write_packet(stream, &response).await?;

    if let Ok(Ok(ping)) =
        tokio::time::timeout(std::time::Duration::from_secs(5), read_packet(stream)).await
    {
        let mut cursor = Cursor::new(ping.as_slice());
        if decode_varint(&mut cursor)? == 1 {
            write_packet(stream, &ping).await?;
        }
    }
    Ok(())
}

pub async fn disconnect_login(stream: &mut TcpStream, message: &str) -> anyhow::Result<()> {
    let json = serde_json::to_string(&serde_json::json!({ "text": message }))?;
    let mut response = Vec::with_capacity(json.len() + 8);
    encode_varint(0, &mut response);
    write_string(&json, &mut response);
    write_packet(stream, &response).await?;
    stream.shutdown().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varint_round_trip() {
        for value in [0, 1, 127, 128, 255, 2_097_151, -1] {
            let mut encoded = Vec::new();
            encode_varint(value, &mut encoded);
            let mut cursor = Cursor::new(encoded.as_slice());
            assert_eq!(decode_varint(&mut cursor).unwrap(), value);
        }
    }

    #[test]
    fn parses_handshake_and_port() {
        let mut packet = Vec::new();
        encode_varint(0, &mut packet);
        encode_varint(769, &mut packet);
        write_string("mc.example.test", &mut packet);
        packet.extend_from_slice(&25565u16.to_be_bytes());
        encode_varint(2, &mut packet);
        let handshake = parse_handshake(&packet).unwrap();
        assert_eq!(handshake.protocol_version, 769);
        assert_eq!(handshake.server_port, 25565);
        assert_eq!(handshake.next_state, 2);
    }

    #[test]
    fn rejects_oversized_handshake_string() {
        let mut packet = vec![0, 1];
        encode_varint(40_000, &mut packet);
        assert!(parse_handshake(&packet).is_err());
    }
}
