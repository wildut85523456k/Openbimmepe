//! Systems of equipment, as Revit's Create Mechanical / Piping System, Edit System and the System Browser have them: a
//! named group of family instances of one system type, with the member the layout starts from. Each function gives the
//! commands of one undoable step; the model is not changed until they are applied.
use crate::mep::{MepSystemDef, MAX_SYSTEM_MEMBERS};
use crate::{Category, Command, Document, ElementId};
use openbim_family::mep::System;

impl Document {
    fn system_index(&self, name: &str) -> Result<usize, String> {
        self.mep().systems.iter().position(|d| d.name == name).ok_or_else(|| format!("there is no system \"{name}\""))
    }

    /// The name a new system of this type would get: the type and the first number not in use.
    pub fn default_system_name(&self, system: System) -> String {
        (1..).map(|n| format!("{} {n}", system.name())).find(|n| self.mep().systems.iter().all(|d| &d.name != n)).unwrap_or_default()
    }

    /// Create Mechanical / Piping System: `members` (family instances) become a system of `system`, called `name` (the
    /// type and a number when empty). The base is left to the layout to choose.
    pub fn create_system_commands(&self, name: &str, system: System, members: &[ElementId]) -> Result<Vec<Command>, String> {
        if !(system.domain_ok(openbim_family::mep::Domain::Duct) || system.domain_ok(openbim_family::mep::Domain::Pipe)) || matches!(system, System::Other | System::Power | System::Data) {
            return Err("a system is for ducts or pipes: choose an air, water or other piping type".into());
        }
        let mut list: Vec<ElementId> = Vec::new();
        for m in members { if !list.contains(m) { list.push(*m); } }
        if list.is_empty() { return Err("select the equipment first: a system needs at least one piece".into()); }
        if list.len() > MAX_SYSTEM_MEMBERS { return Err(format!("a system has at most {MAX_SYSTEM_MEMBERS} members")); }
        for m in &list {
            let e = self.get(*m).filter(|e| e.category == Category::Family).ok_or("only equipment (family instances) can be in a system")?;
            if let Some(d) = self.mep().systems.iter().find(|d| d.system == system && d.members.contains(m)) { return Err(format!("{} is in the system \"{}\" of that type already", e.name, d.name)); }
        }
        let name = if name.trim().is_empty() { self.default_system_name(system) } else { name.trim().to_string() };
        if name.chars().count() > 60 { return Err("the name is longer than 60 characters".into()); }
        if self.mep().systems.iter().any(|d| d.name == name) { return Err(format!("there is a system called \"{name}\" already")); }
        let mut data = self.mep().clone();
        data.systems.push(MepSystemDef { name, system, members: list, base: None });
        Ok(vec![Command::SetMep(Box::new(data))])
    }

    /// Edit System: add `add` to the system and take `remove` out of it. The last member cannot be removed (delete the
    /// system instead), and the base stays unless it is removed.
    pub fn edit_system_commands(&self, name: &str, add: &[ElementId], remove: &[ElementId]) -> Result<Vec<Command>, String> {
        let at = self.system_index(name)?;
        let mut data = self.mep().clone();
        let d = &mut data.systems[at];
        for m in add {
            let e = self.get(*m).filter(|e| e.category == Category::Family).ok_or("only equipment (family instances) can be in a system")?;
            if let Some(o) = self.mep().systems.iter().find(|o| o.system == d.system && o.members.contains(m) && o.name != name) { return Err(format!("{} is in the system \"{}\" of that type already", e.name, o.name)); }
            if !d.members.contains(m) { d.members.push(*m); }
        }
        d.members.retain(|m| !remove.contains(m));
        if d.members.is_empty() { return Err("a system keeps at least one member: delete the system instead".into()); }
        if d.members.len() > MAX_SYSTEM_MEMBERS { return Err(format!("a system has at most {MAX_SYSTEM_MEMBERS} members")); }
        if d.base.is_some_and(|b| !d.members.contains(&b)) { d.base = None; }
        if data == *self.mep() { return Err("nothing changes".into()); }
        Ok(vec![Command::SetMep(Box::new(data))])
    }

    /// Sets the equipment the layout starts from (None: the layout chooses).
    pub fn set_system_base_commands(&self, name: &str, base: Option<ElementId>) -> Result<Vec<Command>, String> {
        let at = self.system_index(name)?;
        if base.is_some_and(|b| !self.mep().systems[at].members.contains(&b)) { return Err("the base is one of the members".into()); }
        if self.mep().systems[at].base == base { return Err("nothing changes".into()); }
        let mut data = self.mep().clone();
        data.systems[at].base = base;
        Ok(vec![Command::SetMep(Box::new(data))])
    }

    pub fn rename_system_commands(&self, name: &str, new_name: &str) -> Result<Vec<Command>, String> {
        let at = self.system_index(name)?;
        let new = new_name.trim();
        if new.is_empty() || new.chars().count() > 60 { return Err("a name is 1 to 60 characters".into()); }
        if new == name { return Err("nothing changes".into()); }
        if self.mep().systems.iter().any(|d| d.name == new) { return Err(format!("there is a system called \"{new}\" already")); }
        let mut data = self.mep().clone();
        data.systems[at].name = new.to_string();
        Ok(vec![Command::SetMep(Box::new(data))])
    }

    /// Deletes the system; its equipment and the ducts and pipes stay.
    pub fn delete_system_commands(&self, name: &str) -> Result<Vec<Command>, String> {
        let at = self.system_index(name)?;
        let mut data = self.mep().clone();
        data.systems.remove(at);
        Ok(vec![Command::SetMep(Box::new(data))])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Document, Geometry};

    fn equipment(d: &mut Document, n: usize) -> Vec<ElementId> {
        let level = match d.get(ElementId(20)).unwrap().geometry { Some(Geometry::Linear { level, .. }) => level, _ => panic!() };
        (0..n).map(|i| {
            let spec = crate::FamilySpec { family: "Fan coil unit, ducted".into(), type_name: String::new(), overrides: vec![], angle_deg: 0.0, elevation_m: 0.0 };
            d.place_family(level, [20.0 + 3.0 * i as f64, 20.0], &spec).unwrap()
        }).collect()
    }
    fn run(d: &mut Document, cmds: Vec<Command>) { d.transact(d.revision(), &cmds).unwrap(); }

    #[test]
    fn a_system_groups_equipment_is_edited_renamed_and_deleted_each_in_one_undo_step() {
        let mut d = Document::spatial_sample();
        let fc = equipment(&mut d, 3);
        let c = d.create_system_commands("", System::ChilledWaterSupply, &fc[..2]).unwrap();
        run(&mut d, c);
        assert_eq!(d.mep().systems.len(), 1);
        assert_eq!(d.mep().systems[0].name, "Chilled water supply 1");
        // a second system of another type may hold the same equipment, one of the same type may not
        let c = d.create_system_commands("Supply", System::SupplyAir, &fc[..1]).unwrap();
        run(&mut d, c);
        let err = d.create_system_commands("Other", System::ChilledWaterSupply, &fc[1..2]).unwrap_err();
        assert!(err.contains("in the system \"Chilled water supply 1\""), "{err}");
        // names are unique, equipment only, and a system is for ducts or pipes
        assert!(d.create_system_commands("Supply", System::ExhaustAir, &fc[2..]).is_err());
        assert!(d.create_system_commands("W", System::ChilledWaterSupply, &[ElementId(20)]).is_err());
        assert!(d.create_system_commands("P", System::Power, &fc[2..]).is_err());
        assert!(d.create_system_commands("E", System::ExhaustAir, &[]).is_err());
        // edit: add the third, take the first out; the base follows its member
        let c = d.edit_system_commands("Chilled water supply 1", &fc[2..], &fc[..1]).unwrap();
        run(&mut d, c);
        assert_eq!(d.mep().systems[0].members, vec![fc[1], fc[2]]);
        let c = d.set_system_base_commands("Chilled water supply 1", Some(fc[1])).unwrap();
        run(&mut d, c);
        assert!(d.set_system_base_commands("Chilled water supply 1", Some(fc[0])).is_err());
        let c = d.edit_system_commands("Chilled water supply 1", &[], &fc[1..2]).unwrap();
        run(&mut d, c);
        assert_eq!(d.mep().systems[0].base, None, "the base went with its member");
        assert!(d.edit_system_commands("Chilled water supply 1", &[], &fc[2..]).unwrap_err().contains("at least one"));
        // rename
        let c = d.rename_system_commands("Chilled water supply 1", "CHWS-A").unwrap();
        run(&mut d, c);
        assert!(d.rename_system_commands("CHWS-A", "Supply").is_err());
        // each step undoes
        d.undo().unwrap();
        assert_eq!(d.mep().systems[0].name, "Chilled water supply 1");
        d.redo().unwrap();
        // deleting a piece of equipment takes it out of its systems; the system goes with its last member
        d.transact(d.revision(), &[Command::Delete { id: fc[2] }]).unwrap();
        assert!(d.mep().systems.iter().all(|s| s.name != "CHWS-A"), "{:?}", d.mep().systems);
        assert_eq!(d.mep().systems.len(), 1);
        let c = d.delete_system_commands("Supply").unwrap();
        run(&mut d, c);
        assert!(d.mep().systems.is_empty());
        assert!(d.delete_system_commands("Supply").is_err());
    }

    #[test]
    fn a_bad_system_is_refused_by_the_model_and_the_records_read_back() {
        let mut d = Document::spatial_sample();
        let fc = equipment(&mut d, 2);
        let c = d.create_system_commands("Loop, \"A\"; 1", System::HeatingWaterReturn, &fc).unwrap();
        run(&mut d, c);
        let c = d.set_system_base_commands("Loop, \"A\"; 1", Some(fc[1])).unwrap();
        run(&mut d, c);
        let back = crate::native::parse_mep(&crate::native::mep_records(d.mep())).unwrap();
        assert_eq!(back.systems, d.mep().systems);
        let mut bad = d.mep().clone();
        bad.systems[0].members.push(fc[0]);
        assert!(d.transact(d.revision(), &[Command::SetMep(Box::new(bad))]).is_err());
        let mut bad = d.mep().clone();
        bad.systems[0].base = Some(ElementId(20));
        // a base that is not a member is cleaned away, as a member that is gone is
        d.transact(d.revision(), &[Command::SetMep(Box::new(bad))]).unwrap();
        assert_eq!(d.mep().systems[0].base, None);
        let mut bad = d.mep().clone();
        bad.systems[0].name = " padded ".into();
        assert!(d.transact(d.revision(), &[Command::SetMep(Box::new(bad))]).is_err());
        let mut bad = d.mep().clone();
        bad.systems[0].system = System::Power;
        assert!(d.transact(d.revision(), &[Command::SetMep(Box::new(bad))]).is_err());
    }
}
