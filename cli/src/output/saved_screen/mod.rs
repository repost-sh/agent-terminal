use serde_json::Value;

pub(crate) fn saved_rows(saved: &str) -> Option<Vec<String>> {
    match serde_json::from_str::<Value>(saved) {
        Ok(json) => saved_json_rows(&json),
        Err(_) => saved_text_rows(saved),
    }
}

fn saved_json_rows(json: &Value) -> Option<Vec<String>> {
    let first = json.get("first_row").and_then(Value::as_u64).map_or(Ok(1), usize::try_from).ok()?;
    let lines = json.get("lines")?.as_array()?;
    let mut rows = vec![String::new(); first.saturating_sub(1)];
    rows.extend(lines.iter().map(|line| line.as_str().map(str::to_string)).collect::<Option<Vec<_>>>()?);
    Some(rows)
}

fn saved_text_rows(saved: &str) -> Option<Vec<String>> {
    let mut rows = Vec::new();
    let mut numbered = false;
    for (label, text) in saved.lines().filter_map(|line| line.split_once('│')) {
        numbered = true;
        if let Ok(row @ 1..) = label.trim().parse::<usize>() {
            rows.resize(rows.len().max(row), String::new());
            if let Some(slot) = rows.get_mut(row - 1) {
                text.clone_into(slot);
            }
        }
    }
    numbered.then_some(rows)
}

#[cfg(test)]
mod tests;
