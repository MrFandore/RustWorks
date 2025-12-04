use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use serde_json::{json, Value};
use bcrypt::{hash, verify, DEFAULT_COST};

type Clients = Arc<Mutex<HashMap<String, TcpStream>>>;

fn main() -> io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:8080")?;
    let clients: Clients = Arc::new(Mutex::new(HashMap::new()));

    println!("Сервер запущен на 127.0.0.1:8080");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let clients = Arc::clone(&clients);
                thread::spawn(move || {
                    handle_client(stream, clients);
                });
            }
            Err(e) => {
                eprintln!("Ошибка подключения: {}", e);
            }
        }
    }

    Ok(())
}

fn handle_client(mut stream: TcpStream, clients: Clients) {
    let mut buffer = [0; 1024];
    let mut username = String::new();

    // Приветственное сообщение
    let welcome_msg = "Добро пожаловать в чат месседжера МАХ! \n\
                      Ниже представлен список команд:\n\
                      /register <имя> <пароль>\n\
                      /login <имя> <пароль>\n\
                      /msg <сообщение>\n\
                      /whisper <имя> <сообщение>\n\
                      /exit - выход\n";
    let _ = stream.write_all(welcome_msg.as_bytes());

    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break, // Клиент отключился
            Ok(n) => {
                let msg = String::from_utf8_lossy(&buffer[..n]);
                let msg = msg.trim();

                if msg.is_empty() {
                    continue;
                }

                let parts: Vec<&str> = msg.splitn(2, ' ').collect();

                match parts[0] {
                    "/register" if parts.len() > 1 => {
                        let data: Vec<&str> = parts[1].splitn(2, ' ').collect();
                        if data.len() == 2 {
                            let (user, pass) = (data[0], data[1]);
                            if register(user, pass) {
                                let _ = stream.write_all("Регистрация успешна\n".as_bytes());
                                username = user.to_string();
                                clients.lock().unwrap().insert(user.to_string(), stream.try_clone().unwrap());
                            } else {
                                let _ = stream.write_all("Ошибка регистрации\n".as_bytes());
                            }
                        }
                    }
                    "/login" if parts.len() > 1 => {
                        let data: Vec<&str> = parts[1].splitn(2, ' ').collect();
                        if data.len() == 2 {
                            let (user, pass) = (data[0], data[1]);
                            if login(user, pass) {
                                let _ = stream.write_all("Вход выполнен\n".as_bytes());
                                username = user.to_string();
                                clients.lock().unwrap().insert(user.to_string(), stream.try_clone().unwrap());
                            } else {
                                let _ = stream.write_all("Ошибка входа\n".as_bytes());
                            }
                        }
                    }
                    "/msg" if parts.len() > 1 => {
                        let message = parts[1];
                        if !username.is_empty() {
                            broadcast(&username, message, &clients);
                            log_message(&username, message);
                        } else {
                            let _ = stream.write_all("Сначала войдите в систему\n".as_bytes());
                        }
                    }
                    "/whisper" if parts.len() > 1 => {
                        let data: Vec<&str> = parts[1].splitn(2, ' ').collect();
                        if data.len() == 2 {
                            let (target, message) = (data[0], data[1]);
                            if !username.is_empty() {
                                send_private(&username, target, message, &clients);
                            } else {
                                let _ = stream.write_all("Сначала войдите в систему\n".as_bytes());
                            }
                        }
                    }
                    "/exit" => {
                        let _ = stream.write_all("До свидания!\n".as_bytes());
                        break;
                    }
                    _ => {
                        if !username.is_empty() {
                            broadcast(&username, msg, &clients);
                            log_message(&username, msg);
                        } else {
                            let _ = stream.write_all("Пожалуйста, войдите или зарегистрируйтесь\n".as_bytes());
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Ошибка чтения: {}", e);
                break;
            }
        }
    }

    if !username.is_empty() {
        clients.lock().unwrap().remove(&username);
        println!("{} отключился", username);
    }
}

fn broadcast(sender: &str, message: &str, clients: &Clients) {
    let msg = format!("[{}]: {}\n", sender, message);
    let mut clients = clients.lock().unwrap();
    for (name, client) in clients.iter_mut() {
        if name != sender { // Не отправляем сообщение отправителю
            let _ = client.write_all(msg.as_bytes());
        }
    }
}

fn send_private(sender: &str, target: &str, message: &str, clients: &Clients) {
    let msg_to_target = format!("[ЛИЧНО от {}]: {}\n", sender, message);
    let msg_to_sender = format!("[ЛИЧНО для {}]: {}\n", target, message);

    let mut clients = clients.lock().unwrap();

    // Отправляем сообщение цели
    if let Some(client) = clients.get_mut(target) {
        let _ = client.write_all(msg_to_target.as_bytes());
    } else {
        // Сообщаем отправителю, что получатель не найден
        if let Some(sender_client) = clients.get_mut(sender) {
            let _ = sender_client.write_all(format!("Пользователь {} не найден\n", target).as_bytes());
        }
    }

    // Отправляем копию отправителю
    if let Some(sender_client) = clients.get_mut(sender) {
        let _ = sender_client.write_all(msg_to_sender.as_bytes());
    }
}

fn log_message(user: &str, msg: &str) {
    let log = format!("{}: {}\n", user, msg);
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("chat.log")
        .unwrap_or_else(|_| File::create("chat.log").unwrap());
    let _ = file.write_all(log.as_bytes());
}

fn register(user: &str, pass: &str) -> bool {
    let hashed = match hash(pass, DEFAULT_COST) {
        Ok(h) => h,
        Err(_) => return false,
    };

    let data = json!({ "username": user, "password": hashed });

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("users.json")
        .unwrap_or_else(|_| File::create("users.json").unwrap());

    writeln!(file, "{}", data).is_ok()
}

fn login(user: &str, pass: &str) -> bool {
    let file = match File::open("users.json") {
        Ok(f) => f,
        Err(_) => return false,
    };

    let reader = BufReader::new(file);
    for line in reader.lines() {
        if let Ok(line) = line {
            if let Ok(data) = serde_json::from_str::<Value>(&line) {
                if data["username"] == user {
                    if let Some(pass_hash) = data["password"].as_str() {
                        return verify(pass, pass_hash).unwrap_or(false);
                    }
                }
            }
        }
    }
    false
}