use serde_json::{json, Value};
use std::io::Write;
use std::process::{Command, Stdio};

fn run(lines: &[&str]) -> Vec<Value> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_basinsync-oracle"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut stdin = child.stdin.take().unwrap();
        for l in lines {
            writeln!(stdin, "{l}").unwrap();
        }
    }
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    String::from_utf8(out.stdout).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect()
}

#[test]
fn requests_in_order() {
    let r = run(&[
        r#"{"op":"lexidecimal.encode","value":"18446744073709551616"}"#,
        r#"{"op":"lexidecimal.decode","text":"0yuef"}"#,
        r#"{"op":"lexidecimal.decode","text":"0zg00"}"#,
        r#"{"op":"lexidecimal.sizePrefix","size":256,"negative":false}"#,
        r#"{"op":"diffuse16.encode","value":"27"}"#,
        r#"{"op":"diffuse16.decode","text":"0w0y"}"#,
        r#"not json"#,
        r#"{"op":"envbuf.encode","format":5,"alignmentPower":3,"content":[1,2,3]}"#,
        r#"{"op":"envbuf.decode","wire":[101,110,86,163,3,0,0,0,1,2,3,238,238,238,238,238,3,0,0,0,163,118,78,69]}"#,
        r#"{"op":"flexbuffers.encodeExtension","wellKnownSymbol":"species"}"#,
        r#"{"op":"flexbuffers.decodeExtension","wire":[2,120,0,2,48,1]}"#,
        r#"{"op":"flexbuffers.encodeExtension","bigint":"-1"}"#,
        r#"{"op":"timeline.key","milestone":"1"}"#,
        r#"{"op":"timeline.milestone","key":"kernelspace/basin/timeline/0yve"}"#,
        r#"{"op":"timeline.key","milestone":"0"}"#,
        r#"{"op":"flow.normalize","id":"My-Flow"}"#,
        r#"{"op":"flow.normalize","id":"a/b"}"#,
        r#"{"op":"keys.prefixes"}"#,
        r#"{"op":"no.such.op"}"#,
        r#"{"op":"lexidecimal.encode","value":"01"}"#,
    ]);
    assert_eq!(r.len(), 20);
    assert_eq!(r[0], json!({"ok": "0zwhg10000000000000000"}));
    assert_eq!(r[1], json!({"ok": "-17"}));
    assert!(r[2].get("error").is_some());
    assert_eq!(r[3], json!({"ok": "xghgg"}));
    assert_eq!(r[4], json!({"ok": "0wb1z"}));
    assert_eq!(r[5], json!({"ok": "-16"}));
    assert!(r[6].get("error").is_some());
    assert_eq!(r[7]["ok"], json!([101,110,86,163,3,0,0,0,1,2,3,238,238,238,238,238,3,0,0,0,163,118,78,69]));
    assert_eq!(r[8], json!({"ok": {"format": 5, "content": [1,2,3]}}));
    assert_eq!(r[9], json!({"ok": [8, 35, 115, 112, 101, 99, 105, 101, 115, 8, 48, 1]}));
    assert_eq!(r[10], json!({"ok": {"bigint": "0"}}));
    assert_eq!(r[11], json!({"ok": [2, 126, 0, 2, 48, 1]}));
    assert_eq!(r[12], json!({"ok": "kernelspace/basin/timeline/0yve"}));
    assert_eq!(r[13], json!({"ok": "1"}));
    assert!(r[14].get("error").is_some());
    assert_eq!(r[15], json!({"ok": "my-flow"}));
    assert!(r[16].get("error").is_some());
    assert_eq!(r[17]["ok"]["emptySentinel"], json!("kernelspace/sentinel/empty"));
    assert_eq!(r[17]["ok"]["userFlows"], json!("userspace/flows/"));
    assert_eq!(r[18], json!({"unsupported": true}));
    assert!(r[19].get("error").is_some());
}
