use std::io::{self, BufRead, Read, Write};
use std::net::TcpStream;
use std::thread;

fn main() -> io::Result<()> {
    let stream = match TcpStream::connect("127.0.0.1:8080") {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to connect to server: {}", e);
            return Ok(());
        }
    };

    println!("Connected to server. Use /exit to quit");

    // Thread for reading messages from server
    let mut read_stream = stream.try_clone()?;
    let reader_thread = thread::spawn(move || {
        let mut buffer = [0; 1024];
        loop {
            match read_stream.read(&mut buffer) {
                Ok(0) => {
                    println!("Server disconnected");
                    break;
                }
                Ok(n) => {
                    print!("{}", String::from_utf8_lossy(&buffer[..n]));
                    io::stdout().flush().unwrap();
                }
                Err(e) => {
                    eprintln!("Read error: {}", e);
                    break;
                }
            }
        }
    });

    // Main thread for sending messages
    let mut writer_stream = stream;

    let stdin = io::stdin();
    let mut input = String::new();

    loop {
        input.clear();
        print!("> ");
        io::stdout().flush().unwrap();

        stdin.read_line(&mut input)?;
        let input = input.trim();

        if input == "/exit" {
            let _ = writer_stream.write_all(b"/exit\n");
            break;
        }

        if !input.is_empty() {
            let msg = format!("{}\n", input);
            if writer_stream.write_all(msg.as_bytes()).is_err() {
                eprintln!("Error sending message");
                break;
            }
        }
    }

    // Wait for reader thread to finish
    let _ = reader_thread.join();

    println!("Goodbye!");
    Ok(())
}