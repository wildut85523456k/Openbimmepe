//! Shared node/port geometry keeps rendered sockets, hit targets and wire ends
//! aligned. Connections use horizontal-tangent cubic Bezier curves.
use eframe::egui::{self, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use openbim_automation::{Graph, Node, NodeKind, Wire};

const WIDTH: f32 = 240.0;
const HEADER: f32 = 30.0;
const ROW: f32 = 28.0;

fn node_rect(origin: Pos2, node: &Node) -> Rect {
    let rows = node.kind.inputs().len().max(1);
    Rect::from_min_size(origin + Vec2::from(node.position), Vec2::new(WIDTH, HEADER + ROW * rows as f32))
}

fn port_position(rect: Rect, input: Option<usize>) -> Pos2 {
    egui::pos2(
        if input.is_some() { rect.left() } else { rect.right() },
        rect.top() + HEADER + ROW * (input.unwrap_or(0) as f32 + 0.5),
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
        NodeKind::Read { property } => format!("Read {property:?}"),
        NodeKind::Set { property } => format!("Set {property:?}"),
        NodeKind::Length { .. } => "Length".into(),
        NodeKind::Ratio { .. } => "Ratio".into(),
        NodeKind::Python { .. } => "Python expression".into(),
    }
}

fn input_name(kind: &NodeKind, index: usize) -> String {
    match (kind, index) {
        (NodeKind::Read { .. } | NodeKind::Set { .. }, 0) => "elements".into(),
        (NodeKind::Set { property }, 1) => format!("{property:?}").to_lowercase(),
        (NodeKind::Python { .. }, 0) => "x".into(),
        _ => "input".into(),
    }
}

fn output_name(kind: &NodeKind) -> String {
    match kind {
        NodeKind::Selection { .. } | NodeKind::Set { .. } => "elements".into(),
        NodeKind::Read { property } => format!("{property:?}").to_lowercase(),
        NodeKind::Length { .. } => "length (m)".into(),
        NodeKind::Ratio { .. } => "ratio".into(),
        NodeKind::Python { .. } => "result".into(),
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

pub fn show(ui: &mut egui::Ui, graph: &mut Graph, pending: &mut Option<u64>, status: &mut String) -> bool {
    ui.label("Drag an output to an input and release. Drag a blue wire to reshape it. Double-click a wire to reset. Esc cancels.");
    ui.weak("Manual execution: Preview passes data through the links. Apply changes updates the model.");
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) { *pending = None; }
    let (canvas, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 350.0), Sense::hover());
    let painter = ui.painter().with_clip_rect(canvas);
    let dark = ui.visuals().dark_mode;
    let blue = if dark { Color32::from_rgb(90, 180, 255) } else { Color32::from_rgb(35, 130, 220) };
    let separator = if dark { Color32::from_gray(25) } else { Color32::from_gray(100) };
    let body = if dark { Color32::from_gray(61) } else { Color32::from_gray(235) };
    let unconnected = if dark { Color32::from_gray(180) } else { Color32::from_gray(115) };
    painter.rect_filled(canvas, 4.0, ui.visuals().extreme_bg_color);
    let mut changed = false;
    let mut delete = None;
    let mut disconnect = None;
    let mut connect = None;

    // Resolve motion first so sockets and both ends of every wire move in the
    // same frame. Context menus stay confined to headers and their own port rows.
    for node in &mut graph.nodes {
        let rect = node_rect(canvas.min, node);
        let header = Rect::from_min_size(rect.min, Vec2::new(WIDTH, HEADER));
        let response = ui.interact(header.intersect(canvas), ui.id().with(("node-header", node.id)), Sense::drag());
        if response.dragged() {
            node.position[0] = (node.position[0] + response.drag_delta().x).max(0.0);
            node.position[1] = (node.position[1] + response.drag_delta().y).max(0.0);
        }
        response.context_menu(|ui| {
            if ui.button("Delete node").clicked() { delete = Some(node.id); ui.close(); }
        });
    }
    let drag_key = ui.id().with("connector-drag");
    let cancel_key = ui.id().with("connector-drag-cancelled");
    if ui.input(|i|i.key_pressed(egui::Key::Escape)) {
        ui.ctx().data_mut(|data|data.insert_temp(cancel_key,true));
    }
    let cancelled = ui.ctx().data(|data|data.get_temp::<bool>(cancel_key).unwrap_or(false));
    let was_connector_drag = ui.ctx().data(|data|data.get_temp::<bool>(drag_key).unwrap_or(false));
    let mut connection_target = None;
    let mut rerouted = false;
    for connection in &mut graph.wires {
        if let (Some(source), Some(target)) = (
            graph.nodes.iter().find(|n| n.id == connection.from),
            graph.nodes.iter().find(|n| n.id == connection.to),
        ) {
            let start = port_position(node_rect(canvas.min, source),None);
            let end = port_position(node_rect(canvas.min, target),Some(connection.input));
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
            let response = ui.interact(hit,ui.id().with(("wire-route",connection.from,connection.to,connection.input)),Sense::click_and_drag());
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
    if let Some(source) = pending.and_then(|id| graph.nodes.iter().find(|n| n.id == id)) {
        if let Some(pointer) = ui.ctx().pointer_hover_pos().filter(|p| canvas.contains(*p)) {
            wire(&painter, port_position(node_rect(canvas.min, source), None), pointer, blue.gamma_multiply(0.75), 2.0);
        }
    }
    for node in &graph.nodes {
        let rect = node_rect(canvas.min, node);
        let header = Rect::from_min_size(rect.min, Vec2::new(WIDTH, HEADER));
        let middle = rect.left() + WIDTH * 0.5;
        painter.rect_filled(rect, 3.0, body);
        painter.rect_filled(header, 3.0, Color32::from_rgb(35, 90, 135));
        painter.rect_stroke(rect, 3.0, Stroke::new(1.0, separator), StrokeKind::Inside);
        painter.text(header.center(), egui::Align2::CENTER_CENTER,
            format!("{} | {}", node.id, title(&node.kind)), FontId::proportional(13.0), Color32::WHITE);

        let inputs = node.kind.inputs();
        let rows = inputs.len().max(1);
        painter.line_segment([egui::pos2(middle, header.bottom()), egui::pos2(middle, rect.bottom())], Stroke::new(1.0, separator));
        for row in 0..rows {
            let top = header.bottom() + row as f32 * ROW;
            painter.line_segment([egui::pos2(rect.left(), top), egui::pos2(rect.right(), top)], Stroke::new(1.0, separator));
        }
        for (index, ty) in inputs.iter().enumerate() {
            let cell = Rect::from_min_size(egui::pos2(rect.left(), header.bottom() + index as f32 * ROW), Vec2::new(WIDTH * 0.5, ROW));
            let response = ui.interact(cell.intersect(canvas), ui.id().with(("input-row", node.id, index)), Sense::click());
            if response.contains_pointer() { connection_target=Some((node.id,index)); }
            let compatible = pending.and_then(|id| graph.nodes.iter().find(|n| n.id == id))
                .is_some_and(|source| source.id != node.id && source.kind.output() == *ty);
            if response.hovered() {
                let color = if pending.is_none() || compatible { blue } else { Color32::from_rgb(210, 70, 65) };
                painter.rect_filled(cell.shrink(1.0), 0.0, color.gamma_multiply(0.16));
            }
            let connected = graph.wires.iter().any(|w| w.to == node.id && w.input == index);
            socket(&painter, cell, true, connected, if connected { blue } else { unconnected });
            painter.text(cell.left_center() + Vec2::new(12.0, 0.0), egui::Align2::LEFT_CENTER,
                input_name(&node.kind, index), FontId::proportional(12.0), ui.visuals().text_color());
            if response.clicked() {
                if !was_connector_drag { if let Some(from) = *pending {
                    if compatible { connect = Some(Wire { from, to: node.id, input: index, route: None }); *pending = None; }
                    else { *status = "Connection rejected: incompatible types or self connection".into(); }
                } }
            }
            response.on_hover_text(format!("{} � {ty:?}\nClick to connect; right-click to disconnect", input_name(&node.kind, index))).context_menu(|ui| {
                if ui.add_enabled(connected, egui::Button::new("Disconnect input")).clicked() {
                    disconnect = Some((node.id, index)); ui.close();
                }
            });
        }
        let cell = Rect::from_min_size(egui::pos2(middle, header.bottom()), Vec2::new(WIDTH * 0.5, ROW));
        let selected = *pending == Some(node.id);
        let response = ui.interact(cell.intersect(canvas), ui.id().with(("output-row", node.id)), Sense::click_and_drag());
        if selected || response.hovered() { painter.rect_filled(cell.shrink(1.0), 0.0, blue.gamma_multiply(0.16)); }
        let connected = graph.wires.iter().any(|w| w.from == node.id);
        socket(&painter, cell, false, connected || selected, if connected || selected { blue } else { unconnected });
        painter.text(cell.right_center() - Vec2::new(12.0, 0.0), egui::Align2::RIGHT_CENTER,
            output_name(&node.kind), FontId::proportional(12.0), ui.visuals().text_color());
        if !cancelled && (response.drag_started() || response.dragged()) {
            *pending=Some(node.id);
            ui.ctx().data_mut(|data|data.insert_temp(drag_key,true));
        } else if response.clicked() { *pending = if selected { None } else { Some(node.id) }; }
        response.on_hover_text(format!("{} | {:?}\nDrag to a compatible input and release; click-click also works", output_name(&node.kind), node.kind.output()));
    }
    if was_connector_drag && !cancelled && ui.input(|i|i.pointer.button_released(egui::PointerButton::Primary)) {
        if let (Some(from),Some((to,input)))=(*pending,connection_target) {
            let source=graph.nodes.iter().find(|n|n.id==from);
            let target=graph.nodes.iter().find(|n|n.id==to);
            if source.zip(target).is_some_and(|(s,t)|s.id!=t.id && t.kind.inputs().get(input)==Some(&s.kind.output())) {
                connect=Some(Wire { from,to,input,route:None });
                *status="Connected. Click Preview to evaluate the graph.".into();
            } else { *status="Connection rejected: incompatible connector types".into(); }
        } else { *status="Connection cancelled: release on a compatible input connector".into(); }
        *pending=None;
        ui.ctx().data_mut(|data|data.insert_temp(drag_key,false));
    }
    if ui.input(|i|i.key_pressed(egui::Key::Escape)) {
        *pending=None;ui.ctx().data_mut(|data|data.insert_temp(drag_key,false));
    }
    if ui.input(|i|i.pointer.button_released(egui::PointerButton::Primary)) {
        ui.ctx().data_mut(|data| { data.insert_temp(drag_key,false);data.insert_temp(cancel_key,false); });
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
        graph.nodes.retain(|n| n.id != id);
        graph.wires.retain(|w| w.from != id && w.to != id);
        if *pending == Some(id) { *pending = None; }
        changed = true;
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event,Modifiers,PointerButton,RawInput};

    fn frame(ctx:&egui::Context,graph:&mut Graph,pending:&mut Option<u64>,events:Vec<Event>)->egui::FullOutput {
        let mut output=ctx.run_ui(RawInput { screen_rect:Some(Rect::from_min_size(Pos2::ZERO,Vec2::new(1000.0,600.0))),events,..Default::default() },|ui| {
            show(ui,graph,pending,&mut String::new());
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
    fn dragging_and_releasing_a_wire_preserves_its_connection() {
        let ctx=egui::Context::default();let mut graph=Graph::example(vec![1]);let mut pending=None;
        let output=frame(&ctx,&mut graph,&mut pending,vec![]);
        let shape=output.shapes.iter().filter_map(|s|if let egui::Shape::CubicBezier(c)=&s.shape {Some(c)}else{None}).last().unwrap();
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
