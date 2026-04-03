//! Tests for HTTP chunked transfer encoding body reader.
//!
//! These tests verify that chunked encoding correctly parses HTTP/1.1 chunked
//! transfer encoding without including CRLF markers in the actual data.

use crate::io::ioutils::SharedByteBufferStream;
use crate::wire::simple_http::impls::{ChunkedData, SimpleHttpChunkIterator};
use crate::wire::simple_http::SimpleHeaders;
use std::io::Cursor;

/// Test: Multi-chunk JSON body - verifies CRLFs are not included in data.
///
/// This test creates a valid HTTP chunked response containing JSON data
/// split across multiple chunks. If the CRLF after each chunk's data is
/// not properly consumed, the output will contain extra newlines.
#[test]
fn test_chunked_json_no_crlf_in_data() {
    // JSON payload split into 3 chunks
    let chunk1_data = b"{\"users\": [";
    let chunk2_data = b"{\"id\": 1, \"name\": \"Alice\"}";
    let chunk3_data = b"]}";

    // Build properly formatted HTTP chunked response
    // Format: <size>\r\n<data>\r\n for each chunk, then 0\r\n\r\n for end
    let mut raw_response = Vec::new();

    // Chunk 1
    raw_response.extend(format!("{:x}\r\n", chunk1_data.len()).as_bytes());
    raw_response.extend_from_slice(chunk1_data);
    raw_response.extend_from_slice(b"\r\n");

    // Chunk 2
    raw_response.extend(format!("{:x}\r\n", chunk2_data.len()).as_bytes());
    raw_response.extend_from_slice(chunk2_data);
    raw_response.extend_from_slice(b"\r\n");

    // Chunk 3
    raw_response.extend(format!("{:x}\r\n", chunk3_data.len()).as_bytes());
    raw_response.extend_from_slice(chunk3_data);
    raw_response.extend_from_slice(b"\r\n");

    // Final chunk (size 0)
    raw_response.extend_from_slice(b"0\r\n\r\n");

    let cursor = Cursor::new(raw_response);
    let stream = SharedByteBufferStream::ref_cell(cursor);

    let headers = SimpleHeaders::new();
    let mut iterator = SimpleHttpChunkIterator::new(vec![], headers, stream, 100);

    let mut collected_bytes = Vec::new();

    for result in &mut iterator {
        match result {
            Ok(ChunkedData::Data(data, _)) => {
                collected_bytes.extend_from_slice(&data);
            }
            Ok(ChunkedData::DataEnded) => break,
            Ok(ChunkedData::Trailers(_)) => {}
            Err(e) => panic!("Chunk iterator error: {:?}", e),
        }
    }

    // Expected: concatenated chunk data without any CRLFs
    let expected = b"{\"users\": [{\"id\": 1, \"name\": \"Alice\"}]}";

    assert_eq!(
        &collected_bytes,
        expected,
        "Chunked data should not include CRLF markers. Got: {:?}, Expected: {:?}",
        String::from_utf8_lossy(&collected_bytes),
        String::from_utf8_lossy(expected)
    );

    // Verify no stray newlines or carriage returns in output
    for (i, byte) in collected_bytes.iter().enumerate() {
        assert_ne!(
            *byte,
            b'\r',
            "Found carriage return (\\r) at position {} in chunk data - CRLF was not properly stripped",
            i
        );
        // Note: \n in actual JSON content is fine, but \r should never appear
    }
}

/// Test: Verify exact byte-for-byte output matches input content.
///
/// This test sends known content through the chunked encoder and verifies
/// the output is exactly what was sent, with no extra characters.
#[test]
fn test_chunked_exact_content_preservation() {
    // Test with various characters that could be confused with HTTP framing
    let test_content = b"{\"message\": \"hello\\nworld\", \"count\": 42}";

    let mut raw_response = Vec::new();

    // Single chunk with the test content
    raw_response.extend(format!("{:x}\r\n", test_content.len()).as_bytes());
    raw_response.extend_from_slice(test_content);
    raw_response.extend_from_slice(b"\r\n");

    // Final chunk
    raw_response.extend_from_slice(b"0\r\n\r\n");

    let cursor = Cursor::new(raw_response);
    let stream = SharedByteBufferStream::ref_cell(cursor);

    let headers = SimpleHeaders::new();
    let mut iterator = SimpleHttpChunkIterator::new(vec![], headers, stream, 100);

    let mut collected_bytes = Vec::new();

    for result in &mut iterator {
        match result {
            Ok(ChunkedData::Data(data, _)) => {
                collected_bytes.extend_from_slice(&data);
            }
            Ok(ChunkedData::DataEnded) => break,
            Err(e) => panic!("Chunk iterator error: {:?}", e),
            _ => {}
        }
    }

    assert_eq!(
        &collected_bytes,
        test_content,
        "Output must exactly match input content. Got: {:?}, Expected: {:?}",
        String::from_utf8_lossy(&collected_bytes),
        String::from_utf8_lossy(test_content)
    );
}

/// Test: Multiple small chunks - catches CRLF accumulation issues.
///
/// When many small chunks are used, any failure to consume CRLFs will
/// accumulate and cause increasingly corrupted output.
#[test]
fn test_many_small_chunks() {
    let chunks: Vec<&[u8]> = vec![
        b"{\"",
        b"key",
        b"\":",
        b"\"",
        b"value",
        b"\"",
        b"}",
    ];

    let mut raw_response = Vec::new();

    for chunk_data in &chunks {
        raw_response.extend(format!("{:x}\r\n", chunk_data.len()).as_bytes());
        raw_response.extend_from_slice(chunk_data);
        raw_response.extend_from_slice(b"\r\n");
    }

    // Final chunk
    raw_response.extend_from_slice(b"0\r\n\r\n");

    let cursor = Cursor::new(raw_response);
    let stream = SharedByteBufferStream::ref_cell(cursor);

    let headers = SimpleHeaders::new();
    let mut iterator = SimpleHttpChunkIterator::new(vec![], headers, stream, 100);

    let mut collected_bytes = Vec::new();

    for result in &mut iterator {
        match result {
            Ok(ChunkedData::Data(data, _)) => {
                collected_bytes.extend_from_slice(&data);
            }
            Ok(ChunkedData::DataEnded) => break,
            Err(e) => panic!("Chunk iterator error: {:?}", e),
            _ => {}
        }
    }

    let expected: Vec<u8> = chunks.iter().flat_map(|c| c.iter().copied()).collect();

    assert_eq!(
        &collected_bytes,
        &expected,
        "Multi-chunk output corrupted. Got: {:?}, Expected: {:?}",
        String::from_utf8_lossy(&collected_bytes),
        String::from_utf8_lossy(&expected)
    );

    // Count any stray CR characters that shouldn't be there
    let cr_count = collected_bytes.iter().filter(|&&b| b == b'\r').count();
    assert_eq!(
        cr_count, 0,
        "Found {} stray \\r characters in output - CRLFs not being consumed",
        cr_count
    );
}

/// Test: Chunk with embedded newlines in content.
///
/// Verifies that newlines that are part of the actual content are preserved,
/// while HTTP framing CRLFs are stripped.
#[test]
fn test_chunked_content_with_embedded_newlines() {
    // Content with embedded \n (which is valid JSON string content)
    let content = b"{\"lines\": [\"line1\\n\", \"line2\\n\", \"line3\"]}";

    let mut raw_response = Vec::new();

    raw_response.extend(format!("{:x}\r\n", content.len()).as_bytes());
    raw_response.extend_from_slice(content);
    raw_response.extend_from_slice(b"\r\n");

    // Final chunk
    raw_response.extend_from_slice(b"0\r\n\r\n");

    let cursor = Cursor::new(raw_response);
    let stream = SharedByteBufferStream::ref_cell(cursor);

    let headers = SimpleHeaders::new();
    let mut iterator = SimpleHttpChunkIterator::new(vec![], headers, stream, 100);

    let mut collected_bytes = Vec::new();

    for result in &mut iterator {
        match result {
            Ok(ChunkedData::Data(data, _)) => {
                collected_bytes.extend_from_slice(&data);
            }
            Ok(ChunkedData::DataEnded) => break,
            Err(e) => panic!("Chunk iterator error: {:?}", e),
            _ => {}
        }
    }

    assert_eq!(
        &collected_bytes,
        content,
        "Content with embedded newlines should be preserved exactly"
    );

    // The embedded \n should be there (it's part of the JSON string)
    let newline_count = collected_bytes.iter().filter(|&&b| b == b'\n').count();
    assert_eq!(
        newline_count, 2,
        "Should have exactly 2 \\n from the JSON content (not counting framing)"
    );

    // But no \r should exist (that's only for HTTP framing)
    let cr_count = collected_bytes.iter().filter(|&&b| b == b'\r').count();
    assert_eq!(cr_count, 0, "Should have no \\r characters in output");
}

/// Test: Empty chunk followed by non-empty chunk.
///
/// Edge case: a chunk with size > 0 but the parsing might have issues
/// with CRLF consumption between chunks.
#[test]
fn test_chunk_boundary_crlf_consumption() {
    // Two chunks back-to-back
    let chunk1 = b"first";
    let chunk2 = b"second";

    let mut raw_response = Vec::new();

    // First chunk
    raw_response.extend(format!("{:x}\r\n", chunk1.len()).as_bytes());
    raw_response.extend_from_slice(chunk1);
    raw_response.extend_from_slice(b"\r\n");

    // Second chunk immediately after
    raw_response.extend(format!("{:x}\r\n", chunk2.len()).as_bytes());
    raw_response.extend_from_slice(chunk2);
    raw_response.extend_from_slice(b"\r\n");

    // Final chunk
    raw_response.extend_from_slice(b"0\r\n\r\n");

    let cursor = Cursor::new(raw_response);
    let stream = SharedByteBufferStream::ref_cell(cursor);

    let headers = SimpleHeaders::new();
    let mut iterator = SimpleHttpChunkIterator::new(vec![], headers, stream, 100);

    let mut collected_bytes = Vec::new();
    let mut chunk_count = 0;

    for result in &mut iterator {
        match result {
            Ok(ChunkedData::Data(data, _)) => {
                chunk_count += 1;
                collected_bytes.extend_from_slice(&data);
            }
            Ok(ChunkedData::DataEnded) => break,
            Err(e) => panic!("Chunk iterator error: {:?}", e),
            _ => {}
        }
    }

    assert_eq!(chunk_count, 2, "Should have received exactly 2 data chunks");

    let expected = b"firstsecond";
    assert_eq!(
        &collected_bytes,
        expected,
        "Chunk boundaries should not introduce extra characters"
    );
}

// ============================================================================
// LF-Only Chunked Encoding Tests (GCP Discovery API compatibility)
// ============================================================================
// These tests verify handling of non-standard LF-only line endings used by
// GCP Discovery API. RFC 7230 requires CRLF, but GCP sends LF-only terminators.

/// Test: GCP-style LF-only chunk terminators.
///
/// GCP Discovery API sends chunked responses with LF-only (\n) instead of
/// proper CRLF (\r\n). This test verifies the parser handles this correctly.
///
/// Format sent by GCP:
///   <chunk-size>\n
///   <chunk-data>\n
///   0\n
///   \n
#[test]
fn test_gcp_lf_only_chunk_terminators() {
    let chunk1_data = b"{\"apis\": [";
    let chunk2_data = b"{\"name\": \"compute\"}";
    let chunk3_data = b"]}";

    // Build GCP-style LF-only chunked response (no \r characters)
    let mut raw_response = Vec::new();

    // Chunk 1 - LF only
    raw_response.extend(format!("{:x}\n", chunk1_data.len()).as_bytes());
    raw_response.extend_from_slice(chunk1_data);
    raw_response.extend_from_slice(b"\n");

    // Chunk 2 - LF only
    raw_response.extend(format!("{:x}\n", chunk2_data.len()).as_bytes());
    raw_response.extend_from_slice(chunk2_data);
    raw_response.extend_from_slice(b"\n");

    // Chunk 3 - LF only
    raw_response.extend(format!("{:x}\n", chunk3_data.len()).as_bytes());
    raw_response.extend_from_slice(chunk3_data);
    raw_response.extend_from_slice(b"\n");

    // Final chunk - LF only
    raw_response.extend_from_slice(b"0\n\n");

    let cursor = Cursor::new(raw_response);
    let stream = SharedByteBufferStream::ref_cell(cursor);

    let headers = SimpleHeaders::new();
    let mut iterator = SimpleHttpChunkIterator::new(vec![], headers, stream, 100);

    let mut collected_bytes = Vec::new();

    for result in &mut iterator {
        match result {
            Ok(ChunkedData::Data(data, _)) => {
                collected_bytes.extend_from_slice(&data);
            }
            Ok(ChunkedData::DataEnded) => break,
            Ok(ChunkedData::Trailers(_)) => {}
            Err(e) => panic!("Chunk iterator error: {:?}", e),
        }
    }

    // Expected: concatenated chunk data without any line endings
    let expected = b"{\"apis\": [{\"name\": \"compute\"}]}";

    assert_eq!(
        &collected_bytes,
        expected,
        "LF-only chunked data should not include line terminators. Got: {:?}, Expected: {:?}",
        String::from_utf8_lossy(&collected_bytes),
        String::from_utf8_lossy(expected)
    );

    // Verify no stray CR or LF in output
    for (i, byte) in collected_bytes.iter().enumerate() {
        assert_ne!(
            *byte, b'\r',
            "Found \\r at position {} - corruption in LF-only parsing",
            i
        );
    }
}

/// Test: Multiple chunks with LF-only terminators.
///
/// Stresses the LF-only handling with many small chunks to catch
/// cumulative corruption from improper line ending consumption.
#[test]
fn test_lf_only_multi_chunk() {
    let chunks: Vec<&[u8]> = vec![
        b"{\"", b"key", b"\":", b"\"", "value\nwith\nnewlines", b"\"", b"}",
    ];

    let mut raw_response = Vec::new();

    for chunk_data in &chunks {
        // LF-only chunk framing
        raw_response.extend(format!("{:x}\n", chunk_data.len()).as_bytes());
        raw_response.extend_from_slice(chunk_data);
        raw_response.extend_from_slice(b"\n");
    }

    // Final chunk - LF only
    raw_response.extend_from_slice(b"0\n\n");

    let cursor = Cursor::new(raw_response);
    let stream = SharedByteBufferStream::ref_cell(cursor);

    let headers = SimpleHeaders::new();
    let mut iterator = SimpleHttpChunkIterator::new(vec![], headers, stream, 100);

    let mut collected_bytes = Vec::new();

    for result in &mut iterator {
        match result {
            Ok(ChunkedData::Data(data, _)) => {
                collected_bytes.extend_from_slice(&data);
            }
            Ok(ChunkedData::DataEnded) => break,
            Err(e) => panic!("Chunk iterator error: {:?}", e),
            _ => {}
        }
    }

    let expected: Vec<u8> = chunks.iter().flat_map(|c| c.iter().copied()).collect();

    assert_eq!(
        &collected_bytes,
        &expected,
        "LF-only multi-chunk output corrupted. Got: {:?}, Expected: {:?}",
        String::from_utf8_lossy(&collected_bytes),
        String::from_utf8_lossy(&expected)
    );

    // Embedded newlines should be preserved
    let expected_newline_count = expected.iter().filter(|&&b| b == b'\n').count();
    let actual_newline_count = collected_bytes.iter().filter(|&&b| b == b'\n').count();
    assert_eq!(
        actual_newline_count, expected_newline_count,
        "Embedded newlines should be preserved"
    );
}

/// Test: Mixed CRLF and LF terminators in same stream.
///
/// Some servers may use inconsistent line endings. The parser should
/// handle both CRLF and LF terminators within the same response.
#[test]
fn test_mixed_crlf_and_lf() {
    let chunk1_data = b"first";   // Will use CRLF
    let chunk2_data = b"second";  // Will use LF
    let chunk3_data = b"third";   // Will use CRLF

    let mut raw_response = Vec::new();

    // Chunk 1 - standard CRLF
    raw_response.extend(format!("{:x}\r\n", chunk1_data.len()).as_bytes());
    raw_response.extend_from_slice(chunk1_data);
    raw_response.extend_from_slice(b"\r\n");

    // Chunk 2 - non-standard LF
    raw_response.extend(format!("{:x}\n", chunk2_data.len()).as_bytes());
    raw_response.extend_from_slice(chunk2_data);
    raw_response.extend_from_slice(b"\n");

    // Chunk 3 - back to CRLF
    raw_response.extend(format!("{:x}\r\n", chunk3_data.len()).as_bytes());
    raw_response.extend_from_slice(chunk3_data);
    raw_response.extend_from_slice(b"\r\n");

    // Final chunk - CRLF
    raw_response.extend_from_slice(b"0\r\n\r\n");

    let cursor = Cursor::new(raw_response);
    let stream = SharedByteBufferStream::ref_cell(cursor);

    let headers = SimpleHeaders::new();
    let mut iterator = SimpleHttpChunkIterator::new(vec![], headers, stream, 100);

    let mut collected_bytes = Vec::new();

    for result in &mut iterator {
        match result {
            Ok(ChunkedData::Data(data, _)) => {
                collected_bytes.extend_from_slice(&data);
            }
            Ok(ChunkedData::DataEnded) => break,
            Err(e) => panic!("Chunk iterator error: {:?}", e),
            _ => {}
        }
    }

    let expected = b"firstsecondthird";
    assert_eq!(
        &collected_bytes,
        expected,
        "Mixed CRLF/LF output corrupted. Got: {:?}, Expected: {:?}",
        String::from_utf8_lossy(&collected_bytes),
        String::from_utf8_lossy(expected)
    );
}

/// Test: Chunk data ending with \r character (content, not framing).
///
/// Verifies that \r characters that are part of the actual content
/// are preserved, while \r from HTTP framing is stripped.
#[test]
fn test_chunk_data_ending_with_cr() {
    // Content includes \r as actual data (like old Mac line endings)
    let chunk1_data = b"line1\rline2";
    let chunk2_data = b"line3";

    let mut raw_response = Vec::new();

    // Chunk 1 with LF-only framing (content has \r)
    raw_response.extend(format!("{:x}\n", chunk1_data.len()).as_bytes());
    raw_response.extend_from_slice(chunk1_data);
    raw_response.extend_from_slice(b"\n");

    // Chunk 2
    raw_response.extend(format!("{:x}\n", chunk2_data.len()).as_bytes());
    raw_response.extend_from_slice(chunk2_data);
    raw_response.extend_from_slice(b"\n");

    // Final chunk
    raw_response.extend_from_slice(b"0\n\n");

    let cursor = Cursor::new(raw_response);
    let stream = SharedByteBufferStream::ref_cell(cursor);

    let headers = SimpleHeaders::new();
    let mut iterator = SimpleHttpChunkIterator::new(vec![], headers, stream, 100);

    let mut collected_bytes = Vec::new();

    for result in &mut iterator {
        match result {
            Ok(ChunkedData::Data(data, _)) => {
                collected_bytes.extend_from_slice(&data);
            }
            Ok(ChunkedData::DataEnded) => break,
            Err(e) => panic!("Chunk iterator error: {:?}", e),
            _ => {}
        }
    }

    let expected = b"line1\rline2line3";
    assert_eq!(
        &collected_bytes,
        expected,
        "Content \\r must be preserved. Got: {:?}, Expected: {:?}",
        String::from_utf8_lossy(&collected_bytes),
        String::from_utf8_lossy(expected)
    );

    // Verify the \r from content is preserved (exactly 1)
    let cr_count = collected_bytes.iter().filter(|&&b| b == b'\r').count();
    assert_eq!(cr_count, 1, "Content \\r should be preserved (found {})", cr_count);
}
