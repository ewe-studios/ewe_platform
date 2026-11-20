use alloc::{boxed::Box, vec, vec::Vec};
use core::{iter::FusedIterator, ops::Range};

use crate::{
    BoolsT, ColumnDataT, ColumnT, DataT, DataTableT, F64sT, I32sT, I64sT, MessageT, StringsT,
};

use super::{finish_message_frame, StreamSequencer, WriteError, WriterConfig};

/// Splits a table into frames. The iterator writes a frame when the caller requests it.
///
/// # Errors
///
/// Returns a column-length error before writing any frame if a vector has the wrong length.
pub fn table_frames<'a>(
    table: &'a DataTableT,
    sequencer: &'a mut StreamSequencer,
    config: &'a WriterConfig,
) -> Result<TableFrames<'a>, WriteError> {
    for (index, column) in table.columns.iter().flatten().enumerate() {
        check_length(index, table.rows, data_length(&column.data))?;
        if let Some(validity) = &column.validity {
            check_length(index, table.rows.div_ceil(8), validity.len())?;
        }
    }

    Ok(TableFrames {
        table,
        sequencer,
        config,
        next_row: 0,
        finished: false,
    })
}

/// Writes one row slice per call to the iterator.
pub struct TableFrames<'a> {
    table: &'a DataTableT,
    sequencer: &'a mut StreamSequencer,
    config: &'a WriterConfig,
    next_row: u64,
    finished: bool,
}

impl Iterator for TableFrames<'_> {
    type Item = Result<Vec<u8>, WriteError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }

        let start = self.next_row;
        let end = start
            .saturating_add(self.config.table_rows() as u64)
            .min(self.table.rows);
        let columns = self.table.columns.as_ref().map(|columns| {
            columns
                .iter()
                .map(|column| slice_column(column, start..end))
                .collect()
        });
        let table = DataTableT {
            target_node: self.table.target_node,
            rows: end - start,
            columns,
        };
        let message = MessageT {
            data: Some(Box::new(DataT {
                target_node: table.target_node,
                table: Some(Box::new(table)),
                ..DataT::default()
            })),
            ..MessageT::default()
        };
        let result = finish_message_frame(vec![message], self.sequencer.header(0), self.config);
        self.next_row = end;
        self.finished = end == self.table.rows || result.is_err();
        Some(result)
    }
}

impl FusedIterator for TableFrames<'_> {}

fn data_length(data: &ColumnDataT) -> usize {
    match data {
        ColumnDataT::NONE => 0,
        ColumnDataT::I32(values) => values.values.as_ref().map_or(0, Vec::len),
        ColumnDataT::I64(values) => values.values.as_ref().map_or(0, Vec::len),
        ColumnDataT::F64(values) => values.values.as_ref().map_or(0, Vec::len),
        ColumnDataT::Bool(values) => values.values.as_ref().map_or(0, Vec::len),
        ColumnDataT::Utf8(values) => values.values.as_ref().map_or(0, Vec::len),
    }
}

fn check_length(column: usize, expected: u64, actual: usize) -> Result<(), WriteError> {
    let actual = actual as u64;
    if actual != expected {
        return Err(WriteError::ColumnLength {
            column,
            expected,
            actual,
        });
    }
    Ok(())
}

fn slice_column(column: &ColumnT, rows: Range<u64>) -> ColumnT {
    // The vector length check proves the bounds fit in an index for this column.
    let rows = usize::try_from(rows.start).expect("The first row fits the column length.")
        ..usize::try_from(rows.end).expect("The end row fits the column length.");
    let validity = column
        .validity
        .as_ref()
        .map(|bitmap| slice_validity(bitmap, rows.clone()));
    let copied = slice_data(&column.data, rows);
    ColumnT {
        name: column.name.clone(),
        data: copied,
        validity,
    }
}

fn slice_data(data: &ColumnDataT, rows: Range<usize>) -> ColumnDataT {
    match data {
        ColumnDataT::NONE => ColumnDataT::NONE,
        ColumnDataT::I32(values) => ColumnDataT::I32(Box::new(I32sT {
            values: slice_values(values.values.as_deref(), rows),
        })),
        ColumnDataT::I64(values) => ColumnDataT::I64(Box::new(I64sT {
            values: slice_values(values.values.as_deref(), rows),
        })),
        ColumnDataT::F64(values) => ColumnDataT::F64(Box::new(F64sT {
            values: slice_values(values.values.as_deref(), rows),
        })),
        ColumnDataT::Bool(values) => ColumnDataT::Bool(Box::new(BoolsT {
            values: slice_values(values.values.as_deref(), rows),
        })),
        ColumnDataT::Utf8(values) => ColumnDataT::Utf8(Box::new(StringsT {
            values: slice_values(values.values.as_deref(), rows),
        })),
    }
}

fn slice_values<T: Clone>(values: Option<&[T]>, rows: Range<usize>) -> Option<Vec<T>> {
    values.map(|values| values[rows].to_vec())
}

fn slice_validity(validity: &[u8], rows: Range<usize>) -> Vec<u8> {
    let mut sliced = vec![0; rows.len().div_ceil(8)];
    for (output_row, input_row) in rows.enumerate() {
        let bit = (validity[input_row / 8] >> (input_row % 8)) & 1;
        sliced[output_row / 8] |= bit << (output_row % 8);
    }
    sliced
}
