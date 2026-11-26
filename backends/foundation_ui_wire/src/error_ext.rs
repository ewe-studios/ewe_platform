use core::fmt;

use crate::ActionErrorT;

impl fmt::Display for ActionErrorT {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.message {
            Some(message) => write!(formatter, "{:?}: {message}", self.code),
            None => write!(formatter, "{:?}", self.code),
        }
    }
}

impl core::error::Error for ActionErrorT {}
