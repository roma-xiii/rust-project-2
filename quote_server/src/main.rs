mod generator;
mod streaming;

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crossbeam_channel::Sender;

use common::{
  StreamCommand, StockQuote,
  QUOTE_INTERVAL_MS, SERVER_TCP_PORT,
};
use generator::QuoteGenerator;
use streaming::{StreamSession, run_stream_session};

fn main() {
  env_logger::init();

  let mut generator = QuoteGenerator::from_file("tickers.txt")
    .expect("Не удалось загрузить тикеры из tickers.txt");

  let tickers: Vec<String> = generator.tickers().iter().map(|s| s.to_string()).collect();
  let client_senders: Arc<Mutex<Vec<Sender<StockQuote>>>> = Arc::new(Mutex::new(Vec::new()));
  let senders_for_generator = Arc::clone(&client_senders);

  thread::spawn(move || {
    log::info!("Генератор запущен, тикеров: {}", tickers.len());
    loop {
      for ticker in &tickers {
        if let Some(quote) = generator.generate_quote(ticker) {
          let mut senders = senders_for_generator.lock().unwrap();

          senders.retain(|tx| tx.send(quote.clone()).is_ok());
        }
      }

      thread::sleep(Duration::from_millis(QUOTE_INTERVAL_MS));
    }
  });

  // Запускаем TCP-сервер.
  let addr = format!("0.0.0.0:{SERVER_TCP_PORT}");
  let listener = TcpListener::bind(&addr).expect("Не удалось запустить TCP-сервер");
  log::info!("Сервер слушает {addr}");

  for incoming in listener.incoming() {
    match incoming {
      Ok(stream) => {
        let senders = Arc::clone(&client_senders);
        thread::spawn(move || handle_client(stream, senders));
      }
      Err(e) => log::warn!("Ошибка входящего соединения: {e}"),
    }
  }
}

// Обрабатывает одного TCP-клиента: читает команду, запускает стриминг.
fn handle_client(stream: TcpStream, client_senders: Arc<Mutex<Vec<Sender<StockQuote>>>>) {
  let peer = stream.peer_addr().map(|a| a.to_string()).unwrap_or_else(|_| "unknown".into());
  log::info!("Подключился клиент: {peer}");

  let mut reader = BufReader::new(&stream);
  let mut line = String::new();

  if let Err(e) = reader.read_line(&mut line) {
    log::warn!("Ошибка чтения команды от {peer}: {e}");
    return;
  }

  // Парсим команду STREAM.
  let cmd = match StreamCommand::parse(&line) {
    Some(cmd) => cmd,
    None => {
      send_tcp_response(&stream, &format!("ERR неверный формат: {}", line.trim()));
      return;
    }
  };

  log::info!("Команда от {peer}: → UDP {} тикеры: {:?}", cmd.udp_addr, cmd.tickers);

  let udp_socket = match UdpSocket::bind("0.0.0.0:0") {
    Ok(s) => s,
    Err(e) => {
      send_tcp_response(&stream, &format!("ERR не удалось создать UDP-сокет: {e}"));
      return;
    }
  };

  let (tx, rx) = crossbeam_channel::unbounded::<StockQuote>();

  client_senders.lock().unwrap().push(tx);

  send_tcp_response(&stream, "OK");

  // Запускаем поток стриминга для этого клиента.
  let session = StreamSession {
    socket: udp_socket,
    client_udp_addr: cmd.udp_addr,
    tickers: cmd.tickers,
    quote_rx: rx,
  };
  thread::spawn(|| run_stream_session(session));
}

fn send_tcp_response(mut stream: &TcpStream, msg: &str) {
  if let Err(e) = writeln!(stream, "{msg}") {
    log::warn!("Ошибка отправки ответа: {e}");
  }
}
