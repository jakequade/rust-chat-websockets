extern crate http_muncher;
extern crate mio;
extern crate rustc_serialize;
extern crate sha1;

use mio::tcp::*;
use mio::*;

use std::collections::HashMap;

use crate::client::WebSocketClient;

pub struct WebSocketServer {
    pub socket: TcpListener,
    pub clients: HashMap<Token, WebSocketClient>,
    pub token_counter: usize,
}

const SERVER_TOKEN: Token = Token(0);

impl Handler for WebSocketServer {
    type Timeout = usize;
    type Message = ();

    fn ready(
        &mut self,
        event_loop: &mut EventLoop<WebSocketServer>,
        token: Token,
        events: EventSet,
    ) {
        // read event logic
        if events.is_readable() {
            match token {
                SERVER_TOKEN => {
                    let client_socket = match self.socket.accept() {
                        Err(e) => {
                            println!("Accept error: {}", e);
                            return;
                        }
                        Ok(None) => unreachable!("Accept has returned 'None'"),
                        Ok(Some((sock, _))) => sock,
                    };

                    self.token_counter += 1;

                    let new_token = Token(self.token_counter);

                    self.clients
                        .insert(new_token, WebSocketClient::new(client_socket));

                    event_loop
                        .register(
                            &self.clients[&new_token].socket,
                            new_token,
                            EventSet::readable(),
                            PollOpt::edge() | PollOpt::oneshot(),
                        )
                        .unwrap();
                }
                token => {
                    let mut client = self.clients.get_mut(&token).unwrap();

                    client.read();

                    event_loop
                        .reregister(
                            &client.socket,
                            token,
                            client.interest,
                            PollOpt::edge() | PollOpt::oneshot(),
                        )
                        .unwrap();
                }
            }
        }

        if events.is_writable() {
            let mut client = self.clients.get_mut(&token).unwrap();

            client.write();

            event_loop
                .reregister(
                    &client.socket,
                    token,
                    client.interest,
                    PollOpt::edge() | PollOpt::oneshot(),
                )
                .unwrap();
        }
    }
}
