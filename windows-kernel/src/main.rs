use windows_kernel::server::WineServer;

fn main() {
    WineServer::new("DESKTOP-CJ7R8").expect("failed to initialize server").start();
}
