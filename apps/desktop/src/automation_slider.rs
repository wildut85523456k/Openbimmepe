use eframe::egui;
use openbim_automation::{NodeKind,Type};

pub fn show(ui:&mut egui::Ui,kind:&mut NodeKind)->bool {
    let NodeKind::Slider {value,min,max,step,quantity}=kind else {return false};
    let before=(*value,*min,*max,*step,*quantity);
    if min.is_finite() && max.is_finite() && min<max && step.is_finite() && *step>0.0 {
        ui.add(egui::Slider::new(value,*min..=*max).step_by(*step).suffix(if *quantity==Type::Length {" m"} else {""}));
    } else { ui.colored_label(egui::Color32::RED,"Set a valid Min / Max / Step"); }
    ui.collapsing("Min / Max / Step",|ui| {
        for (label,number) in [("Min",&mut *min),("Max",&mut *max),("Step",&mut *step)] {
            ui.horizontal(|ui| {ui.label(label);ui.add(egui::DragValue::new(number).speed(0.01));});
        }
        egui::ComboBox::from_id_salt("slider-quantity").selected_text(if *quantity==Type::Length {"Length (m)"} else {"Number / ratio"}).show_ui(ui,|ui| {
            ui.selectable_value(quantity,Type::Length,"Length (m)");
            ui.selectable_value(quantity,Type::Ratio,"Number / ratio");
        });
    });
    if min.is_finite() && max.is_finite() && min<max { *value=value.clamp(*min,*max); }
    before!=(*value,*min,*max,*step,*quantity)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pointer_changes_slider_value() {
        let ctx=egui::Context::default();let mut kind=NodeKind::Slider {value:0.0,min:0.0,max:1.0,step:0.1,quantity:Type::Ratio};
        let mut frame=|events| {
            let mut output=ctx.run_ui(egui::RawInput {screen_rect:Some(egui::Rect::from_min_size(egui::Pos2::ZERO,egui::vec2(400.0,300.0))),events,..Default::default()},|ui| {show(ui,&mut kind);});
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
