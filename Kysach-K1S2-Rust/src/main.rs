use Kursach::{Config, SessionManager};
use tokio::signal;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let config = match Config::from_file("config.toml") {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Ошибка загрузки конфига: {}, использую значения по умолчанию", e);
            Config::default()
        }
    };
    let mut manager = SessionManager::new(config);
    manager.set_shutdown_callback(|| {
        eprintln!("Завершение сессии из за бездействия");
        std::process::exit(0);
    });
    manager.start().await?;
    println!("Модуль работает. Нажмите Ctrl+C для остановки.");
    signal::ctrl_c().await?;
    manager.stop().await;
    Ok(())
}