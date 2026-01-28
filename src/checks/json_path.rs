use serde_json::Value;

/// Very small JSONPath subset used across checks.
///
/// Supported:
/// - `$` (root)
/// - `$.a.b[0].c`
/// - `.a.b` (treated as `$.a.b`)
///
/// Not supported:
/// - filters, wildcards, slices, recursive descent, etc.
pub fn lookup<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    let p = path.trim();
    if p.is_empty() || p == "$" {
        return Some(root);
    }

    // Accept "$" or "$.foo.bar[0]" ... small subset.
    let mut cursor = root;
    let mut s = p;
    if s.starts_with("$.") {
        s = &s[2..];
    } else if s.starts_with('$') {
        s = &s[1..];
        if s.starts_with('.') {
            s = &s[1..];
        }
    } else if s.starts_with('.') {
        s = &s[1..];
    }

    for seg in s.split('.') {
        if seg.is_empty() {
            continue;
        }

        // seg may be "a", "a[0]", "[0]" (rare)
        let mut rest = seg;

        // key part
        if !rest.starts_with('[') {
            let key_end = rest.find('[').unwrap_or(rest.len());
            let key = &rest[..key_end];
            cursor = match cursor {
                Value::Object(map) => map.get(key)?,
                _ => return None,
            };
            rest = &rest[key_end..];
        }

        // zero or more [idx] parts
        while rest.starts_with('[') {
            let close = rest.find(']')?;
            let idx_str = &rest[1..close];
            let idx: usize = idx_str.parse().ok()?;
            cursor = match cursor {
                Value::Array(arr) => arr.get(idx)?,
                _ => return None,
            };
            rest = &rest[close + 1..];
        }

        if !rest.is_empty() {
            // Unsupported token (e.g. filters)
            return None;
        }
    }

    Some(cursor)
}
