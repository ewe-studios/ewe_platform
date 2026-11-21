use foundation_ui_wire::{
    read_frame,
    writer::{
        gate, write_messages, ClientCaps, FilterReason, FilteredMessage, FilteredPart, FrameHeader,
        WriteError, WriteReport, WriterConfig,
    },
    BehaviourId, BindActionT, CreateNodeT, EventId, HtmlMorphT, MessageT, MobileBehaviourT,
    MobileOperation, MobileOperationT, MountSurface, NativeCommandId, NativeCommandT, NodeKind,
    Operation, OperationT, Platform, PropId, ReplaceSubtreeT, SetPropT, SetTextT, UiBehaviourT,
    UiOperationT, UnbindActionT, WebAddClassT, WebAddListenerT, WebBehaviourId, WebBehaviourT,
    WebElementT, WebOperation, WebOperationT, WebPropertyT, WebRemoveAttributeT,
    WebRemoveListenerT, WebSetAttributeT, WebStyleT, WireLimits,
};

use MobileOperationT as Mobile;
use UiOperationT as Ui;
use WebOperationT as Web;

const HEADER: FrameHeader = FrameHeader {
    stream_id: 7,
    sequence_id: 20,
    ack: 5,
};

#[test]
fn no_other_platform_field_in_bytes() {
    let limits = WireLimits::default();
    let source = read_frame(
        include_bytes!("../fixtures/frame_ops_all_forms.bin"),
        &limits,
    )
    .unwrap()
    .kind_as_frame_message()
    .unwrap()
    .unpack()
    .messages
    .unwrap()
    .into_iter()
    .map(|message| *message.operation.unwrap())
    .collect::<Vec<_>>();
    assert_eq!(source.len(), 31);

    let mut seed = 0x327a_8ea4_fbe4_3b29;
    for _ in 0..200 {
        let messages = (0..pick(&mut seed, 12))
            .map(|_| message(random_operation(&source, 3, &mut seed)))
            .collect::<Vec<_>>();
        for caps in [ClientCaps::web_current(), native()] {
            let (bytes, _) =
                write_messages(messages.clone(), &caps, HEADER, &WriterConfig::default()).unwrap();
            let frame = read_frame(&bytes, &limits)
                .unwrap()
                .kind_as_frame_message()
                .unwrap();
            for message in frame.messages().unwrap() {
                assert_platform_fields(caps.mount, message.operation().unwrap());
            }
        }
    }
}

fn pick(seed: &mut u64, length: usize) -> usize {
    *seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
    usize::try_from(*seed >> 32).unwrap() % length
}

fn random_operation(source: &[OperationT], depth: usize, seed: &mut u64) -> OperationT {
    let mut operation = source[pick(seed, source.len())].clone();
    if depth > 0 && pick(seed, 3) == 0 {
        operation.ui = subtree(
            (0..pick(seed, 5))
                .map(|_| random_operation(source, depth - 1, seed))
                .collect(),
        );
    }
    let extra = &source[pick(seed, source.len())];
    if !matches!(extra.web, Web::NONE) {
        operation.web = extra.web.clone();
    }
    if !matches!(extra.mobile, Mobile::NONE) {
        operation.mobile = extra.mobile.clone();
    }
    operation
}

fn assert_platform_fields(mount: MountSurface, operation: Operation<'_>) {
    if mount == MountSurface::Native {
        assert_eq!(operation.web_type(), WebOperation::NONE);
    } else {
        assert_eq!(operation.mobile_type(), MobileOperation::NONE);
    }
    if let Some(subtree) = operation.ui_as_replace_subtree() {
        if let Some(operations) = subtree.operations() {
            for operation in operations {
                assert_platform_fields(mount, operation);
            }
        }
    }
}

#[test]
fn native_client_gets_no_web_field() {
    let result = write(
        vec![message(table!(OperationT {
            ui: text(42),
            web: morph()
        }))],
        &native(),
    );
    assert_eq!(result.0, [message(table!(OperationT { ui: text(42) }))]);
    assert_eq!(
        result.1.filtered,
        [removed(0, FilteredPart::Web, FilterReason::OtherPlatform)]
    );
}

#[test]
fn web_client_gets_no_mobile_field() {
    let mobile = Mobile::NativeCommand(Box::new(table!(NativeCommandT {
        command: NativeCommandId::HAPTIC
    })));
    let result = write(
        vec![message(table!(OperationT {
            ui: text(42),
            mobile: mobile
        }))],
        &ClientCaps::web_current(),
    );
    assert_eq!(result.0, [message(table!(OperationT { ui: text(42) }))]);
    assert_eq!(
        result.1.filtered,
        [removed(
            0,
            FilteredPart::Mobile,
            FilterReason::OtherPlatform
        )]
    );
}

#[test]
fn web_only_message_is_removed_for_native() {
    let result = write(
        vec![message(table!(OperationT { web: morph() }))],
        &native(),
    );
    assert_eq!(result.0, []);
    assert_eq!(
        result.1.filtered,
        [removed(
            0,
            FilteredPart::Message,
            FilterReason::OtherPlatform
        )]
    );
}

#[test]
fn nested_web_form_is_removed_for_native() {
    let kept = table!(OperationT { ui: text(42) });
    let obsolete = table!(OperationT {
        web: Web::WebAddClass(Box::new(table!(WebAddClassT {
            node_id: 42,
            class_name: Some("x".into())
        }))),
    });
    let result = write(
        vec![message(table!(OperationT {
            ui: subtree(vec![kept.clone(), obsolete])
        }))],
        &native(),
    );
    assert_eq!(
        result.0,
        [message(table!(OperationT {
            ui: subtree(vec![kept])
        }))]
    );
    assert_eq!(
        result.1.filtered,
        [removed(0, FilteredPart::Web, FilterReason::OtherPlatform)]
    );
}

#[test]
fn kept_messages_keep_order() {
    let messages = (1..=12)
        .map(|id| {
            if id % 2 == 0 {
                message(table!(OperationT { ui: text(id) }))
            } else {
                message(table!(OperationT { web: morph() }))
            }
        })
        .collect();
    let (kept, report) = write(messages, &native());
    let expected = (1..=12)
        .filter(|id| id % 2 == 0)
        .map(|id| message(table!(OperationT { ui: text(id) })))
        .collect::<Vec<_>>();
    assert_eq!(kept, expected);
    assert_eq!(
        report.filtered,
        (0..12)
            .step_by(2)
            .map(|index| removed(index, FilteredPart::Message, FilterReason::OtherPlatform))
            .collect::<Vec<_>>(),
    );
}

#[test]
fn older_clients_receive_the_fallback_for_new_container_kinds() {
    let create = |kind| {
        Ui::CreateNode(Box::new(table!(CreateNodeT {
            node_id: 9,
            kind: kind
        })))
    };
    let replacement = create(NodeKind::UNKNOWN);
    let caps = ClientCaps {
        vocabulary_version: 4,
        ..native()
    };
    let (kept, report) = write(
        vec![
            message(table!(OperationT { web: morph() })),
            message(table!(OperationT {
                ui: subtree(vec![
                    table!(OperationT {
                        ui: create(NodeKind::MODAL)
                    }),
                    table!(OperationT {
                        ui: create(NodeKind::BOTTOM_SHEET)
                    }),
                ]),
            })),
        ],
        &caps,
    );
    assert_eq!(
        kept,
        [message(table!(OperationT {
            ui: subtree(vec![
                table!(OperationT {
                    ui: replacement.clone()
                }),
                table!(OperationT { ui: replacement }),
            ]),
        })),]
    );
    assert_eq!(report.replaced_kinds, [1]);
    assert_eq!(
        report.filtered,
        [removed(
            0,
            FilteredPart::Message,
            FilterReason::OtherPlatform
        )]
    );
}

#[test]
fn version_1_client_gets_no_behaviour() {
    let web_v1 = ClientCaps {
        vocabulary_version: 1,
        ..ClientCaps::web_current()
    };
    let native_v1 = ClientCaps {
        vocabulary_version: 1,
        ..native()
    };
    for (operation, caps) in [
        (
            table!(OperationT {
                ui: Ui::UiBehaviour(Box::new(table!(UiBehaviourT {
                    behaviour: BehaviourId::SCROLL_LOCK
                })))
            }),
            &web_v1,
        ),
        (
            table!(OperationT {
                web: Web::WebBehaviour(Box::default())
            }),
            &web_v1,
        ),
        (
            table!(OperationT {
                mobile: Mobile::MobileBehaviour(Box::default())
            }),
            &native_v1,
        ),
    ] {
        let (kept, report) = write(vec![message(operation)], caps);
        assert_eq!(kept, []);
        assert_eq!(
            report.filtered,
            [removed(
                0,
                FilteredPart::Message,
                FilterReason::UnknownVocabulary
            )]
        );
    }
}

#[test]
fn unknown_nested_operation_uses_first_report() {
    let invalid = table!(OperationT {
        ui: Ui::BindAction(Box::new(table!(BindActionT {
            event: EventId(u16::MAX)
        }))),
    });
    let input = vec![message(table!(OperationT {
        ui: subtree(vec![
            invalid,
            table!(OperationT { web: morph() }),
            table!(OperationT { ui: text(42) })
        ]),
    }))];
    let expected = [message(table!(OperationT {
        ui: subtree(vec![table!(OperationT { ui: text(42) })])
    }))];
    let (kept, report) = gate(input, &native()).unwrap();
    assert_eq!(kept, expected);
    assert_eq!(
        report,
        [removed(
            0,
            FilteredPart::NestedOperation,
            FilterReason::UnknownVocabulary
        )]
    );
}

#[test]
fn platform_check_precedes_vocabulary_check() {
    let unsupported = Web::WebElement(Box::new(table!(WebElementT { tag_id: u16::MAX })));
    let (kept, report) = write(
        vec![message(table!(OperationT {
            ui: text(42),
            web: unsupported
        }))],
        &native(),
    );
    assert_eq!(kept, [message(table!(OperationT { ui: text(42) }))]);
    assert_eq!(
        report.filtered,
        [removed(0, FilteredPart::Web, FilterReason::OtherPlatform)]
    );
}

#[test]
fn numbers_above_version_maxima_remove_operations() {
    let caps = ClientCaps::web_current();
    let operations = unknown_web_forms()
        .into_iter()
        .map(|web| table!(OperationT { web: web }))
        .chain([
            table!(OperationT {
                ui: Ui::SetProp(Box::new(table!(SetPropT {
                    prop: PropId(u16::MAX)
                })))
            }),
            table!(OperationT {
                ui: Ui::BindAction(Box::new(table!(BindActionT {
                    event: EventId(u16::MAX)
                })))
            }),
            table!(OperationT {
                ui: Ui::UnbindAction(Box::new(table!(UnbindActionT {
                    event: EventId(u16::MAX)
                })))
            }),
            table!(OperationT {
                ui: Ui::UiBehaviour(Box::new(table!(UiBehaviourT {
                    behaviour: BehaviourId(u16::MAX)
                })))
            }),
        ]);
    assert_filtered_for_vocab(operations, &caps, 12);
    let native_ops = [
        table!(OperationT {
            mobile: Mobile::NativeCommand(Box::new(table!(NativeCommandT {
                command: NativeCommandId(u16::MAX)
            })))
        }),
        table!(OperationT {
            mobile: Mobile::MobileBehaviour(Box::new(table!(MobileBehaviourT {
                behaviour: foundation_ui_wire::MobileBehaviourId(u16::MAX)
            })))
        }),
    ];
    assert_filtered_for_vocab(native_ops, &native(), 2);
}

fn unknown_web_forms() -> [Web; 8] {
    [
        Web::WebElement(Box::new(table!(WebElementT { tag_id: u16::MAX }))),
        Web::WebSetAttribute(Box::new(table!(WebSetAttributeT { attr_id: u16::MAX }))),
        Web::WebRemoveAttribute(Box::new(table!(WebRemoveAttributeT { attr_id: u16::MAX }))),
        Web::WebProperty(Box::new(table!(WebPropertyT { name_id: u16::MAX }))),
        Web::WebStyle(Box::new(table!(WebStyleT { name_id: u16::MAX }))),
        Web::WebAddListener(Box::new(table!(WebAddListenerT { name_id: u16::MAX }))),
        Web::WebRemoveListener(Box::new(table!(WebRemoveListenerT { name_id: u16::MAX }))),
        Web::WebBehaviour(Box::new(table!(WebBehaviourT {
            behaviour: WebBehaviourId(u16::MAX)
        }))),
    ]
}

fn assert_filtered_for_vocab(
    operations: impl IntoIterator<Item = OperationT>,
    caps: &ClientCaps,
    count: usize,
) {
    let input = operations.into_iter().map(message).collect();
    let (kept, report) = gate(input, caps).unwrap();
    assert_eq!(kept, []);
    assert_eq!(
        report,
        (0..count)
            .map(|index| removed(
                index,
                FilteredPart::Message,
                FilterReason::UnknownVocabulary
            ))
            .collect::<Vec<_>>(),
    );
}

#[test]
fn gate_rejects_an_empty_message() {
    let input = vec![
        message(table!(OperationT { ui: text(42) })),
        MessageT::default(),
    ];
    assert_eq!(
        gate(input, &native()),
        Err(WriteError::EmptyMessage { message_index: 1 })
    );
}

#[test]
fn unknown_vocab_version_is_rejected() {
    let caps = ClientCaps {
        vocabulary_version: 99,
        ..native()
    };
    assert_eq!(
        gate(vec![], &caps),
        Err(WriteError::UnknownVocabularyVersion { version: 99 })
    );
}

fn write(messages: Vec<MessageT>, caps: &ClientCaps) -> (Vec<MessageT>, WriteReport) {
    let config = WriterConfig::default();
    let (bytes, report) = write_messages(messages, caps, HEADER, &config).unwrap();
    let frame = read_frame(&bytes, config.wire_limits()).unwrap();
    let messages = frame
        .kind_as_frame_message()
        .unwrap()
        .unpack()
        .messages
        .unwrap();
    (messages, report)
}

fn native() -> ClientCaps {
    ClientCaps::native_current(Platform::Ios)
}

fn removed(message_index: usize, part: FilteredPart, reason: FilterReason) -> FilteredMessage {
    FilteredMessage {
        message_index,
        part,
        reason,
    }
}

fn message(operation: OperationT) -> MessageT {
    table!(MessageT {
        operation: Some(Box::new(operation))
    })
}

fn text(node_id: u32) -> Ui {
    Ui::SetText(Box::new(table!(SetTextT {
        node_id: node_id,
        text: Some("1".into())
    })))
}

fn subtree(operations: Vec<OperationT>) -> Ui {
    Ui::ReplaceSubtree(Box::new(table!(ReplaceSubtreeT {
        node_id: 40,
        operations: Some(operations)
    })))
}

fn morph() -> Web {
    Web::HtmlMorph(Box::new(table!(HtmlMorphT {
        content: Some("<b>x</b>".into())
    })))
}

#[test]
fn placement_and_detach_require_vocabulary_three() {
    use foundation_ui_wire::{PlaceT, PlacementKind, TargetSelectorT};
    let place = Ui::Place(Box::new(table!(PlaceT {
        kind: PlacementKind::Remove,
        target: Some(Box::new(TargetSelectorT::default()))
    })));
    let messages = vec![
        message(table!(OperationT { ui: place })),
        message(table!(OperationT {
            web: Web::WebDetachChild(Box::default())
        })),
    ];
    let mut caps = ClientCaps::web_current();
    assert_eq!(gate(messages.clone(), &caps).unwrap().0.len(), 2);
    caps.vocabulary_version = 2;
    let (kept, removed) = gate(messages, &caps).unwrap();
    assert_eq!(kept, []);
    assert_eq!(removed.len(), 2);
}

#[test]
fn full_view_requires_version_six_and_a_client_owned_target() {
    use foundation_ui_wire::{PlaceT, PlacementKind, TargetSelectorT};
    let mut place = table!(PlaceT {
        kind: PlacementKind::FullView,
        root_node: 8,
        operations: Some(vec![table!(OperationT {
            ui: Ui::CreateNode(Box::new(table!(CreateNodeT {
                node_id: 8,
                kind: NodeKind::TEXT
            })))
        })])
    });
    let frame = |p| {
        message(table!(OperationT {
            ui: Ui::Place(Box::new(p))
        }))
    };
    let caps = ClientCaps::web_current();
    let old_caps = ClientCaps {
        vocabulary_version: 5,
        ..caps
    };
    assert_eq!(write(vec![frame(place.clone())], &caps).0.len(), 1);
    assert_eq!(write(vec![frame(place.clone())], &old_caps).0, Vec::new());
    place.target = Some(Box::new(TargetSelectorT::default()));
    assert!(matches!(
        write_messages(vec![frame(place)], &caps, HEADER, &WriterConfig::default()),
        Err(WriteError::InvalidPlacement { .. })
    ));
}

#[test]
fn writer_validates_placement_and_protects_nested_state_attributes() {
    use foundation_ui_wire::{PlaceT, PlacementKind, TargetSelectorT};
    let write = |ui| {
        write_messages(
            vec![message(table!(OperationT { ui: ui }))],
            &ClientCaps::web_current(),
            HEADER,
            &WriterConfig::default(),
        )
    };
    let mut place = table!(PlaceT {
        kind: PlacementKind::Append,
        target: Some(Box::new(TargetSelectorT::default()))
    });
    assert_eq!(
        write(Ui::Place(Box::new(place.clone()))).unwrap_err(),
        WriteError::InvalidPlacement { message_index: 0 }
    );
    for web in [
        Web::WebSetAttribute(Box::new(table!(WebSetAttributeT {
            custom_attr: Some("primal::state".into())
        }))),
        Web::WebRemoveAttribute(Box::new(table!(WebRemoveAttributeT {
            custom_attr: Some("PRIMAL::STATE".into())
        }))),
    ] {
        place.operations = Some(vec![table!(OperationT { web: web })]);
        assert_eq!(
            write(Ui::Place(Box::new(place.clone()))).unwrap_err(),
            WriteError::ReservedStateAttribute { message_index: 0 }
        );
    }
}

#[test]
fn native_gate_drops_a_swap_when_all_its_content_is_web_only() {
    use foundation_ui_wire::{PlaceT, PlacementKind, TargetSelectorT};
    let web_element = table!(OperationT {
        web: Web::WebElement(Box::new(table!(WebElementT { node_id: 8 })))
    });
    let swap = table!(PlaceT {
        kind: PlacementKind::Swap,
        target: Some(Box::new(TargetSelectorT::default())),
        root_node: 8,
        operations: Some(vec![web_element])
    });
    let message = message(table!(OperationT {
        ui: Ui::Place(Box::new(swap))
    }));
    let (messages, filtered) = gate(vec![message], &native()).unwrap();
    assert_eq!(messages, []);
    assert_eq!(filtered[0].reason, FilterReason::OtherPlatform);
}

#[test]
fn reserved_presentation_property_is_rejected_for_each_client() {
    use foundation_ui_wire::{PropId, PropValueT, SetPropT, TextValueT};
    let control = table!(OperationT {
        ui: Ui::CreateNode(Box::new(table!(CreateNodeT {
            node_id: 9,
            kind: NodeKind::TEXT
        })))
    });
    let role = table!(OperationT {
        ui: Ui::SetProp(Box::new(table!(SetPropT {
            node_id: 9,
            prop: PropId::PRESENTATION,
            value: PropValueT::Text(Box::new(table!(TextValueT {
                v: Some("navigation-title".into())
            })))
        })))
    });
    let full = message(table!(OperationT {
        ui: subtree(vec![control.clone(), role])
    }));
    for caps in [
        native(),
        ClientCaps {
            vocabulary_version: 3,
            ..native()
        },
    ] {
        assert!(matches!(
            write_messages(vec![full.clone()], &caps, HEADER, &WriterConfig::default()),
            Err(WriteError::Vocab {
                error: foundation_ui_wire::vocab::VocabError::UnknownProp { prop: 17 },
                ..
            })
        ));
    }
}
