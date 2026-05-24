use ntdll::process::KernelSocket;
use ntdll::thread::ThreadSocket;
use protocol::{WineReply, WineRequest};

fn main() {
    // Connect to the server in order to test it
    let socket = KernelSocket::new("DESKTOP-CJ7R8").expect("failed to connect to Wine server");
    assert_eq!(ThreadSocket::send(&WineRequest::Ping), WineReply::Pong);
}
