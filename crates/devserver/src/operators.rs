use derive_more::From;
use std::sync;
use tokio::sync::broadcast;

use crate::types::JoinHandle;

pub trait Operator {
    fn run(&self, cancel_signal: broadcast::Receiver<()>) -> JoinHandle<()>;
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

pub struct SequentialOps {
    operators: sync::Arc<Vec<Box<dyn Operator + Send + Sync>>>,
}

impl SequentialOps {
    pub fn new(items: Vec<Box<dyn Operator + Send + Sync>>) -> Self {
        Self {
            operators: sync::Arc::new(items),
        }
    }
}

impl Operator for SequentialOps {
    fn run(&self, signal: broadcast::Receiver<()>) -> JoinHandle<()> {
        let jobs = self.operators.clone();

        tokio::spawn(async move {
            for job in jobs.iter() {
                match job.run(signal.resubscribe()).await {
                    Ok(_) => continue,
                    Err(err) => {
                        ewe_logs::error!("Failed to complete operator: {:?}", err);
                        return Err(Box::new(OperationsError::FailedOperatorWait).into());
                    }
                }
            }

            Ok(())
        })
    }
}

pub struct ParrellelOps {
    operators: sync::Arc<Vec<Box<dyn Operator + Send + Sync>>>,
}

impl ParrellelOps {
    pub fn new(items: Vec<Box<dyn Operator + Send + Sync>>) -> Self {
        Self {
            operators: sync::Arc::new(items),
        }
    }
}

impl Operator for ParrellelOps {
    fn run(&self, signal: broadcast::Receiver<()>) -> JoinHandle<()> {
        let operations =
            futures::future::join_all(self.operators.iter().map(|t| t.run(signal.resubscribe())));
        tokio::spawn(async move {
            let result_list = operations.await;
            for result in result_list {
                match result {
                    Ok(_) => continue,
                    Err(err) => {
                        ewe_logs::error!("Failed to complete operator: {:?}", err);
                        return Err(Box::new(OperationsError::FailedOperatorWait).into());
                    }
                }
            }

            Ok(())
        })
    }
}
