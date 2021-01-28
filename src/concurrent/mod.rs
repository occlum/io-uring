//! Concurrent IoUring.

mod cqueue;
mod squeue;

use std::io;

pub use cqueue::CompletionQueue;
#[cfg(not(feature = "sgx"))]
use parking_lot::Mutex;
pub use squeue::SubmissionQueue;
#[cfg(feature = "sgx")]
use std::sync::SgxMutex as Mutex;

/// Concurrent IoUring instance
pub struct IoUring {
    ring: crate::IoUring,
    push_lock: Mutex<()>,
}

unsafe impl Send for IoUring {}
unsafe impl Sync for IoUring {}

impl IoUring {
    pub(crate) fn new(ring: crate::IoUring) -> IoUring {
        IoUring {
            ring,
            push_lock: Mutex::new(()),
        }
    }

    /// Start a thread that runs a busy-loop of calling io_uring_enter syscall.
    ///
    /// This method is intended to emulate the SQPOLL mode of io_uring at the
    /// user space. This is very useful as Linux kernerl's current support of the
    /// SQLPOLL mode is quite limited and buggy.
    ///
    /// This method is unsafe since the thread does not know when the resources associted
    /// with the io_uring instance will be released. So to use this function safely,
    /// the user needs to ensure the io_uring will not be destroyed after starting
    /// the thread.
    pub unsafe fn start_enter_syscall_thread(&self) {
        self.ring.start_enter_syscall_thread();
    }

    /// Initiate and/or complete asynchronous I/O
    ///
    /// # Safety
    ///
    /// This provides a raw interface so developer must ensure that parameters are correct.
    #[inline]
    pub unsafe fn enter(
        &self,
        to_submit: u32,
        min_complete: u32,
        flag: u32,
        sig: Option<&libc::sigset_t>,
    ) -> io::Result<usize> {
        self.ring.enter(to_submit, min_complete, flag, sig)
    }

    /// Initiate asynchronous I/O.
    #[inline]
    pub fn submit(&self) -> io::Result<usize> {
        self.ring.submit()
    }

    /// Initiate and/or complete asynchronous I/O
    #[inline]
    pub fn submit_and_wait(&self, want: usize) -> io::Result<usize> {
        self.ring.submit_and_wait(want)
    }

    /// Get submission queue
    pub fn submission(&self) -> SubmissionQueue<'_> {
        unsafe {
            SubmissionQueue {
                queue: &self.ring.sq,
                push_lock: &self.push_lock,
                ring_mask: self.ring.sq.ring_mask.read(),
                ring_entries: self.ring.sq.ring_entries.read(),
            }
        }
    }

    /// Get completion queue
    pub fn completion(&self) -> CompletionQueue<'_> {
        unsafe {
            CompletionQueue {
                queue: &self.ring.cq,
                ring_mask: self.ring.cq.ring_mask.read(),
                ring_entries: self.ring.cq.ring_entries.read(),
            }
        }
    }

    /// Get original IoUring instance
    pub fn into_inner(self) -> crate::IoUring {
        self.ring
    }
}
