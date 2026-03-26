use std::env;
use std::fs;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 3 {
        eprintln!("Использование: cargo run <путь_к_файлу> <слово>");
        process::exit(1);
    }

    let file_path = &args[1];
    let search_word = &args[2];

    // Чтение файла
    let contents = match read_file(file_path) {
        Ok(text) => text,
        Err(e) => {
            eprintln!("Ошибка чтения файла: {}", e);
            process::exit(1);
        }
    };

    let total_words = count_words(&contents);
    let matches = count_occurrences(&contents, search_word);

    println!("Общее количество слов: {}", total_words);
    println!("Количество повторений слова '{}': {}", search_word, matches);
}

// Чтение файла
fn read_file(path: &str) -> Result<String, std::io::Error> {
    fs::read_to_string(path)
}

// Подсчет всех слов
fn count_words(text: &str) -> usize {
    text.split_whitespace().count()
}

// Подсчет повторений слова
fn count_occurrences(text: &str, word: &str) -> usize {
    text.split_whitespace()
        .filter(|w| w.eq_ignore_ascii_case(word))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_occurrences() {
        let text = "rust Rust programming rust language";
        let result = count_occurrences(text, "rust");
        assert_eq!(result, 3);
    }
}