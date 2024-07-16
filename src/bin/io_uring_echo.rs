use std::collections::VecDeque;
use std::io;
use std::net::TcpListener;
use std::os::fd::{AsRawFd, RawFd};
use std::collections::HashMap;

// https://github.com/tokio-rs/io-uring/blob/66844fbe9b10db40faa31e286ed8fc15fc8ab7f2/src/sys/sys.rs#L121C11-L121C28
const IORING_CQE_F_MORE: u32 = 2;

use io_uring::squeue::Entry;
use io_uring::types::Fd;
use io_uring::{opcode, IoUring, SubmissionQueue};
use slab::Slab;

#[derive(Debug)]
enum Op {
    Accept,
    Recv(RawFd, u16),
    Send(RawFd),
    Close(RawFd),
}

impl From<u64> for Op {
    fn from(value: u64) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl From<Op> for u64 {
    fn from(val: Op) -> Self {
        unsafe { std::mem::transmute(val) }
    }
}

fn push(submission: &mut SubmissionQueue<'_>, entry: Entry, backlog: &mut VecDeque<Entry>) {
    unsafe {
        if submission.push(&entry).is_err() {
            backlog.push_back(entry);
        }
    }
}

fn accept(submission: &mut SubmissionQueue<'_>, fd: RawFd, backlog: &mut VecDeque<Entry>) {
    let entry = opcode::AcceptMulti::new(Fd(fd))
        .build()
        .user_data(Op::Accept.into());
    push(submission, entry, backlog)
}

fn recv(
    submission: &mut SubmissionQueue<'_>,
    fd: RawFd,
    buf: *mut u8,
    buf_len: u32,
    buf_idx: u16,
    backlog: &mut VecDeque<Entry>,
) {
    let entry = opcode::Recv::new(Fd(fd), buf, buf_len)
        .build()
        .user_data(Op::Recv(fd, buf_idx).into());
    push(submission, entry, backlog)
}

fn receive(
    sq: &mut SubmissionQueue<'_>,
    buf_alloc: &mut Slab<[u8; 1024]>,
    fd: RawFd,
    backlog: &mut VecDeque<Entry>,
) -> u16 {
    let buf_entry = buf_alloc.vacant_entry();
    let buf_index = buf_entry.key() as u16;
    let buf = buf_entry.insert([0u8; 1024]);

    recv(sq, fd, buf.as_mut_ptr(), buf.len() as _, buf_index, backlog);
    buf_index
}

fn send(submission: &mut SubmissionQueue<'_>, fd: RawFd, data: &[u8], backlog: &mut VecDeque<Entry>) {
    let entry = opcode::Send::new(Fd(fd), data.as_ptr(), data.len() as _)
        .build()
        .user_data(Op::Send(fd).into());
    push(submission, entry, backlog)
}

fn close(submission: &mut SubmissionQueue<'_>, fd: RawFd, backlog: &mut VecDeque<Entry>) {
    let entry = opcode::Close::new(Fd(fd))
        .build()
        .user_data(Op::Close(fd).into());
    push(submission, entry, backlog)
}

fn main() -> io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:8083")?;
    println!("Server listening on {}", listener.local_addr()?);

    let mut ring = IoUring::new(1024)?;
    let (submitter, mut sq, mut cq) = ring.split();

    let mut backlog = VecDeque::new();
    // TODO: remove
    let mut buf_alloc = Slab::with_capacity(2048);

    let mut fd_to_buffer_index: HashMap<RawFd, u16> = HashMap::new();

    // initialize_buffers(&mut sq, &mut bufs, &mut backlog);
    accept(&mut sq, listener.as_raw_fd(), &mut backlog);
    sq.sync();

    loop {
        match submitter.submit_and_wait(1) {
            Ok(_) => (),
            Err(err) => return Err(err),
        }

        loop {
            if cq.is_full() {
                break;
            }

            if sq.is_full() {
                submitter.squeue_wait()?;
            }

            sq.sync();
            match backlog.pop_front() {
                Some(sqe) => {
                    let _ = unsafe { sq.push(&sqe) };
                }
                None => break,
            };
        }

        cq.sync();

        for cqe in &mut cq {
            let event = cqe.user_data();
            let result = cqe.result();

            if result < 0 {
                eprintln!(
                    "CQE {:?} failed: {:?}",
                    Op::from(event),
                    io::Error::from_raw_os_error(-result)
                );
                match Op::from(event) {
                    Op::Recv(fd, buf_idx) => {
                        close(&mut sq, fd, &mut backlog);
                        buf_alloc.remove(buf_idx as usize); 
                    }
                    Op::Send(fd) => {
                        close(&mut sq, fd, &mut backlog);
                    }
                    _ => {}
                }
                continue;
            }

            match Op::from(event) {
                Op::Accept => {
                    let conn_fd = result;
                    if cqe.flags() & IORING_CQE_F_MORE == 0 {
                        accept(&mut sq, listener.as_raw_fd(), &mut backlog);
                    }
                    //receive(&mut sq, &mut buf_alloc, conn_fd, &mut backlog);
                    let buf_idx = receive(&mut sq, &mut buf_alloc, conn_fd, &mut backlog);
                    fd_to_buffer_index.insert(conn_fd, buf_idx);
                }
                Op::Recv(fd, buf_idx) => {
                    let buf = &buf_alloc[buf_idx as usize];
                    let received_data = &buf[..result as usize];
                
                    if result == 0 {
                        close(&mut sq, fd, &mut backlog);
                        buf_alloc.remove(buf_idx as usize); 
                    } else {
                        // Echo back the received data
                        send(&mut sq, fd, received_data, &mut backlog);
                    }
                }
                Op::Send(fd) => {
                    receive(&mut sq, &mut buf_alloc, fd, &mut backlog);
                }
                Op::Close(fd) => {
                    if let Some(buf_idx) = fd_to_buffer_index.remove(&fd) {  // Remove and get the buffer index
                        buf_alloc.remove(buf_idx as usize);
                        let _ = nix::unistd::close(fd);  // Use the nix crate to handle closing the fd
                        println!("Connection closed: fd {}", fd);
                    } else {
                        eprintln!("Attempted to close an unknown connection: fd {}", fd);
                    }
                }
            }
        }
    }
}