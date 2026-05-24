use std::io;
use std::os::unix::net::UnixStream;
use protocol::socket::recv_fds_from;
use crate::thread::ThreadSocket;

/// Connects to the Windows "kernel" socket.
pub struct KernelSocket(UnixStream, UnixStream);

impl KernelSocket {
    /// The path to the Windows "kernel" socket.
    pub const PATH: &'static str = "/tmp/wine";

    pub fn new(name: &str) -> io::Result<KernelSocket> {
		let kernel = UnixStream::connect(format!(
			"{}/{name}",
			KernelSocket::PATH
		))?;
		let fds = recv_fds_from(&kernel)?;
		assert_eq!(fds.len(), 2);
		let process = UnixStream::from(fds[0]);
		ThreadSocket::connect(fds[1]);
        Ok(KernelSocket(kernel, process))
        //TODO: connect, receive process FD, receive first thread fd
    }
}
