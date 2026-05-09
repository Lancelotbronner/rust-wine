use crate::clock::Clock;
use crate::fd::WindowsFd;
use crate::net::{EventWatch, SystemWatcher};
use crate::process::{WindowsPid, WindowsProcess};
use crate::thread::{WindowsThread, WindowsTid};
use libc::{
    c_int, c_short, nfds_t, poll, pollfd, unlink, EVFILT_READ, EVFILT_WRITE, EV_EOF, EV_ERROR,
    POLLERR, POLLHUP, POLLIN, POLLOUT,
};
use protocol::socket::WineSocket;
use std::ffi::CString;
use std::fs::create_dir_all;
use std::io;
use std::os::unix::net::{SocketAddr, UnixListener, UnixStream};
use std::path::Path;
use std::rc::Rc;
use std::str::FromStr;

pub struct WineServer {
    listener: UnixListener,
    watch: EventWatch,
    poll_users: Vec<*mut WindowsFd>,
    pollfd: Vec<pollfd>,
    clock: Clock,
    processes: Vec<Rc<WindowsProcess>>,
    threads: Vec<Rc<WindowsThread>>,
}

impl WineServer {
    /// Creates a new server.
    pub fn new(name: &str) -> io::Result<WineServer> {
        let path = format!("{}/{name}", WineSocket::PATH);
        println!("Creating Windows kernel at '{path}'");
        if let Some(parent) = Path::new(path.as_str()).parent() {
            create_dir_all(parent)?;
        }
        unsafe {
            unlink(CString::from_str(path.as_str())?.into_raw());
        }
        let server = WineServer {
            listener: UnixListener::bind(path)?,
            watch: EventWatch::default(),
            poll_users: Vec::new(),
            pollfd: Vec::new(),
            clock: Clock::default(),
            processes: Vec::new(),
            threads: Vec::new(),
        };
        server.listener.set_nonblocking(true)?;
        Ok(server)
    }

    pub fn start(mut self) -> ! {
        loop {
            println!("loop");
            self.poll_system();
            self.poll_clients();
            self.accept_clients();
        }
    }

    fn poll_system(&mut self) {
        while !self.poll_users.is_empty() {
            let timeout = self.clock.next_timeout();
            // Check if last user was removed by a timeout or if the event queue had an error
            if self.poll_users.is_empty() || !self.watch.is_valid() {
                break;
            }
            let mut iter = self.watch.poll(timeout);
            self.clock.set_current_time();

            // Put the events into the pollfd array first, like poll does
            for event in &mut iter {
                let user = event.udata as usize;
                let pollfd = &mut self.pollfd[user];
                pollfd.revents = 0;
                match event.filter {
                    EVFILT_READ => pollfd.revents |= POLLIN,
                    EVFILT_WRITE => pollfd.revents |= POLLOUT,
                    _ => (),
                }
                if event.flags & EV_EOF != 0 {
                    pollfd.revents |= POLLHUP;
                }
                if event.flags & EV_ERROR != 0 {
                    pollfd.revents |= POLLERR;
                }
            }

            // Read events from the pollfd array, as set_fd_events may modify them
            iter.reset();
            for event in iter {
                let user = event.udata as usize;

                if self.pollfd[user].revents != 0 {
                    let watcher = SystemWatcher::unpack(event.udata);
                    match watcher {
                        SystemWatcher::Thread(t) => self.poll_thread(unsafe { &*t }),
                        SystemWatcher::Process(p) => self.poll_process(unsafe { &*p }, self.pollfd[user].revents),
                        SystemWatcher::Fd(fd) => self.poll_fd(unsafe { &*fd }),
                    }
                    self.pollfd[user].revents = 0;
                }
            }
        }
    }

    fn poll_clients(&mut self) {
        //TODO: This is the backup system in case system events fail?
        while !self.poll_users.is_empty() {
            let timeout = self
                .clock
                .next_timeout()
                .map(|d| d.as_millis() as c_int)
                .unwrap_or(-1);
            // Check if last user was removed by a timeout
            if self.poll_users.is_empty() {
                break;
            }

            let mut ret = unsafe {
                poll(
                    self.pollfd.as_mut_ptr(),
                    self.poll_users.len() as nfds_t,
                    timeout,
                )
            };
            self.clock.set_current_time();
            if ret <= 0 {
                continue;
            }

            for i in 0..self.poll_users.len() {
                if self.pollfd[i].revents != 0 {
                    unsafe {
                        (*self.poll_users[i]).poll(self.pollfd[i].revents);
                    }
                    ret -= 1;
                    if ret == 0 {
                        break;
                    }
                }
            }
        }
    }

    fn poll_thread(&mut self, thread: &WindowsThread) {}

    fn poll_process(&mut self, process: &WindowsProcess, event: c_short) {
        if (event & (POLLERR | POLLHUP) != 0) {
            process.kill(!process.is_terminating);
        } else if (event & POLLIN != 0) {
            process.receive_fd();
        }
    }

    fn poll_fd(&mut self, fd: &WindowsFd) {}

    fn accept_clients(&mut self) {
        self.listener
            .set_nonblocking(!self.poll_users.is_empty())
            .expect("failed to update non-blocking");
        let Ok((client, addr)) = self.listener.accept() else {
            return;
        };
        self.accept(client, addr).expect("failed to accept client");
    }

    fn accept(&mut self, client: UnixStream, _addr: SocketAddr) -> io::Result<()> {
        client.set_nonblocking(true)?;
        let pid = WindowsPid(self.processes.len() as u32);
        let mut process = Rc::new(WindowsProcess::new_from_client(client, pid));
        self.processes.push(process.clone());
        let tid = WindowsTid(self.threads.len() as u32);
        let thread = Rc::new(WindowsThread::primary_for(process.clone(), tid)?);
        self.threads.push(thread.clone());
        Rc::get_mut(&mut process)
            .unwrap()
            .add_thread(thread.clone());
        self.watch.reads_watch(SystemWatcher::Thread(
            thread.as_ref() as *const WindowsThread
        ));
        Ok(())
    }
}
