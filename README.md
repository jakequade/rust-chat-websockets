# Building a scalable chat app using websockets

- tutorial: https://nbaksalyar.github.io/2015/07/10/writing-chat-in-rust.html

## How it works

### main.rs

1. Creates an event loop
2. Binds a TCP listener to a local port
  - TCP listeners guarantee messages arrive in sequence. UDP would not.
3. Create a websocket server, with:
  - An initial token counter of `1`
  - An empty hashmap of clients
  - The socket we initialised via the TCP listener
4. The event loop then registers the socket with the following properties:
  - An io of the TCP listener's socket
  - A token. This is the unique identifier of the socket. The application currently just uses auto-incrementation for uniqueness.
  - An `EventSet` which describes intent - reading new events, writing when a socket is available, or both.
  - Polling options. We choose `edge`-triggered events (as opposed to level-triggered events).
    - Edge-triggered subscriptions are notified when ever there is _new_ information arrived at a socket.
    - Level-triggered subscriptions are notified when a socket has some data available to read.

### server.rs

Our `WebSocketServer` implements `Handler` from the `mio` crate. It implements `ready`, which is invoked when the socket (identified by its `token`) is ready to be operated on.

When events come to the server, if they're writable, the server will:

1. match the inbound token. When matches are **readable**:
  a. If the token matches the `const SERVER_TOKEN`, it:
    i. accepts the socket request
    ii. increments its internal counter and creates a new token from it
    iii. Adds the new token and socket to its internal list of clients 
    iv. registers the new client's socket to the event loop.
  b. If it is some other new token, it will:
    i. use its internal client list and retrieve the client at the specified `Token` index.
    ii. Read from the new client
    iii. Add a reregister for the designated client to start listening for data again.
2. When the matches are **writable**:
  a. The server retrieves the token-indexed client
  b. It writes events a basic request with the required `Sec-WebSocket-Accept` header. This header:
    i. Is a unique string appended to the actual data the client needs. This massive string is then encrypted in sha1 format and base64 encoded. This is then sent to the client.

### client.rs

The client code actually does 2.b in the above server.rs steps, at `WebSocketClient::read()`. It also:

1. Has a `read` function where it loops through its internal TCP stream, parsing http data. If the response it receives is an upgrade acceptance (i.e. an upgrade from standard HTTP to websockets), then sets its EventSet to writable (as it can now write) and ends the read.

This file also houses a basic HTTP parser, that uses http_muncher under the hood.