use foundation_ui_wire::{
    read_frame,
    writer::{table_frames, ClientCaps, StreamSequencer, WriteError, WriterConfig},
    BoolsT, ColumnDataT, ColumnT, DataTable, DataTableT, F64sT, I32sT, I64sT, MountSurface,
    Platform, StringsT, WireLimits,
};

#[test]
fn table_splits_by_rows() {
    let values = (0_i64..20_000).collect();
    let table = table(20_000, vec![i64_column(values, None)]);
    let config = WriterConfig::default();
    let mut sequencer = StreamSequencer::new(7);
    let frames = table_frames(&table, &mut sequencer, &config).unwrap();
    let mut first_row = 0;
    let mut count = 0;
    for (index, frame) in frames.enumerate() {
        let bytes = frame.unwrap();
        let rows = [8192, 8192, 3616][index];
        let table = read_table(&bytes, index as u64 + 1);
        assert_eq!(table.rows(), rows);
        assert_eq!(table.target_node(), 9);
        let column = table.columns().unwrap().get(0);
        let values = column.data_as_i64().unwrap().values().unwrap();
        assert_eq!(values.len() as u64, rows);
        assert!(values
            .iter()
            .eq(first_row..first_row + i64::try_from(rows).unwrap()));
        first_row += i64::try_from(rows).unwrap();
        count += 1;
    }

    assert_eq!(count, 3);
    assert_eq!(first_row, 20_000);
    assert_eq!(sequencer.last(), 3);
}

#[test]
fn validity_bits_follow_rows() {
    // Unused input bits in the last byte do not become rows.
    let table = table(
        11,
        vec![i64_column(
            (0..11).collect(),
            Some(vec![0b1010_1001, 0b1111_0011]),
        )],
    );
    let config = WriterConfig::default().max_table_rows(3);
    let mut sequencer = StreamSequencer::resume(7, 20);
    let frames = table_frames(&table, &mut sequencer, &config).unwrap();
    let mut count = 0;
    for (index, frame) in frames.enumerate() {
        let bytes = frame.unwrap();
        let table = read_table(&bytes, index as u64 + 21);
        let column = table.columns().unwrap().get(0);
        assert_eq!(table.rows(), [3, 3, 3, 2][index]);
        assert_eq!(
            column.validity().unwrap().bytes(),
            &[0b001, 0b101, 0b110, 0b001][index..=index]
        );
        count += 1;
    }
    assert_eq!(count, 4);
    assert_eq!(sequencer.last(), 24);
}

#[test]
fn empty_table_gives_one_frame() {
    let table = table(0, vec![i64_column(Vec::new(), Some(Vec::new()))]);
    let config = WriterConfig::default();
    let mut sequencer = StreamSequencer::new(7);
    let mut frames = table_frames(&table, &mut sequencer, &config).unwrap();

    let bytes = frames.next().unwrap().unwrap();
    let slice = read_table(&bytes, 1);
    assert_eq!(slice.rows(), 0);
    assert_eq!(slice.target_node(), 9);
    let column = slice.columns().unwrap().get(0);
    assert!(column.validity().unwrap().is_empty());
    assert!(column.data_as_i64().unwrap().values().unwrap().is_empty());
    assert!(frames.next().is_none());
    assert_eq!(sequencer.last(), 1);
}

#[test]
fn bad_column_length_fails_before_first_frame() {
    let table = table(
        3,
        vec![
            i64_column(vec![1, 2, 3], None),
            i64_column(vec![1, 2], None),
        ],
    );
    let config = WriterConfig::default().max_table_rows(1);
    let mut sequencer = StreamSequencer::new(7);
    assert_eq!(
        table_frames(&table, &mut sequencer, &config).err(),
        Some(WriteError::ColumnLength {
            column: 1,
            expected: 3,
            actual: 2
        })
    );
    assert_eq!(sequencer.last(), 0);
}

#[test]
fn bad_bitmap_length_fails_before_first_frame() {
    let config = WriterConfig::default().max_table_rows(1);
    let mut sequencer = StreamSequencer::resume(7, 20);
    for bitmap in [vec![0], vec![0, 0, 0]] {
        let length = bitmap.len() as u64;
        let table = table(9, vec![i64_column(vec![0; 9], Some(bitmap))]);
        assert_eq!(
            table_frames(&table, &mut sequencer, &config).err(),
            Some(WriteError::ColumnLength {
                column: 0,
                expected: 2,
                actual: length
            })
        );
        assert_eq!(sequencer.last(), 20);
    }

    let empty = table(0, vec![i64_column(Vec::new(), Some(vec![0]))]);
    assert_eq!(
        table_frames(&empty, &mut sequencer, &config).err(),
        Some(WriteError::ColumnLength {
            column: 0,
            expected: 0,
            actual: 1
        })
    );
    assert_eq!(sequencer.last(), 20);
}

#[test]
fn table_iterator_advances_only_for_requested_items() {
    let table = table(3, vec![i64_column(vec![1, 2, 3], None)]);
    let config = WriterConfig::default().max_table_rows(1);
    let mut sequencer = StreamSequencer::resume(7, 400);
    {
        let _frames = table_frames(&table, &mut sequencer, &config).unwrap();
    }
    assert_eq!(sequencer.last(), 400);
    {
        let mut frames = table_frames(&table, &mut sequencer, &config).unwrap();
        let bytes = frames.next().unwrap().unwrap();
        assert_eq!(read_table(&bytes, 401).rows(), 1);
    }
    assert_eq!(sequencer.last(), 401);
}

#[test]
fn oversized_item_ends_the_table_iterator() {
    let label = "x".repeat(2048);
    let table = table(
        3,
        vec![column(ColumnDataT::Utf8(Box::new(table!(StringsT {
            values: Some(vec!["ok".into(), label, "unused".into()]),
        }))))],
    );
    let config = WriterConfig::default()
        .max_table_rows(1)
        .limits(WireLimits::default().max_frame_bytes(1024));
    let mut sequencer = StreamSequencer::resume(7, 400);
    let mut frames = table_frames(&table, &mut sequencer, &config).unwrap();
    let first = frames.next().unwrap().unwrap();
    assert_eq!(read_table(&first, 401).rows(), 1);
    assert!(
        matches!(frames.next(), Some(Err(WriteError::TooLarge { limit: 1024, actual })) if actual > 1024)
    );
    assert!(frames.next().is_none());
    assert!(frames.next().is_none());
    assert_eq!(sequencer.last(), 402);
}

#[test]
fn table_slices_each_column_data_type() {
    let columns = vec![
        column(ColumnDataT::I32(Box::new(table!(I32sT {
            values: Some(vec![-1, -2])
        })))),
        i64_column(vec![1, 2], None),
        column(ColumnDataT::F64(Box::new(table!(F64sT {
            values: Some(vec![0.5, 1.5])
        })))),
        column(ColumnDataT::Bool(Box::new(table!(BoolsT {
            values: Some(vec![false, true])
        })))),
        column(ColumnDataT::Utf8(Box::new(table!(StringsT {
            values: Some(vec!["a".into(), "bb".into()])
        })))),
    ];
    let table = table(2, columns);
    let config = WriterConfig::default().max_table_rows(1);
    let mut sequencer = StreamSequencer::new(7);
    let frames = table_frames(&table, &mut sequencer, &config).unwrap();
    let mut count = 0;
    for (index, bytes) in frames.enumerate() {
        let bytes = bytes.unwrap();
        let slice = read_table(&bytes, index as u64 + 1).unpack();
        for (sliced, original) in slice
            .columns
            .unwrap()
            .iter()
            .zip(table.columns.as_ref().unwrap())
        {
            assert_eq!(sliced.name, original.name);
            assert_eq!(sliced.validity, original.validity);
            match (&sliced.data, &original.data) {
                (ColumnDataT::I32(left), ColumnDataT::I32(right)) => {
                    assert_eq!(
                        left.values.as_ref().unwrap(),
                        &right.values.as_ref().unwrap()[index..=index]
                    );
                }
                (ColumnDataT::I64(left), ColumnDataT::I64(right)) => {
                    assert_eq!(
                        left.values.as_ref().unwrap(),
                        &right.values.as_ref().unwrap()[index..=index]
                    );
                }
                (ColumnDataT::F64(left), ColumnDataT::F64(right)) => {
                    assert_eq!(
                        left.values.as_ref().unwrap(),
                        &right.values.as_ref().unwrap()[index..=index]
                    );
                }
                (ColumnDataT::Bool(left), ColumnDataT::Bool(right)) => {
                    assert_eq!(
                        left.values.as_ref().unwrap(),
                        &right.values.as_ref().unwrap()[index..=index]
                    );
                }
                (ColumnDataT::Utf8(left), ColumnDataT::Utf8(right)) => {
                    assert_eq!(
                        left.values.as_ref().unwrap(),
                        &right.values.as_ref().unwrap()[index..=index]
                    );
                }
                _ => panic!("Expected the original column data type"),
            }
        }
        count += 1;
    }
    assert_eq!(count, 2);
}

#[test]
fn writer_defaults_and_capabilities_match_current_vocabulary() {
    use foundation_ui_wire::vocab::VOCABULARY_VERSION;

    let config = WriterConfig::default();
    assert_eq!(config.table_rows(), 8192);
    assert_eq!(config.wire_limits(), &WireLimits::default());
    assert!(!config.unsealed_state_allowed());
    assert_eq!(config.clone().max_table_rows(0).table_rows(), 1);
    assert!(config.allow_unsealed_state(true).unsealed_state_allowed());
    assert_eq!(
        ClientCaps::web_current(),
        ClientCaps {
            platform: Platform::Web,
            mount: MountSurface::Web,
            vocabulary_version: VOCABULARY_VERSION
        }
    );
    assert_eq!(
        ClientCaps::native_current(Platform::Ios),
        ClientCaps {
            platform: Platform::Ios,
            mount: MountSurface::Native,
            vocabulary_version: VOCABULARY_VERSION
        }
    );
}

fn table(rows: u64, columns: Vec<ColumnT>) -> DataTableT {
    table!(DataTableT {
        target_node: 9,
        rows: rows,
        columns: Some(columns)
    })
}

fn column(data: ColumnDataT) -> ColumnT {
    table!(ColumnT {
        name: Some("col".into()),
        data: data
    })
}

fn i64_column(values: Vec<i64>, validity: Option<Vec<u8>>) -> ColumnT {
    table!(ColumnT {
        name: Some("count".into()),
        data: ColumnDataT::I64(Box::new(table!(I64sT {
            values: Some(values)
        }))),
        validity: validity,
    })
}

fn read_table(bytes: &[u8], sequence_id: u64) -> DataTable<'_> {
    let frame = read_frame(bytes, &WireLimits::default()).unwrap();
    assert_eq!(frame.stream_id(), 7);
    assert_eq!(frame.sequence_id(), sequence_id);
    let body = frame.kind_as_frame_message().unwrap();
    assert_eq!(body.ack(), 0);
    let messages = body.messages().unwrap();
    assert_eq!(messages.len(), 1);
    let data = messages.get(0).data().unwrap();
    let table = data.table().unwrap();
    assert_eq!(data.target_node(), table.target_node());
    table
}
