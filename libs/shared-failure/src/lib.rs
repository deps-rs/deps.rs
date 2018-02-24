extern crate failure;

use std::fmt::{Display, Formatter, Result as FmtResult};
use std::sync::Arc;

use failure::{Backtrace, Error, Fail};

#[derive(Clone, Debug)]
pub struct SharedFailure(Arc<Error>);

impl SharedFailure {
    pub fn downcast_ref<T: Fail>(&self) -> Option<&T> {
        self.0.downcast_ref()
    }
}

impl Fail for SharedFailure {
    fn cause(&self) -> Option<&Fail> {
        Some(self.0.cause())
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        Some(self.0.backtrace())
    }
}

impl Display for SharedFailure {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        self.0.fmt(f)
    }
}

impl From<Error> for SharedFailure {
    fn from(err: Error) -> SharedFailure {
        SharedFailure(Arc::new(err))
    }
}
