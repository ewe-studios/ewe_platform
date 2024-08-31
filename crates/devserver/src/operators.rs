use derive_more::From;
use std::result;

use crossbeam::channel;

use crate::types::{BoxedError, JoinHandle};

pub trait Operator {
    fn run(&self, signal: channel::Receiver<()>) -> JoinHandle<()>;
}

#[derive(Debug, From)]
pub enum OperationsError {
    FailedOperatorWait,
}

impl std::error::Error for OperationsError {}

impl core::fmt::Display for OperationsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

pub struct OperationsManager {
    operators: Vec<Box<dyn Operator>>,
}

impl Default for OperationsManager {
    fn default() -> Self {
        Self {
            operators: Vec::new(),
        }
    }
}

impl OperationsManager {
    pub fn add(&mut self, op: Box<dyn Operator>) {
        self.operators.push(op)
    }

    pub fn add_all<I>(&mut self, ops: I)
    where
        I: IntoIterator<Item = Box<dyn Operator>>,
    {
        for op in ops {
            self.operators.push(op)
        }
    }

    pub async fn run(&self, sig: channel::Receiver<()>) -> result::Result<(), BoxedError> {
        let operations =
            futures::future::join_all(self.operators.iter().map(|t| t.run(sig.clone())));
        let result_list = operations.await;
        for result in result_list {
            match result {
                Ok(_) => continue,
                Err(err) => {
                    ewe_logs::error!("Failed to complete operator: {:?}", err);
                    return Err(Box::new(OperationsError::FailedOperatorWait));
                }
            }
        }

        Ok(())
    }
}
