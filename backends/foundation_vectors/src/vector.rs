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
