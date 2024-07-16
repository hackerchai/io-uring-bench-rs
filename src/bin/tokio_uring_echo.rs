use std::io;
use std::net::SocketAddr;
use tokio_uring::buf::BoundedBuf;
use tokio_uring::net::{TcpListener, TcpStream};

const MAX_MESSAGE_LEN: usize = 2048;

fn main() -> io::Result<()> {
    let addr: SocketAddr = "0.0.0.0:8082".to_string().parse().unwrap();

    tokio_uring::start(async {
        let listener = TcpListener::bind(addr)?;
        println!("Server listening on port: 8082");

        loop {
            match listener.accept().await {
                Ok((socket, _)) => {
                    tokio_uring::spawn(handle_connection(socket));
                }
                Err(e) => {
                    eprintln!("Failed to accept connection: {}", e);
                }
            }
        }
    })
}

async fn handle_connection(socket: TcpStream) {
    let mut buf = vec![0; MAX_MESSAGE_LEN];

    loop {
        let read_buf = buf.split_off(0);
        let (res, read_buf) = socket.read(read_buf).await;
        match res {
            Ok(n) if n == 0 => return,
            Ok(n) => {
                let (res, _) = socket.write_all(read_buf.slice(..n)).await;
                if let Err(e) = res {
                    eprintln!("Failed to write to socket; err = {:?}", e);
                    return;
                }
            }
            Err(e) => {
                eprintln!("Failed to read from socket; err = {:?}", e);
                return;
            }
        }
        buf = vec![0; MAX_MESSAGE_LEN]; // reset the buffer
    }
}
// use std::io;
// use std::net::SocketAddr;
// use tokio_uring::buf::BoundedBuf;
// use tokio_uring::net::{TcpListener, TcpStream};

// const MAX_MESSAGE_LEN: usize = 2048;
// const BUFFER_POOL_SIZE: usize = 1024;

// fn main() -> io::Result<()> {
//     let addr: SocketAddr = "0.0.0.0:8082".parse().unwrap();

//     tokio_uring::start(async {
//         let listener = TcpListener::bind(addr)?;
//         println!("Server listening on port: 8082");

//         let buffer_pool = BufferPool::new(BUFFER_POOL_SIZE, MAX_MESSAGE_LEN);

//         loop {
//             match listener.accept().await {
//                 Ok((socket, _)) => {
//                     let buffer_pool = buffer_pool.clone();
//                     tokio_uring::spawn(handle_connection(socket, buffer_pool));
//                 }
//                 Err(e) => {
//                     eprintln!("Failed to accept connection: {}", e);
//                 }
//             }
//         }
//     })
// }

// async fn handle_connection(socket: TcpStream, buffer_pool: BufferPool) {
//     loop {
//         let buf = buffer_pool.get();
//         let (res, read_buf) = socket.read(buf.clone()).await;
//         match res {
//             Ok(0) => return,
//             Ok(n) => {
//                 let (res, _) = socket.write_all(read_buf.slice(..n)).await;
//                 if let Err(e) = res {
//                     eprintln!("Failed to write to socket; err = {:?}", e);
//                     return;
//                 }
//             }
//             Err(e) => {
//                 eprintln!("Failed to read from socket; err = {:?}", e);
//                 return;
//             }
//         }
//         buffer_pool.put(buf.clone());
//     }
// }

// #[derive(Clone)]
// struct BufferPool {
//     inner: std::sync::Arc<parking_lot::Mutex<Vec<Vec<u8>>>>,
//     buffer_size: usize,
// }

// impl BufferPool {
//     fn new(pool_size: usize, buffer_size: usize) -> Self {
//         let mut pool = Vec::with_capacity(pool_size);
//         for _ in 0..pool_size {
//             pool.push(vec![0; buffer_size]);
//         }
//         BufferPool {
//             inner: std::sync::Arc::new(parking_lot::Mutex::new(pool)),
//             buffer_size,
//         }
//     }

//     fn get(&self) -> Vec<u8> {
//         self.inner
//             .lock()
//             .pop()
//             .unwrap_or_else(|| vec![0; self.buffer_size])
//     }

//     fn put(&self, buf: Vec<u8>) {
//         let mut pool = self.inner.lock();
//         if pool.len() < pool.capacity() {
//             pool.push(buf);
//         }
//     }
// }