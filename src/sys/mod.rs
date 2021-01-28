#![allow(
    non_camel_case_types,
    non_upper_case_globals,
    dead_code,
    non_snake_case
)]
#![allow(clippy::unreadable_literal, clippy::missing_safety_doc)]

use libc::*;

#[cfg(all(feature = "bindgen", not(feature = "overwrite")))]
include!(concat!(env!("OUT_DIR"), "/sys.rs"));

#[cfg(any(
    not(feature = "bindgen"),
    all(feature = "bindgen", feature = "overwrite")
))]
include!("sys.rs");

#[cfg(not(feature = "sgx"))]
pub use self::if_linux::*;
#[cfg(feature = "sgx")]
pub use self::if_sgx::*;

#[cfg(not(feature = "sgx"))]
mod if_linux {
    use super::*;

    #[cfg(not(feature = "direct-syscall"))]
    pub unsafe fn io_uring_register(
        fd: c_int,
        opcode: c_uint,
        arg: *const c_void,
        nr_args: c_uint,
    ) -> c_int {
        syscall(
            __NR_io_uring_register as c_long,
            fd as c_long,
            opcode as c_long,
            arg as c_long,
            nr_args as c_long,
        ) as _
    }

    #[cfg(feature = "direct-syscall")]
    pub unsafe fn io_uring_register(
        fd: c_int,
        opcode: c_uint,
        arg: *const c_void,
        nr_args: c_uint,
    ) -> c_int {
        sc::syscall4(
            __NR_io_uring_register as usize,
            fd as usize,
            opcode as usize,
            arg as usize,
            nr_args as usize,
        ) as _
    }

    #[cfg(not(feature = "direct-syscall"))]
    pub unsafe fn io_uring_setup(entries: c_uint, p: *mut io_uring_params) -> c_int {
        syscall(
            __NR_io_uring_setup as c_long,
            entries as c_long,
            p as c_long,
        ) as _
    }

    #[cfg(feature = "direct-syscall")]
    pub unsafe fn io_uring_setup(entries: c_uint, p: *mut io_uring_params) -> c_int {
        sc::syscall2(__NR_io_uring_setup as usize, entries as usize, p as usize) as _
    }

    #[cfg(not(feature = "direct-syscall"))]
    pub unsafe fn io_uring_enter(
        fd: c_int,
        to_submit: c_uint,
        min_complete: c_uint,
        flags: c_uint,
        sig: *const sigset_t,
    ) -> c_int {
        syscall(
            __NR_io_uring_enter as c_long,
            fd as c_long,
            to_submit as c_long,
            min_complete as c_long,
            flags as c_long,
            sig as c_long,
            core::mem::size_of::<sigset_t>() as c_long,
        ) as _
    }

    #[cfg(feature = "direct-syscall")]
    pub unsafe fn io_uring_enter(
        fd: c_int,
        to_submit: c_uint,
        min_complete: c_uint,
        flags: c_uint,
        sig: *const sigset_t,
    ) -> c_int {
        sc::syscall6(
            __NR_io_uring_enter as usize,
            fd as usize,
            to_submit as usize,
            min_complete as usize,
            flags as usize,
            sig as usize,
            core::mem::size_of::<sigset_t>() as usize,
        ) as _
    }

    pub unsafe fn start_enter_syscall_thread(fd: c_int) {
        use std::thread;
        println!("start_enter_syscall_thread");
        thread::spawn(move || loop {
            syscall(
                __NR_io_uring_enter as c_long,
                fd as c_long,
                1,
                0,
                0,
                std::ptr::null() as *const c_void,
                0,
            );
        });
    }
}

#[cfg(feature = "sgx")]
mod if_sgx {
    use super::*;
    use sgx_types::sgx_status_t;

    pub unsafe fn io_uring_register(
        fd: c_int,
        opcode: c_uint,
        arg: *const c_void,
        nr_args: c_uint,
    ) -> c_int {
        use std::os::unix::io::RawFd;
        // OCall needs the extra info of the size of the items in the
        // variable-length argument to copy data properly.
        let arg_size = match opcode {
            IORING_REGISTER_BUFFERS => std::mem::size_of::<libc::iovec>(),
            IORING_REGISTER_FILES => std::mem::size_of::<RawFd>(),
            IORING_REGISTER_EVENTFD => std::mem::size_of::<RawFd>(),
            IORING_REGISTER_FILES_UPDATE => std::mem::size_of::<RawFd>(),
            _ => panic!("invalid opcode"),
        };

        let mut ret: c_int = 0;
        ocall_io_uring_register_syscall(
            &mut ret,
            __NR_io_uring_register as c_long,
            fd as c_long,
            opcode as c_long,
            arg as *const c_void,
            nr_args as c_long,
            arg_size as c_long,
        );
        ret
    }

    pub unsafe fn io_uring_setup(entries: c_uint, p: *mut io_uring_params) -> c_int {
        let mut ret: c_int = 0;
        ocall_io_uring_setup_syscall(
            &mut ret,
            __NR_io_uring_setup as c_long,
            entries as c_long,
            p as *mut c_void,
            core::mem::size_of::<io_uring_params>() as c_long,
        );
        ret
    }

    pub unsafe fn io_uring_enter(
        fd: c_int,
        to_submit: c_uint,
        min_complete: c_uint,
        flags: c_uint,
        sig: *const sigset_t,
    ) -> c_int {
        let mut ret: c_int = 0;
        ocall_io_uring_enter_syscall(
            &mut ret,
            __NR_io_uring_enter as c_long,
            fd as c_long,
            to_submit as c_long,
            min_complete as c_long,
            flags as c_long,
            sig as *const c_void,
            core::mem::size_of::<sigset_t>() as c_long,
        );
        ret
    }

    pub unsafe fn start_enter_syscall_thread(fd: c_int) {
        ocall_start_enter_syscall_thread(
            __NR_io_uring_enter as c_long,
            fd as c_long,
            1,
            0,
            0,
            std::ptr::null() as *const c_void,
            0,
        );
    }

    extern "C" {
        fn ocall_io_uring_register_syscall(
            ret: *mut c_int,
            syscall_code: c_long,
            fd: c_long,
            opcode: c_long,
            arg: *const c_void,
            nr_args: c_long,
            arg_size: c_long,
        ) -> sgx_status_t;

        fn ocall_io_uring_setup_syscall(
            ret: *mut c_int,
            syscall_code: c_long,
            entries: c_long,
            p: *mut c_void,
            p_size: c_long,
        ) -> sgx_status_t;

        fn ocall_io_uring_enter_syscall(
            ret: *mut c_int,
            syscall_code: c_long,
            fd: c_long,
            to_submit: c_long,
            min_complete: c_long,
            flags: c_long,
            sig: *const c_void,
            sig_size: c_long,
        ) -> sgx_status_t;

        fn ocall_start_enter_syscall_thread(
            syscall_code: c_long,
            fd: c_long,
            to_submit: c_long,
            min_complete: c_long,
            flags: c_long,
            sig: *const c_void,
            sig_size: c_long,
        ) -> sgx_status_t;
    }
}
