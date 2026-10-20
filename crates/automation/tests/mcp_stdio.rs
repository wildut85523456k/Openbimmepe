use std::{io::Write,process::{Command,Stdio}};
use serde_json::{json,Value};
#[test]
fn external_binary_initializes_and_returns_snapshot_context() {
    let mut process=Command::new(env!("CARGO_BIN_EXE_openbim-mcp-server"))
        .arg("--snapshot").arg(concat!(env!("CARGO_MANIFEST_DIR"),"/examples/mcp-snapshot.json"))
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    let mut input=process.stdin.take().unwrap();
    for message in [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"smoke","version":"1"}}}),
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"openbim_list_documents","arguments":{}}}),
    ] { writeln!(input,"{message}").unwrap(); }
    drop(input);
    let output=process.wait_with_output().unwrap();assert!(output.status.success(),"{}",String::from_utf8_lossy(&output.stderr));
    let lines:Vec<Value>=String::from_utf8(output.stdout).unwrap().lines().map(|l|serde_json::from_str(l).unwrap()).collect();
    assert_eq!(lines.len(),2);
    assert_eq!(lines[1]["result"]["structuredContent"]["documents"][0]["revision"],42);
}
