use crate::fd::WindowsFd;
use crate::process::WindowsProcess;
use crate::server::WineServer;
use crate::thread::WindowsThread;
use core::ffi::{c_int, c_long, c_short, c_void};
use core::mem;
use core::ptr::{null, null_mut};
use core::time::Duration;
use errno::errno;
use libc::{
	close, kevent, kqueue, perror, pollfd, time_t, timespec, uintptr_t, ENOMEM,
	EVFILT_READ, EVFILT_WRITE, EV_ADD, EV_DELETE, EV_DISABLE, EV_ENABLE, EV_EOF, EV_ERROR, NOTE_LOWAT, POLLERR, POLLHUP,
	POLLIN, POLLOUT,
};
use protocol::ptr::TaggedPtr;
use std::os::fd::RawFd;

pub struct SystemEvents {
    fd: c_int,
    pollfd: Vec<pollfd>,
}

impl SystemEvents {
    pub const fn invalid() -> Self {
        SystemEvents {
            fd: -1,
            pollfd: Vec::new(),
        }
    }

    pub fn is_valid(&self) -> bool {
        self.fd != -1
    }
}

impl Default for SystemEvents {
    fn default() -> Self {
        let fd;
        #[cfg(target_os = "macos")]
        {
            fd = unsafe { kqueue() };
        }
        SystemEvents {
            fd,
            pollfd: Vec::new(),
        }
    }
}

pub trait SystemWatch {
    fn revents_reset(&mut self);
    fn revents_add(&mut self, event: c_short);
    fn as_fd(&self) -> RawFd;
    fn poll(&mut self, wine: &mut WineServer);
}

#[derive(Copy, Clone)]
pub enum SystemWatcher {
    Thread(*mut WindowsThread),
    Process(*mut WindowsProcess),
    Fd(*mut WindowsFd),
}

impl SystemWatch for SystemWatcher {
    fn revents_reset(&mut self) {
        match *self {
            SystemWatcher::Thread(t) => unsafe { (*t).revents_reset() },
            SystemWatcher::Process(p) => unsafe { (*p).revents_reset() },
            SystemWatcher::Fd(fd) => unsafe { (*fd).revents_reset() },
        }
    }

    fn revents_add(&mut self, event: c_short) {
        match *self {
            SystemWatcher::Thread(t) => unsafe { (*t).revents_add(event) },
            SystemWatcher::Process(p) => unsafe { (*p).revents_add(event) },
            SystemWatcher::Fd(fd) => unsafe { (*fd).revents_add(event) },
        }
    }

    fn as_fd(&self) -> RawFd {
        unsafe {
            match *self {
                SystemWatcher::Thread(t) => (*t).as_fd(),
                SystemWatcher::Process(t) => (*t).as_fd(),
                SystemWatcher::Fd(fd) => (*fd).as_fd(),
            }
        }
    }

    fn poll(&mut self, wine: &mut WineServer) {
        unsafe {
            match *self {
                SystemWatcher::Thread(t) => (*t).poll(wine),
                SystemWatcher::Process(t) => (*t).poll(wine),
                SystemWatcher::Fd(fd) => (*fd).poll(wine),
            }
        }
    }
}

impl SystemWatcher {
    pub fn unpack(ptr: *const c_void) -> Self {
        let tagged = TaggedPtr::unpack(ptr);
        match tagged.data() {
            0u8 => SystemWatcher::Thread(tagged.ptr() as *mut WindowsThread),
            1u8 => SystemWatcher::Fd(tagged.ptr() as *mut WindowsFd),
            2u8 => SystemWatcher::Fd(tagged.ptr() as *mut WindowsFd),
            _ => todo!(),
        }
    }

    pub fn tag(&self) -> u8 {
        match &self {
            SystemWatcher::Thread(_) => 0,
            SystemWatcher::Process(_) => 1,
            SystemWatcher::Fd(_) => 2,
        }
    }

    pub fn into_ptr(self) -> *const () {
        match self {
            SystemWatcher::Thread(t) => t as *const (),
            SystemWatcher::Process(t) => t as *const (),
            SystemWatcher::Fd(fd) => fd as *const (),
        }
    }

    pub fn packed(self) -> TaggedPtr<(), u8> {
        TaggedPtr::pack(self.into_ptr(), self.tag())
    }
}

#[cfg(target_os = "macos")]
impl SystemEvents {
    pub fn init(&mut self) {
        self.fd = unsafe { kqueue() };
    }

    pub fn reads_watch(&mut self, watcher: SystemWatcher) {
        let ev = kevent {
            ident: watcher.as_fd() as uintptr_t,
            filter: EVFILT_READ,
            flags: EV_ADD | EV_ENABLE,
            fflags: NOTE_LOWAT,
            data: 1,
            udata: watcher.packed().raw() as *mut c_void,
        };
        self.apply(&[ev]);
    }

    pub fn reads_cancel(&mut self, watcher: SystemWatcher) {
        let ev = kevent {
            ident: watcher.as_fd() as uintptr_t,
            filter: EVFILT_READ,
            flags: EV_DELETE,
            fflags: NOTE_LOWAT,
            data: 0,
            udata: null_mut(),
        };
        self.apply(&[ev]);
    }

    pub fn add(&mut self, fd: RawFd, user: usize, events: c_short) {
        if self.fd == -1 {
            return;
        }
        let mut ev = [
            kevent {
                ident: fd as uintptr_t,
                filter: EVFILT_READ,
                flags: EV_ADD | EV_ENABLE,
                fflags: NOTE_LOWAT,
                data: 1,
                udata: user as *mut c_void,
            },
            kevent {
                ident: fd as uintptr_t,
                filter: EVFILT_WRITE,
                flags: 0,
                fflags: NOTE_LOWAT,
                data: 1,
                udata: user as *mut c_void,
            },
        ];
        let poll_in = if events & POLLIN != 0 {
            EV_ENABLE
        } else {
            EV_DISABLE
        };
        let poll_out = if events & POLLOUT != 0 {
            EV_ENABLE
        } else {
            EV_DISABLE
        };
        if events == -1 {
            // Stop waiting on this fd completely
            if self.pollfd[user].fd == -1 {
                // Already removed
                return;
            }
            ev[0].flags |= EV_DELETE;
            ev[1].flags |= EV_DELETE;
        } else if self.pollfd[user].fd == -1 {
            ev[0].flags |= EV_ADD | poll_in;
            ev[1].flags |= EV_ADD | poll_out;
        } else {
            if self.pollfd[user].events == events {
                // Nothing to do
                return;
            }
            ev[0].flags |= poll_in;
            ev[1].flags |= poll_out;
        }

        self.apply(&ev);
    }

    fn apply(&mut self, ev: &[kevent]) {
        if unsafe {
            kevent(
                self.fd,
                &ev[0] as *const kevent,
                ev.len() as c_int,
                null_mut(),
                0,
                null(),
            ) == -1
        } {
            if errno().0 == ENOMEM {
                // not enough memory, give up on kqueue
                unsafe { close(self.fd) };
                self.fd = -1;
            } else {
                // Should not happen
                unsafe {
                    perror(c"kevent".as_ptr());
                }
            }
        }
    }

    pub fn remove(&mut self, fd: RawFd, user: usize) {
        if self.fd == -1 || self.pollfd[user].fd == -1 {
            return;
        }
        let ev = [
            kevent {
                ident: fd as uintptr_t,
                filter: EVFILT_READ,
                flags: EV_DELETE,
                fflags: 0,
                data: 0,
                udata: null_mut(),
            },
            kevent {
                ident: fd as uintptr_t,
                filter: EVFILT_WRITE,
                flags: EV_DELETE,
                fflags: 0,
                data: 0,
                udata: null_mut(),
            },
        ];
        self.apply(&ev);
    }

    const BUFFER: usize = 128;

    pub fn poll(&self, timeout: Option<Duration>) -> Vec<SystemWatcher> {
        if self.fd == -1 {
            return vec![];
        }
        let timeout = timeout
            .map(|t| timespec {
                tv_sec: t.as_secs() as time_t,
                tv_nsec: t.subsec_nanos() as c_long,
            })
            .as_ref()
            .map(|r| r as *const timespec)
            .unwrap_or(null());
        let mut buffer: [kevent; Self::BUFFER] = unsafe { mem::zeroed() };
        let count = unsafe {
            kevent(
                self.fd,
                null(),
                0,
                &mut buffer as *mut kevent,
                SystemEvents::BUFFER as c_int,
                timeout,
            )
        } as usize;
        buffer[..count]
            .iter()
            .map(|event| {
                let mut watcher = SystemWatcher::unpack(event.udata);
                watcher.revents_reset();
                match event.filter {
                    EVFILT_READ => watcher.revents_add(POLLIN),
                    EVFILT_WRITE => watcher.revents_add(POLLOUT),
                    _ => (),
                }
                if event.flags & EV_EOF != 0 {
                    watcher.revents_add(POLLHUP);
                }
                if event.flags & EV_ERROR != 0 {
                    watcher.revents_add(POLLERR);
                }
                watcher
            })
            .collect()
    }
}
