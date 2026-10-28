use eframe::egui;
use openbim_automation::{NodeKind,Type};

pub fn expanded(ctx:&egui::Context,id:u64)->bool {
    ctx.data(|d|d.get_temp::<bool>(egui::Id::unique(("slider-range",id))).unwrap_or(false))
}

pub fn show(ui:&mut egui::Ui,kind:&mut NodeKind,id:u64)->bool {
    let NodeKind::Slider {value,min,max,step,quantity}=kind else {return false};
    let before=(*value,*min,*max,*step,*quantity);
    let mut open=expanded(ui.ctx(),id);
    ui.horizontal(|ui| {
        if ui.small_button("?").on_hover_text("Edit Min / Max / Step").clicked() {
            open = !open;
            ui.ctx().data_mut(|d|d.insert_temp(egui::Id::unique(("slider-range",id)),open));
            ui.ctx().request_repaint();
        }
        if min.is_finite() && max.is_finite() && min<max && step.is_finite() && *step>0.0 {
            let (rect,response)=ui.allocate_exact_size(egui::vec2(112.0,22.0),egui::Sense::click_and_drag());
            let left=rect.left()+5.0;let right=rect.right()-5.0;
            if response.clicked() || response.dragged() {
                response.request_focus();
                if let Some(pointer)=response.interact_pointer_pos() {
                    let t=((pointer.x-left)/(right-left)).clamp(0.0,1.0) as f64;
                    *value=(*min+((t*(*max-*min))/ *step).round()* *step).clamp(*min,*max);
                }
            }
            if response.has_focus() {
                let direction=ui.input(|i| {
                    i32::from(i.key_pressed(egui::Key::ArrowRight) || i.key_pressed(egui::Key::ArrowUp))-
                    i32::from(i.key_pressed(egui::Key::ArrowLeft) || i.key_pressed(egui::Key::ArrowDown))
                });
                *value=(*value+f64::from(direction)* *step).clamp(*min,*max);
                if ui.input(|i|i.key_pressed(egui::Key::Home)) { *value = *min; }
                if ui.input(|i|i.key_pressed(egui::Key::End)) { *value = *max; }
            }
            let dark=ui.visuals().dark_mode;
            let painter=ui.painter();
            let rail=egui::Rect::from_min_max(egui::pos2(left,rect.center().y-2.0),egui::pos2(right,rect.center().y+2.0));
            painter.rect_filled(rail,2.0,egui::Color32::from_gray(if dark {150} else {75}));
            let t=((*value-*min)/(*max-*min)).clamp(0.0,1.0) as f32;
            let thumb=egui::Rect::from_center_size(egui::pos2(left+t*(right-left),rect.center().y),egui::vec2(9.0,18.0));
            painter.rect_filled(thumb,1.0,egui::Color32::from_gray(if response.dragged() {15} else {40}));
            painter.rect_stroke(thumb,1.0,egui::Stroke::new(if response.has_focus() {2.0} else {1.0},if dark {egui::Color32::from_gray(230)} else {egui::Color32::BLACK}),egui::StrokeKind::Inside);
            response.on_hover_cursor(egui::CursorIcon::ResizeHorizontal).on_hover_text("Drag to set value; arrow keys step; Home/End set limits");
            ui.add(egui::DragValue::new(value).range(*min..=*max).speed(*step).suffix(if *quantity==Type::Length {" m"} else {""}));
        } else { ui.colored_label(egui::Color32::RED,"Invalid range"); }
    });
    if open {
        for (label,number) in [("Min",&mut *min),("Max",&mut *max),("Step",&mut *step)] {
            ui.horizontal(|ui| {ui.label(label);ui.add(egui::DragValue::new(number).speed(0.01));});
        }
        egui::ComboBox::from_id_salt("slider-quantity").selected_text(if *quantity==Type::Length {"Length (m)"} else {"Number / ratio"}).show_ui(ui,|ui| {
            ui.selectable_value(quantity,Type::Length,"Length (m)");
            ui.selectable_value(quantity,Type::Ratio,"Number / ratio");
        });
    }
    if min.is_finite() && max.is_finite() && min<max { *value=value.clamp(*min,*max); }
    before!=(*value,*min,*max,*step,*quantity)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn caret_expands_range_without_changing_the_input() {
        let ctx=egui::Context::default();let mut kind=NodeKind::Slider {value:0.5,min:0.0,max:1.0,step:0.1,quantity:Type::Ratio};
        let mut frame=|events| {
            let mut output=ctx.run_ui(egui::RawInput {screen_rect:Some(egui::Rect::from_min_size(egui::Pos2::ZERO,egui::vec2(400.0,300.0))),events,..Default::default()},|ui| {show(ui,&mut kind,1);});
            output.textures_delta.clear();
        };
        let position=egui::pos2(14.0,16.0);
        frame(vec![]);
        for pressed in [true,false] {frame(vec![egui::Event::PointerMoved(position),egui::Event::PointerButton {pos:position,pressed,button:egui::PointerButton::Primary,modifiers:egui::Modifiers::default()}]);}
        assert!(expanded(&ctx,1));
        let NodeKind::Slider {value,..}=kind else {unreachable!()};assert_eq!(value,0.5);
    }
    #[test]
    fn pointer_changes_slider_value() {
        let ctx=egui::Context::default();let mut kind=NodeKind::Slider {value:0.0,min:0.0,max:1.0,step:0.1,quantity:Type::Ratio};
        let mut frame=|events| {
            let mut output=ctx.run_ui(egui::RawInput {screen_rect:Some(egui::Rect::from_min_size(egui::Pos2::ZERO,egui::vec2(400.0,300.0))),events,..Default::default()},|ui| {show(ui,&mut kind,1);});
            output.textures_delta.clear();
        };
        let position=egui::pos2(85.0,16.0);
        frame(vec![]);frame(vec![egui::Event::PointerMoved(position),egui::Event::PointerButton {pos:position,pressed:true,button:egui::PointerButton::Primary,modifiers:egui::Modifiers::default()}]);
        frame(vec![egui::Event::PointerButton {pos:position,pressed:false,button:egui::PointerButton::Primary,modifiers:egui::Modifiers::default()}]);
        let NodeKind::Slider {value,..}=kind else {unreachable!()};
        assert!(value>0.0 && value<=1.0,"Pointer must change the input value");
        assert!((value*10.0-(value*10.0).round()).abs()<1e-8);
    }
}
