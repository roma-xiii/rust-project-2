// Крейт common содержит всё, что используется и сервером, и клиентом:
// типы данных, константы протокола, вспомогательные функции парсинга.

use serde::{Deserialize, Serialize};

// --- Константы протокола ---

/// TCP-порт, на котором сервер слушает входящие команды.
pub const SERVER_TCP_PORT: u16 = 7777;

/// Как часто генератор выдаёт новые котировки (в миллисекундах).
pub const QUOTE_INTERVAL_MS: u64 = 500;

/// Как часто клиент отправляет PING (в секундах).
pub const PING_INTERVAL_SECS: u64 = 2;

/// Через сколько секунд без PING сервер отключает клиента.
pub const PING_TIMEOUT_SECS: u64 = 5;

/// Максимальный размер UDP-пакета, который мы читаем.
pub const UDP_BUF_SIZE: usize = 4096;

/// Как долго поток стриминга ждёт котировку из канала за одну итерацию цикла.
/// Влияет на частоту проверки PING и нагрузку на CPU.
pub const STREAM_POLL_MS: u64 = 100;

// --- Типы данных ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockQuote {
  pub ticker: String,
  pub price: f64,
  pub volume: u32,
  pub timestamp: u64,
}

impl StockQuote {
  pub fn to_json(&self) -> Result<String, serde_json::Error> {
    serde_json::to_string(self)
  }

  pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
    serde_json::from_slice(bytes).ok()
  }
}

#[derive(Debug)]
pub struct StreamCommand {
  /// Адрес клиентского UDP-сокета, куда слать данные: "127.0.0.1:34254"
  pub udp_addr: String,
  /// Список тикеров, на которые подписался клиент
  pub tickers: Vec<String>,
}

impl StreamCommand {
  pub fn parse(line: &str) -> Option<Self> {
    let parts: Vec<&str> = line.trim().splitn(3, ' ').collect();

    if parts.len() != 3 || parts[0] != "STREAM" {
      return None;
    }

    let udp_url = parts[1];
    let tickers_str = parts[2];

    let udp_addr = udp_url.strip_prefix("udp://")?.to_string();

    let tickers = tickers_str
      .split(',')
      .map(str::trim)
      .filter(|s| !s.is_empty())
      .map(str::to_uppercase)
      .collect();

    Some(StreamCommand { udp_addr, tickers })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_stock_quote_serialization_roundtrip() {
    // Проверяем что to_json → from_bytes сохраняет все поля без изменений.
    let original = StockQuote {
      ticker: "AAPL".to_string(),
      price: 183.42,
      volume: 3821,
      timestamp: 1713500000000,
    };

    let json = original.to_json().expect("сериализация не должна падать");
    let restored = StockQuote::from_bytes(json.as_bytes()).expect("десериализация не должна падать");

    assert_eq!(original.ticker, restored.ticker);
    assert_eq!(original.price, restored.price);
    assert_eq!(original.volume, restored.volume);
    assert_eq!(original.timestamp, restored.timestamp);
  }

  #[test]
  fn test_stock_quote_from_bytes_invalid_json() {
    // Кривые байты должны давать None, не панику.
    assert!(StockQuote::from_bytes(b"not json at all").is_none());
    assert!(StockQuote::from_bytes(b"").is_none());
    assert!(StockQuote::from_bytes(b"{}").is_none());
  }

  #[test]
  fn test_stream_command_parse_valid() {
    let cmd = StreamCommand::parse("STREAM udp://127.0.0.1:34254 AAPL,GOOGL,TSLA")
      .expect("команда валидна");

    assert_eq!(cmd.udp_addr, "127.0.0.1:34254");
    assert_eq!(cmd.tickers, vec!["AAPL", "GOOGL", "TSLA"]);
  }

  #[test]
  fn test_stream_command_parse_normalizes_case() {
    // Тикеры должны быть в верхнем регистре независимо от входа.
    let cmd = StreamCommand::parse("STREAM udp://127.0.0.1:34254 aapl,tsla").unwrap();
    assert_eq!(cmd.tickers, vec!["AAPL", "TSLA"]);
  }

  #[test]
  fn test_stream_command_parse_trims_whitespace() {
    // Пробелы вокруг тикеров не должны ломать парсинг.
    let cmd = StreamCommand::parse("STREAM udp://127.0.0.1:34254  AAPL , TSLA ").unwrap();
    assert_eq!(cmd.tickers, vec!["AAPL", "TSLA"]);
  }

  #[test]
  fn test_stream_command_parse_with_newline() {
    // Команда иногда приходит с \n в конце.
    let cmd = StreamCommand::parse("STREAM udp://127.0.0.1:34254 AAPL\n").unwrap();
    assert_eq!(cmd.udp_addr, "127.0.0.1:34254");
    assert_eq!(cmd.tickers, vec!["AAPL"]);
  }

  #[test]
  fn test_stream_command_parse_invalid() {
    // Все эти команды должны возвращать None.
    assert!(StreamCommand::parse("").is_none());
    assert!(StreamCommand::parse("STREAM").is_none());
    assert!(StreamCommand::parse("CONNECT udp://127.0.0.1:34254 AAPL").is_none());
    assert!(StreamCommand::parse("STREAM 127.0.0.1:34254 AAPL").is_none()); // нет udp://
  }
}
