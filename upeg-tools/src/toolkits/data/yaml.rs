use serde_json::{Map, Value};
use yaml_rust2::Yaml;
use yaml_rust2::parser::{Event, Parser};
use yaml_rust2::scanner::TScalarStyle;

const MAX_EVENTS: usize = 40_000;
const MAX_DEPTH: usize = 64;

pub(super) fn parse(input: &str) -> Result<Value, String> {
    let mut parser = Parser::new_from_str(input);
    let mut events = Vec::new();
    let mut depth = 0_usize;
    loop {
        let (event, _) = parser
            .next_token()
            .map_err(|e| format!("invalid YAML: {e}"))?;
        match &event {
            Event::SequenceStart(anchor, tag) | Event::MappingStart(anchor, tag) => {
                plain_features(*anchor, tag.is_some())?;
                depth += 1;
                // Root has depth zero; an empty collection at depth 64 is
                // valid, just like a scalar at that depth in the JSON parser.
                if depth > MAX_DEPTH + 1 {
                    return Err("YAML exceeds the 64-level nesting limit".into());
                }
            }
            Event::SequenceEnd | Event::MappingEnd => depth = depth.saturating_sub(1),
            Event::Scalar(_, _, anchor, tag) => plain_features(*anchor, tag.is_some())?,
            Event::Alias(_) => return Err("YAML aliases are unsupported".into()),
            _ => {}
        }
        let end = matches!(event, Event::StreamEnd);
        events.push(event);
        if events.len() > MAX_EVENTS {
            return Err("YAML exceeds the 40000 event limit".into());
        }
        if end {
            break;
        }
    }
    let docs = events
        .iter()
        .filter(|event| matches!(event, Event::DocumentStart))
        .count();
    if docs != 1 {
        return Err("YAML input must contain exactly one document".into());
    }
    let first = events
        .iter()
        .position(|event| matches!(event, Event::DocumentStart))
        .ok_or_else(|| "YAML input must contain one document".to_string())?;
    let mut index = first + 1;
    if matches!(events.get(index), Some(Event::DocumentEnd)) {
        return Err("empty YAML document is not a JSON-compatible value".into());
    }
    let value = node(&events, &mut index, 0)?;
    if !matches!(events.get(index), Some(Event::DocumentEnd)) {
        return Err("YAML input must contain exactly one document".into());
    }
    Ok(value)
}

fn node(events: &[Event], index: &mut usize, depth: usize) -> Result<Value, String> {
    if depth > MAX_DEPTH {
        return Err("YAML exceeds the 64-level nesting limit".into());
    }
    let event = events
        .get(*index)
        .ok_or_else(|| "incomplete YAML document".to_string())?;
    *index += 1;
    match event {
        Event::Scalar(text, style, anchor, tag) => {
            plain_features(*anchor, tag.is_some())?;
            scalar(text, *style)
        }
        Event::SequenceStart(anchor, tag) => {
            plain_features(*anchor, tag.is_some())?;
            let mut values = Vec::new();
            while !matches!(events.get(*index), Some(Event::SequenceEnd)) {
                values.push(node(events, index, depth + 1)?);
            }
            *index += 1;
            Ok(Value::Array(values))
        }
        Event::MappingStart(anchor, tag) => {
            plain_features(*anchor, tag.is_some())?;
            let mut map = Map::new();
            while !matches!(events.get(*index), Some(Event::MappingEnd)) {
                let merge_key = matches!(
                    events.get(*index),
                    Some(Event::Scalar(text, TScalarStyle::Plain, _, _)) if text == "<<"
                );
                let key = node(events, index, depth + 1)?;
                let key = key
                    .as_str()
                    .ok_or_else(|| "YAML mapping keys must be strings".to_string())?;
                if merge_key {
                    return Err("YAML merge keys are unsupported".into());
                }
                let value = node(events, index, depth + 1)?;
                if map.insert(key.to_owned(), value).is_some() {
                    return Err(format!("duplicate YAML mapping key `{key}`"));
                }
            }
            *index += 1;
            Ok(Value::Object(map))
        }
        Event::Alias(_) => Err("YAML aliases are unsupported".into()),
        _ => Err("invalid YAML node".into()),
    }
}

fn plain_features(anchor: usize, has_tag: bool) -> Result<(), String> {
    if anchor != 0 {
        return Err("YAML anchors are unsupported".into());
    }
    if has_tag {
        return Err("YAML tags are unsupported".into());
    }
    Ok(())
}

fn scalar(text: &str, style: TScalarStyle) -> Result<Value, String> {
    if style != TScalarStyle::Plain {
        return Ok(Value::String(text.to_owned()));
    }
    // yaml-rust2's scalar helper omits the two Core-schema null spellings.
    if matches!(text, "Null" | "NULL") {
        return Ok(Value::Null);
    }
    // serde_json preserves u64 values that yaml-rust2 would classify as
    // reals after its i64 parser rejects them.
    if let Ok(Value::Number(number)) = serde_json::from_str::<Value>(text) {
        return Ok(Value::Number(number));
    }
    match Yaml::from_str(text) {
        Yaml::Null => Ok(Value::Null),
        Yaml::Boolean(value) => Ok(Value::Bool(value)),
        Yaml::Integer(_) | Yaml::Real(_) => {
            Err(format!("YAML scalar `{text}` is not a finite JSON number"))
        }
        Yaml::String(value) => Ok(Value::String(value)),
        _ => Err("unsupported YAML scalar".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_plain_and_quoted_scalars() {
        assert_eq!(
            parse("a: 1\nb: '1'\nc: null\n").unwrap(),
            serde_json::json!({"a":1,"b":"1","c":null})
        );
    }
    #[test]
    fn rejects_non_json_yaml_constructs() {
        for source in [
            "a: 1\na: 2",
            "a: &x 1\nb: *x",
            "a: !custom hi",
            "a: .inf",
            "a: 1e309",
            "a: 1e9999",
            "a: 1.",
            "1: x",
            "a: 1\n---\nb: 2",
            "<<: {a: 1}",
        ] {
            assert!(parse(source).is_err(), "{source}");
        }
    }

    #[test]
    fn depth_is_rejected_before_constructing_nodes() {
        let source = format!("{}0", "[".repeat(65)) + &"]".repeat(65);
        assert!(parse(&source).unwrap_err().contains("nesting limit"));
        assert_eq!(
            parse("a: foo.inf").unwrap(),
            serde_json::json!({"a":"foo.inf"})
        );
    }
    #[test]
    fn date_like_plain_scalars_remain_strings() {
        assert_eq!(
            parse("date: 2026-10-08\nrange: 1-2\n0xfoobar: value").unwrap(),
            serde_json::json!({"date":"2026-10-08","range":"1-2","0xfoobar":"value"})
        );
    }
}
