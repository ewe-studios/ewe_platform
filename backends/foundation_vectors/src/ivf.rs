use crate::flat::flat_top_k;
use crate::index::{
    IndexKind, Reader, VectorError, VectorIndex, validate_insert, write_common_header,
    write_f32, write_str, write_u32, write_u64,
};
use crate::metric::DistanceMetric;
use crate::store::VectorMatch;
use std::collections::HashMap;
use std::sync::RwLock;

struct IvfInner {
    centroids: Vec<Vec<f32>>,
    postings: Vec<Vec<(String, Vec<f32>)>>,
    assignments: HashMap<String, usize>,
    insert_count_since_build: usize,
    size_at_build: usize,
}

pub struct IvfIndex {
    metric: DistanceMetric,
    dimension: usize,
    nprobe: usize,
    nlist: usize,
    seed: u64,
    inner: RwLock<IvfInner>,
}

impl IvfIndex {
    #[must_use]
    pub fn new(metric: DistanceMetric, dimension: usize, nlist: usize, nprobe: usize) -> Self {
        Self::with_seed(metric, dimension, nlist, nprobe, 42)
    }

    #[must_use]
    pub fn with_seed(
        metric: DistanceMetric,
        dimension: usize,
        nlist: usize,
        nprobe: usize,
        seed: u64,
    ) -> Self {
        Self {
            metric,
            dimension,
            nprobe,
            nlist,
            seed,
            inner: RwLock::new(IvfInner {
                centroids: Vec::new(),
                postings: Vec::new(),
                assignments: HashMap::new(),
                insert_count_since_build: 0,
                size_at_build: 0,
            }),
        }
    }

    fn nearest_centroid(
        centroids: &[Vec<f32>],
        vector: &[f32],
        metric: DistanceMetric,
    ) -> usize {
        centroids
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| {
                let sa = metric.score(vector, a);
                let sb = metric.score(vector, b);
                sa.partial_cmp(&sb).unwrap_or(core::cmp::Ordering::Equal)
            })
            .map_or(0, |(i, _)| i)
    }

    fn nearest_centroids(
        centroids: &[Vec<f32>],
        query: &[f32],
        n: usize,
        metric: DistanceMetric,
    ) -> Vec<usize> {
        let mut scored: Vec<(usize, f32)> = centroids
            .iter()
            .enumerate()
            .map(|(i, c)| (i, metric.score(query, c)))
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(core::cmp::Ordering::Equal));
        scored.iter().take(n).map(|(i, _)| *i).collect()
    }

    fn build_centroids(&self, inner: &mut IvfInner) {
        let all_vectors: Vec<(&str, &[f32])> = inner
            .postings
            .iter()
            .flat_map(|p| p.iter().map(|(id, v)| (id.as_str(), v.as_slice())))
            .collect();

        if all_vectors.is_empty() {
            inner.centroids.clear();
            inner.postings.clear();
            return;
        }

        let n = all_vectors.len();
        let k = self.nlist.min(n);

        let centroids = kmeans(&all_vectors, k, self.dimension, self.metric, self.seed);

        let mut new_postings = vec![Vec::new(); k];
        let mut new_assignments = HashMap::with_capacity(n);

        for (id, vec) in &all_vectors {
            let ci = Self::nearest_centroid(&centroids, vec, self.metric);
            new_assignments.insert((*id).to_string(), ci);
            new_postings[ci].push(((*id).to_string(), vec.to_vec()));
        }

        inner.centroids = centroids;
        inner.postings = new_postings;
        inner.assignments = new_assignments;
        inner.insert_count_since_build = 0;
        inner.size_at_build = n;
    }

    fn maybe_rebuild(&self, inner: &mut IvfInner) {
        if inner.size_at_build > 0
            && inner.insert_count_since_build > inner.size_at_build
        {
            self.build_centroids(inner);
        }
    }
}

impl VectorIndex for IvfIndex {
    fn insert(&self, id: &str, vector: &[f32]) -> Result<(), VectorError> {
        validate_insert(self.dimension, vector)?;
        let mut inner = self.inner.write().unwrap();

        if let Some(old_ci) = inner.assignments.remove(id) {
            if let Some(posting) = inner.postings.get_mut(old_ci) {
                posting.retain(|(pid, _)| pid != id);
            }
        }

        if inner.centroids.is_empty() {
            if inner.postings.is_empty() {
                inner.postings.push(Vec::new());
            }
            inner.postings[0].push((id.to_string(), vector.to_vec()));
            inner.assignments.insert(id.to_string(), 0);
            inner.insert_count_since_build += 1;

            let total: usize = inner.postings.iter().map(Vec::len).sum();
            if total >= self.nlist * 2 && inner.centroids.is_empty() {
                self.build_centroids(&mut inner);
            }
        } else {
            let ci = Self::nearest_centroid(&inner.centroids, vector, self.metric);
            inner.postings[ci].push((id.to_string(), vector.to_vec()));
            inner.assignments.insert(id.to_string(), ci);
            inner.insert_count_since_build += 1;
            self.maybe_rebuild(&mut inner);
        }

        Ok(())
    }

    fn remove(&self, id: &str) -> Result<(), VectorError> {
        let mut inner = self.inner.write().unwrap();
        let ci = inner
            .assignments
            .remove(id)
            .ok_or_else(|| VectorError::NotFound {
                id: id.to_string(),
            })?;
        if let Some(posting) = inner.postings.get_mut(ci) {
            posting.retain(|(pid, _)| pid != id);
        }
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

        if inner.centroids.is_empty() {
            let iter = inner
                .postings
                .iter()
                .flat_map(|p| p.iter().map(|(id, v)| (id.as_str(), v.as_slice())));
            return Ok(flat_top_k(query, iter, k, self.metric));
        }

        let probes = Self::nearest_centroids(&inner.centroids, query, self.nprobe, self.metric);
        let iter = probes
            .iter()
            .filter_map(|&ci| inner.postings.get(ci))
            .flat_map(|p| p.iter().map(|(id, v)| (id.as_str(), v.as_slice())));
        Ok(flat_top_k(query, iter, k, self.metric))
    }

    fn len(&self) -> usize {
        self.inner.read().unwrap().assignments.len()
    }

    fn to_bytes(&self) -> Vec<u8> {
        let inner = self.inner.read().unwrap();
        let mut buf = Vec::new();
        write_common_header(&mut buf, IndexKind::Ivf, self.metric, self.dimension as u32);

        write_u32(&mut buf, self.nlist as u32);
        write_u32(&mut buf, self.nprobe as u32);
        write_u64(&mut buf, self.seed);

        write_u32(&mut buf, inner.centroids.len() as u32);
        for centroid in &inner.centroids {
            for &v in centroid {
                write_f32(&mut buf, v);
            }
        }

        write_u32(&mut buf, inner.postings.len() as u32);
        for posting in &inner.postings {
            write_u32(&mut buf, posting.len() as u32);
            for (id, vec) in posting {
                write_str(&mut buf, id);
                for &v in vec {
                    write_f32(&mut buf, v);
                }
            }
        }

        buf
    }
}

pub(crate) fn load_ivf_from_reader(
    reader: &mut Reader<'_>,
    metric: DistanceMetric,
    dimension: usize,
) -> Result<Box<dyn VectorIndex>, VectorError> {
    let nlist = reader.read_u32()? as usize;
    let nprobe = reader.read_u32()? as usize;
    let seed = reader.read_u64()?;

    let num_centroids = reader.read_u32()? as usize;
    let mut centroids = Vec::with_capacity(num_centroids);
    for _ in 0..num_centroids {
        let mut c = Vec::with_capacity(dimension);
        for _ in 0..dimension {
            c.push(reader.read_f32()?);
        }
        centroids.push(c);
    }

    let num_postings = reader.read_u32()? as usize;
    let mut postings = Vec::with_capacity(num_postings);
    let mut assignments = HashMap::new();
    for ci in 0..num_postings {
        let count = reader.read_u32()? as usize;
        let mut posting = Vec::with_capacity(count);
        for _ in 0..count {
            let id = reader.read_str()?;
            let mut vec = Vec::with_capacity(dimension);
            for _ in 0..dimension {
                vec.push(reader.read_f32()?);
            }
            assignments.insert(id.clone(), ci);
            posting.push((id, vec));
        }
        postings.push(posting);
    }

    let total: usize = postings.iter().map(Vec::len).sum();
    let index = IvfIndex {
        metric,
        dimension,
        nprobe,
        nlist,
        seed,
        inner: RwLock::new(IvfInner {
            centroids,
            postings,
            assignments,
            insert_count_since_build: 0,
            size_at_build: total,
        }),
    };
    Ok(Box::new(index))
}

// ---------------------------------------------------------------------------
// k-means (seeded, deterministic)

fn kmeans(
    vectors: &[(&str, &[f32])],
    k: usize,
    dimension: usize,
    metric: DistanceMetric,
    seed: u64,
) -> Vec<Vec<f32>> {
    if vectors.is_empty() || k == 0 {
        return Vec::new();
    }

    let k = k.min(vectors.len());
    let mut centroids = kmeans_pp_init(vectors, k, metric, seed);

    for _ in 0..20 {
        let mut sums = vec![vec![0.0f32; dimension]; k];
        let mut counts = vec![0usize; k];

        for (_, vec) in vectors {
            let ci = centroids
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| {
                    let sa = metric.score(vec, a);
                    let sb = metric.score(vec, b);
                    sa.partial_cmp(&sb).unwrap_or(core::cmp::Ordering::Equal)
                })
                .map_or(0, |(i, _)| i);

            counts[ci] += 1;
            for (j, &v) in vec.iter().enumerate() {
                sums[ci][j] += v;
            }
        }

        let mut changed = false;
        for (i, centroid) in centroids.iter_mut().enumerate() {
            if counts[i] == 0 {
                continue;
            }
            let inv = 1.0 / counts[i] as f32;
            for (j, val) in centroid.iter_mut().enumerate() {
                let new_val = sums[i][j] * inv;
                if (*val - new_val).abs() > 1e-7 {
                    changed = true;
                }
                *val = new_val;
            }
        }

        if !changed {
            break;
        }
    }

    centroids
}

fn kmeans_pp_init(
    vectors: &[(&str, &[f32])],
    k: usize,
    metric: DistanceMetric,
    seed: u64,
) -> Vec<Vec<f32>> {
    let mut rng = SimpleRng::new(seed);
    let mut centroids = Vec::with_capacity(k);

    let first = rng.next_usize(vectors.len());
    centroids.push(vectors[first].1.to_vec());

    for _ in 1..k {
        let mut distances = Vec::with_capacity(vectors.len());
        let mut total = 0.0f64;

        for (_, vec) in vectors {
            let min_dist = centroids
                .iter()
                .map(|c| {
                    let s = metric.score(vec, c);
                    // Convert similarity to distance (higher similarity = lower distance)
                    1.0 - s.clamp(-1.0, 1.0)
                })
                .fold(f32::INFINITY, f32::min);

            let d = f64::from((min_dist * min_dist).max(0.0));
            total += d;
            distances.push(d);
        }

        if total <= 0.0 {
            let idx = rng.next_usize(vectors.len());
            centroids.push(vectors[idx].1.to_vec());
            continue;
        }

        let threshold = rng.next_f64() * total;
        let mut cumulative = 0.0;
        let mut chosen = vectors.len() - 1;
        for (i, &d) in distances.iter().enumerate() {
            cumulative += d;
            if cumulative >= threshold {
                chosen = i;
                break;
            }
        }
        centroids.push(vectors[chosen].1.to_vec());
    }

    centroids
}

struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    fn new(seed: u64) -> Self {
        Self {
            state: seed.wrapping_add(1),
        }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        let mut x = self.state;
        x ^= x >> 30;
        x = x.wrapping_mul(0xbf58_476d_1ce4_e5b9);
        x ^= x >> 27;
        x = x.wrapping_mul(0x94d0_49bb_1331_11eb);
        x ^= x >> 31;
        x
    }

    fn next_usize(&mut self, bound: usize) -> usize {
        (self.next_u64() % bound as u64) as usize
    }

    fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}
