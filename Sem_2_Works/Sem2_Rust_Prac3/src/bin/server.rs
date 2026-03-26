use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;
use std::fs;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct FileRequest {
    filename: String,
    content: String,
}

#[derive(Serialize)]
struct FileResponse {
    filename: String,
    lines: usize,
    words: usize,
    chars: usize,
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    println!("Сервер запущен на 127.0.0.1:8080");

    loop {
        let (mut socket, addr) = listener.accept().await?;
        println!("Клиент подключился: {}", addr);

        tokio::spawn(async move {
            let mut buffer: Vec<u8> = Vec::new();

            if let Err(e) = socket.read_to_end(&mut buffer).await {
                eprintln!("Ошибка чтения: {}", e);
                return;
            }

            let request: FileRequest = match serde_json::from_slice(&buffer) {
                Ok(req) => req,
                Err(e) => {
                    eprintln!("Ошибка JSON: {}", e);
                    return;
                }
            };

            let unique_name = format!("{}_{}", Uuid::new_v4(), request.filename);

            if let Err(e) = fs::write(&unique_name, &request.content) {
                eprintln!("Ошибка записи файла: {}", e);
                return;
            }

            let lines = request.content.lines().count();
            let words = request.content.split_whitespace().count();
            let chars = request.content.chars().count();

            let response = FileResponse {
                filename: unique_name.clone(),
                lines,
                words,
                chars,
            };

            let log = format!(
                "Файл: {}\nСтрок: {}\nСлов: {}\nСимволов: {}\n\n",
                response.filename, lines, words, chars
            );

            let _ = fs::write("analysis_result.txt", log);

            let json = serde_json::to_vec(&response).unwrap();

            let _ = socket.write_all(&json).await;
        });
    }
}