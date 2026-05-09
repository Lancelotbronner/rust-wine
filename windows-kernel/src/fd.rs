use crate::net::SystemWatch;
use crate::server::WineServer;
use core::ffi::c_short;
use std::os::fd::RawFd;

pub struct WindowsFd {
    /// Unix file descriptor.
    unix: RawFd,
    // struct object        obj;         /* object header */
    // const struct fd_ops *fd_ops;      /*  */
    // struct object       *sync;        /* sync object for wait/signal */
    // struct inode        *inode;       /* inode that this fd belongs to */
    // struct list          inode_entry; /* entry in inode fd list */
    // struct closed_fd    *closed;      /* structure to store the unix fd at destroy time */
    // struct object       *user;        /* object using this file descriptor */
    // struct list          locks;       /* list of locks on this fd */
    // client_ptr_t         map_addr;    /* default mapping address for PE files */
    // mem_size_t           map_size;    /* mapping size for PE files */
    // unsigned int         access;      /* file access (FILE_READ_DATA etc.) */
    // unsigned int         options;     /* file options (FILE_DELETE_ON_CLOSE, FILE_SYNCHRONOUS...) */
    // unsigned int         sharing;     /* file sharing mode */
    // char                *unix_name;   /* unix file name */
    // WCHAR               *nt_name;     /* NT file name */
    // data_size_t          nt_namelen;  /* length of NT file name */
    // unsigned int         no_fd_status;/* status to return when unix_fd is -1 */
    // unsigned int         cacheable :1;/* can the fd be cached on the client side? */
    // unsigned int         fs_locks :1; /* can we use filesystem locks for this fd? */
    // int                  poll_index;  /* index of fd in poll array */
    // struct async_queue   read_q;      /* async readers of this fd */
    // struct async_queue   write_q;     /* async writers of this fd */
    // struct async_queue   wait_q;      /* other async waiters of this fd */
    // struct completion   *completion;  /* completion object attached to this fd */
    // apc_param_t          comp_key;    /* completion key to set in completion events */
    // unsigned int         comp_flags;  /* completion flags */
    revents: c_short,
}

impl SystemWatch for WindowsFd {
    fn revents_reset(&mut self) {
        self.revents = 0;
    }

    fn revents_add(&mut self, event: c_short) {
        self.revents |= event;
    }

    fn as_fd(&self) -> RawFd {
        self.unix
    }

    fn poll(&mut self, wine: &mut WineServer) {}
}

impl WindowsFd {}
