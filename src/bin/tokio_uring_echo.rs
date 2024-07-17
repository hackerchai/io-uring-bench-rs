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