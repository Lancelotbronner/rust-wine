use protocol::{WineReply, WineRequest};
use std::cell::RefCell;
use std::mem;
use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;

thread_local! {
    static THREAD_SOCKET: RefCell<UnixStream> = panic!("Current thread is not associated to a Wine socket");
}

/// Current thread socket.
pub struct ThreadSocket(UnixStream);

impl ThreadSocket {
	pub fn connect(fd: OwnedFd) {
		THREAD_SOCKET.set(UnixStream::from(fd))
	}

    pub fn send(request: &WineRequest) -> WineReply {
        THREAD_SOCKET.with_borrow_mut(|s| {
            request.write_to(s);
            WineReply::read_from(s)
        })
    }
}
