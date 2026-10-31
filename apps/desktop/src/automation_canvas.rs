//! Shared node/port geometry keeps rendered sockets, hit targets and wire ends
//! aligned. Connections use horizontal-tangent cubic Bezier curves.
use eframe::egui::{self, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use openbim_automation::{Graph, Node, NodeKind, Wire};

const WIDTH: f32 = 240.0;
const HEADER: f32 = 30.0;
const ROW: f32 = 28.0;

fn node_rect(origin: Pos2, node: &Node) -> Rect {
    let rows = node.kind.inputs().len().max(node.kind.outputs().len()).max(1);
    Rect::from_min_size(origin + Vec2::from(node.position), Vec2::new(WIDTH, HEADER + ROW * rows as f32 + footer_height(&node.kind)))
}
fn footer_height(kind:&NodeKind)->f32 {if matches!(kind,NodeKind::Slider {..}) {34.0} else {56.0}}

#[derive(Clone)]
struct Marquee { start:Pos2, base:std::collections::BTreeSet<u64>, additive:bool }

fn move_selection(graph:&mut Graph,selected:&std::collections::BTreeSet<u64>,mut delta:Vec2) {
    let left=graph.nodes.iter().filter(|n|selected.contains(&n.id)).map(|n|n.position[0]).fold(f32::INFINITY,f32::min);
    let top=graph.nodes.iter().filter(|n|selected.contains(&n.id)).map(|n|n.position[1]).fold(f32::INFINITY,f32::min);
    delta.x=delta.x.max(-left);delta.y=delta.y.max(-top);
    for n in graph.nodes.iter_mut().filter(|n|selected.contains(&n.id)) {n.position[0]+=delta.x;n.position[1]+=delta.y;}
}

fn port_position(rect: Rect, input: Option<usize>, output:u16) -> Pos2 {
    egui::pos2(
        if input.is_some() { rect.left() } else { rect.right() },
        rect.top() + HEADER + ROW * (input.unwrap_or(output as usize) as f32 + 0.5),
    )
}

fn curve(start: Pos2, end: Pos2) -> egui::epaint::CubicBezierShape {
    // The control points remain to the right of the output and left of the
    // input, including when connecting backwards across the canvas.
    let gap = end.x - start.x;
    let reach = if gap > 1.0 {
        (gap * 0.45).clamp(8.0, 180.0).min(gap * 0.5)
    } else {
        (gap.abs() * 0.45 + 40.0).clamp(60.0, 180.0)
    };
    egui::epaint::CubicBezierShape::from_points_stroke(
        [start, start + Vec2::new(reach, 0.0), end - Vec2::new(reach, 0.0), end],
        false, Color32::TRANSPARENT, Stroke::NONE,
    )
}

fn wire(painter: &egui::Painter, start: Pos2, end: Pos2, color: Color32, width: f32) {
    let mut shape = curve(start, end);
    shape.stroke = Stroke::new(width, color).into();
    painter.add(shape);
}

fn midpoint(start: Pos2, end: Pos2) -> Pos2 { start + (end - start) * 0.5 }

fn route_point(start: Pos2, end: Pos2, route: Option<[i32; 2]>) -> Pos2 {
    match route {
        Some([x,y]) => midpoint(start,end) + Vec2::new(x as f32,y as f32),
        None => curve(start,end).sample(0.5),
    }
}

fn near_curve(start: Pos2, end: Pos2, pointer: Pos2) -> Option<Pos2> {
    let shape = curve(start,end);
    (0..=48).map(|index|shape.sample(index as f32/48.0))
        .min_by(|a,b|a.distance_sq(pointer).total_cmp(&b.distance_sq(pointer)))
        .filter(|p|p.distance_sq(pointer)<=64.0)
}

fn title(kind: &NodeKind) -> String {
    match kind {
        NodeKind::Selection { .. } => "Select elements".into(),
        NodeKind::Dimensions => "Read dimensions".into(),
        NodeKind::Read { property } => format!("Read {property:?}"),
        NodeKind::Set { property, .. } => format!("Set {property:?}"),
        NodeKind::Length { .. } => "Length".into(),
        NodeKind::Ratio { .. } => "Ratio".into(),
        NodeKind::Slider { .. } => "Number slider".into(),
        NodeKind::Python { .. } => "Python expression".into(),
        _ => "node".into(),
    }
}

fn input_name(kind: &NodeKind, index: usize) -> String {
    match (kind, index) {
        (NodeKind::Read { .. } | NodeKind::Set { .. } | NodeKind::Dimensions, 0) => "elements".into(),
        (NodeKind::Set { property, .. }, 1) => format!("{property:?}").to_lowercase(),
        (NodeKind::Python { .. }, 0) => "x".into(),
        _ => "input".into(),
    }
}

fn output_name(kind: &NodeKind) -> String {
    match kind {
        NodeKind::Selection { .. } | NodeKind::Set { .. } => "elements".into(),
        NodeKind::Dimensions => "Read dimensions".into(),
        NodeKind::Read { property } => format!("{property:?}").to_lowercase(),
        NodeKind::Length { .. } => "length (m)".into(),
        NodeKind::Ratio { .. } => "ratio".into(),
        NodeKind::Slider { .. } => "value".into(),
        NodeKind::Python { .. } => "result".into(),
        _ => "value".into(),
    }
}

fn socket(painter: &egui::Painter, cell: Rect, input: bool, connected: bool, color: Color32) {
    // The attachment is a small tab INSIDE its own port cell. The wire meets
    // the exact outside edge of that tab; there are no floating port circles.
    let tab = Rect::from_center_size(
        egui::pos2(if input { cell.left() + 3.0 } else { cell.right() - 3.0 }, cell.center().y),
        Vec2::new(6.0, 12.0),
    );
    if connected { painter.rect_filled(tab, 0.0, color); }
    painter.rect_stroke(tab, 0.0, Stroke::new(1.2, color), StrokeKind::Inside);
}

pub fn show(ui: &mut egui::Ui, graph: &mut Graph, pending: &mut Option<(u64,u16)>, status: &mut String) -> bool {
    let ctx=ui.ctx().clone();
    let node_rect=|origin:Pos2,node:&Node| {
        let mut rect=node_rect(origin,node);
        if matches!(node.kind,NodeKind::Slider {..}) && crate::automation_slider::expanded(&ctx,node.id) {rect.max.y+=146.0;}
        rect
    };
    let selection_key=ui.scope_id().with("selected-blocks");
    let mut selected=ui.ctx().data(|d|d.get_temp::<std::collections::BTreeSet<u64>>(selection_key)).unwrap_or_default();
    selected.retain(|id|graph.nodes.iter().any(|n|n.id==*id));
    let group_requested=ui.button("Group selected (Ctrl+G)").clicked() || ui.input_mut(|i|i.consume_key(egui::Modifiers::COMMAND,egui::Key::G));
    if group_requested {crate::automation_groups::create(graph,&selected);}
    ui.weak("Click a header to select; Ctrl-click toggles. Drag empty canvas to select blocks; drag a selected header to move the selection.");
    ui.label("Drag an output to an input and release. Drag a blue wire to reshape it. Double-click a wire to reset. Esc cancels.");
    ui.weak("Manual execution: Preview passes data through the links. Apply changes updates the model.");
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) { *pending = None; }
    let height=graph.nodes.iter().map(|n|node_rect(Pos2::ZERO,n).bottom()+32.0).fold(350.0,f32::max).min(4000.0);
    let (canvas, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::click_and_drag());
    let painter = ui.painter().with_clip_rect(canvas);
    let dark = ui.visuals().dark_mode;
    let blue = if dark { Color32::from_rgb(90, 180, 255) } else { Color32::from_rgb(35, 130, 220) };
    let separator = if dark { Color32::from_gray(25) } else { Color32::from_gray(100) };
    let body = if dark { Color32::from_gray(61) } else { Color32::from_gray(235) };
    let unconnected = if dark { Color32::from_gray(180) } else { Color32::from_gray(115) };
    painter.rect_filled(canvas, 4.0, ui.visuals().extreme_bg_color);
    let mut changed = group_requested;
    let marquee_key=ui.scope_id().with("selection-marquee");
    let mut marquee=ui.ctx().data(|d|d.get_temp::<Marquee>(marquee_key));
    let escaped=ui.input(|i|i.key_pressed(egui::Key::Escape));
    if escaped {
        if let Some(m)=marquee.take() {selected=m.base;}
    } else if ui.input(|i|i.pointer.button_pressed(egui::PointerButton::Primary)) {
        if let Some(pointer)=ui.ctx().pointer_interact_pos().filter(|p|canvas.contains(*p)) {
            let on_block=graph.nodes.iter().any(|n|node_rect(canvas.min,n).expand(10.0).contains(pointer));
            let on_wire=graph.wires.iter().any(|w| {
                let source=graph.nodes.iter().find(|n|n.id==w.from);
                let target=graph.nodes.iter().find(|n|n.id==w.to);
                source.zip(target).is_some_and(|(s,t)| {
                    let start=port_position(node_rect(canvas.min,s),None,w.output);
                    let end=port_position(node_rect(canvas.min,t),Some(w.input),0);
                    let handle=route_point(start,end,w.route);
                    if w.route.is_some() {near_curve(start,handle,pointer).or_else(||near_curve(handle,end,pointer)).is_some()} else {near_curve(start,end,pointer).is_some()}
                })
            });
            // Group headers own their gestures; empty group backgrounds remain selectable.
            let on_group=graph.groups.iter().any(|g| {
                let bounds=graph.nodes.iter().filter(|n|g.nodes.contains(&n.id)).fold(Rect::NOTHING,|r,n|r.union(node_rect(canvas.min,n))).expand(12.0);
                Rect::from_min_size(bounds.min-Vec2::new(0.0,28.0),Vec2::new(bounds.width(),28.0)).contains(pointer)
            });
            if !on_block && !on_wire && !on_group && pending.is_none() {
                marquee=Some(Marquee {start:pointer,base:selected.clone(),additive:ui.input(|i|i.modifiers.command || i.modifiers.ctrl)});
            }
        }
    }
    if let Some(m)=&marquee {
        if let Some(pointer)=ui.ctx().pointer_interact_pos() {
            let area=Rect::from_two_pos(m.start,pointer);
            selected=if m.additive {m.base.clone()} else {Default::default()};
            if area.width()>3.0 || area.height()>3.0 {
                for node in &graph.nodes {
                    let rect=node_rect(canvas.min,node);
                    if if pointer.x>=m.start.x {area.contains_rect(rect)} else {area.intersects(rect)} {selected.insert(node.id);}
                }
                painter.rect_filled(area,0.0,blue.gamma_multiply(0.12));
                painter.rect_stroke(area,0.0,Stroke::new(1.0,blue),StrokeKind::Inside);
            }
        }
    }
    if ui.input(|i|i.pointer.button_released(egui::PointerButton::Primary)) {marquee=None;}
    ui.ctx().data_mut(|d| {if let Some(m)=marquee {d.insert_temp(marquee_key,m);} else {d.remove::<Marquee>(marquee_key);}});
    changed |= crate::automation_groups::show(ui,graph,canvas,&mut selected,|node|node_rect(canvas.min,node));
    let mut delete = None;
    let mut disconnect = None;
    let mut connect = None;

    // Resolve motion first so sockets and both ends of every wire move in the
    // same frame. Context menus stay confined to headers and their own port rows.
    let mut selection_motion=None;
    for node in &graph.nodes {
        let rect = node_rect(canvas.min, node);
        let header = Rect::from_min_size(rect.min, Vec2::new(WIDTH, HEADER));
        let response = ui.interact(header.intersect(canvas), ui.scope_id().with(("node-header", node.id)), Sense::click_and_drag());
        if response.double_clicked() && matches!(node.kind,NodeKind::Python {..}) {
            ui.ctx().data_mut(|d|d.insert_temp(ui.scope_id().with("python-editor"),node.id));
        }
        if response.clicked() {
            if ui.input(|i|i.modifiers.command || i.modifiers.ctrl) {
                if !selected.insert(node.id) {selected.remove(&node.id);}
            } else {selected.clear();selected.insert(node.id);}
        }
        if response.dragged() {
            if !selected.contains(&node.id) {selected.clear();selected.insert(node.id);}
            selection_motion=Some(response.drag_delta());
        }
        response.context_menu(|ui| {
            if matches!(node.kind,NodeKind::Python {..}) && ui.button("Edit Python expression").clicked() {
                ui.ctx().data_mut(|d|d.insert_temp(selection_key.with("python-editor-request"),node.id));ui.close();
            }
            if ui.button(if selected.contains(&node.id) && selected.len()>1 {"Delete selected blocks"} else {"Delete block"}).clicked() { delete = Some(node.id); ui.close(); }
        });
    }
    if let Some(delta)=selection_motion {move_selection(graph,&selected,delta);}
    let drag_key = ui.scope_id().with("connector-drag");
    let cancel_key = ui.scope_id().with("connector-drag-cancelled");
    if ui.input(|i|i.key_pressed(egui::Key::Escape)) {
        ui.ctx().data_mut(|data|data.insert_temp(cancel_key,true));
    }
    let cancelled = ui.ctx().data(|data|data.get_temp::<bool>(cancel_key).unwrap_or(false));
    // Capture the socket on press, before the enclosing scroll area can claim
    // the gesture. Include the visible edge and a small outside hit margin.
    if !cancelled && ui.input(|i|i.pointer.button_pressed(egui::PointerButton::Primary)) {
        if let Some(pointer)=ui.ctx().pointer_interact_pos().filter(|p|canvas.contains(*p)) {
            if let Some((node,output))=graph.nodes.iter().rev().find_map(|n| {
                let rect=node_rect(canvas.min,n);
                n.kind.outputs().into_iter().enumerate().find_map(|(row,(id,_,_))| {
                    Rect::from_min_max(egui::pos2(rect.center().x,rect.top()+HEADER+row as f32*ROW),egui::pos2(rect.right()+10.0,rect.top()+HEADER+(row+1) as f32*ROW)).contains(pointer).then_some((n,id))
                })
            }) {
                *pending=Some((node.id,output));
                ui.ctx().data_mut(|data|data.insert_temp(drag_key,true));
            }
        }
    }
    let was_connector_drag = ui.ctx().data(|data|data.get_temp::<bool>(drag_key).unwrap_or(false));
    let mut connection_target = None;
    let mut rerouted = false;
    for connection in &mut graph.wires {
        if let (Some(source), Some(target)) = (
            graph.nodes.iter().find(|n| n.id == connection.from),
            graph.nodes.iter().find(|n| n.id == connection.to),
        ) {
            let start = port_position(node_rect(canvas.min, source),None,connection.output);
            let end = port_position(node_rect(canvas.min, target),Some(connection.input),0);
            let mut handle = route_point(start,end,connection.route);
            let pointer = ui.ctx().pointer_hover_pos().filter(|p|canvas.contains(*p));
            let blocked = pointer.is_some_and(|p|graph.nodes.iter().any(|n|node_rect(canvas.min,n).contains(p)));
            let nearby = pointer.and_then(|p| {
                if blocked || pending.is_some() { return None; }
                if connection.route.is_some() {
                    near_curve(start,handle,p).or_else(||near_curve(handle,end,p))
                } else { near_curve(start,end,p) }
            });
            let hit = if let Some(point)=nearby {
                Rect::from_center_size(point,Vec2::splat(18.0))
            } else if !blocked && pending.is_none() {
                Rect::from_center_size(handle,Vec2::splat(14.0)).intersect(canvas)
            } else { Rect::from_min_size(canvas.min,Vec2::ZERO) };
            let response = ui.interact(hit,ui.scope_id().with(("wire-route",connection.from,connection.output,connection.to,connection.input)),Sense::click_and_drag());
            if response.double_clicked() {
                connection.route=None;
                handle=route_point(start,end,None);
                rerouted=true;
            } else if response.dragged() {
                if let Some(position)=response.interact_pointer_pos() {
                    let delta=position-midpoint(start,end);
                    connection.route=Some([delta.x.round().clamp(-100000.0,100000.0) as i32,delta.y.round().clamp(-100000.0,100000.0) as i32]);
                    handle=route_point(start,end,connection.route);
                    rerouted=true;
                }
            }
            if connection.route.is_some() {
                wire(&painter,start,handle,blue,2.0);
                wire(&painter,handle,end,blue,2.0);
            } else { wire(&painter,start,end,blue,2.0); }
            if response.hovered() || response.dragged() || connection.route.is_some() {
                painter.circle_filled(handle,4.5,body);
                painter.circle_stroke(handle,4.5,Stroke::new(1.5,blue));
            }
            response.on_hover_cursor(egui::CursorIcon::Grab).on_hover_text("Drag to change the wire path; release to keep it. Double-click to restore the automatic curve.");
        }
    }
    if let Some(source) = pending.and_then(|(id,_)| graph.nodes.iter().find(|n| n.id == id)) {
        if let Some(pointer) = ui.ctx().pointer_hover_pos().filter(|p| canvas.contains(*p)) {
            wire(&painter, port_position(node_rect(canvas.min, source), None,pending.unwrap().1), pointer, blue.gamma_multiply(0.75), 2.0);
        }
    }
    for node in &graph.nodes {
        let rect = node_rect(canvas.min, node);
        let header = Rect::from_min_size(rect.min, Vec2::new(WIDTH, HEADER));
        let middle = rect.left() + WIDTH * 0.5;
        painter.rect_filled(rect, 3.0, body);
        painter.rect_filled(header, 3.0, if dark {Color32::from_gray(74)} else {Color32::from_gray(214)});
        painter.rect_stroke(rect, 3.0, Stroke::new(if selected.contains(&node.id) {2.5} else {1.0},if selected.contains(&node.id) {blue} else {separator}), StrokeKind::Inside);
        painter.text(header.center(), egui::Align2::CENTER_CENTER,
            title(&node.kind), FontId::proportional(13.0), ui.visuals().text_color());

        let inputs = node.kind.inputs();
        let rows = inputs.len().max(node.kind.outputs().len()).max(1);
        painter.line_segment([egui::pos2(middle, header.bottom()), egui::pos2(middle, header.bottom()+rows as f32*ROW)], Stroke::new(1.0, separator));
        for row in 0..rows {
            let top = header.bottom() + row as f32 * ROW;
            painter.line_segment([egui::pos2(rect.left(), top), egui::pos2(rect.right(), top)], Stroke::new(1.0, separator));
        }
        for (index, ty) in inputs.iter().enumerate() {
            let cell = Rect::from_min_size(egui::pos2(rect.left(), header.bottom() + index as f32 * ROW), Vec2::new(WIDTH * 0.5, ROW));
            let response = ui.interact(cell.intersect(canvas), ui.scope_id().with(("input-row", node.id, index)), Sense::click());
            // Hover ownership belongs to the dragged output during a drag;
            // drop targeting must use geometry, not widget hover ownership.
            if ui.ctx().pointer_interact_pos().is_some_and(|p|canvas.contains(p) && cell.expand2(Vec2::new(10.0,0.0)).contains(p)) {
                connection_target=Some((node.id,index));
            }
            let compatible = pending.and_then(|(id,_)| graph.nodes.iter().find(|n| n.id == id))
                .is_some_and(|source| source.id != node.id && source.kind.output_type(pending.unwrap().1) == Some(*ty));
            if response.hovered() {
                let color = if pending.is_none() || compatible { blue } else { Color32::from_rgb(210, 70, 65) };
                painter.rect_filled(cell.shrink(1.0), 0.0, color.gamma_multiply(0.16));
            }
            let connected = graph.wires.iter().any(|w| w.to == node.id && w.input == index);
            socket(&painter, cell, true, connected, if connected { blue } else { unconnected });
            painter.text(cell.left_center() + Vec2::new(12.0, 0.0), egui::Align2::LEFT_CENTER,
                input_name(&node.kind, index), FontId::proportional(12.0), ui.visuals().text_color());
            if response.clicked() && !was_connector_drag {
                if let Some(from) = *pending {
                    if compatible { connect = Some(Wire { output:from.1, from:from.0, to: node.id, input: index, route: None }); *pending = None; }
                    else { *status = "Connection rejected: incompatible types or self connection".into(); }
                }
            }
            response.on_hover_text(format!("{} � {ty:?}\nClick to connect; right-click to disconnect", input_name(&node.kind, index))).context_menu(|ui| {
                if ui.add_enabled(connected, egui::Button::new("Disconnect input")).clicked() {
                    disconnect = Some((node.id, index)); ui.close();
                }
            });
        }
        for (row,(output,name,ty)) in node.kind.outputs().into_iter().enumerate() {
        let cell = Rect::from_min_size(egui::pos2(middle, header.bottom()+row as f32*ROW), Vec2::new(WIDTH * 0.5, ROW));
        let selected = *pending == Some((node.id,output));
        let response = ui.interact(cell.intersect(canvas), ui.scope_id().with(("output-row", node.id,output)), Sense::click_and_drag());
        if selected || response.hovered() { painter.rect_filled(cell.shrink(1.0), 0.0, blue.gamma_multiply(0.16)); }
        let connected = graph.wires.iter().any(|w| w.from == node.id && w.output==output);
        socket(&painter, cell, false, connected || selected, if connected || selected { blue } else { unconnected });
        painter.text(cell.right_center() - Vec2::new(12.0, 0.0), egui::Align2::RIGHT_CENTER,
            if matches!(node.kind,NodeKind::Dimensions) {name.to_owned()} else {output_name(&node.kind)}, FontId::proportional(12.0), ui.visuals().text_color());
        if !cancelled && (response.drag_started() || response.dragged()) {
            *pending=Some((node.id,output));
            ui.ctx().data_mut(|data|data.insert_temp(drag_key,true));
        } else if response.clicked() && !was_connector_drag { *pending = if selected { None } else { Some((node.id,output)) }; }
        response.on_hover_text(format!("{name} | {ty:?}\nDrag to a compatible input and release; click-click also works"));
        }
    }
    if was_connector_drag && !cancelled && ui.input(|i|i.pointer.button_released(egui::PointerButton::Primary)) {
        if let (Some((from,output)),Some((to,input)))=(*pending,connection_target) {
            let source=graph.nodes.iter().find(|n|n.id==from);
            let target=graph.nodes.iter().find(|n|n.id==to);
            if source.zip(target).is_some_and(|(s,t)|s.id!=t.id && t.kind.inputs().get(input)==s.kind.output_type(output).as_ref()) {
                connect=Some(Wire { output, from,to,input,route:None });
                *status="Connected. Click Preview to evaluate the graph.".into();
            } else { *status="Connection rejected: incompatible connector types".into(); }
            *pending=None;
        } else if ui.input(|i|i.pointer.is_decidedly_dragging()) {
            *status="Connection cancelled: release on a compatible input connector".into();
            *pending=None;
        }
        ui.ctx().data_mut(|data|data.insert_temp(drag_key,false));
    }
    if ui.input(|i|i.key_pressed(egui::Key::Escape)) {
        *pending=None;ui.ctx().data_mut(|data|data.insert_temp(drag_key,false));
    }
    if ui.input(|i|i.pointer.button_released(egui::PointerButton::Primary)) {
        ui.ctx().data_mut(|data| { data.insert_temp(drag_key,false);data.insert_temp(cancel_key,false); });
    }
    // Edit values directly inside each block, outside the port hit regions.
    for node in &mut graph.nodes {
        let rect=node_rect(canvas.min,node);
        let editor=Rect::from_min_max(egui::pos2(rect.left()+8.0,rect.top()+HEADER+ROW * node.kind.inputs().len().max(node.kind.outputs().len()).max(1) as f32+4.0),rect.right_bottom()-Vec2::splat(8.0));
        ui.scope_builder(egui::UiBuilder::new().id_salt(("block-editor",node.id)).max_rect(editor),|ui| {
            ui.set_clip_rect(editor.intersect(canvas));
            match &mut node.kind {
                kind @ NodeKind::Slider {..} => {changed|=crate::automation_slider::show(ui,kind,node.id);}
                NodeKind::Length { metres } => { changed |= ui.add(egui::DragValue::new(metres).speed(0.01).suffix(" m")).changed(); }
                NodeKind::Ratio { value } => { changed |= ui.add(egui::DragValue::new(value).speed(0.001)).changed(); }
                NodeKind::Python { expression,.. } => { changed |= ui.text_edit_singleline(expression).changed(); }
                NodeKind::Read { property } | NodeKind::Set { property, .. } => {
                    egui::ComboBox::from_id_salt("property").selected_text(format!("{property:?}")).show_ui(ui,|ui| {
                        for p in [openbim_automation::Property::Width,openbim_automation::Property::Height,openbim_automation::Property::Elevation,openbim_automation::Property::Slope] {
                            changed |= ui.selectable_value(property,p,format!("{p:?}")).changed();
                        }
                    });
                }
                NodeKind::Dimensions => { ui.label("Width / height / elevation"); },
                NodeKind::Selection { ids } => {
                    let key=ui.scope_id().with("selection-text");
                    let mut text=ui.ctx().data(|d|d.get_temp::<String>(key)).unwrap_or_else(||ids.iter().map(u64::to_string).collect::<Vec<_>>().join(", "));
                    let response=ui.add(egui::TextEdit::singleline(&mut text).hint_text("Element IDs, comma separated"));
                    if response.changed() {
                        if let Ok(parsed)=text.split(',').map(str::trim).filter(|v|!v.is_empty()).map(str::parse::<u64>).collect::<Result<Vec<_>,_>>() {
                            *ids=parsed; changed=true;
                        }
                    }
                    if response.has_focus() { ui.ctx().data_mut(|d|d.insert_temp(key,text)); }
                    else { ui.ctx().data_mut(|d|d.remove::<String>(key)); }
                }
                _ => {},
            }
        });
    }
    let editor_key=ui.scope_id().with("python-editor");
    if let Some(id)=ui.ctx().data(|d|d.get_temp::<u64>(selection_key.with("python-editor-request"))) {
        ui.ctx().data_mut(|d| {d.remove::<u64>(selection_key.with("python-editor-request"));d.insert_temp(editor_key,id);});
    }
    if let Some(id)=ui.ctx().data(|d|d.get_temp::<u64>(editor_key)) {
        let mut open=true;
        if let Some(node)=graph.nodes.iter_mut().find(|n|n.id==id) {
            if let NodeKind::Python {expression,quantity,..}=&mut node.kind {
                egui::Window::new(format!("Python expression | Node {id}")).id(editor_key).open(&mut open).default_width(480.0).show(ui.ctx(),|ui| {
                    ui.label("Numeric expression using x. The worker currently supports expressions, not general Python scripts.");
                    changed|=ui.add(egui::TextEdit::multiline(expression).code_editor().desired_rows(8)).changed();
                    egui::ComboBox::from_id_salt("expression-quantity").selected_text(format!("{quantity:?}")).show_ui(ui,|ui| {
                        changed|=ui.selectable_value(quantity,openbim_automation::Type::Length,"Length (m)").changed();
                        changed|=ui.selectable_value(quantity,openbim_automation::Type::Ratio,"Number / ratio").changed();
                    });
                    ui.weak("Changes invalidate the preview. Close this editor, then Preview before Apply.");
                });
            } else {open=false;}
        } else {open=false;}
        if !open {ui.ctx().data_mut(|d|d.remove::<u64>(editor_key));}
    }
    if rerouted { *status="Wire path adjusted; its data connection is unchanged.".into(); }
    if let Some(connection) = connect {
        graph.wires.retain(|w| w.to != connection.to || w.input != connection.input);
        graph.wires.push(connection);
        *status="Connected. Click Preview to evaluate the graph.".into();
        changed = true;
    }
    if let Some((id, index)) = disconnect {
        graph.wires.retain(|w| w.to != id || w.input != index);
        changed = true;
    }
    if let Some(id) = delete {
        let removed=if selected.contains(&id) {selected.clone()} else {std::collections::BTreeSet::from([id])};
        graph.nodes.retain(|n| !removed.contains(&n.id));
        graph.wires.retain(|w| !removed.contains(&w.from) && !removed.contains(&w.to));
        for group in &mut graph.groups {group.nodes.retain(|member|!removed.contains(member));}
        graph.groups.retain(|group|!group.nodes.is_empty());
        if pending.is_some_and(|p|removed.contains(&p.0)) { *pending = None; }
        changed = true;
    }
    ui.ctx().data_mut(|d|d.insert_temp(selection_key,selected));
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event,Modifiers,PointerButton,RawInput};

    fn frame(ctx:&egui::Context,graph:&mut Graph,pending:&mut Option<(u64,u16)>,mut events:Vec<Event>)->egui::FullOutput {
        if let Some(modifiers)=events.iter().rev().find_map(|e|match e {Event::PointerButton {modifiers,..}|Event::Key {modifiers,..}=>Some(*modifiers),_=>None}) {
            events.insert(0,Event::ModifiersChanged(modifiers));
        }
        let mut output=ctx.run_ui(RawInput { screen_rect:Some(Rect::from_min_size(Pos2::ZERO,Vec2::new(1000.0,600.0))),events,..Default::default() },|ui| {
            egui::ScrollArea::vertical().show(ui,|ui| {
                ui.ctx().data_mut(|d|d.insert_temp(egui::Id::unique("test-canvas-id"),ui.scope_id()));
                show(ui,graph,pending,&mut String::new());
            });
        });
        output.textures_delta.clear();
        output
    }
    fn button(pos:Pos2,pressed:bool)->Event {
        Event::PointerButton { pos,button:PointerButton::Primary,pressed,modifiers:Modifiers::default() }
    }
    fn first_curve(output:&egui::FullOutput)->egui::epaint::CubicBezierShape {
        output.shapes.iter().find_map(|s|if let egui::Shape::CubicBezier(curve)=&s.shape {Some(curve.clone())}else{None}).expect("Visible wire")
    }
    #[test]
    fn ctrl_click_toggles_selection_and_double_click_opens_python_editor() {
        let ctx=egui::Context::default();let mut graph=Graph::example(vec![1]);let mut pending=None;
        graph.nodes[1].kind=NodeKind::Python {expression:"x + 0.3".into(),quantity:openbim_automation::Type::Length, lacing: openbim_automation::Lacing::Auto};
        graph.wires[0].from=3;graph.validate().unwrap();
        let origin=first_curve(&frame(&ctx,&mut graph,&mut pending,vec![])).points[3]-Vec2::new(290.0,74.0);
        let first=origin+Vec2::new(140.0,45.0);let second=origin+Vec2::new(410.0,45.0);
        frame(&ctx,&mut graph,&mut pending,vec![Event::PointerMoved(first),button(first,true)]);
        frame(&ctx,&mut graph,&mut pending,vec![button(first,false)]);
        let ctrl=Modifiers {ctrl:true,command:true,..Default::default()};
        for pressed in [true,false] {frame(&ctx,&mut graph,&mut pending,vec![Event::PointerMoved(second),Event::PointerButton {pos:second,pressed,button:PointerButton::Primary,modifiers:ctrl}]);}
        let id=ctx.data(|d|d.get_temp::<egui::Id>(egui::Id::unique("test-canvas-id"))).unwrap();
        assert_eq!(ctx.data(|d|d.get_temp::<std::collections::BTreeSet<u64>>(id.with("selected-blocks"))).unwrap(),std::collections::BTreeSet::from([1,2]));
        for pressed in [true,false] {frame(&ctx,&mut graph,&mut pending,vec![Event::PointerMoved(first),Event::PointerButton {pos:first,pressed,button:PointerButton::Primary,modifiers:ctrl}]);}
        assert_eq!(ctx.data(|d|d.get_temp::<std::collections::BTreeSet<u64>>(id.with("selected-blocks"))).unwrap(),std::collections::BTreeSet::from([2]));
        // A plain double-click opens the numeric Python editor.
        let ctx=egui::Context::default();
        let origin=first_curve(&frame(&ctx,&mut graph,&mut pending,vec![])).points[3]-Vec2::new(290.0,74.0);
        let second=origin+Vec2::new(410.0,45.0);
        let id=ctx.data(|d|d.get_temp::<egui::Id>(egui::Id::unique("test-canvas-id"))).unwrap();
        for _ in 0..2 {
            frame(&ctx,&mut graph,&mut pending,vec![Event::PointerMoved(second),button(second,true)]);
            frame(&ctx,&mut graph,&mut pending,vec![button(second,false)]);
        }
        assert_eq!(ctx.data(|d|d.get_temp::<u64>(id.with("python-editor"))),Some(2));
    }
    #[test]
    fn escape_restores_selection_and_empty_click_clears_it() {
        let ctx=egui::Context::default();let mut graph=Graph::example(vec![1]);let mut pending=None;
        let origin=first_curve(&frame(&ctx,&mut graph,&mut pending,vec![])).points[0]-Vec2::new(260.0,74.0);
        let id=ctx.data(|d|d.get_temp::<egui::Id>(egui::Id::unique("test-canvas-id"))).unwrap();
        ctx.data_mut(|d|d.insert_temp(id.with("selected-blocks"),std::collections::BTreeSet::from([4_u64])));
        let start=origin+Vec2::new(10.0,15.0);let end=origin+Vec2::new(550.0,170.0);
        frame(&ctx,&mut graph,&mut pending,vec![Event::PointerMoved(start),button(start,true)]);
        frame(&ctx,&mut graph,&mut pending,vec![Event::PointerMoved(end)]);
        frame(&ctx,&mut graph,&mut pending,vec![Event::Key {key:egui::Key::Escape,physical_key:None,pressed:true,repeat:false,modifiers:Modifiers::default()}]);
        assert_eq!(ctx.data(|d|d.get_temp::<std::collections::BTreeSet<u64>>(id.with("selected-blocks"))).unwrap(),std::collections::BTreeSet::from([4]));
        frame(&ctx,&mut graph,&mut pending,vec![button(end,false)]);
        let empty=origin+Vec2::new(900.0,300.0);
        frame(&ctx,&mut graph,&mut pending,vec![Event::PointerMoved(empty),button(empty,true)]);
        frame(&ctx,&mut graph,&mut pending,vec![button(empty,false)]);
        assert!(ctx.data(|d|d.get_temp::<std::collections::BTreeSet<u64>>(id.with("selected-blocks"))).unwrap().is_empty());
    }
    #[test]
    fn marquee_selects_two_blocks_and_header_drag_moves_both() {
        let ctx=egui::Context::default();let mut graph=Graph::example(vec![1]);let mut pending=None;
        let output=frame(&ctx,&mut graph,&mut pending,vec![]);
        let origin=first_curve(&output).points[0]-Vec2::new(260.0,74.0);
        let start=origin+Vec2::new(10.0,15.0);let end=origin+Vec2::new(550.0,170.0);
        frame(&ctx,&mut graph,&mut pending,vec![Event::PointerMoved(start),button(start,true)]);
        frame(&ctx,&mut graph,&mut pending,vec![Event::PointerMoved(end)]);
        frame(&ctx,&mut graph,&mut pending,vec![button(end,false)]);
        let id=ctx.data(|d|d.get_temp::<egui::Id>(egui::Id::unique("test-canvas-id"))).unwrap();
        let selected=ctx.data(|d|d.get_temp::<std::collections::BTreeSet<u64>>(id.with("selected-blocks"))).unwrap();
        assert_eq!(selected,std::collections::BTreeSet::from([1,2]));
        let before=graph.nodes.clone();let wires=graph.wires.clone();
        let header=origin+Vec2::new(140.0,45.0);let finish=header+Vec2::new(25.0,30.0);
        frame(&ctx,&mut graph,&mut pending,vec![Event::PointerMoved(header),button(header,true)]);
        frame(&ctx,&mut graph,&mut pending,vec![Event::PointerMoved(finish)]);
        frame(&ctx,&mut graph,&mut pending,vec![button(finish,false)]);
        for index in [0,1] {assert_eq!(graph.nodes[index].position,[before[index].position[0]+25.0,before[index].position[1]+30.0]);}
        assert_eq!(graph.nodes[2],before[2]);assert_eq!(graph.wires,wires);
    }
    #[test]
    fn second_output_socket_connects_and_preserves_other_outputs() {
        let ctx=egui::Context::default();let mut graph=Graph::example(vec![1]);let mut pending=None;
        graph.nodes[1].kind=NodeKind::Dimensions;
        let shape=first_curve(&frame(&ctx,&mut graph,&mut pending,vec![]));
        // First curve ends at the Dimensions input; its output is WIDTH to the right.
        let start=shape.points[3]+Vec2::new(WIDTH-3.0,ROW);
        let output=frame(&ctx,&mut graph,&mut pending,vec![]);
        let curves:Vec<_>=output.shapes.iter().filter_map(|s|if let egui::Shape::CubicBezier(c)=&s.shape {Some(c)}else{None}).collect();
        let finish=curves.last().unwrap().points[3]+Vec2::new(3.0,0.0);
        frame(&ctx,&mut graph,&mut pending,vec![Event::PointerMoved(start),button(start,true)]);
        frame(&ctx,&mut graph,&mut pending,vec![Event::PointerMoved(finish),button(finish,false)]);
        assert!(graph.wires.iter().any(|w|w.from==2 && w.output==1 && w.to==4 && w.input==1));
        assert!(graph.wires.iter().any(|w|w.from==1 && w.to==4 && w.input==0));
        graph.validate().unwrap();
    }
    #[test]
    fn socket_edge_drag_in_one_motion_keeps_output_fanout() {
        let ctx=egui::Context::default();let mut graph=Graph::example(vec![1]);let mut pending=None;
        let shape=first_curve(&frame(&ctx,&mut graph,&mut pending,vec![]));
        let start=shape.points[0]+Vec2::new(2.0,0.0);
        let finish=shape.points[3]-Vec2::new(2.0,0.0);
        graph.wires.retain(|w|!(w.to==2 && w.input==0));
        frame(&ctx,&mut graph,&mut pending,vec![]);
        frame(&ctx,&mut graph,&mut pending,vec![Event::PointerMoved(start),button(start,true)]);
        frame(&ctx,&mut graph,&mut pending,vec![Event::PointerMoved(finish),button(finish,false)]);
        assert_eq!(graph.wires.iter().filter(|w|w.from==1).count(),2);
        assert_eq!(graph.wires.iter().filter(|w|w.to==2 && w.input==0).count(),1);
        graph.validate().unwrap();
    }
    #[test]
    fn dragging_and_releasing_a_wire_preserves_its_connection() {
        let ctx=egui::Context::default();let mut graph=Graph::example(vec![1]);let mut pending=None;
        let output=frame(&ctx,&mut graph,&mut pending,vec![]);
        let shape=output.shapes.iter().filter_map(|s|if let egui::Shape::CubicBezier(c)=&s.shape {Some(c)}else{None}).next_back().unwrap();
        let start=shape.sample(0.5);let finish=start+Vec2::new(0.0,50.0);
        frame(&ctx,&mut graph,&mut pending,vec![Event::PointerMoved(start),button(start,true)]);
        frame(&ctx,&mut graph,&mut pending,vec![Event::PointerMoved(finish)]);
        frame(&ctx,&mut graph,&mut pending,vec![button(finish,false)]);
        let connection=&graph.wires[2];
        assert_eq!((connection.from,connection.to,connection.input),(3,4,1));
        assert!(connection.route.is_some(),"Dragging a curve must save its new route");
        assert_eq!(graph.wires.len(),3);
    }
    #[test]
    fn dragging_an_output_and_releasing_on_input_creates_a_real_wire() {
        let ctx=egui::Context::default();let mut graph=Graph::example(vec![1]);let mut pending=None;
        let shape=first_curve(&frame(&ctx,&mut graph,&mut pending,vec![]));
        let start=shape.points[0]-Vec2::new(3.0,0.0);
        let finish=shape.points[3]+Vec2::new(3.0,0.0);
        graph.wires.retain(|w|!(w.from==1 && w.to==2 && w.input==0));
        frame(&ctx,&mut graph,&mut pending,vec![]);
        frame(&ctx,&mut graph,&mut pending,vec![Event::PointerMoved(start),button(start,true)]);
        frame(&ctx,&mut graph,&mut pending,vec![Event::PointerMoved(start+Vec2::new(12.0,0.0))]);
        frame(&ctx,&mut graph,&mut pending,vec![Event::PointerMoved(finish)]);
        frame(&ctx,&mut graph,&mut pending,vec![button(finish,false)]);
        assert!(graph.wires.iter().any(|w|w.from==1 && w.to==2 && w.input==0),"Releasing over an input must connect it");
        assert!(pending.is_none());
        graph.validate().unwrap();
    }
    #[test]
    fn releasing_on_an_incompatible_input_preserves_the_existing_link() {
        let ctx=egui::Context::default();let mut graph=Graph::example(vec![1]);let mut pending=None;
        let output=frame(&ctx,&mut graph,&mut pending,vec![]);
        let curves:Vec<_>=output.shapes.iter().filter_map(|s|if let egui::Shape::CubicBezier(c)=&s.shape {Some(c)}else{None}).collect();
        let start=curves[0].points[0]-Vec2::new(3.0,0.0);
        let finish=curves.last().unwrap().points[3]+Vec2::new(3.0,0.0);
        let original=graph.wires.clone();
        frame(&ctx,&mut graph,&mut pending,vec![Event::PointerMoved(start),button(start,true)]);
        frame(&ctx,&mut graph,&mut pending,vec![Event::PointerMoved(start+Vec2::new(12.0,0.0))]);
        frame(&ctx,&mut graph,&mut pending,vec![Event::PointerMoved(finish)]);
        frame(&ctx,&mut graph,&mut pending,vec![button(finish,false)]);
        assert_eq!(graph.wires,original);
        assert!(pending.is_none());
    }
}
