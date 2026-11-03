use serde_json::Value;

static NULL: Value = Value::Null;

pub trait Json {
    fn at(&self, key: &str) -> &Value;
    fn text(&self, key: &str) -> &str;
    fn id(&self, key: &str) -> String;
    fn num(&self, key: &str) -> u64;
    fn flag(&self, key: &str) -> bool;
    fn list(&self, key: &str) -> &[Value];
    fn unwrap(&self, key: &str) -> &Value;
}

impl Json for Value {
    fn at(&self, key: &str) -> &Value {
        if key.starts_with('/') { self.pointer(key).unwrap_or(&NULL) } else { self.get(key).unwrap_or(&NULL) }
    }

    fn text(&self, key: &str) -> &str {
        self.at(key).as_str().unwrap_or_default()
    }

    fn id(&self, key: &str) -> String {
        match self.at(key) {
            Value::String(text) => text.clone(),
            Value::Number(number) => number.to_string(),
            _ => String::new(),
        }
    }

    fn num(&self, key: &str) -> u64 {
        match self.at(key) {
            Value::Number(number) => number.as_u64().unwrap_or_default(),
            Value::String(text) => text.parse().unwrap_or_default(),
            _ => 0,
        }
    }

    fn flag(&self, key: &str) -> bool {
        match self.at(key) {
            Value::Bool(value) => *value,
            Value::String(text) => text == "true",
            _ => false,
        }
    }

    fn list(&self, key: &str) -> &[Value] {
        self.at(key).as_array().map(Vec::as_slice).unwrap_or_default()
    }

    fn unwrap(&self, key: &str) -> &Value {
        self.get(key).filter(|inner| inner.is_object()).unwrap_or(self)
    }
}
