use crate::{WineReply, WineRequest};
use const_format::concatcp;
use std::env::consts::ARCH;
use std::io;
use std::io::{IoSlice, IoSliceMut};
use std::mem::transmute;
use std::os::fd::{BorrowedFd, FromRawFd, OwnedFd};
use std::os::unix::net::{AncillaryData, SocketAncillary, UnixStream};

/// Connects to the Windows "kernel" socket.
pub struct WineSocket {
    /// Process socket to exchange file descriptors with the server.
    process: UnixStream,
    /// Thread sockets.
    threads: Vec<UnixStream>,
}

impl WineSocket {
    /// The path to the Windows "kernel" socket.
    pub const PATH: &'static str = concatcp!("/tmp/wine/", ARCH);

    pub fn new(name: &str) -> io::Result<WineSocket> {
        let process = UnixStream::connect(format!("{}/{name}", WineSocket::PATH))?;
        let fds = recv_fds_from(&process)?;
        assert_eq!(fds.len(), 1);
        let threads = fds.iter().map(UnixStream::from).collect();
        Ok(WineSocket { process, threads })
        //TODO: connect, receive process FD, receive first thread fd
    }

    const CMSG_BUFFER: usize = 256;

    pub fn recv_fds(&self) -> io::Result<Vec<OwnedFd>> {
        recv_fds_from(&self.process)
    }

    pub fn send_fds<'fd>(&self, fds: &[BorrowedFd<'fd>]) -> io::Result<()> {
        send_fds_to(&self.process, fds)
    }

    /// Send a request to the Wine server and receive a reply.
    pub fn send(&mut self, req: &WineRequest) -> WineReply {
        serde_json::to_writer(&mut self.process, req)
            .expect(format!("failed to write {:#?}", req).as_str());
        serde_json::from_reader(&mut self.process).expect("failed to read reply")
    }

    /// Read an incoming request from a Wine client.
    pub fn read(&mut self) -> WineRequest {
        serde_json::from_reader(&mut self.process).expect("failed to read request")
    }

    /// Reply to a Wine client's last request.
    pub fn reply(&mut self, reply: &WineReply) {
        serde_json::to_writer(&mut self.process, reply)
            .expect(format!("failed to write {:#?}", reply).as_str())
    }
}

const ANCILLARY_SIZE: usize = 128;

pub fn recv_fds_from(from: &UnixStream) -> io::Result<Vec<OwnedFd>> {
    let mut ancillary_buffer = [0; ANCILLARY_SIZE];
    let mut ancillary = SocketAncillary::new(&mut ancillary_buffer[..]);

    let bufs = &mut [IoSliceMut::new(&mut [])][..];
    from.recv_vectored_with_ancillary(bufs, &mut ancillary)?;

    let mut all_fds: Vec<OwnedFd> = Vec::new();
    for ancillary_result in ancillary.messages() {
        if let AncillaryData::ScmRights(scm_rights) = ancillary_result.unwrap() {
            for fd in scm_rights {
                all_fds.push(unsafe { OwnedFd::from_raw_fd(fd) });
            }
        }
    }
    Ok(all_fds)
}

pub fn send_fds_to(to: &UnixStream, fds: &[BorrowedFd]) -> io::Result<()> {
    let mut ancillary_buffer = [0; 128];
    let mut ancillary = SocketAncillary::new(&mut ancillary_buffer[..]);
    if !ancillary.add_fds(unsafe { transmute(fds) }) {
        panic!("Unable to write fd to ancillary data");
    }

    let bufs = &[IoSlice::new(&[])];
    to.send_vectored_with_ancillary(bufs, &mut ancillary)?;
    Ok(())
}
