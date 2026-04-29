use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;

use common::SERVER_TCP_PORT;

pub fn send_stream_command(
  server_addr: &str,
  udp_port: u16,
  tickers: &[String],
) -> Result<(), String> {
  let tcp_addr = format!("{server_addr}:{SERVER_TCP_PORT}");

  let stream =
    TcpStream::connect(&tcp_addr).map_err(|e| format!("Не удалось подключиться к {tcp_addr}: {e}"))?;

  let command = format!(
    "STREAM udp://127.0.0.1:{udp_port} {}\n",
    tickers.join(",")
  );

  log::info!("Отправляем команду: {}", command.trim());

  (&stream)
    .write_all(command.as_bytes())
    .map_err(|e| format!("Ошибка отправки команды: {e}"))?;

  let mut reader = BufReader::new(&stream);
  let mut response = String::new();

  reader
    .read_line(&mut response)
    .map_err(|e| format!("Ошибка чтения ответа: {e}"))?;

  let response = response.trim();
  log::info!("Ответ сервера: {response}");

  if response == "OK" {
    Ok(())
  } else if response.starts_with("ERR") {
    Err(format!("Сервер отклонил команду: {response}"))
  } else {
    Err(format!("Неожиданный ответ сервера: {response}"))
  }
}
