//! Preview-only external automation runner. Never applies a remote change list.
use openbim_automation::{protocol::PreviewRequest, worker::PythonWorker};
use std::{io::{self, Read}, path::PathBuf, sync::atomic::AtomicBool, time::Duration};
fn run() -> Result<(),String> {
    let mut args=std::env::args().skip(1);
    let python=match args.next().as_deref() {
        None => None,
        Some("--python") => Some(PythonWorker {
            executable:PathBuf::from(args.next().ok_or("--python requires an executable path")?),
            timeout:Duration::from_secs(3),
        }),
        _ => return Err("Usage: openbim-automation [--python EXECUTABLE] < request.json".into()),
    };
    if args.next().is_some() { return Err("Unexpected arguments".into()); }
    let mut bytes=Vec::new();
    io::stdin().take(16_777_217).read_to_end(&mut bytes).map_err(|e|e.to_string())?;
    if bytes.len()>16_777_216 { return Err("Request exceeds 16 MiB".into()); }
    let request:PreviewRequest=serde_json::from_slice(&bytes).map_err(|e|e.to_string())?;
    let response=request.evaluate(python.as_ref(),&AtomicBool::new(false))?;
    println!("{}",serde_json::to_string(&response).map_err(|e|e.to_string())?);
    Ok(())
}
fn main() {
    if let Err(error)=run() {
        println!("{}",serde_json::json!({"version":1,"error":error}));
        std::process::exit(1);
    }
}
