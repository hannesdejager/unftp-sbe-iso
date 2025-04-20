# unftp-sbe-iso

[![Crate Version](https://img.shields.io/crates/v/unftp-sbe-iso.svg)](https://crates.io/crates/unftp-sbe-iso)
[![API Docs](https://docs.rs/unftp-sbe-iso/badge.svg)](https://docs.rs/unftp-sbe-iso)
[![Crate License](https://img.shields.io/crates/l/unftp-sbe-iso.svg)](https://crates.io/crates/unftp-sbe-iso)
[![Follow on Telegram](https://img.shields.io/badge/Follow%20on-Telegram-brightgreen.svg)](https://t.me/unftp)  


A [libunftp](https://github.com/bolcom/libunftp) back-end that exposes the contents of ISO 9660 files — such as CD-ROM and DVD images — over FTP or FTPS.

This crate allows FTP clients to connect and browse ISO images as if they were regular FTP file systems. Files can be downloaded, but modification operations (upload, delete, rename, etc.) are intentionally disabled for read-only access.

The ISO files supported conform to the **ISO 9660** standard, including common extensions such as **Joliet** (for Unicode file names) and **Rock Ridge** (for POSIX-like metadata), where supported by the underlying [`cdfs`](https://crates.io/crates/cdfs) crate.

📚 See the [documentation](https://docs.rs/unftp-sbe-iso) for usage and examples.

## Features

- 📀 **Read-only FTP access to ISO files**  
- ✅ Supports **ISO 9660** format — the industry-standard file system for CD-ROM media  
- 🔤 Optional support for **Joliet** extensions (Windows-style Unicode filenames)  
- 🐧 Optional support for **Rock Ridge** extensions (UNIX-style metadata and longer filenames)  
- 🔐 Works over both **FTP and FTPS** via libunftp  

🔒 Note: This backend is read-only by design. Operations such as upload, delete, or rename are not permitted.

## Usage

Add the `libunftp`, `unftp-sbe-iso` and `tokio` crates to your project's dependencies in `Cargo.toml`:

```toml
[dependencies]
libunftp = "0.20.3"
unftp-sbe-iso = "0.1"
tokio = { version = "1", features = ["full"] }
```

Then, configure it in your libunftp server:

```rust
use libunftp::ServerBuilder;
use unftp_sbe_iso::Storage;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let addr = "127.0.0.1:2121";

    let server = ServerBuilder::new(Box::new(move || Storage::new("/path/to/your/image.iso")))
        .greeting("Welcome to my ISO over FTP")
        .passive_ports(50000..65535)
        .build()
        .unwrap();

    println!("Starting FTP server on {}", addr);
    server.listen(addr).await.unwrap();
}
```
## License

Licensed under the [Apache License, Version 2.0](./LICENSE).
