# External runner and Python expressions

This increment adds a standalone Rust preview executable and a Python expression
worker. The graph editor still lives in the OpenBIM desktop. A separate graphical
editor and authenticated live IPC are subsequent work.

## Build and test

```powershell
cargo test -p openbim-automation -p openbim-document -p openbim-desktop
cargo build -p openbim-automation --bin openbim-automation
cargo run -p openbim-desktop
```

Manage > Automation > Graph Editor opens the workspace. Preview now executes on a
background thread. Stop cancels its result and terminates the active Python child
process; it cannot leave model edits because preview never mutates the host.
Apply remains one host transaction after a fresh preview. Changing graph inputs
or Python settings discards the old preview and cancels any active job.

## External preview protocol

Export runner request copies a graph plus immutable document snapshot into the
JSON text area. Copy JSON and save it as a request file. A version-1 request has
request_id, snapshot.document_id, snapshot.revision, snapshot.elements and graph.
The temporary Rust document is reconstructed using host validation. Its private
revision does not replace the source revision carried in the response.

```powershell
Get-Content -Raw crates/automation/examples/preview-request.json |
    .\target\debug\openbim-automation.exe
```

The response includes source identity/revision, typed node outputs and proposed
changes. Error responses return JSON with an error field and exit code 1. The
runner accepts one request per process, up to 16 MiB. It does not open live model
files, listen on a socket or apply external change lists. Live apply will require
host-side revalidation and its own transaction; a report is never an apply token.

## Python worker

Install Python 3.9+ separately, enable numeric expressions in Python worker
settings, and set its interpreter executable. The graph itself cannot choose an
executable or alter the timeout. Add a Python node with quantity type Length or
Ratio. Connect Read property -> Python -> Set property; the Set element input
still comes from Selection. Example: `x + 0.3`, where x is each input value in
metres. Expressions preserve their declared quantity type; dimensional inference
inside expressions is not implemented.

```powershell
Get-Content -Raw crates/automation/examples/python-request.json |
    .\target\debug\openbim-automation.exe --python 'C:\Path\To\python.exe'
python -m unittest discover -s crates/automation/python -p test_worker.py
$env:OPENBIM_TEST_PYTHON='C:\Path\To\python.exe'
cargo test -p openbim-automation --test python -- --ignored
```

The child receives numeric inputs only, through stdin. It returns JSON on stdout.
It is launched with -I -S, without a visible console on Windows, and is killed and
reaped on timeout, cancellation, malformed output or errors. The deadline is
three seconds per Python node; expressions are limited to 4 KiB, 256 AST nodes,
10,000 numeric values and 1 MiB response output. Non-finite numbers and changed
output cardinality are rejected. These are execution limits, not an OS memory
sandbox. Only a host-configured trusted Python interpreter should be used.

This first worker interprets a restricted Python expression AST without eval or
exec. It supports x, numeric constants, + - * / % **, unary signs and approved
numeric functions: abs, min, max, round, sqrt, sin and cos. It does not expose
imports, files, network, arbitrary scripts, package installation or model memory.
Full Python scripting and packages need a separate permissions/runtime design.

## Verification

The desktop compile check and automation/desktop Cargo tests now pass with the
real JSON dependencies (34 tests passed). The optional real-Python interpreter
test remains skipped. Python subprocess behavior and visual desktop acceptance
still require the steps above; see automation-verification.md.
