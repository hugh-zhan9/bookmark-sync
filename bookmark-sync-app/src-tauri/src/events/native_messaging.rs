use std::io::{self, Read, Write};
use serde_json::Value;

/// Reads a single message from Chrome Native Messaging protocol
pub fn read_message() -> io::Result<Option<Value>> {
    let mut stdin = io::stdin();
    
    // Read message length (first 4 bytes, native byte order)
    let mut length_bytes = [0u8; 4];
    let bytes_read = stdin.read(&mut length_bytes)?;
    
    if bytes_read == 0 {
        return Ok(None); // EOF
    }
    
    let length = u32::from_ne_bytes(length_bytes) as usize;
    
    // Read the actual JSON message
    let mut msg_buf = vec![0u8; length];
    stdin.read_exact(&mut msg_buf)?;
    
    let msg: Value = serde_json::from_slice(&msg_buf)?;
    Ok(Some(msg))
}

/// Helper function (often unused by pure listeners) to send messages back to Chrome
#[allow(dead_code)]
pub fn write_message(msg: &Value) -> io::Result<()> {
    let mut stdout = io::stdout();
    let msg_bytes = serde_json::to_vec(msg)?;
    
    // Write 4-byte length header
    let length = msg_bytes.len() as u32;
    stdout.write_all(&length.to_ne_bytes())?;
    
    // Write actual message
    stdout.write_all(&msg_bytes)?;
    stdout.flush()?;
    
    Ok(())
}
