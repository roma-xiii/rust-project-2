use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use common::{StockQuote, PING_INTERVAL_SECS, UDP_BUF_SIZE};

pub fn run_receiver(udp_port: u16, running: Arc<AtomicBool>) -> Result<(), String> {
  let socket = UdpSocket::bind(format!("0.0.0.0:{udp_port}"))
    .map_err(|e| format!("Не удалось открыть UDP-порт {udp_port}: {e}"))?;

  socket
    .set_read_timeout(Some(Duration::from_millis(500)))
    .map_err(|e| format!("Ошибка настройки таймаута: {e}"))?;

  log::info!("Слушаем UDP на порту {udp_port}...");

  let mut buf = [0u8; UDP_BUF_SIZE];

  let server_udp_addr = loop {
    if !running.load(Ordering::Relaxed) {
      return Ok(());
    }

    match socket.recv_from(&mut buf) {
      Ok((len, addr)) => {
        if let Some(quote) = StockQuote::from_bytes(&buf[..len]) {
          print_quote(&quote);
        }
        break addr; // выходим из loop с адресом сервера
      }
      Err(e) if is_timeout(&e) => continue, // таймаут — пробуем снова
      Err(e) => return Err(format!("Ошибка ожидания первого пакета: {e}")),
    }
  };

  log::info!("Сервер найден: {server_udp_addr}. Запускаем PING.");

  let ping_socket = socket
    .try_clone()
    .map_err(|e| format!("Не удалось клонировать сокет: {e}"))?;

  let running_for_ping = Arc::clone(&running);

  thread::spawn(move || run_ping(ping_socket, server_udp_addr, running_for_ping));

  while running.load(Ordering::Relaxed) {
    match socket.recv_from(&mut buf) {
      Ok((len, _addr)) => {
        if let Some(quote) = StockQuote::from_bytes(&buf[..len]) {
          print_quote(&quote);
        }
      }
      Err(e) if is_timeout(&e) => continue, // таймаут — проверяем running и читаем снова
      Err(e) => {
        log::warn!("Ошибка чтения UDP: {e}");
      }
    }
  }

  log::info!("Приём котировок остановлен");
  Ok(())
}

fn run_ping(socket: UdpSocket, server_addr: SocketAddr, running: Arc<AtomicBool>) {
  log::info!("PING-поток запущен → {server_addr}");

  while running.load(Ordering::Relaxed) {
    if let Err(e) = socket.send_to(b"PING", server_addr) {
      log::warn!("Ошибка отправки PING: {e}");
    } else {
      log::debug!("PING → {server_addr}");
    }

    let steps = PING_INTERVAL_SECS * 10;
    for _ in 0..steps {
      if !running.load(Ordering::Relaxed) {
        break;
      }
      thread::sleep(Duration::from_millis(100));
    }
  }

  log::info!("PING-поток завершён");
}

fn print_quote(quote: &StockQuote) {
  println!(
    "[{}]  {:>8.2}  vol: {}",
    quote.ticker, quote.price, quote.volume
  );
}

fn is_timeout(e: &std::io::Error) -> bool {
  use std::io::ErrorKind;
  matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut)
}
