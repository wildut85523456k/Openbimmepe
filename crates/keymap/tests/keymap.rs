//! All shortcut values below are invented test data, not the shortcuts of any product.
use openbim_keymap::*;
use proptest::prelude::*;

fn km() -> Keymap {
    let mut k = Keymap::new(&["wall", "door", "window", "move", "undo", "zoom"]).unwrap();
    k.set_preset(&[("wall", &["Q W"]), ("door", &["Q D"]), ("window", &["Q N"]), ("move", &["M V"]), ("undo", &["Ctrl+Z"]), ("zoom", &["Z Z", "F8"])]).unwrap();
    k
}
fn ch(s: &str) -> Chord { Chord::parse(s).unwrap() }

#[test]
fn parsing_and_printing_keys() {
    assert_eq!(format_sequence(&parse_sequence("ctrl+shift+z").unwrap()), "Ctrl+Shift+Z");
    assert_eq!(format_sequence(&parse_sequence("q  w").unwrap()), "Q W");
    for bad in ["", "Ctrl+", "Ctrl+Ctrl+A", "Foo+A", "AB", "F0", "F25", "F01", "Q W E R T", "a+b", "é"] { assert!(parse_sequence(bad).is_err(), "{bad:?}"); }
    for ok in ["F1", "F24", "Escape", "Alt+Delete", "0", "Shift+Tab", "Q W E R"] { assert!(parse_sequence(ok).is_ok(), "{ok:?}"); }
    // typed-letter style
    assert_eq!(format_sequence(&parse_revit_token("wa").unwrap()), "W A");
    assert_eq!(format_sequence(&parse_revit_token("F8").unwrap()), "F8");
    assert_eq!(format_sequence(&parse_revit_token("Ctrl+Z").unwrap()), "Ctrl+Z");
    assert_eq!(format_sequence(&parse_revit_token("g").unwrap()), "G");
    assert!(parse_revit_token("abcde").is_err() && parse_revit_token("a-b").is_err() && parse_revit_token(" ").is_err());
}

#[test]
fn preset_lookup_and_settings_changes() {
    let mut k = km();
    assert_eq!(k.shortcuts("zoom"), vec!["Z Z", "F8"]);
    assert_eq!(k.command_for("Q W"), Some("wall"));
    assert_eq!(k.command_for("q w"), Some("wall"));
    assert_eq!(k.command_for("Q"), None);
    // change one
    k.set("wall", &["W A"]).unwrap();
    assert_eq!(k.command_for("Q W"), None);
    assert_eq!(k.command_for("W A"), Some("wall"));
    assert!(k.is_customised("wall") && !k.is_customised("door"));
    // taken: refused, nothing changes
    assert_eq!(k.set("door", &["W A"]), Err(KeymapError::Conflict { shortcut: "W A".into(), with_command: "wall".into() }));
    assert_eq!(k.shortcuts("door"), vec!["Q D"]);
    // a beginning of another shortcut, and an extension of one, are also refused
    assert!(matches!(k.set("door", &["W"]), Err(KeymapError::Conflict { .. })));
    assert!(matches!(k.set("door", &["W A B"]), Err(KeymapError::Conflict { .. })));
    // own shortcuts may be reused and rearranged
    k.set("wall", &["W A", "W B"]).unwrap();
    k.set("wall", &["W B"]).unwrap();
    assert!(matches!(k.set("wall", &["W", "W A"]), Err(KeymapError::SelfConflict(_))));
    assert!(matches!(k.set("nope", &["X"]), Err(KeymapError::UnknownCommand(_))));
    assert!(matches!(k.set("wall", &["Q W E R T"]), Err(KeymapError::TooLong)));
    assert!(matches!(k.set("wall", &[""]), Err(KeymapError::EmptySequence)));
    // removing: the shortcut becomes free
    k.set("window", &[]).unwrap();
    assert_eq!(k.shortcuts("window"), Vec::<String>::new());
    k.set("door", &["Q N"]).unwrap();
    assert_eq!(k.command_for("Q N"), Some("door"));
    // reset: refused while the preset shortcut is taken by someone else, allowed after
    assert!(matches!(k.reset("window"), Err(KeymapError::Conflict { .. })));
    k.reset("door").unwrap();
    k.reset("window").unwrap();
    assert_eq!(k.shortcuts("window"), vec!["Q N"]);
    k.reset_all();
    assert_eq!(k.shortcuts("wall"), vec!["Q W"]);
    assert!(!k.is_customised("wall"));
}

#[test]
fn reassign_takes_the_shortcut_from_its_owner() {
    let mut k = km();
    let stripped = k.reassign("door", "Q W").unwrap();
    assert_eq!(stripped, vec!["wall"]);
    assert_eq!(k.command_for("Q W"), Some("door"));
    assert_eq!(k.shortcuts("wall"), Vec::<String>::new());
    assert_eq!(k.shortcuts("door"), vec!["Q D", "Q W"]);
    // a beginning of other shortcuts takes them all
    let mut k = km();
    let mut s = k.reassign("move", "Q").unwrap(); s.sort();
    assert_eq!(s, vec!["door", "wall", "window"]);
    assert_eq!(k.shortcuts("move"), vec!["M V", "Q"]);
    k.validate().unwrap();
    // a failed reassign changes nothing
    let mut k = km();
    assert!(k.reassign("nope", "X").is_err() && k.reassign("door", "bad key").is_err());
    assert_eq!(k.shortcuts("wall"), vec!["Q W"]);
}

#[test]
fn typing_sequences() {
    let k = km();
    let mut m = k.matcher();
    assert_eq!(m.feed(ch("Q")), Match::Pending);
    assert_eq!(m.feed(ch("W")), Match::Command("wall".into()));
    assert_eq!(m.feed(ch("Q")), Match::Pending);
    assert_eq!(m.feed(ch("X")), Match::None);                      // wrong second key: start over
    assert_eq!(m.feed(ch("Q")), Match::Pending);
    m.cancel();
    assert_eq!(m.feed(ch("D")), Match::None);                      // no shortcut starts with D
    assert_eq!(m.feed(ch("Ctrl+Z")), Match::Command("undo".into()));
    assert_eq!(m.feed(ch("F8")), Match::Command("zoom".into()));
    assert_eq!(m.feed(ch("Z")), Match::Pending);
    assert_eq!(m.feed(ch("Z")), Match::Command("zoom".into()));
}

#[test]
fn settings_file_round_trip_and_bad_files_change_nothing() {
    let mut k = km();
    k.set("wall", &["W A"]).unwrap();
    k.set("window", &[]).unwrap();
    k.set("zoom", &["Z Z", "Shift+F8"]).unwrap();
    let text = k.save_overrides();
    let mut k2 = km();
    k2.load_overrides(&text).unwrap();
    for c in ["wall", "door", "window", "move", "undo", "zoom"] { assert_eq!(k.shortcuts(c), k2.shortcuts(c), "{c}"); }
    assert_eq!(k2.save_overrides(), text);
    for bad in ["wall W A\n", "nope\tX\n", "wall\tW A\nwall\tW B\n", "wall\tQ D\n", "wall\tW A|W\n", "wall\t\u{0}\n"] {
        let mut k3 = km();
        assert!(k3.load_overrides(bad).is_err(), "{bad:?}");
        assert_eq!(k3.shortcuts("wall"), vec!["Q W"], "{bad:?} must leave the map alone");
        assert!(!k3.is_customised("wall"));
    }
    // a file that frees a preset shortcut and gives it away in the same file is fine in any line order
    let mut k4 = km();
    k4.load_overrides("door\tQ W\nwall\tW A\n").unwrap();
    assert_eq!(k4.command_for("Q W"), Some("door"));
    assert!(km().load_overrides(&"x".repeat(MAX_TEXT + 1)).is_err());
}

#[test]
fn a_bad_preset_is_refused_whole() {
    let mut k = Keymap::new(&["a", "b"]).unwrap();
    assert!(matches!(k.set_preset(&[("a", &["X"]), ("b", &["X"])]), Err(KeymapError::Conflict { .. })));
    assert!(matches!(k.set_preset(&[("a", &["X"]), ("b", &["X Y"])]), Err(KeymapError::Conflict { .. })));
    assert!(matches!(k.set_preset(&[("a", &["X"]), ("c", &["Y"])]), Err(KeymapError::UnknownCommand(_))));
    assert!(matches!(k.set_preset(&[("a", &["X"]), ("a", &["Y"])]), Err(KeymapError::SelfConflict(_))));
    assert_eq!(k.shortcuts("a"), Vec::<String>::new());
    // a new preset that clashes with a user's override is refused, nothing changes
    k.set_preset(&[("a", &["X"])]).unwrap();
    k.set("b", &["Y"]).unwrap();
    assert!(k.set_preset(&[("a", &["Y"])]).is_err());
    assert_eq!(k.shortcuts("a"), vec!["X"]);
}

const SAMPLE_XML: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<Shortcuts>
  <ShortcutItem CommandName="Wall" CommandId="ID_WALL" Shortcuts="QW" Paths="A>B" />
  <ShortcutItem CommandName="Door &amp; frame" CommandId="ID_DOOR" Shortcuts="QD#Ctrl+D" Paths="A>B" />
  <ShortcutItem CommandName="Undo" CommandId="ID_UNDO" Shortcuts="Ctrl+Z" />
  <ShortcutItem CommandName="No keys" CommandId="ID_WALL2" Shortcuts="" />
  <ShortcutItem CommandName="Unknown" CommandId="ID_OTHER" Shortcuts="ZZ" />
  <ShortcutItem CommandName="Bad" CommandId="ID_MOVE" Shortcuts="MV#abcdefg#M-V" />
  <ShortcutItem CommandId="ID_ZOOM" Shortcuts="F8" />
</Shortcuts>"#;

#[test]
fn reading_a_revit_style_shortcut_file() {
    let map = |id: &str| match id { "ID_WALL" => Some("wall"), "ID_DOOR" => Some("door"), "ID_UNDO" => Some("undo"), "ID_MOVE" => Some("move"), "ID_ZOOM" => Some("zoom"), _ => None }.map(String::from);
    let (entries, rep) = Keymap::parse_revit_xml(SAMPLE_XML, &map).unwrap();
    assert_eq!(entries, vec![
        ("wall".to_string(), vec!["Q W".to_string()]), ("door".to_string(), vec!["Q D".to_string(), "Ctrl+D".to_string()]),
        ("undo".to_string(), vec!["Ctrl+Z".to_string()]), ("move".to_string(), vec!["M V".to_string()]), ("zoom".to_string(), vec!["F8".to_string()])]);
    assert_eq!(rep.bound, 5);
    assert_eq!(rep.skipped.len(), 3, "{:?}", rep.skipped);                    // unknown command + two bad tokens
    assert!(rep.skipped.iter().any(|s| s.contains("Unknown")) && rep.skipped.iter().filter(|s| s.starts_with("Bad")).count() == 2);
    // and it loads as a preset
    let mut k = Keymap::new(&["wall", "door", "undo", "move", "zoom"]).unwrap();
    let pairs: Vec<(&str, Vec<&str>)> = entries.iter().map(|(c, v)| (c.as_str(), v.iter().map(|s| s.as_str()).collect())).collect();
    let refs: Vec<(&str, &[&str])> = pairs.iter().map(|(c, v)| (*c, v.as_slice())).collect();
    k.set_preset(&refs).unwrap();
    assert_eq!(k.command_for("Q D"), Some("door"));
    // hostile input: no panic
    for t in ["", "<ShortcutItem", "<ShortcutItem Shortcuts=\"", "<ShortcutItem CommandId=\"a\" Shortcuts=\"\u{0}\"", "<ShortcutItem XCommandId=\"ID_WALL\" Shortcuts=\"QW\">"] {
        let (e, _) = Keymap::parse_revit_xml(t, &map).unwrap(); assert!(e.is_empty(), "{t:?}");
    }
    assert!(Keymap::parse_revit_xml(&"x".repeat(MAX_TEXT + 1), &map).is_err());
}

proptest! {
    // whatever the user does in the settings menu, the map stays free of clashes, and the matcher agrees with lookup
    #[test]
    fn random_settings_keep_the_map_valid(ops in proptest::collection::vec((0usize..5, 0usize..6, 0usize..8, any::<bool>()), 1..60)) {
        let cmds = ["a", "b", "c", "d", "e"];
        let keys = ["Q", "Q W", "Q W E", "W", "W Q", "Ctrl+Q", "F2", "Z Z"];
        let mut k = Keymap::new(&cmds).unwrap();
        for (c, mode, key, flag) in ops {
            let _ = match mode {
                0 | 1 => k.set(cmds[c], &[keys[key]]),
                2 => k.reassign(cmds[c], keys[key]).map(|_| ()),
                3 => k.reset(cmds[c]),
                4 => k.set(cmds[c], &[]),
                _ => { if flag { k.reset_all(); } Ok(()) }
            };
            prop_assert!(k.validate().is_ok());
        }
        // lookup and matcher agree for every shortcut in force
        for c in cmds {
            for s in k.shortcuts(c) {
                prop_assert_eq!(k.command_for(&s), Some(c));
                let mut m = k.matcher();
                let chords: Vec<Chord> = parse_sequence(&s).unwrap();
                let mut last = Match::None;
                for ch in chords { last = m.feed(ch); }
                prop_assert_eq!(last, Match::Command(c.to_string()));
            }
        }
        // saving and loading reproduces the map
        let mut k2 = Keymap::new(&cmds).unwrap();
        k2.load_overrides(&k.save_overrides()).unwrap();
        // the preset is empty here, so overrides alone describe the map
        for c in cmds { prop_assert_eq!(k.shortcuts(c), k2.shortcuts(c)); }
    }
}

#[test]
fn the_revit_style_preset_file_loads_without_clashes() {
    let text = include_str!("../presets/revit-style.txt");
    let entries = parse_preset_text(text).unwrap();
    assert!(entries.len() >= 60, "{}", entries.len());
    let ids: Vec<&str> = entries.iter().map(|(c, _)| c.as_str()).collect();
    let mut k = Keymap::new(&ids).unwrap();
    let refs: Vec<(&str, Vec<&str>)> = entries.iter().map(|(c, v)| (c.as_str(), v.iter().map(|s| s.as_str()).collect())).collect();
    let refs2: Vec<(&str, &[&str])> = refs.iter().map(|(c, v)| (*c, v.as_slice())).collect();
    k.set_preset(&refs2).unwrap();
    // spot checks against the source page, written by hand
    for (keys, cmd) in [("W A", "create.wall"), ("D R", "create.door"), ("W N", "create.window"), ("M V", "modify.move"), ("C O", "modify.copy"), ("C C", "modify.copy"),
                        ("V V", "view.visibility-graphics"), ("V G", "view.visibility-graphics"), ("Z E", "nav.zoom-to-fit"), ("Z Z", "nav.zoom-in-region"), ("3 W", "nav.walk-mode"), ("S W", "snap.snap-work-plane-grid"), ("K S", "view.keyboard-shortcuts")] {
        assert_eq!(k.command_for(keys), Some(cmd), "{keys}");
    }
    // a typed sequence runs the command on its last key
    let mut m = k.matcher();
    assert_eq!(m.feed(ch("W")), Match::Pending);
    assert_eq!(m.feed(ch("A")), Match::Command("create.wall".into()));
    // the settings menu can still change it
    k.set("create.wall", &["Q W"]).unwrap();
    assert_eq!(k.command_for("W A"), None);
    assert!(parse_preset_text("onlyid\n").is_err() && parse_preset_text("a\tabcdef\n").is_err());
}
