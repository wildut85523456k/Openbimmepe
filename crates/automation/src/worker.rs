//! Python expressions run out of process. The interpreter path is a host setting,
//! never supplied by graph JSON. Process isolation is not an OS security sandbox.
use serde::{Deserialize, Serialize};
use std::{io::{Read, Write}, path::PathBuf, process::{Child, Command, Stdio}, sync::{atomic::{AtomicBool, Ordering}, mpsc}, thread, time::{Duration, Instant}};

const WORKER: &str = include_str!("../python/worker.py");
const LIMIT: usize = 1_048_576;
#[derive(Clone, Debug)]
pub struct PythonWorker { pub executable: PathBuf, pub timeout: Duration }
#[derive(Serialize)]
struct Request<'a> { version: u32, expression: &'a str, values: &'a [f64] }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Response { version: u32, values: Option<Vec<f64>>, error: Option<String> }
struct Running(Child);
impl Drop for Running {
    fn drop(&mut self) { let _ = self.0.kill(); let _ = self.0.wait(); }
}
impl PythonWorker {
    pub fn evaluate(&self, expression: &str, values: &[f64], cancel: &AtomicBool) -> Result<Vec<f64>, String> {
        if expression.len() > 4096 || values.len() > 10_000 || values.iter().any(|x| !x.is_finite()) {
            return Err("Python input limit or finite-number validation failed".into());
        }
        if cancel.load(Ordering::Relaxed) { return Err("Cancelled".into()); }
        let request = serde_json::to_vec(&Request { version:1,expression,values }).map_err(|e|e.to_string())?;
        let mut command = Command::new(&self.executable);
        command.args(["-I","-S","-c",WORKER]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
        #[cfg(windows)] {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        let mut child = Running(command.spawn().map_err(|e|format!("Cannot start Python: {e}"))?);
        let mut stdin = child.0.stdin.take().ok_or("Python stdin unavailable")?;
        // Write on another thread so a non-reading executable cannot block the deadline.
        let (write_tx,write_rx) = mpsc::channel();
        thread::spawn(move || { let result=stdin.write_all(&request).map_err(|e|e.to_string()); drop(stdin); let _=write_tx.send(result); });
        let stdout = child.0.stdout.take().ok_or("Python stdout unavailable")?;
        let (tx,rx) = mpsc::channel();
        thread::spawn(move || {
            let mut data=Vec::new();
            let result=stdout.take((LIMIT+1) as u64).read_to_end(&mut data).map(|_|data).map_err(|e|e.to_string());
            let _=tx.send(result);
        });
        let deadline=Instant::now()+self.timeout;
        let mut output=None;
        let mut written=false;
        loop {
            if cancel.load(Ordering::Relaxed) { return Err("Cancelled".into()); }
            if Instant::now() >= deadline { return Err("Python execution timed out".into()); }
            if !written {
                match write_rx.try_recv() {
                    Ok(result) => { result?; written=true; }
                    Err(mpsc::TryRecvError::Disconnected) => return Err("Python input writer failed".into()),
                    Err(mpsc::TryRecvError::Empty) => {}
                }
            }
            if output.is_none() {
                match rx.try_recv() {
                    Ok(result) => {
                        let bytes=result?;
                        if bytes.len()>LIMIT { return Err("Python output limit exceeded".into()); }
                        output=Some(bytes);
                    }
                    Err(mpsc::TryRecvError::Disconnected) => return Err("Python output reader failed".into()),
                    Err(mpsc::TryRecvError::Empty) => {}
                }
            }
            if let Some(status)=child.0.try_wait().map_err(|e|e.to_string())? {
                if !status.success() { return Err(format!("Python exited with {status}")); }
                if written {
                    if let Some(bytes)=output.take() {
                        let response:Response=serde_json::from_slice(&bytes).map_err(|e|format!("Invalid Python response: {e}"))?;
                        if response.version!=1 { return Err("Unsupported Python protocol version".into()); }
                        if let Some(error)=response.error { return Err(error); }
                        let result=response.values.ok_or("Python response missing values")?;
                        if result.len()!=values.len() || result.iter().any(|v|!v.is_finite()) {
                            return Err("Invalid Python output cardinality or values".into());
                        }
                        return Ok(result);
                    }
                }
            }
            thread::sleep(Duration::from_millis(5));
        }
    }
}
