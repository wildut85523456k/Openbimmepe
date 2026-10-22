//! The command registry: the single source the ribbon, keymap, graph node set and
//! MCP tools are generated from.
#![forbid(unsafe_code)]

use super::ParameterId;

/// What a command's value is: a quantity, a text, a flag, or an element reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueKind {
    /// A [`Quantity`](super::Quantity); its dimension is the parameter's.
    Quantity,
    Text,
    Flag,
    /// One element.
    Element,
    /// A selection of elements.
    Elements,
}

/// One typed input of a command, named by a stable parameter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InputSpec {
    pub parameter: ParameterId,
    pub kind: ValueKind,
    /// The input may be left empty, taking the parameter's default.
    pub optional: bool,
    /// The input is a list: a run, a selection.
    pub list: bool,
}

impl InputSpec {
    pub const fn new(parameter: ParameterId, kind: ValueKind) -> InputSpec {
        InputSpec {
            parameter,
            kind,
            optional: false,
            list: false,
        }
    }

    /// The input may be left empty.
    pub const fn optional(mut self) -> InputSpec {
        self.optional = true;
        self
    }

    /// The input is a list.
    pub const fn list(mut self) -> InputSpec {
        self.list = true;
        self
    }
}

/// A registered command: its identity, labels, icon, tab and group, description and
/// typed inputs. `writes` says whether it changes the model or only reads it.
///
/// The ribbon, keymap, graph node set and MCP tools are generated from these
/// (04-visual-automation.md, rule 9).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommandDef {
    pub id: &'static str,
    pub en: &'static str,
    pub fr: &'static str,
    /// Icon identifier, resolved by the client's icon set.
    pub icon: &'static str,
    /// Ribbon tab.
    pub tab: &'static str,
    /// Ribbon group within the tab.
    pub group: &'static str,
    pub description: &'static str,
    pub inputs: &'static [InputSpec],
    pub writes: bool,
}

impl CommandDef {
    /// A command definition: identity, labels, icon, tab and group, description and
    /// typed inputs. Nine fields, so the lint is allowed rather than a builder.
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        id: &'static str,
        en: &'static str,
        fr: &'static str,
        icon: &'static str,
        tab: &'static str,
        group: &'static str,
        description: &'static str,
        inputs: &'static [InputSpec],
        writes: bool,
    ) -> CommandDef {
        CommandDef {
            id,
            en,
            fr,
            icon,
            tab,
            group,
            description,
            inputs,
            writes,
        }
    }

    /// The English label, for a client that has no translation.
    pub fn label(self) -> &'static str {
        self.en
    }
}

/// The registered commands.
pub static BUILTIN: &[CommandDef] = &[
    // Manage tab: the automation commands, AUTO-001's own scope.
    CommandDef::new(
        "automation_editor",
        "Graph Editor",
        "Éditeur de graphe",
        "automation_editor",
        "Manage",
        "Automation",
        "Open OpenBIM Graph, the Dynamo-like graph editor.",
        &[],
        false,
    ),
    CommandDef::new(
        "automation_player",
        "Player",
        "Lecteur",
        "automation_player",
        "Manage",
        "Automation",
        "Run a saved graph without opening the canvas.",
        &[],
        false,
    ),
    CommandDef::new(
        "automation_mcp",
        "MCP Setup",
        "Configuration MCP",
        "automation_mcp",
        "Manage",
        "AI Integration",
        "Set up the MCP server: stdio, or local HTTP with a token.",
        &[],
        false,
    ),
    CommandDef::new(
        "automation_snapshot",
        "Export Snapshot",
        "Exporter l'instantané",
        "automation_snapshot",
        "Manage",
        "AI Integration",
        "Export a read-only snapshot of the open model.",
        &[],
        false,
    ),
    // Systems tab: the duct and pipe commands, with their quantity inputs.
    CommandDef::new(
        "duct",
        "Duct",
        "Conduit d'air",
        "duct",
        "Systems",
        "HVAC",
        "Place a duct run: size, height and air velocity.",
        &[
            InputSpec::new(ParameterId::new("duct.width"), ValueKind::Quantity),
            InputSpec::new(ParameterId::new("duct.height"), ValueKind::Quantity),
            InputSpec::new(ParameterId::new("duct.velocity"), ValueKind::Quantity),
        ],
        true,
    ),
    CommandDef::new(
        "pipe",
        "Pipe",
        "Conduite",
        "pipe",
        "Systems",
        "Plumbing & Piping",
        "Place a pipe run: diameter and flow.",
        &[
            InputSpec::new(ParameterId::new("pipe.diameter"), ValueKind::Quantity),
            InputSpec::new(ParameterId::new("pipe.flow"), ValueKind::Quantity),
        ],
        true,
    ),
    // Architecture tab.
    CommandDef::new(
        "wall",
        "Wall",
        "Mur",
        "wall",
        "Architecture",
        "Build",
        "Draw a wall: thickness and height.",
        &[
            InputSpec::new(ParameterId::new("wall.thickness"), ValueKind::Quantity),
            InputSpec::new(ParameterId::new("element.height"), ValueKind::Quantity),
            InputSpec::new(ParameterId::new("element.elevation"), ValueKind::Quantity),
        ],
        true,
    ),
    CommandDef::new(
        "door",
        "Door",
        "Porte",
        "door",
        "Architecture",
        "Build",
        "Place a door in a wall: width and height.",
        &[
            InputSpec::new(ParameterId::new("element.width"), ValueKind::Quantity),
            InputSpec::new(ParameterId::new("element.height"), ValueKind::Quantity),
        ],
        true,
    ),
    CommandDef::new(
        "window",
        "Window",
        "Fenêtre",
        "window",
        "Architecture",
        "Build",
        "Place a window in a wall: width and height.",
        &[
            InputSpec::new(ParameterId::new("element.width"), ValueKind::Quantity),
            InputSpec::new(ParameterId::new("element.height"), ValueKind::Quantity),
        ],
        true,
    ),
    CommandDef::new(
        "level",
        "Level",
        "Niveau",
        "level",
        "Architecture",
        "Datum",
        "Add a level at an elevation.",
        &[InputSpec::new(
            ParameterId::new("element.elevation"),
            ValueKind::Quantity,
        )],
        true,
    ),
    CommandDef::new(
        "slab",
        "Floor",
        "Dalle",
        "slab",
        "Architecture",
        "Build",
        "Lay a slab: thickness and top offset.",
        &[
            InputSpec::new(ParameterId::new("element.height"), ValueKind::Quantity),
            InputSpec::new(ParameterId::new("element.elevation"), ValueKind::Quantity),
        ],
        true,
    ),
    // Structure tab.
    CommandDef::new(
        "beam",
        "Beam",
        "Poutre",
        "beam",
        "Structure",
        "Structure",
        "Place a beam: depth, width, elevation and angle.",
        &[
            InputSpec::new(ParameterId::new("element.height"), ValueKind::Quantity),
            InputSpec::new(ParameterId::new("element.width"), ValueKind::Quantity),
            InputSpec::new(ParameterId::new("element.elevation"), ValueKind::Quantity),
            InputSpec::new(ParameterId::new("element.angle"), ValueKind::Quantity),
        ],
        true,
    ),
    CommandDef::new(
        "column",
        "Column",
        "Poteau",
        "column",
        "Structure",
        "Structure",
        "Place a column: width and depth.",
        &[
            InputSpec::new(ParameterId::new("element.width"), ValueKind::Quantity),
            InputSpec::new(ParameterId::new("element.height"), ValueKind::Quantity),
        ],
        true,
    ),
];

/// The command with this ID, if it is registered.
pub fn lookup(id: &str) -> Option<&'static CommandDef> {
    BUILTIN.iter().find(|c| c.id == id)
}

/// Every registered command of a tab, in registration order.
pub fn tab(name: &str) -> Vec<&'static CommandDef> {
    BUILTIN.iter().filter(|c| c.tab == name).collect()
}

/// The distinct groups of a tab, in the order they first appear.
pub fn groups(name: &str) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for c in tab(name) {
        if !out.contains(&c.group) {
            out.push(c.group);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_registry_has_unique_ids_and_known_parameters() {
        let params = super::super::parameter::builtin();
        let mut ids = std::collections::BTreeSet::new();
        for c in BUILTIN {
            assert!(ids.insert(c.id), "duplicate command ID {}", c.id);
            assert!(
                !c.en.is_empty() && !c.fr.is_empty(),
                "{} needs both labels",
                c.id
            );
            for input in c.inputs {
                assert!(
                    params.find(input.parameter.as_str()).is_some(),
                    "{} names an unknown parameter {}",
                    c.id,
                    input.parameter
                );
            }
        }
    }

    #[test]
    fn commands_are_found_by_id_and_listed_by_tab() {
        assert_eq!(lookup("automation_mcp").unwrap().en, "MCP Setup");
        assert_eq!(lookup("duct").unwrap().fr, "Conduit d'air");
        assert!(lookup("no_such_command").is_none());
        assert_eq!(groups("Manage"), vec!["Automation", "AI Integration"]);
        assert_eq!(tab("Systems").len(), 2);
    }

    #[test]
    fn a_command_declares_whether_it_writes_the_model() {
        assert!(
            !lookup("automation_editor").unwrap().writes,
            "the editor does not write the model"
        );
        assert!(
            lookup("wall").unwrap().writes,
            "a wall command writes the model"
        );
    }
}
