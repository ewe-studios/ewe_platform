use std::collections::BTreeSet;

use foundation_ui_wire::{MobileOperation, UiOperation, WebOperation};

const SHARED: &[(&str, &[&str])] = &[
    (
        include_str!("../src/generated/ewe/ui/create_node.rs"),
        &["NODE_ID", "KIND"],
    ),
    (
        include_str!("../src/generated/ewe/ui/set_text.rs"),
        &["NODE_ID", "TEXT"],
    ),
    (
        include_str!("../src/generated/ewe/ui/set_prop.rs"),
        &["NODE_ID", "PROP", "VALUE_TYPE", "VALUE"],
    ),
    (
        include_str!("../src/generated/ewe/ui/append_child.rs"),
        &["PARENT_ID", "CHILD_ID"],
    ),
    (
        include_str!("../src/generated/ewe/ui/insert_before.rs"),
        &["PARENT_ID", "CHILD_ID", "BEFORE_ID"],
    ),
    (
        include_str!("../src/generated/ewe/ui/remove.rs"),
        &["NODE_ID"],
    ),
    (
        include_str!("../src/generated/ewe/ui/replace_subtree.rs"),
        &["NODE_ID", "OPERATIONS"],
    ),
    (
        include_str!("../src/generated/ewe/ui/bind_action.rs"),
        &["NODE_ID", "EVENT", "ACTION"],
    ),
    (
        include_str!("../src/generated/ewe/ui/unbind_action.rs"),
        &["NODE_ID", "EVENT"],
    ),
    (
        include_str!("../src/generated/ewe/ui/attach_state.rs"),
        &["ROOT_NODE", "STATE"],
    ),
    (
        include_str!("../src/generated/ewe/ui/live_mount.rs"),
        &[
            "NODE_ID",
            "KIND",
            "CHANNEL_ID",
            "TRANSPORT",
            "ENDPOINT",
            "RESUME_FROM",
        ],
    ),
    (
        include_str!("../src/generated/ewe/ui/ui_behaviour.rs"),
        &["NODE_ID", "BEHAVIOUR", "ARGS"],
    ),
];

fn generated_fields(source: &str) -> Vec<&str> {
    source
        .lines()
        .filter_map(|line| line.trim_start().strip_prefix("pub const VT_"))
        .filter_map(|field| field.split_once(':').map(|(name, _)| name))
        .collect()
}

#[test]
fn each_form_is_in_one_union() {
    // The union-number test also checks the exact order of each group.
    let mut seen = BTreeSet::new();
    let names = UiOperation::ENUM_VALUES[1..]
        .iter()
        .map(|form| form.variant_name().unwrap())
        .chain(
            WebOperation::ENUM_VALUES[1..]
                .iter()
                .map(|form| form.variant_name().unwrap()),
        )
        .chain(
            MobileOperation::ENUM_VALUES[1..]
                .iter()
                .map(|form| form.variant_name().unwrap()),
        );
    for name in names {
        assert!(seen.insert(name), "Repeated form name {name}");
    }
    assert_eq!(seen.len(), 31);
}

#[test]
fn shared_forms_hold_no_platform_field() {
    assert_eq!(SHARED.len(), UiOperation::ENUM_VALUES.len() - 1);
    for (generated, expected) in SHARED {
        assert_eq!(generated_fields(generated), *expected);
    }
}

#[test]
fn shared_forms_have_no_custom_name() {
    for (generated, _) in SHARED {
        assert!(!generated_fields(generated)
            .iter()
            .any(|field| field.starts_with("CUSTOM_") || *field == "NAME"));
    }
}

#[test]
fn markup_and_script_forms_are_web_only() {
    // The disjoint-set test proves that each of these names occurs in only one union.
    let web = WebOperation::ENUM_VALUES;
    assert!(web.contains(&WebOperation::HtmlMorph));
    assert!(web.contains(&WebOperation::ScriptNode));
    assert!(web.contains(&WebOperation::WebBehaviour));
    let mobile = MobileOperation::ENUM_VALUES;
    assert!(mobile.contains(&MobileOperation::MountWebView));
    assert!(mobile.contains(&MobileOperation::NativeCommand));
    assert!(mobile.contains(&MobileOperation::MobileBehaviour));
}

#[test]
fn platform_forms_hold_only_their_fields() {
    let cases = [
        (
            include_str!("../src/generated/ewe/ui/web_element.rs"),
            &["NODE_ID", "TAG_ID", "CUSTOM_TAG", "CLASS_NAME"][..],
        ),
        (
            include_str!("../src/generated/ewe/ui/web_text.rs"),
            &["NODE_ID", "CONTENT"][..],
        ),
        (
            include_str!("../src/generated/ewe/ui/web_set_attribute.rs"),
            &["NODE_ID", "ATTR_ID", "CUSTOM_ATTR", "VALUE"][..],
        ),
        (
            include_str!("../src/generated/ewe/ui/web_remove_attribute.rs"),
            &["NODE_ID", "ATTR_ID", "CUSTOM_ATTR"][..],
        ),
        (
            include_str!("../src/generated/ewe/ui/web_property.rs"),
            &["NODE_ID", "NAME_ID", "CUSTOM_NAME", "VALUE"][..],
        ),
        (
            include_str!("../src/generated/ewe/ui/web_add_listener.rs"),
            &["NODE_ID", "NAME_ID", "CUSTOM_NAME"][..],
        ),
        (
            include_str!("../src/generated/ewe/ui/web_remove_listener.rs"),
            &["NODE_ID", "NAME_ID", "CUSTOM_NAME"][..],
        ),
        (
            include_str!("../src/generated/ewe/ui/web_replace_node.rs"),
            &["OLD_ID", "NEW_ID"][..],
        ),
        (
            include_str!("../src/generated/ewe/ui/web_style.rs"),
            &["NODE_ID", "NAME_ID", "CUSTOM_NAME", "VALUE"][..],
        ),
        (
            include_str!("../src/generated/ewe/ui/web_add_class.rs"),
            &["NODE_ID", "CLASS_NAME"][..],
        ),
        (
            include_str!("../src/generated/ewe/ui/web_remove_class.rs"),
            &["NODE_ID", "CLASS_NAME"][..],
        ),
        (
            include_str!("../src/generated/ewe/ui/web_register_node.rs"),
            &["NODE_ID"][..],
        ),
        (
            include_str!("../src/generated/ewe/ui/web_unregister_node.rs"),
            &["NODE_ID"][..],
        ),
        (
            include_str!("../src/generated/ewe/ui/html_morph.rs"),
            &["TARGET", "ACTION", "CONTENT"][..],
        ),
        (
            include_str!("../src/generated/ewe/ui/script_node.rs"),
            &["NODE_ID", "PARENT_ID", "BODY"][..],
        ),
        (
            include_str!("../src/generated/ewe/ui/web_behaviour.rs"),
            &["NODE_ID", "BEHAVIOUR", "CUSTOM", "ARGS"][..],
        ),
        (
            include_str!("../src/generated/ewe/ui/mount_web_view.rs"),
            &["NODE_ID", "PARENT_ID", "URL"][..],
        ),
        (
            include_str!("../src/generated/ewe/ui/native_command.rs"),
            &["COMMAND", "ARGS"][..],
        ),
        (
            include_str!("../src/generated/ewe/ui/mobile_behaviour.rs"),
            &["NODE_ID", "BEHAVIOUR", "CUSTOM", "ARGS"][..],
        ),
    ];
    assert_eq!(
        cases.len(),
        WebOperation::ENUM_VALUES.len() + MobileOperation::ENUM_VALUES.len() - 2
    );
    for (generated, expected) in cases {
        assert_eq!(generated_fields(generated), expected);
    }
}
