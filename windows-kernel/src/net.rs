use std::ffi::c_void;
use std::mem;
use std::os::fd::RawFd;
use std::ptr::{null, null_mut};
use std::time::Duration;
use errno::errno;
use libc::{c_int, c_long, c_short, close, kevent, kqueue, perror, pollfd, time_t, timespec, uintptr_t, ENOMEM, EVFILT_READ, EVFILT_WRITE, EV_ADD, EV_DELETE, EV_DISABLE, EV_ENABLE, NOTE_LOWAT, POLLIN, POLLOUT};
use protocol::ptr::TaggedPtr;
use crate::fd::WindowsFd;
use crate::process::WindowsProcess;
use crate::thread::WindowsThread;

pub struct EventWatch {
    fd: c_int,
    pollfd: Vec<pollfd>,
}

impl EventWatch {
    pub const fn invalid() -> Self {
        EventWatch {
            fd: -1,
            pollfd: Vec::new()
        }
    }

    pub fn is_valid(&self) -> bool {
        self.fd != -1
    }
}

impl Default for EventWatch {
    fn default() -> Self {
        let fd;
        #[cfg(target_os = "macos")]
        {
            fd = unsafe { kqueue() };
        }
        EventWatch {
            fd,
            pollfd: Vec::new(),
        }
    }
}

#[derive(Copy, Clone)]
pub enum SystemWatcher {
    Thread(*const WindowsThread),
    Process(*const WindowsProcess),
    Fd(*const WindowsFd),
}

impl SystemWatcher {
    pub fn unpack(ptr: *const c_void) -> Self {
        let tagged = TaggedPtr::unpack(ptr);
        match tagged.data() {
            0u8 => SystemWatcher::Thread(tagged.ptr() as *const WindowsThread),
            1u8 => SystemWatcher::Fd(tagged.ptr() as *const WindowsFd),
            2u8 => SystemWatcher::Fd(tagged.ptr() as *const WindowsFd),
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

    pub fn fd(self) -> RawFd {
        unsafe {
            match self {
                SystemWatcher::Thread(t) => (*t).request_fd(),
                SystemWatcher::Process(t) => (*t).fd(),
                SystemWatcher::Fd(fd) => (*fd).unix,
            }
        }
    }
}

#[cfg(target_os = "macos")]
impl EventWatch {
    pub fn init(&mut self) {
        self.fd = unsafe { kqueue() };
    }

    pub fn reads_watch(&mut self, watcher: SystemWatcher) {
        let ev = kevent {
            ident: watcher.fd() as uintptr_t,
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
            ident: watcher.fd() as uintptr_t,
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

    pub fn poll(&self, timeout: Option<Duration>) -> EventIter {
        if self.fd == -1 {
            return EventIter::EMPTY;
        }
        let timeout = timeout
            .map(|t| timespec {
                tv_sec: t.as_secs() as time_t,
                tv_nsec: t.subsec_nanos() as c_long,
            })
            .as_ref()
            .map(|r| r as *const timespec)
            .unwrap_or(null());
        let mut iter = EventIter::EMPTY;
        iter.count = unsafe {
            kevent(
                self.fd,
                null(),
                0,
                &mut iter.events as *mut kevent,
                EventIter::BUFFER as c_int,
                timeout,
            )
        };
        iter
    }
}

pub struct EventIter {
    events: [kevent; EventIter::BUFFER],
    count: c_int,
    i: c_int,
}

impl EventIter {
    const EMPTY: EventIter = unsafe { mem::zeroed() };
    const BUFFER: usize = 128;

    pub fn reset(&mut self) {
        self.i = 0
    }
}

impl Iterator for EventIter {
    type Item = kevent;

    fn next(&mut self) -> Option<Self::Item> {
        if self.i == self.count {
            None
        } else {
            let event = self.events[self.i as usize];
            self.i += 1;
            Some(event)
        }
    }
}
