use crate::index::{
    IndexKind, Reader, VectorError, VectorIndex, validate_insert, write_common_header, write_f32,
    write_str, write_u32, write_u64,
};
use crate::metric::{DistanceMetric, OrderedScore};
use crate::store::VectorMatch;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::sync::RwLock;

struct HnswNode {
    id: String,
    vector: Vec<f32>,
    level: usize,
    neighbors: Vec<Vec<usize>>,
    deleted: bool,
}

struct HnswInner {
    nodes: Vec<HnswNode>,
    id_map: HashMap<String, usize>,
    entry_point: Option<usize>,
    max_level: usize,
    rng_state: u64,
    deleted_count: usize,
}

pub struct HnswIndex {
    metric: DistanceMetric,
    dimension: usize,
    m: usize,
    m_max0: usize,
    ef_construction: usize,
    ef_search: usize,
    seed: u64,
    level_mult: f32,
    inner: RwLock<HnswInner>,
}

impl HnswIndex {
    #[must_use]
    pub fn new(metric: DistanceMetric, dimension: usize) -> Self {
        Self::with_params(metric, dimension, 16, 200, 64, 42)
    }

    #[must_use]
    pub fn with_params(
        metric: DistanceMetric,
        dimension: usize,
        m: usize,
        ef_construction: usize,
        ef_search: usize,
        seed: u64,
    ) -> Self {
        let m_max0 = m * 2;
        let level_mult = 1.0 / libm::logf(m as f32);
        Self {
            metric,
            dimension,
            m,
            m_max0,
            ef_construction,
            ef_search,
            seed,
            level_mult,
            inner: RwLock::new(HnswInner {
                nodes: Vec::new(),
                id_map: HashMap::new(),
                entry_point: None,
                max_level: 0,
                rng_state: seed.wrapping_add(1),
                deleted_count: 0,
            }),
        }
    }

    fn random_level(inner: &mut HnswInner, level_mult: f32) -> usize {
        let r = next_rng(&mut inner.rng_state);
        let f = (r >> 11) as f64 / (1u64 << 53) as f64;
        let f = f.max(1e-15);
        (-libm::logf(f as f32) * level_mult) as usize
    }

    fn distance(&self, a: &[f32], b: &[f32]) -> f32 {
        self.metric.score(a, b)
    }

    fn search_layer(
        &self,
        inner: &HnswInner,
        query: &[f32],
        entry: usize,
        ef: usize,
        level: usize,
    ) -> Vec<(usize, f32)> {
        let mut visited = HashSet::new();
        let mut candidates: BinaryHeap<std::cmp::Reverse<(OrderedScore, usize)>> = BinaryHeap::new();
        let mut results: BinaryHeap<(OrderedScore, usize)> = BinaryHeap::new();

        let d = self.distance(query, &inner.nodes[entry].vector);
        visited.insert(entry);
        candidates.push(std::cmp::Reverse((OrderedScore(d), entry)));
        results.push((OrderedScore(d), entry));

        while let Some(std::cmp::Reverse((c_dist, c_idx))) = candidates.pop() {
            let worst_result = results.peek().map_or(f32::NEG_INFINITY, |(s, _)| s.0);
            if c_dist.0 < worst_result && results.len() >= ef {
                break;
            }

            let node = &inner.nodes[c_idx];
            if level < node.neighbors.len() {
                for &neighbor_idx in &node.neighbors[level] {
                    if visited.contains(&neighbor_idx) {
                        continue;
                    }
                    visited.insert(neighbor_idx);

                    if inner.nodes[neighbor_idx].deleted {
                        continue;
                    }

                    let nd = self.distance(query, &inner.nodes[neighbor_idx].vector);
                    let worst = results.peek().map_or(f32::NEG_INFINITY, |(s, _)| s.0);

                    if results.len() < ef || nd > worst {
                        candidates.push(std::cmp::Reverse((OrderedScore(nd), neighbor_idx)));
                        results.push((OrderedScore(nd), neighbor_idx));
                        if results.len() > ef {
                            results.pop();
                        }
                    }
                }
            }
        }

        results.into_iter().map(|(s, idx)| (idx, s.0)).collect()
    }

    fn greedy_search(
        &self,
        inner: &HnswInner,
        query: &[f32],
        entry: usize,
        level: usize,
    ) -> usize {
        let mut current = entry;
        let mut current_dist = self.distance(query, &inner.nodes[current].vector);

        loop {
            let mut changed = false;
            let node = &inner.nodes[current];
            if level < node.neighbors.len() {
                for &neighbor_idx in &node.neighbors[level] {
                    let nd = self.distance(query, &inner.nodes[neighbor_idx].vector);
                    if nd > current_dist {
                        current = neighbor_idx;
                        current_dist = nd;
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }

        current
    }

    fn select_neighbors(
        inner: &HnswInner,
        candidates: &[(usize, f32)],
        m: usize,
    ) -> Vec<usize> {
        let mut sorted: Vec<(usize, f32)> = candidates
            .iter()
            .filter(|(idx, _)| !inner.nodes[*idx].deleted)
            .copied()
            .collect();
        sorted.sort_by(|a, b| {
            b.1.partial_cmp(&a.1).unwrap_or(core::cmp::Ordering::Equal)
        });
        sorted.truncate(m);
        sorted.iter().map(|(idx, _)| *idx).collect()
    }
}

impl VectorIndex for HnswIndex {
    fn insert(&self, id: &str, vector: &[f32]) -> Result<(), VectorError> {
        validate_insert(self.dimension, vector)?;
        let mut inner = self.inner.write().unwrap();

        let mut ep_was_deleted = false;
        if let Some(&existing) = inner.id_map.get(id) {
            inner.nodes[existing].deleted = true;
            inner.deleted_count += 1;
            if inner.entry_point == Some(existing) {
                ep_was_deleted = true;
            }
        }

        let level = Self::random_level(&mut inner, self.level_mult);
        let node_idx = inner.nodes.len();
        let mut neighbors = Vec::with_capacity(level + 1);
        for _ in 0..=level {
            neighbors.push(Vec::new());
        }

        inner.nodes.push(HnswNode {
            id: id.to_string(),
            vector: vector.to_vec(),
            level,
            neighbors,
            deleted: false,
        });
        inner.id_map.insert(id.to_string(), node_idx);

        let Some(ep) = inner.entry_point else {
            inner.entry_point = Some(node_idx);
            inner.max_level = level;
            return Ok(());
        };

        let mut current_ep = ep;

        for lev in (level + 1..=inner.max_level).rev() {
            current_ep = self.greedy_search(&inner, vector, current_ep, lev);
        }

        let insert_level = level.min(inner.max_level);
        for lev in (0..=insert_level).rev() {
            let candidates = self.search_layer(&inner, vector, current_ep, self.ef_construction, lev);

            let max_connections = if lev == 0 { self.m_max0 } else { self.m };
            let selected = Self::select_neighbors(&inner, &candidates, max_connections);

            inner.nodes[node_idx].neighbors[lev].clone_from(&selected);

            for &neighbor in &selected {
                let neighbor_max = if lev == 0 { self.m_max0 } else { self.m };
                if lev < inner.nodes[neighbor].neighbors.len() {
                    inner.nodes[neighbor].neighbors[lev].push(node_idx);
                    if inner.nodes[neighbor].neighbors[lev].len() > neighbor_max {
                        let neighbor_vec = inner.nodes[neighbor].vector.clone();
                        let nb_neighbors: Vec<(usize, f32)> = inner.nodes[neighbor].neighbors[lev]
                            .iter()
                            .map(|&ni| (ni, self.distance(&neighbor_vec, &inner.nodes[ni].vector)))
                            .collect();
                        let pruned = Self::select_neighbors(&inner, &nb_neighbors, neighbor_max);
                        inner.nodes[neighbor].neighbors[lev] = pruned;
                    }
                }
            }

            if !candidates.is_empty() {
                current_ep = candidates
                    .iter()
                    .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(core::cmp::Ordering::Equal))
                    .map_or(current_ep, |(idx, _)| *idx);
            }
        }

        if level > inner.max_level || ep_was_deleted {
            inner.entry_point = Some(node_idx);
            inner.max_level = inner.max_level.max(level);
        }

        Ok(())
    }

    fn remove(&self, id: &str) -> Result<(), VectorError> {
        let mut inner = self.inner.write().unwrap();
        let &idx = inner
            .id_map
            .get(id)
            .ok_or_else(|| VectorError::NotFound {
                id: id.to_string(),
            })?;

        if !inner.nodes[idx].deleted {
            inner.nodes[idx].deleted = true;
            inner.deleted_count += 1;
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

        let Some(ep) = inner.entry_point else {
            return Ok(Vec::new());
        };

        let mut current_ep = ep;
        for lev in (1..=inner.max_level).rev() {
            current_ep = self.greedy_search(&inner, query, current_ep, lev);
        }

        let ef = self.ef_search.max(k);
        let candidates = self.search_layer(&inner, query, current_ep, ef, 0);

        let mut results: Vec<VectorMatch> = candidates
            .into_iter()
            .filter(|(idx, _)| !inner.nodes[*idx].deleted)
            .map(|(idx, score)| VectorMatch {
                id: inner.nodes[idx].id.clone(),
                score,
            })
            .collect();

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(core::cmp::Ordering::Equal));
        results.truncate(k);
        Ok(results)
    }

    fn len(&self) -> usize {
        let inner = self.inner.read().unwrap();
        inner.id_map.len() - inner.deleted_count
    }

    fn to_bytes(&self) -> Vec<u8> {
        let inner = self.inner.read().unwrap();
        let mut buf = Vec::new();
        write_common_header(&mut buf, IndexKind::Hnsw, self.metric, self.dimension as u32);

        write_u32(&mut buf, self.m as u32);
        write_u32(&mut buf, self.ef_construction as u32);
        write_u32(&mut buf, self.ef_search as u32);
        write_u64(&mut buf, self.seed);

        let live_nodes: Vec<(usize, &HnswNode)> = inner
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| !n.deleted)
            .collect();

        let mut old_to_new: HashMap<usize, usize> = HashMap::new();
        for (new_idx, (old_idx, _)) in live_nodes.iter().enumerate() {
            old_to_new.insert(*old_idx, new_idx);
        }

        write_u64(&mut buf, live_nodes.len() as u64);
        write_u32(&mut buf, inner.max_level as u32);
        let new_ep = inner
            .entry_point
            .and_then(|ep| old_to_new.get(&ep).copied())
            .unwrap_or(0);
        write_u64(&mut buf, new_ep as u64);

        for (_, node) in &live_nodes {
            write_str(&mut buf, &node.id);
            write_u32(&mut buf, node.level as u32);
            for &v in &node.vector {
                write_f32(&mut buf, v);
            }
            write_u32(&mut buf, node.neighbors.len() as u32);
            for level_neighbors in &node.neighbors {
                let remapped: Vec<u32> = level_neighbors
                    .iter()
                    .filter_map(|&old| old_to_new.get(&old).map(|&n| n as u32))
                    .collect();
                write_u32(&mut buf, remapped.len() as u32);
                for idx in remapped {
                    write_u32(&mut buf, idx);
                }
            }
        }

        buf
    }
}

pub(crate) fn load_hnsw_from_reader(
    reader: &mut Reader<'_>,
    metric: DistanceMetric,
    dimension: usize,
) -> Result<Box<dyn VectorIndex>, VectorError> {
    let m = reader.read_u32()? as usize;
    let ef_construction = reader.read_u32()? as usize;
    let ef_search = reader.read_u32()? as usize;
    let seed = reader.read_u64()?;

    let num_nodes = reader.read_u64()? as usize;
    let max_level = reader.read_u32()? as usize;
    let entry_point_idx = reader.read_u64()? as usize;

    let mut nodes = Vec::with_capacity(num_nodes);
    let mut id_map = HashMap::with_capacity(num_nodes);

    for node_idx in 0..num_nodes {
        let id = reader.read_str()?;
        let level = reader.read_u32()? as usize;
        let mut vector = Vec::with_capacity(dimension);
        for _ in 0..dimension {
            vector.push(reader.read_f32()?);
        }
        let num_levels = reader.read_u32()? as usize;
        let mut neighbors = Vec::with_capacity(num_levels);
        for _ in 0..num_levels {
            let count = reader.read_u32()? as usize;
            let mut level_neighbors = Vec::with_capacity(count);
            for _ in 0..count {
                level_neighbors.push(reader.read_u32()? as usize);
            }
            neighbors.push(level_neighbors);
        }
        id_map.insert(id.clone(), node_idx);
        nodes.push(HnswNode {
            id,
            vector,
            level,
            neighbors,
            deleted: false,
        });
    }

    let m_max0 = m * 2;
    let level_mult = 1.0 / libm::logf(m as f32);
    let entry_point = if num_nodes > 0 {
        Some(entry_point_idx)
    } else {
        None
    };

    let index = HnswIndex {
        metric,
        dimension,
        m,
        m_max0,
        ef_construction,
        ef_search,
        seed,
        level_mult,
        inner: RwLock::new(HnswInner {
            nodes,
            id_map,
            entry_point,
            max_level,
            rng_state: seed.wrapping_add(1),
            deleted_count: 0,
        }),
    };
    Ok(Box::new(index))
}

fn next_rng(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    let mut x = *state;
    x ^= x >> 30;
    x = x.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^= x >> 31;
    x
}
