//! Generic JSON -> NBT conversion.
//!
//! Minecraft's `NbtOps` is lenient about numeric tag types (a codec expecting a
//! float happily reads any numeric tag and calls `.floatValue()`), so we don't
//! need a per-field type schema. We map:
//!
//! - bool      -> Byte(0/1)
//! - integer   -> Int (or Long if it doesn't fit i32)
//! - decimal   -> Double
//! - string    -> String
//! - array     -> List (numeric arrays are made homogeneous)
//! - object    -> Compound (null values are dropped)
//!
//! Types we can't represent (null) are skipped.

use leather_protocol::Nbt;
use serde_json::Value;

/// Converts a JSON value to NBT. Returns `None` for `null`.
pub fn json_to_nbt(value: &Value) -> Option<Nbt> {
    match value {
        Value::Null => None,
        Value::Bool(b) => Some(Nbt::Byte(i8::from(*b))),
        Value::Number(n) => Some(number_to_nbt(n)),
        Value::String(s) => Some(Nbt::String(s.clone())),
        Value::Array(items) => Some(array_to_nbt(items)),
        Value::Object(map) => {
            let entries = map
                .iter()
                .filter_map(|(k, v)| json_to_nbt(v).map(|n| (k.clone(), n)))
                .collect();
            Some(Nbt::Compound(entries))
        }
    }
}

fn number_to_nbt(n: &serde_json::Number) -> Nbt {
    if let Some(i) = n.as_i64() {
        if i32::try_from(i).is_ok() {
            Nbt::Int(i as i32)
        } else {
            Nbt::Long(i)
        }
    } else {
        // Unsigned-too-big-for-i64 or a real decimal: a Double round-trips fine
        // through the lenient codecs.
        Nbt::Double(n.as_f64().unwrap_or(0.0))
    }
}

fn is_numeric(n: &Nbt) -> bool {
    matches!(
        n,
        Nbt::Byte(_) | Nbt::Short(_) | Nbt::Int(_) | Nbt::Long(_) | Nbt::Float(_) | Nbt::Double(_)
    )
}

fn as_f64(n: &Nbt) -> f64 {
    match n {
        Nbt::Byte(v) => f64::from(*v),
        Nbt::Short(v) => f64::from(*v),
        Nbt::Int(v) => f64::from(*v),
        Nbt::Long(v) => *v as f64,
        Nbt::Float(v) => f64::from(*v),
        Nbt::Double(v) => *v,
        _ => 0.0,
    }
}

fn array_to_nbt(items: &[Value]) -> Nbt {
    let converted: Vec<Nbt> = items.iter().filter_map(json_to_nbt).collect();

    // An NBT list must be homogeneous. A JSON array like [0, 0.5] yields mixed
    // Int/Double tags; promote every element to Double in that case.
    if let Some(first) = converted.first() {
        let all_numeric = converted.iter().all(is_numeric);
        let mixed = converted.iter().any(|n| n.tag_id() != first.tag_id());
        if all_numeric && mixed {
            return Nbt::List(converted.iter().map(|n| Nbt::Double(as_f64(n))).collect());
        }
    }

    Nbt::List(converted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitives() {
        assert_eq!(json_to_nbt(&serde_json::json!(true)), Some(Nbt::Byte(1)));
        assert_eq!(json_to_nbt(&serde_json::json!(42)), Some(Nbt::Int(42)));
        assert_eq!(
            json_to_nbt(&serde_json::json!(5_000_000_000i64)),
            Some(Nbt::Long(5_000_000_000))
        );
        assert_eq!(json_to_nbt(&serde_json::json!(0.5)), Some(Nbt::Double(0.5)));
        assert_eq!(json_to_nbt(&serde_json::Value::Null), None);
    }

    #[test]
    fn object_drops_nulls_and_nests() {
        let v = serde_json::json!({"keep": 1, "skip": null, "child": {"x": 2}});
        let nbt = json_to_nbt(&v).unwrap();
        match nbt {
            Nbt::Compound(entries) => {
                assert_eq!(entries.len(), 2, "null dropped");
                assert!(entries.iter().any(|(k, _)| k == "keep"));
                assert!(entries.iter().any(|(k, _)| k == "child"));
            }
            other => panic!("expected compound, got {other:?}"),
        }
    }

    #[test]
    fn mixed_numeric_array_promoted_to_double() {
        let nbt = json_to_nbt(&serde_json::json!([0, 0.5, 1])).unwrap();
        match nbt {
            Nbt::List(items) => {
                assert!(items.iter().all(|n| matches!(n, Nbt::Double(_))));
            }
            other => panic!("expected list, got {other:?}"),
        }
    }
}
