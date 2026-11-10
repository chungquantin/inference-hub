//! `cargo run -p ih-ingest --example parse_file -- <file> <base-url>`: parses a saved feed.
fn main() {
    let mut args = std::env::args().skip(1);
    let (Some(path), Some(base)) = (args.next(), args.next()) else {
        eprintln!("usage: parse_file <file> <base-url>");
        std::process::exit(2);
    };
    let body = std::fs::read(&path).expect("readable file");
    match ih_ingest::feed::parse(&body, &base) {
        Ok(feed) => {
            println!(
                "{:?}: {} entries, {} skipped",
                feed.title,
                feed.entries.len(),
                feed.skipped
            );
            for e in feed.entries.iter().take(3) {
                println!("- {} | {}", e.title, e.url);
            }
        }
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}
