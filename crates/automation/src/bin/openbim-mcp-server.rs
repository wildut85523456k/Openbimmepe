use openbim_automation::{mcp::{self,McpServer},mcp_model::SnapshotService,protocol::Snapshot};
use std::{fs::File,io::{self,Read}};
fn run()->Result<(),String> {
    let mut args=std::env::args().skip(1);
    if args.next().as_deref()!=Some("--snapshot") { return Err("Usage: openbim-mcp-server --snapshot exported-snapshot.json".into()); }
    let path=args.next().ok_or("--snapshot requires a local file path")?;
    if args.next().is_some() { return Err("Unexpected arguments".into()); }
    let mut bytes=Vec::new();
    File::open(path).map_err(|e|format!("Cannot read configured snapshot: {e}"))?.take(16_777_217).read_to_end(&mut bytes).map_err(|e|e.to_string())?;
    if bytes.len()>16_777_216 { return Err("Snapshot exceeds 16 MiB".into()); }
    let snapshot:Snapshot=serde_json::from_slice(&bytes).map_err(|e|format!("Invalid snapshot JSON: {e}"))?;
    let mut server=McpServer::new(SnapshotService::new(snapshot)?);
    mcp::serve(io::stdin().lock(),io::stdout().lock(),&mut server).map_err(|e|e.to_string())
}
fn main() {
    if let Err(error)=run() { eprintln!("OpenBIM MCP: {error}");std::process::exit(1); }
}
