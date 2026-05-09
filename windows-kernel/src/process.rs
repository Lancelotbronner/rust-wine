use crate::net::SystemWatch;
use crate::server::WineServer;
use crate::thread::WindowsThread;
use core::ffi::{c_short, c_uint, c_void};
use core::ptr::null_mut;
use libc::{
	cmsghdr, iovec, msghdr, sendmsg, socklen_t, CMSG_DATA, CMSG_FIRSTHDR, CMSG_LEN, POLLERR,
	POLLHUP, POLLIN, SCM_RIGHTS, SOL_SOCKET,
};
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::net::UnixStream;
use std::rc::Rc;

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

    pub fn add_thread(&mut self, thread: Rc<WindowsThread>) {
        self.threads.push(thread);
    }

    const CMSG_BUFFER: usize = 256;

    pub fn send_fd(&self, fd: RawFd) -> bool {
        let mut vec = iovec {
            iov_base: self as *const WindowsProcess as *mut c_void,
            iov_len: size_of::<WindowsProcess>(),
        };
        let mut cmsg_buffer = [0u8; WindowsProcess::CMSG_BUFFER];
        let msghdr = msghdr {
            msg_name: null_mut(),
            msg_namelen: 0,
            msg_iov: &mut vec as *mut iovec,
            msg_iovlen: 1,
            msg_control: &mut cmsg_buffer as *mut u8 as *mut c_void,
            msg_controllen: WindowsProcess::CMSG_BUFFER as socklen_t,
            msg_flags: 0,
        };
        let mut cmsg = unsafe { &mut *CMSG_FIRSTHDR(&msghdr) };
        *cmsg = cmsghdr {
            cmsg_len: unsafe { CMSG_LEN(size_of_val(&fd) as c_uint) },
            cmsg_level: SOL_SOCKET,
            cmsg_type: SCM_RIGHTS,
        };
        unsafe { *(CMSG_DATA(cmsg) as *mut RawFd) = fd };

        let ret = unsafe { sendmsg(self.fd.as_raw_fd(), &msghdr, 0) };
        if ret == size_of::<Self>() as isize {
            return true;
        }

        if ret >= 0 {
            panic!("partial sendmsg in WindowsProcess::send_fd");
            //TODO: kill process instead
            // kill_process( process, 1 );
        }

        /*
        if (ret >= 0)
        {
            fprintf( stderr, "Protocol error: process %04x: partial sendmsg %d\n", process->id, ret );

        }
        else if (errno == EPIPE)
        {
            kill_process( process, 0 );
        }
        else
        {
            fprintf( stderr, "Protocol error: process %04x: ", process->id );
            perror( "sendmsg" );
            kill_process( process, 1 );
        }
        return -1;
        */

        false
    }

    pub fn receive_fd(&self) {}

    pub fn kill(&self, unknown: bool) {}
}

impl WineServer {}
