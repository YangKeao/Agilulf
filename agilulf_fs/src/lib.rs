#![feature(async_await)]

#[macro_use]
extern crate quick_error;
#[macro_use]
extern crate lazy_static;

use nix::fcntl;
use std::borrow::Borrow;
use std::os::raw::c_int;
use std::path::Path;
use std::pin::Pin;
use std::sync::{Arc, Once};
use std::sync::{RwLock, RwLockReadGuard};

type RawFd = c_int;

pub mod error;
pub use error::{FSError, Result};
use futures::task::{Context, Waker};
use futures::{Future, Poll};
use nix::sys::aio::{AioCb, LioOpcode};
use nix::sys::signal::{self, SigevNotify, Signal};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

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

pub struct File {
    fd: RawFd,
}

impl File {
    fn open(path: &str) -> Result<Self> {
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

    fn write<'a>(&self, offset: i64, buf: &'a [u8]) -> WriteFile<'a> {
        let mut aio_cb = AioCb::from_slice(
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
}

pub struct WriteFile<'a> {
    aio_cb: AioCb<'a>,
    register: AtomicUsize,
}

impl<'a> Future for WriteFile<'a> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let this = self.get_mut();
        if this.register.fetch_add(1, Ordering::SeqCst) == 0 {
            let waker = cx.waker().clone();

            this.aio_cb.set_sigev_notify(SigevNotify::SigevSignal {
                signal: Signal::SIGIO,
                si_value: add_to_waker_list(waker) as isize,
            });

            this.aio_cb.write().unwrap(); // TODO: handle this error
        }

        if let Err(_) = this.aio_cb.error() {
            return Poll::Pending; // TODO: handle other error here
        } else {
            this.aio_cb.aio_return().unwrap(); // TODO: handle error here
            return Poll::Ready(());
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::io::Read;

    #[test]
    fn write_test() {
        let file = File::open("/tmp/test_file").unwrap();
        futures::executor::block_on(file.write(0, b"TEST_CONTENT"));

        let mut reader = std::fs::File::open("/tmp/test_file").unwrap();
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf);

        assert_eq!(buf.as_slice(), b"TEST_CONTENT");
    }

    #[test]
    fn lifetime_test() {
        let file = File::open("/tmp/test_file2").unwrap();
        futures::executor::block_on(async move {
            let content = b"TEST_CONTENT".to_vec();

            file.write(0, content.as_slice());

            let mut reader = std::fs::File::open("/tmp/test_file").unwrap();
            let mut buf = Vec::new();
            reader.read_to_end(&mut buf);

            assert_eq!(buf.as_slice(), b"TEST_CONTENT");
        })
    }
}
