//! `basinsync-oracle`: JSON-lines driver for the basinsync codecs and key
//! layout (ORACLE-INTERFACE.md). One request per input line, one response per
//! output line, flushed; never exits before end of input.

use basinsync_codec::{
    bigint, diffuse16, envbuf, flexbuffers, ids, keys, lexidecimal, BigInt,
};
use serde_json::{json, Map, Value};
use std::io::{self, BufRead, Write};

type R = Result<Value, String>;

fn field<'a>(req: &'a Map<String, Value>, name: &str) -> Result<&'a Value, String> {
    req.get(name).ok_or_else(|| format!("missing field {name:?}"))
}

/// Integer from a decimal string (canonical) or a JSON integer number.
fn int_field(req: &Map<String, Value>, name: &str) -> Result<BigInt, String> {
    match field(req, name)? {
        Value::String(s) => bigint::parse_decimal(s).map_err(|e| e.to_string()),
        Value::Number(n) => bigint::parse_decimal(&n.to_string())
            .map_err(|_| format!("field {name:?} must be an integer")),
        _ => Err(format!("field {name:?} must be a decimal string")),
    }
}

fn small_int(req: &Map<String, Value>, name: &str, max: u64) -> Result<u64, String> {
    let n = int_field(req, name)?;
    match u64::try_from(&n) {
        Ok(v) if v <= max => Ok(v),
        _ => Err(format!("field {name:?} out of range")),
    }
}

fn str_field<'a>(req: &'a Map<String, Value>, name: &str) -> Result<&'a str, String> {
    field(req, name)?.as_str().ok_or_else(|| format!("field {name:?} must be a string"))
}

fn bytes_field(req: &Map<String, Value>, name: &str) -> Result<Vec<u8>, String> {
    let arr = field(req, name)?.as_array().ok_or_else(|| format!("field {name:?} must be a byte array"))?;
    arr.iter()
        .map(|v| match v.as_u64() {
            Some(b) if b <= 255 => Ok(b as u8),
            _ => Err(format!("field {name:?} must contain numbers 0-255")),
        })
        .collect()
}

fn bytes_value(b: &[u8]) -> Value {
    Value::Array(b.iter().map(|&x| Value::from(x)).collect())
}

fn dec(n: &BigInt) -> Value {
    Value::String(bigint::to_decimal(n))
}

fn handle(req: &Map<String, Value>) -> Option<R> {
    let op = match req.get("op").and_then(Value::as_str) {
        Some(op) => op,
        None => return Some(Err("missing string field \"op\"".into())),
    };
    let e = |e: basinsync_codec::CodecError| e.to_string();
    Some((|| -> R {
        match op {
            "lexidecimal.encode" => Ok(Value::String(lexidecimal::encode(&int_field(req, "value")?))),
            "lexidecimal.decode" => Ok(dec(&lexidecimal::decode(str_field(req, "text")?).map_err(e)?)),
            "lexidecimal.sizePrefix" => {
                let size = small_int(req, "size", u64::MAX)?;
                let negative = field(req, "negative")?
                    .as_bool()
                    .ok_or_else(|| "field \"negative\" must be a boolean".to_string())?;
                Ok(Value::String(lexidecimal::size_prefix(size, negative)))
            }
            "diffuse16.encode" => Ok(Value::String(diffuse16::encode(&int_field(req, "value")?))),
            "diffuse16.decode" => Ok(dec(&diffuse16::decode(str_field(req, "text")?).map_err(e)?)),
            "envbuf.encode" => {
                let format = small_int(req, "format", 255)? as u8;
                let ap = small_int(req, "alignmentPower", 255)? as u8;
                let content = bytes_field(req, "content")?;
                Ok(bytes_value(&envbuf::encode(format, ap, &content).map_err(e)?))
            }
            "envbuf.decode" => {
                let wire = bytes_field(req, "wire")?;
                let env = envbuf::decode_one(&wire).map_err(e)?;
                Ok(json!({"format": env.format, "content": bytes_value(&wire[env.message])}))
            }
            "flexbuffers.encodeExtension" => {
                let present: Vec<&str> = ["bigint", "wellKnownSymbol", "globalSymbol"]
                    .into_iter()
                    .filter(|k| req.contains_key(*k))
                    .collect();
                if present.len() != 1 {
                    return Err("exactly one of bigint, wellKnownSymbol, globalSymbol is required".into());
                }
                let v = match present[0] {
                    "bigint" => flexbuffers::Value::BigInt(int_field(req, "bigint")?),
                    "wellKnownSymbol" => {
                        let name = str_field(req, "wellKnownSymbol")?;
                        let name = name.strip_prefix("Symbol.").unwrap_or(name);
                        flexbuffers::Value::WellKnownSymbol(name.to_string())
                    }
                    _ => flexbuffers::Value::GlobalSymbol(str_field(req, "globalSymbol")?.to_string()),
                };
                Ok(bytes_value(&flexbuffers::encode_extension(&v).map_err(e)?))
            }
            "flexbuffers.decodeExtension" => {
                let wire = bytes_field(req, "wire")?;
                Ok(match flexbuffers::decode_extension(&wire).map_err(e)? {
                    flexbuffers::Value::BigInt(n) => json!({"bigint": bigint::to_decimal(&n)}),
                    flexbuffers::Value::WellKnownSymbol(s) => json!({"wellKnownSymbol": s}),
                    flexbuffers::Value::GlobalSymbol(s) => json!({"globalSymbol": s}),
                    _ => return Err("not an extension".into()),
                })
            }
            "timeline.key" => Ok(Value::String(keys::timeline_key(&int_field(req, "milestone")?).map_err(e)?)),
            "timeline.milestone" => {
                Ok(dec(&keys::milestone_from_timeline_key(str_field(req, "key")?).map_err(e)?))
            }
            "flow.normalize" => Ok(Value::String(ids::normalize_flow_id(str_field(req, "id")?).map_err(e)?)),
            "keys.prefixes" => Ok(json!({
                "timeline": keys::TIMELINE_PREFIX,
                "basinEpilogue": keys::BASIN_EPILOGUE_PREFIX,
                "analogue": keys::ANALOGUE_PREFIX,
                "kernelHeap": keys::KERNEL_HEAP_PREFIX,
                "userHeap": keys::USER_HEAP_PREFIX,
                "userFlows": keys::USER_FLOWS_PREFIX,
                "emptySentinel": keys::EMPTY_SENTINEL,
            })),
            _ => Err("\0unsupported".into()),
        }
    })())
}

fn respond(line: &str) -> Value {
    let req: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(err) => return json!({"error": format!("malformed JSON request: {err}")}),
    };
    let obj = match req.as_object() {
        Some(o) => o,
        None => return json!({"error": "request must be a JSON object"}),
    };
    match handle(obj) {
        Some(Ok(v)) => json!({"ok": v}),
        Some(Err(m)) if m == "\0unsupported" => json!({"unsupported": true}),
        Some(Err(m)) => json!({"error": m}),
        None => json!({"unsupported": true}),
    }
}

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let mut input = stdin.lock();
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match input.read_until(b'\n', &mut buf) {
            Ok(0) => break,
            Ok(_) => {}
            Err(_) => break,
        }
        let line = String::from_utf8_lossy(&buf);
        let line = line.trim_end_matches(['\n', '\r']);
        if line.trim().is_empty() {
            continue;
        }
        let resp = std::panic::catch_unwind(|| respond(line))
            .unwrap_or_else(|_| json!({"error": "internal error"}));
        if writeln!(out, "{resp}").and_then(|_| out.flush()).is_err() {
            break;
        }
    }
}
