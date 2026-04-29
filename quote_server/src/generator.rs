use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use common::StockQuote;

const INITIAL_PRICE_MIN: f64 = 50.0;
const INITIAL_PRICE_MAX: f64 = 1000.0;
const MAX_PRICE_CHANGE_PCT: f64 = 0.005;

pub struct QuoteGenerator {
  prices: HashMap<String, f64>,
  rng: StdRng,
}

impl QuoteGenerator {
  pub fn from_file(path: impl AsRef<Path>) -> Result<Self, std::io::Error> {
    let mut rng = StdRng::from_entropy();

    let content = fs::read_to_string(path)?;

    let prices: HashMap<String, f64> = content
      .lines()
      .map(str::trim)
      .filter(|line| !line.is_empty())
      .map(|ticker| {
        let price = rng.gen_range(INITIAL_PRICE_MIN..INITIAL_PRICE_MAX);
        (ticker.to_uppercase(), price)
      })
      .collect();

    Ok(QuoteGenerator {
      prices,
      rng,
    })
  }

  pub fn generate_quote(&mut self, ticker: &str) -> Option<StockQuote> {
    let price = self.prices.get_mut(ticker)?;

    let change_pct = self.rng.gen_range(-MAX_PRICE_CHANGE_PCT..=MAX_PRICE_CHANGE_PCT);

    *price *= 1.0 + change_pct;
    *price = price.max(1.0);

    let current_price = *price;

    let volume = match ticker {
      "AAPL" | "MSFT" | "NVDA" | "TSLA" | "GOOGL" => {
        1000 + self.rng.gen_range(0..5000)
      }
      _ => 100 + self.rng.gen_range(0..1000),
    };

    let timestamp = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap()
      .as_millis() as u64;

    Some(StockQuote {
      ticker: ticker.to_string(),
      price: current_price,
      volume,
      timestamp,
    })
  }

  pub fn tickers(&self) -> Vec<&str> {
    self.prices.keys().map(String::as_str).collect()
  }
}
