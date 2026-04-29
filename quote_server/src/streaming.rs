use std::io::ErrorKind;
use std::net::UdpSocket;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crossbeam_channel::Receiver;

use common::{StockQuote, PING_TIMEOUT_SECS, STREAM_POLL_MS, UDP_BUF_SIZE};


pub struct StreamSession {
  pub socket: UdpSocket,
  pub client_udp_addr: String,
  pub tickers: Vec<String>,
  pub quote_rx: Receiver<StockQuote>,
}

pub fn run_stream_session(session: StreamSession) {
  let StreamSession {
    socket,
    client_udp_addr,
    tickers,
    quote_rx,
  } = session;

  if let Err(e) = socket.set_nonblocking(true) {
    log::error!("Не удалось перевести сокет в неблокирующий режим: {e}");
    return;
  }

  let mut last_ping_ms = current_time_ms();
  let mut buf = [0u8; UDP_BUF_SIZE];

  log::info!(
    "Стриминг запущен для {} (тикеры: {})",
    client_udp_addr,
    tickers.join(", ")
  );

  loop {
    // --- Шаг 1: Проверяем входящие PING ---
    match socket.recv_from(&mut buf) {
      Ok((len, addr)) => {
        if &buf[..len] == b"PING" {
          last_ping_ms = current_time_ms();
          log::debug!("PING от {addr}");
        }
      }
      Err(e) if e.kind() == ErrorKind::WouldBlock => {
        // Нет данных — это нормально, ничего не делаем и идём дальше.
      }
      Err(e) => {
        log::warn!("Ошибка чтения UDP: {e}");
      }
    }

    // --- Шаг 2: Проверяем таймаут PING ---
    let silence_secs = (current_time_ms() - last_ping_ms) / 1000;
    if silence_secs >= PING_TIMEOUT_SECS {
      log::info!(
        "Клиент {client_udp_addr} не отвечает {silence_secs}с — отключаем"
      );
      break;
    }

    // --- Шаг 3: Получаем котировку из канала и шлём клиенту ---
    match quote_rx.recv_timeout(Duration::from_millis(STREAM_POLL_MS)) {
      Ok(quote) => {
        // Фильтруем: клиент подписан не на все тикеры.
        if tickers.contains(&quote.ticker) {
          send_quote(&socket, &quote, &client_udp_addr);
        }
      }

      // Timeout — генератор просто ещё не выдал котировку. Продолжаем цикл.
      Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}

      // Disconnected — канал закрыт, сервер завершает работу.
      Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
        log::info!("Канал котировок закрыт, завершаем стриминг для {client_udp_addr}");
        break;
      }
    }
  }

  log::info!("Поток стриминга для {client_udp_addr} завершён");
}

// Сериализует котировку в JSON и отправляет по UDP.
fn send_quote(socket: &UdpSocket, quote: &StockQuote, addr: &str) {
  match quote.to_json() {
    Ok(json) => {
      if let Err(e) = socket.send_to(json.as_bytes(), addr) {
        log::warn!("Ошибка отправки UDP на {addr}: {e}");
      }
    }
    Err(e) => {
      log::warn!("Ошибка сериализации котировки: {e}");
    }
  }
}

// Возвращает текущее время в миллисекундах
fn current_time_ms() -> u64 {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap()
    .as_millis() as u64
}
