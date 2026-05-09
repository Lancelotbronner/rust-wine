use crate::net::SystemWatch;
use crate::process::WindowsProcess;
use crate::server::WineServer;
use core::ffi::c_short;
use std::io;
use std::io::{pipe, PipeReader};
use std::os::fd::{AsRawFd, RawFd};
use std::rc::Rc;

pub struct WindowsTid(pub u32);

pub struct WindowsThread {
    tid: WindowsTid,
    process: Rc<WindowsProcess>,
    request: PipeReader,
    revents: c_short,
}

impl SystemWatch for WindowsThread {
    fn revents_reset(&mut self) {
        self.revents = 0;
    }

    fn revents_add(&mut self, event: c_short) {
        self.revents |= event;
    }

    fn as_fd(&self) -> RawFd {
        self.request.as_raw_fd()
    }

    fn poll(&mut self, wine: &mut WineServer) {}
}

impl WindowsThread {
    pub fn primary_for(process: Rc<WindowsProcess>, tid: WindowsTid) -> io::Result<WindowsThread> {
        let pipe = pipe()?;
        let thread = WindowsThread {
            tid,
            process,
            request: pipe.0,
            revents: 0,
        };
        Ok(thread)
    }
}
