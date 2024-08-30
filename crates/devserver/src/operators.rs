use derive_more::From;
use std::result;
use tokio::task::JoinHandle;

use crossbeam::channel;

use crate::types::BoxedError;

pub trait Operator {
    fn run(&self, signal: channel::Receiver<()>) -> JoinHandle<result::Result<(), BoxedError>>;
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
        for handle in self.operators.iter() {
            let joiner = handle.run(sig.clone());
            match joiner.await {
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
