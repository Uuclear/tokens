use serde_json::Value;

const MAX_DEPTH: usize = 12;

/// Best-effort model name from heterogeneous JSON blobs.
pub fn extract_model(v: &Value) -> Option<String> {
    extract_model_shallow(v).or_else(|| extract_model_deep(v, 0))
}

fn extract_model_shallow(v: &Value) -> Option<String> {
    const PATHS: &[&str] = &[
        "/model",
        "/modelId",
        "/modelName",
        "/modelID",
        "/activeModel",
        "/modelDetails/modelName",
        "/modelDetails/name",
        "/composerState/model",
        "/composerState/modelConfig/model",
        "/tokenCount/model",
        "/payload/model",
        "/payload/collaboration_mode/settings/model",
    ];
    for p in PATHS {
        if let Some(s) = v.pointer(p).and_then(value_as_model_str) {
            return Some(s);
        }
    }
    if let Some(obj) = v.as_object() {
        for key in [
            "model",
            "modelId",
            "modelName",
            "modelID",
            "activeModel",
        ] {
            if let Some(s) = obj.get(key).and_then(value_as_model_str) {
                return Some(s);
            }
        }
    }
    None
}

fn extract_model_deep(v: &Value, depth: usize) -> Option<String> {
    if depth > MAX_DEPTH {
        return None;
    }
    match v {
        Value::Object(map) => {
            for key in [
                "model",
                "modelId",
                "modelName",
                "modelID",
                "activeModel",
                "selectedModel",
                "defaultModel",
                "aiModel",
            ] {
                if let Some(s) = map.get(key).and_then(value_as_model_str) {
                    return Some(s);
                }
            }
            for val in map.values() {
                if let Some(s) = extract_model_deep(val, depth + 1) {
                    return Some(s);
                }
            }
        }
        Value::Array(arr) => {
            for val in arr {
                if let Some(s) = extract_model_deep(val, depth + 1) {
                    return Some(s);
                }
            }
        }
        _ => {}
    }
    None
}

fn value_as_model_str(v: &Value) -> Option<String> {
    v.as_str()
        .map(String::from)
        .or_else(|| v.get("name").and_then(|n| n.as_str()).map(String::from))
}
