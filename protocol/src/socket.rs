use std::env::consts::ARCH;
use crate::{WineRequest, WineReply};
use std::{io, mem};
use std::io::Read;
use std::os::fd::RawFd;
use std::os::unix::net::{UnixStream, SocketAddr};
use const_format::concatcp;
use libc::socket;

/// Connects to the Windows "kernel" socket.
pub struct WineSocket {
    stream: UnixStream,
}

impl WineSocket {
    /// The path to the Windows "kernel" socket.
    pub const PATH: &'static str = concatcp!("/tmp/wine/", ARCH);
    
    pub fn new(name: &str) -> io::Result<WineSocket> {
        Ok(WineSocket {
            stream: UnixStream::connect(format!("{}/{name}", WineSocket::PATH))?
        })
    }

    /// Send a request to the Wine server and receive a reply.
    pub fn send(&mut self, req: &WineRequest) -> WineReply {
        serde_json::to_writer(&mut self.stream, req).expect(format!("failed to write {:#?}", req).as_str());
        serde_json::from_reader(&mut self.stream).expect("failed to read reply")
    }

    /// Read an incoming request from a Wine client.
    pub fn read(&mut self) -> WineRequest {
        serde_json::from_reader(&mut self.stream).expect("failed to read request")
    }

    /// Reply to a Wine client's last request.
    pub fn reply(&mut self, reply: &WineReply) {
        serde_json::to_writer(&mut self.stream, reply).expect(format!("failed to write {:#?}", reply).as_str())
    }
}
