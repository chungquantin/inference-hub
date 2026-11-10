//! The OpenAPI document must be self-contained: the TypeScript client generator
//! fails on a `$ref` to a schema that is not in `components`.

#[test]
fn every_schema_reference_resolves() {
    let doc = serde_json::to_value(api::openapi()).unwrap();
    let schemas = doc["components"]["schemas"]
        .as_object()
        .expect("components.schemas");

    let mut missing = Vec::new();
    let mut stack = vec![&doc];
    while let Some(value) = stack.pop() {
        match value {
            serde_json::Value::Object(map) => {
                if let Some(serde_json::Value::String(target)) = map.get("$ref")
                    && let Some(name) = target.strip_prefix("#/components/schemas/")
                    && !schemas.contains_key(name)
                {
                    missing.push(name.to_owned());
                }
                stack.extend(map.values());
            }
            serde_json::Value::Array(items) => stack.extend(items),
            _ => {}
        }
    }
    missing.sort();
    missing.dedup();
    assert!(
        missing.is_empty(),
        "unresolved schema references: {missing:?}"
    );
}
