use eframe::egui;
use openbim_automation::{Error, Graph, Node, NodeKind, Plan, Property, Type};
use openbim_automation::{protocol::{PreviewRequest,Snapshot}, worker::PythonWorker};
use openbim_document::Document;
use std::{sync::{Arc, atomic::{AtomicBool,Ordering}, mpsc::{self,Receiver}}, time::Duration};

struct Job { receiver: Receiver<Result<Plan,Error>>, cancel: Arc<AtomicBool> }
impl Drop for Job {
    fn drop(&mut self) { self.cancel.store(true,Ordering::Relaxed); }
}

pub struct Automation {
    graph: Graph, plan: Option<Plan>, pending: Option<(u64,u16)>,
    pub player: bool, json: String, show_json: bool, status: String, history: Vec<String>,
    job: Option<Job>, python_enabled: bool, python_executable: String,
    runner_json: Option<String>,
    pub show_mcp: bool, mcp_executable:String, mcp_snapshot_path:String, snapshot_json:Option<String>,
}
impl Default for Automation {
    fn default() -> Self {
        Self { graph: Graph::example(vec![1]), plan: None, pending: None, player: false,
            json: String::new(), show_json: false, status: "Ready. Preview before applying changes.".into(), history: vec![],
            job:None, python_enabled:false, python_executable:if cfg!(windows) { "python" } else { "python3" }.into(),runner_json:None,
            show_mcp:false,
            mcp_executable:std::env::current_exe().ok().map(|p|p.with_file_name(if cfg!(windows) { "openbim-mcp-server.exe" } else { "openbim-mcp-server" }).to_string_lossy().into_owned()).unwrap_or_default(),
            mcp_snapshot_path:std::env::current_dir().ok().map(|p|p.join("openbim-snapshot.json").to_string_lossy().into_owned()).unwrap_or_default(),snapshot_json:None }
    }
}
impl Automation {
    pub fn export_snapshot(&mut self,document:&Document) {
        self.show_mcp=true;
        match Snapshot::capture("desktop-in-memory-sample",document).to_json() {
            Ok(json)=>self.snapshot_json=Some(json),Err(e)=>self.status=e,
        }
    }
    pub fn ui(&mut self, ui: &mut egui::Ui, document: &mut Document) -> bool {
        let mut applied = false;
        let mut dirty = false;
        if let Some(job) = &self.job {
            match job.receiver.try_recv() {
                Ok(result) => {
                    self.plan=None;
                    match result {
                        Ok(plan) => { self.status=format!("Preview: {} changes",plan.changes().len()); self.plan=Some(plan); }
                        Err(e) => self.status=format!("Preview rejected: {e:?}"),
                    }
                    self.job=None;
                }
                Err(mpsc::TryRecvError::Disconnected) => { self.status="Preview worker stopped unexpectedly".into(); self.job=None; }
                Err(mpsc::TryRecvError::Empty) => { ui.ctx().request_repaint_after(Duration::from_millis(30)); }
            }
        }
        ui.heading(if self.player { "Automation Player" } else { "OpenBIM Automation" });
        ui.label("Native typed graphs | metres and slope ratios | in-memory sample document");
        if self.show_mcp {
            ui.group(|ui| {
                ui.heading("AI Integration - MCP");
                ui.label("Read-only exported snapshot. The separate MCP process has no live model connection.");
                ui.horizontal(|ui| { ui.label("Server executable");ui.text_edit_singleline(&mut self.mcp_executable); });
                ui.horizontal(|ui| { ui.label("Saved snapshot file");ui.text_edit_singleline(&mut self.mcp_snapshot_path); });
                ui.horizontal(|ui| {
                    if ui.button("Prepare snapshot JSON").clicked() { self.export_snapshot(document); }
                    if ui.button("Copy client configuration").clicked() {
                        match openbim_automation::mcp::client_config(&self.mcp_executable,&self.mcp_snapshot_path) {
                            Ok(config)=>{ui.ctx().copy_text(config);self.status="Configuration copied. Save the snapshot at the configured path before connecting.".into();},Err(e)=>self.status=e,
                        }
                    }
                    if ui.button("Close setup").clicked() { self.show_mcp=false; }
                });
                if let Some(json)=&mut self.snapshot_json {
                    ui.label("Save this JSON to the snapshot file above. Export again and restart the server to refresh it.");
                    ui.add(egui::TextEdit::multiline(json).desired_rows(5).code_editor());
                    if ui.button("Copy snapshot JSON").clicked() { ui.ctx().copy_text(json.clone()); }
                }
            });
        }
        ui.horizontal_wrapped(|ui| {
            ui.checkbox(&mut self.player, "Player");
            if ui.button("Import JSON").clicked() { self.show_json = true; }
            if ui.button("New example").clicked() { *self = Self::default(); }
            if ui.button("Graph JSON").clicked() {
                match self.graph.to_json() {
                    Ok(json) => { self.json = json; self.show_json = true; }
                    Err(e) => self.status = format!("Export rejected: {e:?}"),
                }
            }
            if ui.add_enabled(self.job.is_none(),egui::Button::new("Preview")).clicked() {
                self.plan = None;
                let graph=self.graph.clone();
                let snapshot=document.clone();
                let python=self.python_enabled.then(||PythonWorker { executable:self.python_executable.clone().into(),timeout:Duration::from_secs(3) });
                let cancel=Arc::new(AtomicBool::new(false));
                let worker_cancel=cancel.clone();
                let (tx,receiver)=mpsc::channel();
                match std::thread::Builder::new().name("openbim-preview".into()).spawn(move || {
                    let _=tx.send(graph.preview_with(&snapshot,python.as_ref(),&worker_cancel));
                }) {
                    Ok(_) => { self.job=Some(Job { receiver,cancel }); self.status="Calculating preview.".into(); ui.ctx().request_repaint(); }
                    Err(e) => self.status=format!("Cannot start preview: {e}"),
                }
            }
            if ui.add_enabled(self.job.is_some(),egui::Button::new("Stop")).clicked() {
                self.job=None; self.plan=None; self.status="Preview cancelled; model unchanged".into();
            }
            if ui.add_enabled(self.plan.as_ref().is_some_and(|p| !p.changes().is_empty()), egui::Button::new("Apply changes")).clicked() {
                if let Some(plan) = self.plan.take() {
                    match plan.apply(document) {
                        Ok(rev) => { self.status = format!("Applied at revision {rev}. Ctrl+Z undoes the run."); applied = true; }
                        Err(e) => self.status = format!("Apply rejected: {e:?}. Preview again."),
                    }
                    self.history.push(self.status.clone());
                }
            }
            if ui.button("Discard preview").clicked() { self.job=None; self.plan = None; }
            if ui.button("Export runner request").clicked() {
                let request=PreviewRequest { version:1, request_id:format!("desktop-{}",document.revision()),
                    snapshot:Snapshot::capture("desktop-in-memory-sample",document), graph:self.graph.clone() };
                match request.to_json() { Ok(json)=>self.runner_json=Some(json), Err(e)=>self.status=e }
            }
        });
        ui.collapsing("Python worker settings",|ui| {
            dirty |= ui.checkbox(&mut self.python_enabled,"Enable numeric Python expressions").changed();
            ui.horizontal(|ui| { ui.label("Python executable:"); dirty |= ui.text_edit_singleline(&mut self.python_executable).changed(); });
            ui.label("Scalar x expressions, e.g. x + 0.3. Separate process; no model access or arbitrary scripts. Maximum 3 seconds per node.");
        });
        ui.label(&self.status);
        if let Some(json)=&mut self.runner_json {
            ui.collapsing("External runner request - save as request.json",|ui| {
                ui.add(egui::TextEdit::multiline(json).desired_rows(6).code_editor());
                if ui.button("Copy runner request").clicked() { ui.ctx().copy_text(json.clone()); }
            });
        }
        if self.show_json {
            ui.collapsing("Graph JSON - copy to a .obgraph.json file, or paste and load", |ui| {
                ui.add(egui::TextEdit::multiline(&mut self.json).desired_rows(6).code_editor());
                if ui.button("Copy JSON").clicked() { ui.ctx().copy_text(self.json.clone()); }
                if ui.button("Load JSON").clicked() {
                    match Graph::from_json(&self.json) {
                        Ok(graph) => { self.graph = graph; dirty = true; self.pending = None; }
                        Err(e) => self.status = format!("Load rejected: {e:?}"),
                    }
                }
            });
        }
        // Graph mode edits values inside blocks; Player exposes the same inputs.
        if self.player {
        for node in &mut self.graph.nodes {
            ui.horizontal(|ui| {
                ui.label(format!("Node {}", node.id));
                match &mut node.kind {
                    kind @ NodeKind::Slider {..} => {ui.push_id(node.id,|ui| {dirty|=crate::automation_slider::show(ui,kind);});}
                    NodeKind::Dimensions => { ui.label("Width / height / elevation"); },
                NodeKind::Selection { ids } => {
                        ui.label(format!("Element IDs: {ids:?}"));
                        if ui.button("Use all document elements").clicked() { *ids = document.elements().map(|e| e.id.0).collect(); dirty = true; }
                    }
                    NodeKind::Length { metres } => { dirty |= ui.add(egui::DragValue::new(metres).speed(0.01).suffix(" m")).changed(); }
                    NodeKind::Ratio { value } => { dirty |= ui.add(egui::DragValue::new(value).speed(0.001).prefix("Ratio ")).changed(); }
                    NodeKind::Python { expression,quantity } => {
                        dirty |= ui.text_edit_singleline(expression).changed();
                        let before=*quantity;
                        egui::ComboBox::from_id_salt(("python-type",node.id)).selected_text(format!("{quantity:?}")).show_ui(ui,|ui| {
                            ui.selectable_value(quantity,Type::Length,"Length (m)");
                            ui.selectable_value(quantity,Type::Ratio,"Ratio");
                        });
                        dirty |= before != *quantity;
                    }
                    NodeKind::Read { property } | NodeKind::Set { property } => {
                        let before = *property;
                        egui::ComboBox::from_id_salt(("property", node.id)).selected_text(format!("{property:?}")).show_ui(ui, |ui| {
                            for p in [Property::Width, Property::Height, Property::Elevation, Property::Slope] {
                                ui.selectable_value(property, p, format!("{p:?}"));
                            }
                        });
                        dirty |= before != *property;
                    }
                }
            });
        }
        }
        if !self.player {
            ui.horizontal_wrapped(|ui| {
                ui.label("Add:");
                for (label, kind) in [
                    ("Selection", NodeKind::Selection { ids: vec![] }),
                    ("Dimensions", NodeKind::Dimensions),
                    ("Read", NodeKind::Read { property: Property::Elevation }),
                    ("Length", NodeKind::Length { metres: 1. }),
                    ("Ratio", NodeKind::Ratio { value: 0.01 }),
                    ("Length slider",NodeKind::Slider {value:1.0,min:0.0,max:10.0,step:0.01,quantity:Type::Length}),
                    ("Number slider",NodeKind::Slider {value:0.5,min:0.0,max:1.0,step:0.01,quantity:Type::Ratio}),
                    ("Python", NodeKind::Python { expression:"x + 0.3".into(),quantity:Type::Length }),
                    ("Set", NodeKind::Set { property: Property::Elevation }),
                ] {
                    if ui.button(label).clicked() {
                        let id = self.graph.nodes.iter().map(|n| n.id).max().unwrap_or(0).saturating_add(1);
                        self.graph.nodes.push(Node { id, kind, position: [40., 40.] }); dirty = true;
                    }
                }
                if ui.button("Disconnect all").clicked() { self.graph.wires.clear(); dirty = true; }
            });
            dirty |= crate::automation_canvas::show(ui, &mut self.graph, &mut self.pending, &mut self.status);
        }
        if dirty { self.job=None; self.plan = None; }
        if let Some(plan) = &self.plan {
            ui.collapsing("Proposed changes",|ui| { for c in plan.changes() { ui.label(format!("Element {} | {:?}: {} -> {}",c.element,c.property,c.before,c.after)); } });
            ui.collapsing("Watch outputs",|ui| { for ((id,port),value) in plan.port_values() { ui.label(format!("Node {id}, output {port}: {value:?}")); } });
        }
        ui.collapsing("Run history (session)",|ui| { for entry in &self.history { ui.label(entry); } });
        applied
    }
}
