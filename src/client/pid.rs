//! Hyper server bindings for unix domain sockets

use std::fmt;

#[derive(Clone, Copy, Debug)]
pub enum Pid {
    None,
    Any,
    Value(i32),
}

impl fmt::Display for Pid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Pid::None => write!(f, "none"),
            Pid::Any => write!(f, "any"),
            Pid::Value(pid) => write!(f, "{}", pid),
        }
    }
}

#[cfg(target_os = "linux")]
pub use self::impl_linux::get_pid;

#[cfg(target_os = "linux")]
mod impl_linux {
    use libc::{c_void, getsockopt, ucred, SOL_SOCKET, SO_PEERCRED};
    use std::os::unix::io::AsRawFd;
    use std::{io, mem};
    use tokio_uds::UnixStream;

    use super::*;

    pub fn get_pid(sock: &UnixStream) -> io::Result<Pid> {
        let raw_fd = sock.as_raw_fd();
        let mut ucred = ucred {
            pid: 0,
            uid: 0,
            gid: 0,
        };
        let ucred_size = mem::size_of::<ucred>();

        // These paranoid checks should be optimized-out
        assert!(mem::size_of::<u32>() <= mem::size_of::<usize>());
        assert!(ucred_size <= u32::max_value() as usize);

        let mut ucred_size = ucred_size as u32;

        let ret = unsafe {
            getsockopt(
                raw_fd,
                SOL_SOCKET,
                SO_PEERCRED,
                &mut ucred as *mut ucred as *mut c_void,
                &mut ucred_size,
            )
        };
        if ret == 0 && ucred_size as usize == mem::size_of::<ucred>() {
            Ok(Pid::Value(ucred.pid))
        } else {
            Err(io::Error::last_os_error())
        }
    }
}

#[cfg(target_os = "macos")]
pub use self::impl_macos::get_pid;

#[cfg(target_os = "macos")]
pub mod impl_macos {
    use edgelet_core::pid::Pid;
    use libc::getpeereid;
    use std::os::unix::io::AsRawFd;
    use std::{io, mem};
    use tokio_uds::{UCred, UnixStream};

    pub fn get_pid(sock: &UnixStream) -> io::Result<Pid> {
        unsafe {
            let raw_fd = sock.as_raw_fd();

            let mut ucred: UCred = mem::uninitialized();

            let ret = getpeereid(raw_fd, &mut ucred.uid, &mut ucred.gid);

            if ret == 0 {
                Ok(Pid::Value(ucred.uid as _))
            } else {
                Err(io::Error::last_os_error())
            }
        }
    }
}

#[cfg(windows)]
pub use self::impl_windows::get_pid;

#[cfg(windows)]
mod impl_windows {
    use std::io;
    use std::os::windows::io::AsRawSocket;
    use winapi::ctypes::c_long;
    use winapi::um::winsock2::{ioctlsocket, WSAGetLastError, SOCKET_ERROR};

    use super::*;

    // SIO_AF_UNIX_GETPEERPID is defined in the Windows header afunix.h.
    const SIO_AF_UNIX_GETPEERPID: c_long = 0x5800_0100;

    pub fn get_pid(sock: &UnixStream) -> io::Result<Pid> {
        let raw_socket = sock.as_raw_socket();
        let mut pid = 0_u32;
        let ret = unsafe {
            ioctlsocket(
                raw_socket as _,
                SIO_AF_UNIX_GETPEERPID,
                &mut pid as *mut u32,
            )
        };
        if ret == SOCKET_ERROR {
            Err(io::Error::from_raw_os_error(unsafe { WSAGetLastError() }))
        } else {
            Ok(Pid::Value(pid as _))
        }
    }
}
