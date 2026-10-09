use crate::flat::flat_top_k;
use crate::metric::DistanceMetric;
use crate::store::VectorMatch;
use std::collections::HashMap;
use std::sync::RwLock;

#[derive(Debug, Clone, PartialEq)]
pub enum VectorError {
    DimensionMismatch { expected: usize, got: usize },
    ContainsNaN,
    NotFound { id: String },
    Serialization(String),
    InvalidData(String),
}

impl core::fmt::Display for VectorError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::DimensionMismatch { expected, got } => {
                write!(f, "dimension mismatch: expected {expected}, got {got}")
            }
            Self::ContainsNaN => write!(f, "vector contains NaN"),
            Self::NotFound { id } => write!(f, "vector not found: {id}"),
            Self::Serialization(msg) => write!(f, "serialization error: {msg}"),
            Self::InvalidData(msg) => write!(f, "invalid data: {msg}"),
        }
    }
}

pub trait VectorIndex: Send + Sync {
    fn insert(&self, id: &str, vector: &[f32]) -> Result<(), VectorError>;
    fn remove(&self, id: &str) -> Result<(), VectorError>;
    fn search(&self, query: &[f32], k: usize) -> Result<Vec<VectorMatch>, VectorError>;
    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn to_bytes(&self) -> Vec<u8>;
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IndexKind {
    Flat = 0,
    Ivf = 1,
    Hnsw = 2,
}

const MAGIC: [u8; 4] = *b"VIDX";
const VERSION: u8 = 1;

fn metric_to_byte(m: DistanceMetric) -> u8 {
    match m {
        DistanceMetric::Cosine => 0,
        DistanceMetric::L2 => 1,
        DistanceMetric::Dot => 2,
    }
}

fn byte_to_metric(b: u8) -> Result<DistanceMetric, VectorError> {
    match b {
        0 => Ok(DistanceMetric::Cosine),
        1 => Ok(DistanceMetric::L2),
        2 => Ok(DistanceMetric::Dot),
        _ => Err(VectorError::InvalidData(format!("unknown metric: {b}"))),
    }
}

pub(crate) fn write_u32(buf: &mut Vec<u8>, v: u32) {
    buf.extend_from_slice(&v.to_le_bytes());
}

pub(crate) fn write_u64(buf: &mut Vec<u8>, v: u64) {
    buf.extend_from_slice(&v.to_le_bytes());
}

pub(crate) fn write_f32(buf: &mut Vec<u8>, v: f32) {
    buf.extend_from_slice(&v.to_le_bytes());
}

pub(crate) fn write_str(buf: &mut Vec<u8>, s: &str) {
    write_u32(buf, s.len() as u32);
    buf.extend_from_slice(s.as_bytes());
}

pub(crate) struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub(crate) fn read_bytes(&mut self, n: usize) -> Result<&'a [u8], VectorError> {
        if self.pos + n > self.data.len() {
            return Err(VectorError::Serialization("unexpected end of data".into()));
        }
        let slice = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(slice)
    }

    pub(crate) fn read_u8(&mut self) -> Result<u8, VectorError> {
        Ok(self.read_bytes(1)?[0])
    }

    pub(crate) fn read_u32(&mut self) -> Result<u32, VectorError> {
        let bytes = self.read_bytes(4)?;
        Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
    }

    pub(crate) fn read_u64(&mut self) -> Result<u64, VectorError> {
        let bytes = self.read_bytes(8)?;
        Ok(u64::from_le_bytes(bytes.try_into().unwrap()))
    }

    pub(crate) fn read_f32(&mut self) -> Result<f32, VectorError> {
        let bytes = self.read_bytes(4)?;
        Ok(f32::from_le_bytes(bytes.try_into().unwrap()))
    }

    pub(crate) fn read_str(&mut self) -> Result<String, VectorError> {
        let len = self.read_u32()? as usize;
        let bytes = self.read_bytes(len)?;
        String::from_utf8(bytes.to_vec())
            .map_err(|e| VectorError::Serialization(format!("invalid utf8: {e}")))
    }
}

pub(crate) fn validate_insert(dimension: usize, vector: &[f32]) -> Result<(), VectorError> {
    if vector.len() != dimension {
        return Err(VectorError::DimensionMismatch {
            expected: dimension,
            got: vector.len(),
        });
    }
    if vector.iter().any(|v| v.is_nan()) {
        return Err(VectorError::ContainsNaN);
    }
    Ok(())
}

pub(crate) fn write_common_header(buf: &mut Vec<u8>, kind: IndexKind, metric: DistanceMetric, dimension: u32) {
    buf.extend_from_slice(&MAGIC);
    buf.push(VERSION);
    buf.push(kind as u8);
    buf.push(metric_to_byte(metric));
    write_u32(buf, dimension);
}

fn read_common_header(reader: &mut Reader<'_>) -> Result<(IndexKind, DistanceMetric, usize), VectorError> {
    let magic = reader.read_bytes(4)?;
    if magic != MAGIC {
        return Err(VectorError::Serialization("invalid magic".into()));
    }
    let version = reader.read_u8()?;
    if version != VERSION {
        return Err(VectorError::Serialization(format!("unsupported version: {version}")));
    }
    let kind_byte = reader.read_u8()?;
    let kind = match kind_byte {
        0 => IndexKind::Flat,
        1 => IndexKind::Ivf,
        2 => IndexKind::Hnsw,
        _ => return Err(VectorError::Serialization(format!("unknown index kind: {kind_byte}"))),
    };
    let metric = byte_to_metric(reader.read_u8()?)?;
    let dimension = reader.read_u32()? as usize;
    Ok((kind, metric, dimension))
}

pub fn load_index(bytes: &[u8]) -> Result<Box<dyn VectorIndex>, VectorError> {
    let mut reader = Reader::new(bytes);
    let (kind, metric, dimension) = read_common_header(&mut reader)?;

    match kind {
        IndexKind::Flat => {
            let count = reader.read_u64()? as usize;
            let index = FlatIndex::new(metric, dimension);
            for _ in 0..count {
                let id = reader.read_str()?;
                let mut vec = Vec::with_capacity(dimension);
                for _ in 0..dimension {
                    vec.push(reader.read_f32()?);
                }
                index.insert(&id, &vec)?;
            }
            Ok(Box::new(index))
        }
        IndexKind::Ivf => {
            crate::ivf::load_ivf_from_reader(&mut reader, metric, dimension)
        }
        IndexKind::Hnsw => {
            crate::hnsw::load_hnsw_from_reader(&mut reader, metric, dimension)
        }
    }
}

// ---------------------------------------------------------------------------
// FlatIndex

struct FlatInner {
    entries: HashMap<String, Vec<f32>>,
}

pub struct FlatIndex {
    metric: DistanceMetric,
    dimension: usize,
    inner: RwLock<FlatInner>,
}

impl FlatIndex {
    #[must_use]
    pub fn new(metric: DistanceMetric, dimension: usize) -> Self {
        Self {
            metric,
            dimension,
            inner: RwLock::new(FlatInner {
                entries: HashMap::new(),
            }),
        }
    }
}

impl VectorIndex for FlatIndex {
    fn insert(&self, id: &str, vector: &[f32]) -> Result<(), VectorError> {
        validate_insert(self.dimension, vector)?;
        let mut inner = self.inner.write().unwrap();
        inner.entries.insert(id.to_string(), vector.to_vec());
        Ok(())
    }

    fn remove(&self, id: &str) -> Result<(), VectorError> {
        let mut inner = self.inner.write().unwrap();
        inner
            .entries
            .remove(id)
            .ok_or_else(|| VectorError::NotFound {
                id: id.to_string(),
            })?;
        Ok(())
    }

    fn search(&self, query: &[f32], k: usize) -> Result<Vec<VectorMatch>, VectorError> {
        if query.len() != self.dimension {
            return Err(VectorError::DimensionMismatch {
                expected: self.dimension,
                got: query.len(),
            });
        }
        let inner = self.inner.read().unwrap();
        let iter = inner
            .entries
            .iter()
            .map(|(id, vec)| (id.as_str(), vec.as_slice()));
        Ok(flat_top_k(query, iter, k, self.metric))
    }

    fn len(&self) -> usize {
        self.inner.read().unwrap().entries.len()
    }

    fn to_bytes(&self) -> Vec<u8> {
        let inner = self.inner.read().unwrap();
        let mut buf = Vec::new();
        write_common_header(&mut buf, IndexKind::Flat, self.metric, self.dimension as u32);
        write_u64(&mut buf, inner.entries.len() as u64);
        for (id, vec) in &inner.entries {
            write_str(&mut buf, id);
            for &v in vec {
                write_f32(&mut buf, v);
            }
        }
        buf
    }
}
