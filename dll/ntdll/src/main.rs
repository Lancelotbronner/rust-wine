use protocol::socket::WineSocket;
use protocol::{WineReply, WineRequest};

fn main() {
    // Connect to the server in order to test it
    let mut socket = WineSocket::new("DESKTOP-CJ7R8").expect("failed to connect to server");
    assert_eq!(socket.send(&WineRequest::Ping), WineReply::Pong);
}
