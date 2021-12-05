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
    pub http_parser: Parser<HttpParser>,
    pub headers: Rc<RefCell<HashMap<String, String>>>,
    pub interest: EventSet,
    pub state: ClientState,
}

#[derive(PartialEq)]
pub enum ClientState {
    AwaitingHandshake,
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
    pub fn read(&mut self) {
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
                    self.http_parser.parse(&buf[0..len]);

                    // Handshake was successful
                    if self.http_parser.is_upgrade() {
                        self.state = ClientState::HandshakeResponse;

                        self.interest.remove(EventSet::readable());
                        self.interest.insert(EventSet::writable());

                        break;
                    }
                }
            }
        }
    }

    pub fn new(socket: TcpStream) -> WebSocketClient {
        let headers = Rc::new(RefCell::new(HashMap::new()));

        WebSocketClient {
            socket,
            // reads headers contents
            headers: headers.clone(),
            http_parser: Parser::request(HttpParser {
                current_key: None,
                // writes new headers
                headers: headers.clone(),
            }),
            interest: EventSet::readable(),
            state: ClientState::AwaitingHandshake,
        }
    }

    pub fn write(&mut self) {
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
