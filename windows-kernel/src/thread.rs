use crate::process::WindowsProcess;
use std::io;
use std::io::{pipe, PipeReader};
use std::os::fd::{AsRawFd, RawFd};
use std::rc::Rc;

pub struct WindowsTid(pub u32);

pub struct WindowsThread {
    tid: WindowsTid,
    process: Rc<WindowsProcess>,
    request: PipeReader,
}

impl WindowsThread {
    pub fn primary_for(process: Rc<WindowsProcess>, tid: WindowsTid) -> io::Result<WindowsThread> {
        let pipe = pipe()?;
        let thread = WindowsThread {
            tid,
            process,
            request: pipe.0,
        };
        Ok(thread)
    }

    pub fn request_fd(&self) -> RawFd {
        self.request.as_raw_fd()
    }
}
