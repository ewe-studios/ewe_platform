// Implements idle management handler for handling idle checks.

use crate::retries::{ExponentialBackoffDecider, RetryDecider, RetryState};

#[derive(Clone, Debug)]
pub enum IdleState {
    Ongoing(Option<RetryState>),
    Expired,
}

pub struct Idleman {
    max_idles: u32,
    last_state: IdleState,
    retry_decider: ExponentialBackoffDecider,
}

impl Idleman {
    pub fn new(max_idles: u32, retry_decider: ExponentialBackoffDecider) -> Self {
        Self {
            max_idles,
            retry_decider,
            last_state: IdleState::Ongoing(None),
        }
    }

    pub fn state(&self) -> IdleState {
        self.last_state.clone()
    }

    pub fn reset(&mut self) {
        self.last_state = IdleState::Ongoing(None);
    }

    pub fn next_idle(&mut self) -> IdleState {
        match &self.last_state {
            IdleState::Ongoing(retry_state) => {
                let retry_state = if retry_state.is_some() {
                    retry_state.clone().unwrap()
                } else {
                    RetryState::new(0, self.max_idles, None)
                };

                self.last_state = match self.retry_decider.decide(retry_state) {
                    Some(inner) => IdleState::Ongoing(Some(inner.clone())),
                    None => IdleState::Expired,
                };

                self.last_state.clone()
            }
            IdleState::Expired => IdleState::Expired,
        }
    }
}
