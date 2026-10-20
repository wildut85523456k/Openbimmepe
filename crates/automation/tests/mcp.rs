use openbim_automation::{mcp::{self,McpServer},mcp_model::SnapshotService,protocol::Snapshot};
use serde_json::{json,Value};
use std::io::Cursor;
fn server()->McpServer {
    let snapshot:Snapshot=serde_json::from_str(include_str!("../examples/mcp-snapshot.json")).unwrap();
    McpServer::new(SnapshotService::new(snapshot).unwrap())
}
fn initialize(s:&mut McpServer) {
    let response=s.handle(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "protocolVersion":mcp::PROTOCOL,"capabilities":{},"clientInfo":{"name":"test","version":"1"}}})).unwrap();
    assert_eq!(response["result"]["protocolVersion"],mcp::PROTOCOL);
    assert!(s.handle(json!({"jsonrpc":"2.0","method":"notifications/initialized"})).is_none());
}
fn call(s:&mut McpServer,name:&str,args:Value)->Value {
    static NEXT:std::sync::atomic::AtomicU64=std::sync::atomic::AtomicU64::new(100);
    let id=NEXT.fetch_add(1,std::sync::atomic::Ordering::Relaxed);
    s.handle(json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":args}})).unwrap()
}
#[test]
fn lifecycle_and_protocol_errors() {
    let mut s=server();
    assert_eq!(s.handle(json!({"jsonrpc":"2.0","id":0,"method":"tools/list"})).unwrap()["error"]["code"],-32000);
    assert_eq!(s.handle(json!({"jsonrpc":"2.0","id":null,"method":"ping"})).unwrap()["error"]["code"],-32600);
    initialize(&mut s);
    assert!(s.handle(json!({"jsonrpc":"2.0","method":"tools/call","params":{"name":"openbim_get_capabilities"}})).is_none());
    assert_eq!(call(&mut s,"openbim_apply_changes",json!({}))["error"]["code"],-32602);
}
#[test]
fn tool_schemas_and_queries() {
    let mut s=server();initialize(&mut s);
    let list=s.handle(json!({"jsonrpc":"2.0","id":2,"method":"tools/list"})).unwrap();
    let tools=list["result"]["tools"].as_array().unwrap();assert_eq!(tools.len(),8);
    for tool in tools { assert_eq!(tool["annotations"]["readOnlyHint"],true);assert_eq!(tool["inputSchema"]["type"],"object"); }
    let page=call(&mut s,"openbim_query_elements",json!({"document_id":"mcp-example","category":"Duct","limit":1}));
    assert_eq!(page["result"]["structuredContent"]["items"][0]["id"],1);
    assert_eq!(page["result"]["structuredContent"]["total_count"],2);
    assert_eq!(page["result"]["structuredContent"]["next_offset"],1);
    let bad=call(&mut s,"openbim_query_elements",json!({"document_id":"mcp-example","limit":101}));
    assert_eq!(bad["result"]["isError"],true);
}
#[test]
fn native_preview_does_not_write_model() {
    let mut s=server();initialize(&mut s);
    let graph=call(&mut s,"openbim_build_property_graph",json!({"document_id":"mcp-example","element_ids":[1,4],"property":"Elevation","value":3.}))["result"]["structuredContent"]["graph"].clone();
    let preview=call(&mut s,"openbim_preview_graph",json!({"document_id":"mcp-example","expected_revision":42,"graph":graph}));
    assert_eq!(preview["result"]["structuredContent"]["total_count"],2);
    assert_eq!(preview["result"]["structuredContent"]["applied"],false);
    let element=call(&mut s,"openbim_get_element",json!({"document_id":"mcp-example","element_id":1}));
    assert_eq!(element["result"]["structuredContent"]["element"]["elevation_m"],2.7);
    let bad=call(&mut s,"openbim_preview_graph",json!({"document_id":"mcp-example","expected_revision":41,"graph":graph}));
    assert_eq!(bad["result"]["isError"],true);
}
#[test]
fn stdio_framing_parse_errors_and_size_limit() {
    let mut s=server();let mut out=Vec::new();
    mcp::serve(Cursor::new(b"invalid\n{\"jsonrpc\":\"2.0\",\"id\":7,\"method\":\"ping\"}\n"),&mut out,&mut s).unwrap();
    let lines:Vec<Value>=String::from_utf8(out).unwrap().lines().map(|l|serde_json::from_str(l).unwrap()).collect();
    assert_eq!(lines.len(),2);assert_eq!(lines[0]["error"]["code"],-32700);assert_eq!(lines[1]["id"],7);
    assert!(mcp::serve(Cursor::new(vec![b' ';mcp::MESSAGE_LIMIT+1]),Vec::new(),&mut server()).is_err());
}
#[test]
fn client_config_requires_absolute_paths() {
    assert!(mcp::client_config("relative-server","snapshot.json").is_err());
    let root=std::env::temp_dir();
    let config=mcp::client_config(&root.join("server").to_string_lossy(),&root.join("snapshot.json").to_string_lossy()).unwrap();
    let config:Value=serde_json::from_str(&config).unwrap();
    assert_eq!(config["mcpServers"]["openbim"]["args"][0],"--snapshot");
}
