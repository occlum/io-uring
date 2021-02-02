// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License..

#![crate_name = "helloworldsampleenclave"]
#![crate_type = "staticlib"]
#![cfg_attr(not(target_env = "sgx"), no_std)]
#![cfg_attr(target_env = "sgx", feature(rustc_private))]

extern crate sgx_trts;
extern crate sgx_types;
#[cfg(not(target_env = "sgx"))]
#[macro_use]
extern crate sgx_tstd as std;

extern crate io_uring;
extern crate slab;

use sgx_trts::libc;
use sgx_types::*;
use std::os::unix::io::RawFd;
use std::prelude::v1::*;
use std::{io, ptr};

use io_uring::opcode::{self, types};
use io_uring::{squeue, IoUring};
use slab::Slab;

#[derive(Clone, Debug)]
enum Token {
    Accept,
    Poll {
        fd: RawFd,
    },
    Read {
        fd: RawFd,
        buf_index: usize,
    },
    Write {
        fd: RawFd,
        buf_index: usize,
        offset: usize,
        len: usize,
    },
}

pub struct AcceptCount {
    entry: squeue::Entry,
    count: usize,
}

impl AcceptCount {
    fn new(fd: RawFd, token: usize, count: usize) -> AcceptCount {
        AcceptCount {
            entry: opcode::Accept::new(types::Fd(fd), ptr::null_mut(), ptr::null_mut())
                .build()
                .user_data(token as _),
            count,
        }
    }

    pub fn push(&mut self, sq: &mut squeue::AvailableQueue) {
        while self.count > 0 {
            unsafe {
                match sq.push(self.entry.clone()) {
                    Ok(_) => self.count -= 1,
                    Err(_) => break,
                }
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn run_sgx_example() -> sgx_status_t {
    // std::backtrace::enable_backtrace("enclave.signed.so", std::backtrace::PrintFormat::Full);
    println!("[ECALL] run_sgx_example");

    let mut ring = IoUring::new(256).unwrap();

    let socket_fd = {
        let socket_fd = unsafe { libc::ocall::socket(libc::AF_INET, libc::SOCK_STREAM, 0) };
        if socket_fd < 0 {
            println!("[ECALL] create socket failed, ret: {}", socket_fd);
            return sgx_status_t::SGX_ERROR_UNEXPECTED;
        }

        let ret = unsafe {
            let servaddr = libc::sockaddr_in {
                sin_family: libc::AF_INET as u16,
                sin_port: 3456_u16.to_be(),
                sin_addr: libc::in_addr { s_addr: 0 },
                sin_zero: [0; 8],
            };
            libc::ocall::bind(
                socket_fd,
                &servaddr as *const _ as *const libc::sockaddr,
                core::mem::size_of::<libc::sockaddr_in>() as u32,
            )
        };
        if ret < 0 {
            println!("[ECALL] bind failed, ret: {}", ret);
            unsafe {
                libc::ocall::close(socket_fd);
            }
            return sgx_status_t::SGX_ERROR_UNEXPECTED;
        }

        let ret = unsafe { libc::ocall::listen(socket_fd, 10) };
        if ret < 0 {
            println!("[ECALL] listen failed, ret: {}", ret);
            unsafe {
                libc::ocall::close(socket_fd);
            }
            return sgx_status_t::SGX_ERROR_UNEXPECTED;
        }
        socket_fd
    };

    let mut backlog = Vec::new();
    let mut bufpool = Vec::with_capacity(64);
    let mut buf_alloc = Slab::with_capacity(64);
    let mut token_alloc = Slab::with_capacity(64);

    println!("[ECALL] listen 127.0.0.1:3456");

    let (submitter, sq, cq) = ring.split();

    let mut accept = AcceptCount::new(socket_fd, token_alloc.insert(Token::Accept), 3);

    accept.push(&mut sq.available());

    loop {
        match submitter.submit_and_wait(1) {
            Ok(_) => (),
            Err(ref err) if err.raw_os_error() == Some(libc::EBUSY) => (),
            Err(_) => {
                println!("[ECALL] submitter.submit_and_wait(1) failed");
                return sgx_status_t::SGX_ERROR_UNEXPECTED;
            }
        }

        let mut sq = sq.available();
        let mut iter = backlog.drain(..);

        // clean backlog
        loop {
            if sq.is_full() {
                match submitter.submit() {
                    Ok(_) => (),
                    Err(ref err) if err.raw_os_error() == Some(libc::EBUSY) => break,
                    Err(_) => {
                        println!("[ECALL] submitter.submit() failed");
                        return sgx_status_t::SGX_ERROR_UNEXPECTED;
                    }
                }
                sq.sync();
            }

            match iter.next() {
                Some(sqe) => unsafe {
                    let _ = sq.push(sqe);
                },
                None => break,
            }
        }

        drop(iter);

        accept.push(&mut sq);

        for cqe in cq.available() {
            let ret = cqe.result();
            let token_index = cqe.user_data() as usize;

            if ret < 0 {
                eprintln!(
                    "[ECALL] token {:?} error: {:?}",
                    token_alloc.get(token_index),
                    io::Error::from_raw_os_error(-ret)
                );
                continue;
            }

            let token = &mut token_alloc[token_index];
            match token.clone() {
                Token::Accept => {
                    println!("[ECALL] accept");

                    accept.count += 1;

                    let fd = ret;
                    let poll_token = token_alloc.insert(Token::Poll { fd });

                    let poll_e = opcode::PollAdd::new(types::Fd(fd), libc::POLLIN as _)
                        .build()
                        .user_data(poll_token as _);

                    unsafe {
                        if let Err(entry) = sq.push(poll_e) {
                            backlog.push(entry);
                        }
                    }
                }
                Token::Poll { fd } => {
                    let (buf_index, buf) = match bufpool.pop() {
                        Some(buf_index) => (buf_index, &mut buf_alloc[buf_index]),
                        None => {
                            let buf = Box::new(unsafe {
                                std::slice::from_raw_parts_mut(
                                    libc::ocall::malloc(2048) as *mut u8,
                                    2048,
                                )
                            });
                            let buf_entry = buf_alloc.vacant_entry();
                            let buf_index = buf_entry.key();
                            (buf_index, buf_entry.insert(buf))
                        }
                    };

                    *token = Token::Read { fd, buf_index };

                    let read_e = opcode::Read::new(types::Fd(fd), buf.as_mut_ptr(), buf.len() as _)
                        .build()
                        .user_data(token_index as _);

                    unsafe {
                        if let Err(entry) = sq.push(read_e) {
                            backlog.push(entry);
                        }
                    }
                }
                Token::Read { fd, buf_index } => {
                    if ret == 0 {
                        bufpool.push(buf_index);
                        token_alloc.remove(token_index);

                        println!("[ECALL] shutdown");

                        unsafe {
                            libc::ocall::close(fd);
                        }
                    } else {
                        let len = ret as usize;
                        let buf = &buf_alloc[buf_index];

                        *token = Token::Write {
                            fd,
                            buf_index,
                            len,
                            offset: 0,
                        };

                        let write_e = opcode::Write::new(types::Fd(fd), buf.as_ptr(), len as _)
                            .build()
                            .user_data(token_index as _);

                        unsafe {
                            if let Err(entry) = sq.push(write_e) {
                                backlog.push(entry);
                            }
                        }
                    }
                }
                Token::Write {
                    fd,
                    buf_index,
                    offset,
                    len,
                } => {
                    let write_len = ret as usize;

                    let entry = if offset + write_len >= len {
                        bufpool.push(buf_index);

                        *token = Token::Poll { fd };

                        opcode::PollAdd::new(types::Fd(fd), libc::POLLIN as _)
                            .build()
                            .user_data(token_index as _)
                    } else {
                        let offset = offset + write_len;
                        let len = len - offset;

                        let buf = &buf_alloc[buf_index][offset..];

                        *token = Token::Write {
                            fd,
                            buf_index,
                            offset,
                            len,
                        };

                        opcode::Write::new(types::Fd(fd), buf.as_ptr(), len as _)
                            .build()
                            .user_data(token_index as _)
                    };

                    unsafe {
                        if let Err(entry) = sq.push(entry) {
                            backlog.push(entry);
                        }
                    }
                }
            }
        }
    }
}
