use tokio::net::TcpStream;
use tokio::io::{AsyncWriteExt, AsyncReadExt};
use serde::{Serialize, Deserialize};
use std::fs;

#[derive(Serialize)]
struct FileRequest {
    filename: String,
    content: String,
}

#[derive(Deserialize)]
struct FileResponse {
    filename: String,
    lines: usize,
    words: usize,
    chars: usize,
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let filepath = "file1.txt";

    let bytes = match fs::read(filepath) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Ошибка чтения файла: {}", e);
            return Ok(());
        }
    };

    let content = String::from_utf8_lossy(&bytes).to_string();

    let request = FileRequest {
        filename: filepath.to_string(),
        content,
    };

    let json = serde_json::to_vec(&request).unwrap();

    let mut stream = TcpStream::connect("127.0.0.1:8080").await?;
    println!("Подключено к серверу");

    // отправка
    stream.write_all(&json).await?;

    // ВАЖНО!
    stream.shutdown().await?;

    // получение ответа
    let mut buffer: Vec<u8> = Vec::new();
    stream.read_to_end(&mut buffer).await?;

    let response: FileResponse = serde_json::from_slice(&buffer).unwrap();

    println!("\nРезультат анализа:");
    println!("Файл: {}", response.filename);
    println!("Строк: {}", response.lines);
    println!("Слов: {}", response.words);
    println!("Символов: {}", response.chars);

    Ok(())
}