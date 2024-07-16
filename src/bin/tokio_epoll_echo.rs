use std::io;
use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::main]
async fn main() -> io::Result<()> {
    let addr: SocketAddr = "0.0.0.0:8081".to_string().parse().unwrap();
    let listener = TcpListener::bind(&addr).await?;
    println!("Server listening on port: 8081");

    loop {
        let (socket, _) = listener.accept().await?;
        tokio::spawn(async move {
            if let Err(e) = handle_connection(socket).await {
                eprintln!("Failed to handle connection: {}", e);
            }
        });
    }
}

async fn handle_connection(mut socket: TcpStream) -> io::Result<()> {
    let mut buf = vec![0; 2048];

    loop {
        let n = match socket.read(&mut buf).await {
            Ok(n) if n == 0 => return Ok(()),
            Ok(n) => n,
            Err(e) => {
                eprintln!("Failed to read from socket; err = {:?}", e);
                return Err(e);
            }
        };

        if let Err(e) = socket.write_all(&buf[0..n]).await {
            eprintln!("Failed to write to socket; err = {:?}", e);
            return Err(e);
        }
    }
}