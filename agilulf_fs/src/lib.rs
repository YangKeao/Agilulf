//! An abstract layer for async write file.
//!
//! Tokio doesn't support latest async/await syntax in rust which is unbearable for this project (as
//! this project is an experiment for writing projects with async/await syntax in rust). So I have to
//! write an asynchronous file I/O library.
//!
//! The implementation of it is AIO so it supports Linux only.

#![feature(async_await)]

#[macro_use]
extern crate quick_error;
#[macro_use]
extern crate lazy_static;

use nix::fcntl;
use std::os::raw::c_int;
use std::os::unix::io::FromRawFd;
use std::pin::Pin;
use std::sync::Once;
use std::sync::RwLock;

type RawFd = c_int;

mod error;
pub use error::{FSError, Result};
use futures::task::{Context, Waker};
use futures::{Future, Poll};
use nix::sys::aio::{AioCb, LioOpcode};
use nix::sys::signal::{self, SigevNotify, Signal};
use std::sync::atomic::{AtomicUsize, Ordering};

static AIO_SIGNAL_HANDLER: Once = Once::new();
lazy_static! {
    static ref WAKER_LIST: RwLock<Vec<Waker>> = RwLock::new(Vec::new());
}

fn add_to_waker_list(waker: Waker) -> usize {
    let mut waker_list = WAKER_LIST.write().unwrap();
    waker_list.push(waker);
    return waker_list.len() - 1;
}

extern "C" fn handle_sig_io(_: i32, info: *mut libc::siginfo_t, _: *mut libc::c_void) {
    let index: usize = unsafe { std::mem::transmute((*info).si_value()) };
    WAKER_LIST
        .read()
        .unwrap()
        .get(index)
        .unwrap()
        .clone()
        .wake();
}

/// A struct contains only one fd.
///
/// As the constant style of Linux system call, aio accepts fd. And I cannot get a fd from `std::fs::File`.
///
/// ```ignore
///pub struct File {
///    fd: RawFd,
///}
/// ```
///
/// ## WARNING
///
/// 1. This struct will close the fd automatically when it's dropped.
///
/// 2. Open file with this struct will take over the SIGIO signal. AIO will send a SIGIO signal with
/// request id to this process and then this library will lookup `WAKER_LIST` and find the waker
/// corresponding to this id. Then it will wake that task.
///
pub struct File {
    fd: RawFd,
}

impl Drop for File {
    fn drop(&mut self) {
        unsafe {
            std::fs::File::from_raw_fd(self.fd.clone());
        }
    }
}

impl File {
    /// Open file from given path.
    ///
    /// The oflag in this function is set as `O_RDWR | O_CREAT`. And the mode is set as `S_IRUSR|S_IWUSR`.
    /// While opening the first file, it will take over the SIGIO signal (by set it's handler).
    pub fn open(path: &str) -> Result<Self> {
        use fcntl::OFlag;
        use nix::sys::stat::Mode;

        let fd = fcntl::open(
            path,
            OFlag::O_RDWR | OFlag::O_CREAT,
            Mode::S_IRUSR | Mode::S_IWUSR,
        )?;

        AIO_SIGNAL_HANDLER.call_once(|| {
            let sig_action = signal::SigAction::new(
                signal::SigHandler::SigAction(handle_sig_io),
                signal::SaFlags::empty(),
                signal::SigSet::empty(),
            );

            unsafe {
                signal::sigaction(signal::SIGIO, &sig_action).unwrap();
            }
        });

        Ok(File { fd })
    }

    /// Write buffer into file with aio.
    //noinspection RsTypeCheck
    pub fn write<'a>(&self, offset: i64, buf: &'a [u8]) -> WriteFile<'a> {
        let aio_cb = AioCb::from_slice(
            self.fd,
            offset,
            buf,
            0,
            SigevNotify::SigevNone,
            LioOpcode::LIO_NOP,
        );

        WriteFile {
            aio_cb,
            register: AtomicUsize::new(0),
        }
    }

    /// fallocate the file.
    //noinspection RsTypeCheck
    pub fn fallocate(&self, offset: i64, len: i64) -> Result<()> {
        fcntl::fallocate(self.fd, fcntl::FallocateFlags::empty(), offset, len)?;

        Ok(())
    }
}

/// A Future represents a write task.
///
/// It's the return value of the write method of a File. When it's polled the first time, it will
/// clone the waker and add the waker to `WAKER_LIST` and set `aio_cb.sigev_notify` with SIGIO signal
/// and set `aio_cb.sigev_notify.si_value` with the index of its waker in `WAKER_LIST`.
pub struct WriteFile<'a> {
    aio_cb: AioCb<'a>,
    register: AtomicUsize,
}

impl<'a> Future for WriteFile<'a> {
    type Output = Result<()>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let this = self.get_mut();
        if this.register.fetch_add(1, Ordering::SeqCst) == 0 {
            let waker = cx.waker().clone();

            this.aio_cb.set_sigev_notify(SigevNotify::SigevSignal {
                signal: Signal::SIGIO,
                si_value: add_to_waker_list(waker) as isize,
            });

            match this.aio_cb.write() {
                Ok(()) => {}
                Err(err) => return Poll::Ready(Err(err.into())),
            }
        }

        if let Err(_) = this.aio_cb.error() {
            return Poll::Pending; // TODO: handle other error here
        } else {
            match this.aio_cb.aio_return() {
                Ok(_status) => return Poll::Ready(Ok(())),
                Err(err) => return Poll::Ready(Err(err.into())),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn write_test() {
        let file = File::open("/tmp/test_file").unwrap();
        futures::executor::block_on(file.write(0, b"TEST_CONTENT")).unwrap();

        let mut reader = std::fs::File::open("/tmp/test_file").unwrap();
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf).unwrap();

        assert_eq!(buf.as_slice(), b"TEST_CONTENT");
    }

    #[test]
    fn lifetime_test() {
        let file = File::open("/tmp/test_file2").unwrap();
        futures::executor::block_on(async move {
            let content = b"TEST_CONTENT".to_vec();

            file.write(0, content.as_slice()).await.unwrap();

            let mut reader = std::fs::File::open("/tmp/test_file2").unwrap();
            let mut buf = Vec::new();
            reader.read_to_end(&mut buf).unwrap();

            assert_eq!(buf.as_slice(), b"TEST_CONTENT");
        })
    }
}
