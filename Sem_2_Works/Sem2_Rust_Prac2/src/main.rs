use std::env;
use std::fs;
use std::sync::{Arc, Mutex};
use std::thread;

// структура для хранения результата анализа
struct FileAnalysis {
    filename: String,
    words: usize,
    chars: usize,
}

// функция анализа файла
fn analyze_file(path: &str) -> Result<FileAnalysis, std::io::Error> {
    let content = fs::read_to_string(path)?;

    let words = content.split_whitespace().count();
    let chars = content.chars().count();

    Ok(FileAnalysis {
        filename: path.to_string(),
        words,
        chars,
    })
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("Использование: cargo run <file1> <file2> ...");
        return;
    }

    let results: Arc<Mutex<Vec<FileAnalysis>>> = Arc::new(Mutex::new(Vec::new()));

    let mut handles = vec![];

    for file in &args[1..] {
        let file = file.clone();
        let results = Arc::clone(&results);

        let handle = thread::spawn(move || {
            match analyze_file(&file) {
                Ok(analysis) => {
                    let mut res = results.lock().unwrap();
                    res.push(analysis);
                }
                Err(e) => {
                    println!("Ошибка чтения {}: {}", file, e);
                }
            }
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let res = results.lock().unwrap();

    println!("Результаты анализа:\n");

    let mut total_words = 0;
    let mut total_chars = 0;

    for (i, file) in res.iter().enumerate() {
        println!(
            "{}. {}: {} слов, {} символов",
            i + 1,
            file.filename,
            file.words,
            file.chars
        );

        total_words += file.words;
        total_chars += file.chars;
    }

    println!("\nИтог: {} слов, {} символов", total_words, total_chars);
}