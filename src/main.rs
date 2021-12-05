extern crate http_muncher;
extern crate mio;
extern crate rustc_serialize;
extern crate sha1;

use mio::tcp::*;
use mio::*;

use std::collections::HashMap;

use std::error::Error;
use std::net::SocketAddr;

mod client;
mod server;

use crate::server::WebSocketServer;

pub const SERVER_TOKEN: Token = Token(0);

fn main() -> Result<(), Box<dyn Error>> {
    let mut event_loop = EventLoop::new().unwrap();

    let address = "0.0.0.0:10000".parse::<SocketAddr>().unwrap();
    let server_socket = TcpListener::bind(&address).unwrap();

    let mut server = WebSocketServer {
        token_counter: 1,
        clients: HashMap::new(),
        socket: server_socket,
    };

    event_loop
        .register(
            &server.socket,
            SERVER_TOKEN,
            EventSet::readable(),
            PollOpt::edge(),
        )
        .unwrap();

    event_loop.run(&mut server).unwrap();

    Ok(())
}
