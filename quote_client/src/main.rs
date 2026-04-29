mod connection;
mod receiver;

use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use clap::Parser;

use connection::send_stream_command;
use receiver::run_receiver;

#[derive(Parser)]
#[command(name = "client", about = "Клиент котировок")]
struct Args {
  /// Адрес TCP-сервера (без порта)
  #[arg(long, default_value = "127.0.0.1")]
  server_addr: String,

  /// UDP-порт для приёма котировок
  #[arg(long, default_value_t = 34254)]
  udp_port: u16,

  /// Путь к файлу со списком тикеров
  #[arg(long, default_value = "tickers.txt")]
  tickers_file: String,
}

fn main() {
  env_logger::init();

  let args = Args::parse();

  let tickers = match read_tickers(&args.tickers_file) {
    Ok(t) => t,
    Err(e) => {
      eprintln!("Ошибка чтения файла тикеров: {e}");
      std::process::exit(1);
    }
  };

  if tickers.is_empty() {
    eprintln!("Файл тикеров пуст: {}", args.tickers_file);
    std::process::exit(1);
  }

  log::info!("Загружено тикеров: {} ({:?})", tickers.len(), tickers);

  let running = Arc::new(AtomicBool::new(true));

  let running_for_ctrlc = Arc::clone(&running);
  ctrlc::set_handler(move || {
    log::info!("Получен сигнал остановки (Ctrl+C)");
    running_for_ctrlc.store(false, Ordering::Relaxed);
  })
  .expect("Не удалось установить обработчик Ctrl+C");

  if let Err(e) = send_stream_command(&args.server_addr, args.udp_port, &tickers) {
    eprintln!("Ошибка подключения к серверу: {e}");
    std::process::exit(1);
  }

  println!("Подключено. Получаем котировки (Ctrl+C для выхода)...\n");

  if let Err(e) = run_receiver(args.udp_port, running) {
    eprintln!("Ошибка приёма котировок: {e}");
    std::process::exit(1);
  }

  println!("\nКлиент завершён.");
}

fn read_tickers(path: &str) -> Result<Vec<String>, std::io::Error> {
  let content = fs::read_to_string(path)?;

  let tickers = content
    .lines()
    .map(str::trim)
    .filter(|line| !line.is_empty())
    .map(str::to_uppercase)
    .map(String::from)
    .collect();

  Ok(tickers)
}
