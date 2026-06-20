//! Dense embedding vector with dimension tracking.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Vector {
    pub data: Vec<f32>,
}

impl Vector {
    #[must_use]
    pub fn new(data: Vec<f32>) -> Self {
        Self { data }
    }

    #[must_use]
    pub fn dimension(&self) -> usize {
        self.data.len()
    }

    #[must_use]
    pub fn as_slice(&self) -> &[f32] {
        &self.data
    }

    #[must_use]
    pub fn magnitude_squared(&self) -> f32 {
        self.data.iter().map(|x| x * x).sum()
    }

    #[must_use]
    pub fn magnitude(&self) -> f32 {
        libm::sqrtf(self.magnitude_squared())
    }

    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.magnitude_squared() == 0.0
    }

    #[must_use]
    pub fn normalize(&self) -> Option<Self> {
        let mag = self.magnitude();
        if mag == 0.0 {
            return None;
        }
        let inv = 1.0 / mag;
        Some(Self {
            data: self.data.iter().map(|x| x * inv).collect(),
        })
    }
}

impl From<Vec<f32>> for Vector {
    fn from(data: Vec<f32>) -> Self {
        Self { data }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dimension_and_slice() {
        let v = Vector::new(vec![1.0, 2.0, 3.0]);
        assert_eq!(v.dimension(), 3);
        assert_eq!(v.as_slice(), &[1.0, 2.0, 3.0]);
    }

    #[test]
    fn magnitude() {
        let v = Vector::new(vec![3.0, 4.0]);
        assert!((v.magnitude() - 5.0).abs() < 1e-6);
    }

    #[test]
    fn zero_vector() {
        let v = Vector::new(vec![0.0, 0.0]);
        assert!(v.is_zero());
        assert!(v.normalize().is_none());
    }

    #[test]
    fn normalize_unit() {
        let v = Vector::new(vec![3.0, 4.0]);
        let n = v.normalize().unwrap();
        assert!((n.magnitude() - 1.0).abs() < 1e-6);
        assert!((n.data[0] - 0.6).abs() < 1e-6);
        assert!((n.data[1] - 0.8).abs() < 1e-6);
    }
}
