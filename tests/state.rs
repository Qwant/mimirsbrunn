use std::any::Any;
use std::convert::Infallible;
use std::ops::{Deref, DerefMut};
use std::sync::{Mutex, MutexGuard, TryLockError};

use async_trait::async_trait;
use cucumber::WorldInit;
use lazy_static::lazy_static;

use crate::error::Error;
use test_harness::{bano, cosmogony, download, ntfs, osm};

/// Exit status for a step.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum StepStatus {
    Done,
    Skipped,
}

impl From<download::Status> for StepStatus {
    fn from(status: download::Status) -> Self {
        match status {
            download::Status::Skipped => StepStatus::Skipped,
            download::Status::Done => StepStatus::Done,
        }
    }
}

impl From<cosmogony::Status> for StepStatus {
    fn from(status: cosmogony::Status) -> Self {
        match status {
            cosmogony::Status::Skipped => StepStatus::Skipped,
            cosmogony::Status::Done => StepStatus::Done,
        }
    }
}

impl From<bano::Status> for StepStatus {
    fn from(status: bano::Status) -> Self {
        match status {
            bano::Status::Skipped => StepStatus::Skipped,
            bano::Status::Done => StepStatus::Done,
        }
    }
}

impl From<ntfs::Status> for StepStatus {
    fn from(status: ntfs::Status) -> Self {
        match status {
            ntfs::Status::Skipped => StepStatus::Skipped,
            ntfs::Status::Done => StepStatus::Done,
        }
    }
}

impl From<osm::Status> for StepStatus {
    fn from(status: osm::Status) -> Self {
        match status {
            osm::Status::Skipped => StepStatus::Skipped,
            osm::Status::Done => StepStatus::Done,
        }
    }
}

/// A step which can be run from current state.
#[async_trait(?Send)]
pub trait Step: Sized + 'static {
    async fn execute(&mut self, world: &State) -> Result<StepStatus, Error>;
}

/// Register the steps that have been executed so far.
///
/// This acts as a very generic history used to query what steps have been
/// executed before, filtered by kind (using `steps_for`) or exact match (using
/// `status_of`).
#[derive(Debug, Default)]
pub struct State(Vec<(Box<dyn Any + Send + Sync + 'static>, StepStatus)>);

impl State {
    /// Execute a step and update state accordingly.
    pub async fn execute<S: Step + Send + Sync>(
        &mut self,
        mut step: S,
    ) -> Result<StepStatus, Error> {
        let status = step.execute(self).await?;
        self.0.push((Box::new(step), status));
        Ok(status)
    }

    /// Execute a step and update state accordingly if and only if it has not
    /// been executed before.
    pub async fn execute_once<S: Step + Send + Sync + PartialEq>(
        &mut self,
        step: S,
    ) -> Result<StepStatus, Error> {
        match self.status_of(&step) {
            Some(status) => Ok(status),
            None => self.execute(step).await,
        }
    }

    /// Check if given step has already been performed according to current state
    /// and return the status of last run.
    pub fn status_of<S: Step + PartialEq>(&self, step: &S) -> Option<StepStatus> {
        self.steps_for::<S>()
            .filter(|(step_from_state, _)| *step_from_state == step)
            .map(|(_, status)| status)
            .next_back()
    }

    /// Get all steps of type `S` from current state.
    pub fn steps_for<S: Step>(&self) -> impl DoubleEndedIterator<Item = (&S, StepStatus)> {
        self.0
            .iter()
            .filter_map(|(step, status)| Some((step.downcast_ref()?, *status)))
    }
}

// Define a wrapper around the state to use it globally around the cucumber
// run.

lazy_static! {
    pub static ref SHARED_STATE: Mutex<State> = Mutex::default();
}

#[derive(Debug, WorldInit)]
pub struct GlobalState(MutexGuard<'static, State>);

#[async_trait(?Send)]
impl cucumber::World for GlobalState {
    type Error = Infallible;

    async fn new() -> Result<Self, Self::Error> {
        let guard = SHARED_STATE.try_lock().unwrap_or_else(|err| match err {
            TryLockError::Poisoned(poison) => poison.into_inner(),
            TryLockError::WouldBlock => {
                panic!("you can only execute one test at a time")
            }
        });

        Ok(Self(guard))
    }
}

impl Deref for GlobalState {
    type Target = State;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

impl DerefMut for GlobalState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.deref_mut()
    }
}
