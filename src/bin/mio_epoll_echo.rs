use mio::{Events, Interest, Poll, Token};
use mio::net::{TcpListener, TcpStream};
use std::io::{self, Read, Write};
use std::collections::HashMap;
use std::net::SocketAddr;

const SERVER: Token = Token(0);
const MAX_EVENTS: usize = 128;
const MAX_MESSAGE_LEN: usize = 2048;

fn main() -> io::Result<()> {
    let addr: SocketAddr = "0.0.0.0:8084".parse().unwrap();
    let mut poll = Poll::new()?;
    let mut events = Events::with_capacity(MAX_EVENTS);
    let mut server = TcpListener::bind(addr)?;
    println!("Server listening on {}", server.local_addr()?);

    poll.registry().register(&mut server, SERVER, Interest::READABLE)?;

    let mut connections: HashMap<Token, TcpStream> = HashMap::new();
    let mut unique_token = Token(SERVER.0 + 1);  // Start after SERVER token

    loop {
        poll.poll(&mut events, None)?;

        for event in &events {
            match event.token() {
                SERVER => {
                    // Accept connections
                    while let Ok((mut connection, _)) = server.accept() {
                        let token = unique_token;
                        poll.registry().register(&mut connection, token, Interest::READABLE | Interest::WRITABLE)?;
                        connections.insert(token, connection);
                        unique_token = Token(token.0 + 1);
                    }
                },
                token => {
                    // Handle existing connections
                    if let Some(connection) = connections.get_mut(&token) {
                        if event.is_readable() {
                            let mut buf = [0; MAX_MESSAGE_LEN];
                            match connection.read(&mut buf) {
                                Ok(0) => {
                                    // Connection closed by the client
                                    connections.remove(&token);
                                },
                                Ok(n) => {
                                    // Echo back the data received
                                    if connection.write_all(&buf[..n]).is_err() {
                                        connections.remove(&token);
                                    }
                                },
                                Err(ref err) if would_block(err) => {},
                                Err(_) => {
                                    // Unexpected error, terminate connection
                                    connections.remove(&token);
                                },
                            }
                        }
                    }
                }
            }
        }
    }
}

fn would_block(err: &io::Error) -> bool {
    err.kind() == io::ErrorKind::WouldBlock
}