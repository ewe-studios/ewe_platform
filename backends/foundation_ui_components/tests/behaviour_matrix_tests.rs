//! Compare the shipped component matrix with the Rust declarations and wire vocabulary.

use foundation_ui_components::behaviour::{Behaviour, BehaviourWire, COMPONENT_BEHAVIOURS};
use std::collections::{BTreeMap, BTreeSet};

#[test]
fn every_behaviour_and_component_matches_the_shipped_matrix() {
    let matrix: toml::Table = toml::from_str(include_str!("../behaviours.toml")).unwrap();
    let rows = matrix["behaviours"].as_table().unwrap();
    let names: BTreeSet<_> = Behaviour::ALL.iter().map(|b| b.name()).collect();
    assert_eq!(names, rows.keys().map(String::as_str).collect());
    for behaviour in Behaviour::ALL {
        let name = behaviour.name();
        let row = &rows[name];
        assert_eq!(Behaviour::from_name(name), Some(*behaviour));
        match behaviour.wire() {
            BehaviourWire::Shared(id) => {
                assert_eq!(row["id"].as_integer(), Some(i64::from(id.0)));
                assert_eq!(row["shared"].as_str(), id.variant_name());
            }
            BehaviourWire::Web(custom) => {
                assert_eq!(row["custom"].as_str(), Some(custom));
                assert!(foundation_ui_wire::vocab::is_web_behaviour(custom));
            }
        }
    }
    let expected: BTreeMap<_, Vec<_>> = COMPONENT_BEHAVIOURS
        .iter()
        .map(|(name, behaviours)| (*name, behaviours.iter().map(|b| b.name()).collect()))
        .collect();
    let actual: BTreeMap<_, Vec<_>> = matrix["components"]
        .as_table()
        .unwrap()
        .iter()
        .map(|(name, row)| {
            (
                name.as_str(),
                row["behaviours"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|b| b.as_str().unwrap())
                    .collect(),
            )
        })
        .collect();
    assert_eq!(expected, actual);
}

#[test]
fn component_lists_match_machinery_calls() {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut actual = BTreeMap::new();
    for file in std::fs::read_dir(source).unwrap() {
        let path = file.unwrap().path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        let names: BTreeSet<_> = text
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .flat_map(|line| line.split("machinery::").skip(1))
            .filter_map(|tail| tail.split_once("::"))
            .filter_map(|(module, _)| Behaviour::from_name(module))
            .map(Behaviour::name)
            .collect();
        if !names.is_empty() {
            actual.insert(
                path.file_stem().unwrap().to_str().unwrap().to_owned(),
                names,
            );
        }
    }
    let declared: BTreeMap<_, BTreeSet<_>> = COMPONENT_BEHAVIOURS
        .iter()
        .map(|(module, behaviours)| {
            (
                (*module).to_owned(),
                behaviours.iter().map(|b| b.name()).collect(),
            )
        })
        .collect();
    assert_eq!(actual, declared);
}

#[test]
fn fragment_and_anchored_markers_bind_to_the_supplied_parent() {
    use foundation_ui_components::machinery::gestures::{snap_behavior, swipe_behavior};
    use foundation_ui_wire::{PropValueT, WebOperationT};
    use foundation_wasm_ui::{mount_before, mount_fragment, App};
    let (app, sent) = App::mock();
    let (ctx, receiver) = app.context();
    assert_eq!(mount_fragment(&ctx, &receiver, swipe_behavior(), 42), 42);
    assert!(mount_before(&ctx, &receiver, snap_behavior(), 42, 99).is_empty());
    app.stabilize().unwrap();
    let batches = sent.borrow();
    let forms: Vec<_> = batches
        .iter()
        .flatten()
        .map(|op| {
            let WebOperationT::WebBehaviour(row) = &op.web else {
                panic!("expected a parent behaviour")
            };
            assert_eq!(row.node_id, 42);
            assert_eq!(row.custom.as_deref(), Some("gestures"));
            let arg = &row.args.as_ref().unwrap()[0];
            assert_eq!(arg.name.as_deref(), Some("mode"));
            let PropValueT::Text(mode) = &arg.value else {
                panic!("expected text mode")
            };
            mode.v.as_deref().unwrap()
        })
        .collect();
    assert_eq!(forms, ["swipe", "snap"]);
}
