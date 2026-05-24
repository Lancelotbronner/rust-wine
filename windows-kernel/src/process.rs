use crate::net::SystemWatch;
use crate::server::WineServer;
use crate::thread::WindowsThread;
use core::ffi::{c_short, c_uint, c_void};
use core::ptr::null_mut;
use std::io;
use std::io::{IoSlice, IoSliceMut};
use libc::{cmsghdr, iovec, msghdr, sendmsg, size_t, socklen_t, CMSG_DATA, CMSG_FIRSTHDR, CMSG_LEN, POLLERR, POLLHUP, POLLIN, SCM_RIGHTS, SOL_SOCKET};
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::net::{SocketAncillary, UnixStream};
use std::rc::Rc;
use mach_sys::task::thread_create_running;

#[derive(Copy, Clone)]
pub struct WindowsPid(pub u32);

pub struct WindowsProcess {
    fd: UnixStream,
    pid: WindowsPid,
    parent: Option<Rc<WindowsProcess>>,
    threads: Vec<Rc<WindowsThread>>,
    is_terminating: bool,
    revents: c_short,
}

impl SystemWatch for WindowsProcess {
    fn revents_reset(&mut self) {
        self.revents = 0;
    }

    fn revents_add(&mut self, event: c_short) {
        self.revents |= event;
    }

    fn as_fd(&self) -> RawFd {
        self.fd.as_raw_fd()
    }

    fn poll(&mut self, _wine: &mut WineServer) {
        if (self.revents & (POLLERR | POLLHUP) != 0) {
            self.kill(!self.is_terminating);
        } else if (self.revents & POLLIN != 0) {
            self.receive_fd();
        }
    }
}

impl WindowsProcess {
    pub fn new_from_client(client: UnixStream, pid: WindowsPid) -> WindowsProcess {
        WindowsProcess {
            fd: client,
            pid,
            parent: None,
            threads: Vec::new(),
            is_terminating: false,
            revents: 0,
        }
    }

    pub fn add_thread(&mut self, thread: Rc<WindowsThread>) -> io::Result<()> {
        self.threads.push(thread.clone());
		self.send_fd(thread.as_fd())
    }



    pub fn receive_fd(&self) {}

    pub fn kill(&self, unknown: bool) {}
}

impl WineServer {}
