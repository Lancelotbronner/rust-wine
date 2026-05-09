use crate::server::WineServer;
use crate::thread::WindowsThread;
use libc::{
    c_uint, cmsghdr, iovec, msghdr, sendmsg, socklen_t, CMSG_DATA,
    CMSG_FIRSTHDR, CMSG_LEN, SCM_RIGHTS, SOL_SOCKET,
};
use std::ffi::c_void;
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::net::UnixStream;
use std::ptr::null_mut;
use std::rc::Rc;

#[derive(Copy, Clone)]
pub struct WindowsPid(pub u32);

pub struct WindowsProcess {
    fd: UnixStream,
    pid: WindowsPid,
    parent: Option<Rc<WindowsProcess>>,
    threads: Vec<Rc<WindowsThread>>,
    pub(crate) is_terminating: bool,
}

impl WindowsProcess {
    pub fn new_from_client(client: UnixStream, pid: WindowsPid) -> WindowsProcess {
        WindowsProcess {
            fd: client,
            pid,
            parent: None,
            threads: Vec::new(),
            is_terminating: false,
        }
    }

    pub fn add_thread(&mut self, thread: Rc<WindowsThread>) {
        self.threads.push(thread);
    }

    pub fn fd(&self) -> RawFd {
        self.fd.as_raw_fd()
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
