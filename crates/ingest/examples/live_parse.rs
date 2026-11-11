//! Fetches every market adapter once and prints what it would store, without
//! a database. `cargo run -p ih-ingest --example live_parse [adapter ...]`
use ih_ingest::adapters::{Endpoints, MarketAdapter, market_adapters};

#[tokio::main]
async fn main() {
    let only: Vec<String> = std::env::args().skip(1).collect();
    let client = ih_ingest::http_client().unwrap();
    let adapters = market_adapters(&Endpoints::default());
    let mut handles = Vec::new();
    for a in adapters {
        let id = a.info().id();
        if !only.is_empty() && !only.iter().any(|o| o == id) {
            continue;
        }
        let client = client.clone();
        handles.push(tokio::spawn(async move {
            let started = std::time::Instant::now();
            let line = match &a {
                MarketAdapter::Gateway(g) => match g.fetch_models(&client).await {
                    Ok(l) => {
                        let s = l
                            .iter()
                            .take(2)
                            .map(|m| {
                                format!(
                                    "{} ${}/${}",
                                    m.external_id, m.input_per_mtok, m.output_per_mtok
                                )
                            })
                            .collect::<Vec<_>>()
                            .join(" | ");
                        format!("ok {:>5} models  {s}", l.len())
                    }
                    Err(e) => format!("ERR {e}"),
                },
                MarketAdapter::NeoCloud(g) => match g.fetch_gpus(&client).await {
                    Ok(l) => {
                        let s = l
                            .iter()
                            .take(3)
                            .map(|m| {
                                format!("{}x{} ${}", m.gpu_count, m.gpu_type, m.price_per_hour)
                            })
                            .collect::<Vec<_>>()
                            .join(" | ");
                        format!("ok {:>5} gpus    {s}", l.len())
                    }
                    Err(e) => format!("ERR {e}"),
                },
            };
            println!("{id:<14} {:>5}ms {line}", started.elapsed().as_millis());
        }));
    }
    for h in handles {
        let _ = h.await;
    }
}
