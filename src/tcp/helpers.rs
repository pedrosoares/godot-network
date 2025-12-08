use std::{io::Read, net::TcpStream};

fn read_exact_bytes(stream: &mut TcpStream, size: usize) -> std::io::Result<Vec<u8>> {
    let mut buf = vec![0u8; size];
    stream.read_exact(&mut buf)?;
    Ok(buf)
}

pub fn read_message(stream: &mut TcpStream) -> std::io::Result<Vec<u8>> {
    // Read length header
    let header = read_exact_bytes(stream, 4)?;
    let len = u32::from_be_bytes(header.try_into().unwrap()) as usize;

    // Read full message body
    read_exact_bytes(stream, len)
}
