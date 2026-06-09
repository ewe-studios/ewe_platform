use super::types::D1ChunkRef;

pub fn chunk_data(ino: u64, data: &[u8], chunk_size: usize) -> Vec<D1ChunkRef> {
    data.chunks(chunk_size)
        .enumerate()
        .map(|(idx, chunk)| D1ChunkRef {
            ino,
            chunk_idx: idx as u32,
            data: chunk.to_vec(),
        })
        .collect()
}

pub fn assemble_chunks(chunks: &mut [D1ChunkRef]) -> Vec<u8> {
    chunks.sort_by_key(|c| c.chunk_idx);
    chunks.iter().flat_map(|c| c.data.iter().copied()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let data = b"hello world, this is a test of chunking";
        let chunks = chunk_data(42, data, 10);
        assert_eq!(chunks.len(), 4);
        assert_eq!(chunks[0].chunk_idx, 0);
        assert_eq!(chunks[3].chunk_idx, 3);

        let mut reassembled = chunks;
        let result = assemble_chunks(&mut reassembled);
        assert_eq!(result, data);
    }

    #[test]
    fn exact_boundary() {
        let data = vec![0u8; 4096];
        let chunks = chunk_data(1, &data, 4096);
        assert_eq!(chunks.len(), 1);
    }

    #[test]
    fn empty_data() {
        let chunks = chunk_data(1, &[], 4096);
        assert!(chunks.is_empty());
    }
}
