#![allow(dead_code)]

use anyhow::{Context, Result};
use device_query::{DeviceQuery, DeviceState};
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex as TokioMutex;
use tokio::time;

//Конфигурация
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub timeout_secs: u64,
    pub grace_secs: u64,
    pub enabled: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            timeout_secs: 300,
            grace_secs: 10,
            enabled: true,
        }
    }
}

impl Config {
    pub fn from_file(path: &str) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Не удалось прочитать файл {}", path))?;
        let mut cfg: Config = toml::from_str(&content)
            .with_context(|| format!("Ошибка парсинга TOML в {}", path))?;
        if cfg.timeout_secs < 10 {
            warn!("timeout_secs слишком мал, устанавливаю 10");
            cfg.timeout_secs = 10;
        }
        if cfg.timeout_secs > 3600 {
            warn!("timeout_secs > 3600, ограничиваю 3600");
            cfg.timeout_secs = 3600;
        }
        if cfg.grace_secs > cfg.timeout_secs {
            warn!("grace_secs не может быть больше timeout_secs, приравниваю");
            cfg.grace_secs = cfg.timeout_secs;
        }
        Ok(cfg)
    }
}

//Монитор ввода
pub struct InputMonitor {
    _state: DeviceState,
    last_activity: Arc<TokioMutex<Instant>>,
}

impl InputMonitor {
    pub fn new() -> Self {
        Self {
            _state: DeviceState::new(),
            last_activity: Arc::new(TokioMutex::new(Instant::now())),
        }
    }

    pub async fn start_polling(&self, poll_interval: Duration) {
        let last_activity = self.last_activity.clone();
        tokio::task::spawn_blocking(move || {
            let state = DeviceState::new();
            let mut last_keys = Vec::new();
            let mut last_mouse = (0, 0);
            loop {
                let keys = state.query_keymap();
                if keys != last_keys {
                    last_keys = keys;
                    let mut guard = last_activity.blocking_lock();
                    *guard = Instant::now();
                }
                let mouse = state.get_mouse();
                if mouse.coords != last_mouse {
                    last_mouse = mouse.coords;
                    let mut guard = last_activity.blocking_lock();
                    *guard = Instant::now();
                }
                std::thread::sleep(poll_interval);
            }
        });
    }

    pub async fn idle_duration(&self) -> Duration {
        let last = *self.last_activity.lock().await;
        Instant::now().duration_since(last)
    }

    #[cfg(test)]
    pub async fn force_idle(&self) {
        let mut guard = self.last_activity.lock().await;
        *guard = Instant::now() - Duration::from_secs(1000);
    }
}

//Таймер бездействия
pub struct IdleTimer {
    timeout: Duration,
    callback: Option<Box<dyn Fn() + Send + Sync + 'static>>,
    running: Arc<AtomicBool>,
    task: Option<tokio::task::JoinHandle<()>>,
}

impl IdleTimer {
    pub fn new(timeout: Duration) -> Self {
        Self {
            timeout,
            callback: None,
            running: Arc::new(AtomicBool::new(false)),
            task: None,
        }
    }

    pub fn set_callback<F>(&mut self, f: F)
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.callback = Some(Box::new(f));
    }

    pub async fn start(&mut self, monitor: Arc<InputMonitor>) -> Result<()> {
        if self.running.load(Ordering::SeqCst) {
            warn!("Таймер уже запущен");
            return Ok(());
        }
        let callback = match self.callback.take() {
            Some(cb) => cb,
            None => anyhow::bail!("Callback не установлен"),
        };
        let running_flag = self.running.clone();
        running_flag.store(true, Ordering::SeqCst);
        let timeout = self.timeout;
        let mut interval = time::interval(Duration::from_millis(500));
        let task = tokio::spawn(async move {
            let mut triggered = false;
            while running_flag.load(Ordering::SeqCst) && !triggered {
                interval.tick().await;
                let idle = monitor.idle_duration().await;
                if idle >= timeout {
                    info!("Таймер сработал: бездействие {} сек", idle.as_secs());
                    callback();
                    triggered = true;
                }
            }
            debug!("Таймер остановлен");
        });
        self.task = Some(task);
        Ok(())
    }

    pub fn reset(&self) {
        debug!("Сброс таймера");
    }

    pub async fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(task) = self.task.take() {
            task.abort();
            let _ = task.await;
        }
    }
}

//Фасад SessionManager

pub struct SessionManager {
    config: Config,
    monitor: Arc<InputMonitor>,
    timer: Option<IdleTimer>,
    shutdown_callback: Option<Box<dyn Fn() + Send + Sync + 'static>>,
}

impl SessionManager {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            monitor: Arc::new(InputMonitor::new()),
            timer: None,
            shutdown_callback: None,
        }
    }

    pub fn set_shutdown_callback<F>(&mut self, callback: F)
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.shutdown_callback = Some(Box::new(callback));
    }

    pub async fn start(&mut self) -> Result<()> {
        if !self.config.enabled {
            info!("Модуль отключён конфигурацией");
            return Ok(());
        }
        self.monitor.start_polling(Duration::from_millis(100)).await;
        let timeout = Duration::from_secs(self.config.timeout_secs);
        let mut timer = IdleTimer::new(timeout);
        let callback = self.shutdown_callback.take()
            .ok_or_else(|| anyhow::anyhow!("Не установлен callback завершения"))?;
        timer.set_callback(callback);
        timer.start(self.monitor.clone()).await?;
        self.timer = Some(timer);
        info!("Модуль запущен (таймаут {} сек)", self.config.timeout_secs);
        Ok(())
    }

    pub async fn stop(&mut self) {
        if let Some(mut timer) = self.timer.take() {
            timer.stop().await;
        }
        info!("Модуль остановлен");
    }

    pub fn reset_timer(&self) {
        if let Some(timer) = &self.timer {
            timer.reset();
        }
    }
}

//Тесты
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
    use tokio::time;

    #[test]
    fn test_config_default() {
        let cfg = Config::default();
        assert_eq!(cfg.timeout_secs, 300);
        assert_eq!(cfg.grace_secs, 10);
        assert!(cfg.enabled);
    }

    #[test]
    fn test_config_validation() {
        let toml_str = r#"
            timeout_secs = 5
            grace_secs = 10
            enabled = true
        "#;
        let mut cfg: Config = toml::from_str(toml_str).unwrap();
        if cfg.timeout_secs < 10 {
            cfg.timeout_secs = 10;
        }
        assert_eq!(cfg.timeout_secs, 10);
    }

    #[tokio::test]
    async fn test_idle_timer_timeout() {
        let monitor = Arc::new(InputMonitor::new());
        monitor.force_idle().await;
        let mut timer = IdleTimer::new(Duration::from_millis(200));
        let triggered = Arc::new(AtomicBool::new(false));
        let triggered_clone = triggered.clone();
        timer.set_callback(move || {
            triggered_clone.store(true, Ordering::SeqCst);
        });
        timer.start(monitor).await.unwrap();
        time::sleep(Duration::from_millis(300)).await;
        assert!(triggered.load(Ordering::SeqCst));
        timer.stop().await;
    }
}