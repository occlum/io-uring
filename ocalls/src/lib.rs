extern crate libc;
extern crate sgx_types;
extern crate sgx_urts;

use libc::syscall;
use sgx_types::{c_int, c_long, c_void};
use std::thread;

#[no_mangle]
pub extern "C" fn ocall_io_uring_register_syscall(
    syscall_code: c_long,
    fd: c_long,
    opcode: c_long,
    arg: *const c_void,
    nr_args: c_long,
    _arg_size: c_long, // only useful to EDL
) -> c_int {
    unsafe { syscall(syscall_code, fd, opcode, arg as c_long, nr_args) as _ }
}

#[no_mangle]
pub extern "C" fn ocall_io_uring_setup_syscall(
    syscall_code: c_long,
    entries: c_long,
    p: *mut c_void,
    _p_size: c_long, // only useful to EDL
) -> c_int {
    unsafe { syscall(syscall_code, entries, p as c_long) as _ }
}

#[no_mangle]
pub extern "C" fn ocall_io_uring_enter_syscall(
    syscall_code: c_long,
    fd: c_long,
    to_submit: c_long,
    min_complete: c_long,
    flags: c_long,
    sig: *const c_void,
    sig_size: c_long,
) -> c_int {
    unsafe {
        syscall(
            syscall_code,
            fd,
            to_submit,
            min_complete,
            flags,
            sig as c_long,
            sig_size,
        ) as _
    }
}

#[no_mangle]
pub extern "C" fn ocall_start_enter_syscall_thread(
    syscall_code: c_long,
    fd: c_long,
    to_submit: c_long,
    min_complete: c_long,
    flags: c_long,
    sig: *const c_void,
    sig_size: c_long,
) {
    println!("ocall_start_enter_syscall_thread");
    let sig_addr = sig as c_long;
    thread::spawn(move || loop {
        unsafe {
            syscall(
                syscall_code,
                fd,
                to_submit,
                min_complete,
                flags,
                sig_addr,
                sig_size,
            );
        }
    });
}
