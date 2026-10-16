use eframe::egui::{self,Rect,Vec2,Color32,Stroke,StrokeKind,Sense};
use openbim_automation::{Graph,Group};
use std::collections::BTreeSet;

pub fn create(graph:&mut Graph,selected:&BTreeSet<u64>)->bool {
    let nodes:Vec<_>=graph.nodes.iter().filter(|n|selected.contains(&n.id) && !graph.groups.iter().any(|g|g.nodes.contains(&n.id))).map(|n|n.id).collect();
    if nodes.is_empty() {return false}
    let id=graph.groups.iter().map(|g|g.id).max().unwrap_or(0).saturating_add(1);
    graph.groups.push(Group {id,title:"New group".into(),nodes,color:[70,120,160]});true
}

pub fn show(ui:&mut egui::Ui,graph:&mut Graph,canvas:Rect,selected:&BTreeSet<u64>,rect:impl Fn(&openbim_automation::Node)->Rect)->bool {
    let mut changed=false;let mut remove=None;let mut transfer=None;
    for group in &mut graph.groups {
        let mut bounds=Rect::NOTHING;
        for node in graph.nodes.iter().filter(|n|group.nodes.contains(&n.id)) {bounds=bounds.union(rect(node));}
        if !bounds.is_finite() {continue}
        bounds=bounds.expand(12.0);bounds.min.y-=28.0;
        let header=Rect::from_min_size(bounds.min,Vec2::new(bounds.width(),28.0));
        let response=ui.interact(header.intersect(canvas),ui.id().with(("group-header",group.id)),Sense::click_and_drag());
        if response.dragged() {
            let mut delta=response.drag_delta();
            let left=graph.nodes.iter().filter(|n|group.nodes.contains(&n.id)).map(|n|n.position[0]).fold(f32::INFINITY,f32::min);
            let top=graph.nodes.iter().filter(|n|group.nodes.contains(&n.id)).map(|n|n.position[1]).fold(f32::INFINITY,f32::min);
            delta.x=delta.x.max(-left);delta.y=delta.y.max(-top);
            for node in graph.nodes.iter_mut().filter(|n|group.nodes.contains(&n.id)) {node.position[0]+=delta.x;node.position[1]+=delta.y;}
            bounds=bounds.translate(delta);changed=true;
        }
        let color=Color32::from_rgb(group.color[0],group.color[1],group.color[2]);
        let painter=ui.painter().with_clip_rect(canvas);
        painter.rect_filled(bounds,6.0,color.gamma_multiply(0.15));
        painter.rect_stroke(bounds,6.0,Stroke::new(1.5,color),StrokeKind::Inside);
        let header=Rect::from_min_size(bounds.min,Vec2::new(bounds.width(),28.0));
        painter.rect_filled(header,4.0,color);
        painter.text(header.left_center()+Vec2::new(8.0,0.0),egui::Align2::LEFT_CENTER,&group.title,egui::FontId::proportional(14.0),Color32::WHITE);
        response.context_menu(|ui| {
            ui.label("Group title"); changed|=ui.add(egui::TextEdit::singleline(&mut group.title).char_limit(128)).changed();
            changed|=ui.color_edit_button_srgb(&mut group.color).changed();
            if ui.button("Add selected blocks").clicked() {
                for id in selected {if !group.nodes.contains(id) {group.nodes.push(*id);}}
                transfer=Some(group.id);
                changed=true;ui.close();
            }
            if ui.button("Remove selected blocks").clicked() {group.nodes.retain(|id|!selected.contains(id));changed=true;ui.close();}
            if ui.button("Ungroup (keep blocks)").clicked() {remove=Some(group.id);ui.close();}
        });
    }
    if let Some(id)=remove {graph.groups.retain(|g|g.id!=id);changed=true;}
    if let Some(id)=transfer {for group in &mut graph.groups {if group.id!=id {group.nodes.retain(|n|!selected.contains(n));}}}
    graph.groups.retain(|g|!g.nodes.is_empty());
    // A block belongs to one visual group. Adding to a group transfers membership.
    let mut assigned=BTreeSet::new();
    for g in graph.groups.iter_mut().rev() {g.nodes.retain(|id|assigned.insert(*id));}
    graph.groups.retain(|g|!g.nodes.is_empty());
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dragging_group_moves_members_together_and_keeps_wires() {
        let ctx=egui::Context::default();let mut graph=Graph::example(vec![1]);
        assert!(create(&mut graph,&BTreeSet::from([1,2])));
        assert!(!create(&mut graph,&BTreeSet::from([1,2])));
        let wires=graph.wires.clone();let before=graph.nodes.clone();
        let mut frame=|events| {
            let mut output=ctx.run_ui(egui::RawInput {screen_rect:Some(Rect::from_min_size(egui::Pos2::ZERO,Vec2::new(1000.0,600.0))),events,..Default::default()},|ui| {
                show(ui,&mut graph,Rect::from_min_size(egui::Pos2::ZERO,Vec2::new(1000.0,600.0)),&BTreeSet::new(),|n|Rect::from_min_size(egui::pos2(n.position[0],n.position[1]),Vec2::new(240.0,100.0)));
            });
            output.textures_delta.clear();
        };
        let start=egui::pos2(60.0,5.0);let finish=start+Vec2::new(30.0,40.0);
        let button=|pos,pressed|egui::Event::PointerButton {pos,pressed,button:egui::PointerButton::Primary,modifiers:egui::Modifiers::default()};
        frame(vec![]);frame(vec![egui::Event::PointerMoved(start),button(start,true)]);
        frame(vec![egui::Event::PointerMoved(finish)]);frame(vec![button(finish,false)]);
        for index in [0,1] {assert_eq!(graph.nodes[index].position,[before[index].position[0]+30.0,before[index].position[1]+40.0]);}
        assert_eq!(graph.nodes[2],before[2]);assert_eq!(graph.wires,wires);
    }
}
