use serde_json::Value;

pub fn get(language: &str, key: &str) -> String {
    lookup(catalog(language), key)
        .or_else(|| lookup(catalog("en"), key))
        .unwrap_or_else(|| key.to_string())
}

fn catalog(language: &str) -> Value {
    let file = if language == "bg" {
        include_str!("../../src/locales/bg.json")
    } else {
        include_str!("../../src/locales/en.json")
    };
    serde_json::from_str(file).unwrap_or(Value::Null)
}

fn lookup(value: Value, key: &str) -> Option<String> {
    let mut current = value;
    for part in key.split('.') {
        current = current.get(part)?.clone();
    }
    current.as_str().map(str::to_string)
}

#[cfg(test)]
fn flatten(value: &Value, prefix: &str, keys: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let next = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                flatten(child, &next, keys);
            }
        }
        Value::String(_) => keys.push(prefix.to_string()),
        _ => keys.push(prefix.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bulgarian_has_every_english_key() {
        let english: Value = serde_json::from_str(include_str!("../../src/locales/en.json")).unwrap();
        let bulgarian: Value = serde_json::from_str(include_str!("../../src/locales/bg.json")).unwrap();
        let mut english_keys = Vec::new();
        let mut bulgarian_keys = Vec::new();
        flatten(&english, "", &mut english_keys);
        flatten(&bulgarian, "", &mut bulgarian_keys);
        english_keys.sort();
        bulgarian_keys.sort();
        assert_eq!(english_keys, bulgarian_keys);
    }
}
