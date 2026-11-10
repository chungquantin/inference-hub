//! Prints the OpenAPI document to stdout. Used by `pnpm gen:api`.

fn main() {
    let doc = api::openapi()
        .to_pretty_json()
        .expect("OpenAPI document serializes");
    println!("{doc}");
}
