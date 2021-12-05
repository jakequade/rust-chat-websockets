extern crate http_muncher;
extern crate mio;
extern crate rustc_serialize;
extern crate sha1;

use http_muncher::{Parser, ParserHandler};
use mio::tcp::*;
use mio::*;
use rustc_serialize::base64::{ToBase64, STANDARD};

use std::cell::RefCell;
use std::collections::HashMap;

use std::rc::Rc;

use crate::frame::WebSocketFrame;

pub struct HttpParser {
    current_key: Option<String>,
    headers: Rc<RefCell<HashMap<String, String>>>,
}

impl ParserHandler for HttpParser {
    fn on_header_field(&mut self, s: &[u8]) -> bool {
        self.current_key = Some(std::str::from_utf8(s).unwrap().to_string());
        true
    }

    fn on_header_value(&mut self, s: &[u8]) -> bool {
        self.headers.borrow_mut().insert(
            self.current_key.clone().unwrap(),
            std::str::from_utf8(s).unwrap().to_string(),
        );

        true
    }

    fn on_headers_complete(&mut self) -> bool {
        false
    }
}

pub struct WebSocketClient {
    pub socket: TcpStream,
    pub headers: Rc<RefCell<HashMap<String, String>>>,
    pub interest: EventSet,
    pub state: ClientState,
    outgoing: Vec<WebSocketFrame>,
}

pub enum ClientState {
    AwaitingHandshake(RefCell<Parser<HttpParser>>),
    HandshakeResponse,
    Connected,
}

fn gen_key(key: &String) -> String {
    let mut m = sha1::Sha1::new();
    let mut buf = [0u8; 20];

    m.update(key.as_bytes());
    m.update("258EAFA5-E914-47DA-95CA-C5AB0DC85B11".as_bytes());

    m.output(&mut buf);

    buf.to_base64(STANDARD)
}

impl WebSocketClient {
    pub fn read_handshake(&mut self) {
        loop {
            let mut buf = [0; 2048];
            match self.socket.try_read(&mut buf) {
                Err(e) => {
                    println!("Error while reading socket: {:?}", e);
                    return;
                }
                Ok(None) => {
                    // end of buffer
                    break;
                }
                Ok(Some(len)) => {
                    let is_upgrade =
                        if let ClientState::AwaitingHandshake(ref parser_state) = self.state {
                            let mut parser = parser_state.borrow_mut();
                            parser.parse(&buf);
                            parser.is_upgrade()
                        } else {
                            false
                        };

                    // Handshake was successful
                    if is_upgrade {
                        self.state = ClientState::HandshakeResponse;

                        self.interest.remove(EventSet::readable());
                        self.interest.insert(EventSet::writable());

                        break;
                    }
                }
            }
        }
    }

    pub fn read(&mut self) {
        let frame = WebSocketFrame::read(&mut self.socket);
        match frame {
            Ok(frame) => {
                println!("{:?}", frame);

                // Add a reply frame to the queue:
                let reply_frame = WebSocketFrame::from("Hi there!");
                self.outgoing.push(reply_frame);

                // Switch the event subscription to the write mode if the queue is not empty:
                if (self.outgoing.len() > 0) {
                    self.interest.remove(EventSet::readable());
                    self.interest.insert(EventSet::writable());
                }
            }
            Err(e) => println!("error while reading frame: {}", e),
        }
    }

    pub fn new(socket: TcpStream) -> WebSocketClient {
        let headers = Rc::new(RefCell::new(HashMap::new()));

        WebSocketClient {
            socket,
            // reads headers contents
            headers: headers.clone(),
            interest: EventSet::readable(),
            state: ClientState::AwaitingHandshake(RefCell::new(Parser::request(HttpParser {
                current_key: None,
                headers: headers.clone(),
            }))),
            outgoing: vec![],
        }
    }

    pub fn write(&mut self) {
        match self.state {
            ClientState::AwaitingHandshake(_) => {
                self.write_handshake();
            }
            ClientState::Connected => {
                println!("sending {} frames", self.outgoing.len());

                for frame in self.outgoing.iter() {
                    if let Err(e) = frame.write(&mut self.socket) {
                        println!("error on write: {}", e);
                    }
                }

                self.outgoing.clear();

                self.interest.remove(EventSet::writable());
                self.interest.insert(EventSet::readable());
            }
            _ => {}
        }
    }

    pub fn write_handshake(&mut self) {
        let headers = self.headers.borrow();
        let response_key = gen_key(&headers.get("Sec-WebSocket-Key").unwrap());

        let response = std::fmt::format(format_args!(
            "HTTP/1.1 101 Switching Protocols\r\n\
            Connection: Upgrade\r\n\
            Sec-WebSocket-Accept: {}\r\n\
            Upgrade: websocket\r\n\r\n",
            response_key
        ));

        self.socket.try_write(response.as_bytes()).unwrap();
        self.state = ClientState::Connected;

        self.interest.remove(EventSet::writable());
        self.interest.insert(EventSet::readable());
    }
}
