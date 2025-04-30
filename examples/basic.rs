//! The most basic usage

use libunftp::ServerBuilder;
use unftp_sbe_iso::Storage;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let addr = "127.0.0.1:2121";

    let server = ServerBuilder::new(Box::new(move || Storage::new("examples/my.iso")))
        .greeting("Welcome to my ISO over FTP")
        .passive_ports(50000..=65535)
        .build()
        .unwrap();

    println!("Starting FTP server on {}", addr);
    server.listen(addr).await.unwrap();
}
