use async_trait::async_trait;
use cucumber::{StepContext, World};
use std::any::Any;
use std::convert::Infallible;

use crate::error::Error;
use tests::{admin, bano, download, ntfs};

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

impl From<admin::Status> for StepStatus {
    fn from(status: admin::Status) -> Self {
        match status {
            admin::Status::Skipped => StepStatus::Skipped,
            admin::Status::Done => StepStatus::Done,
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

/// A step which can be run from current state.
#[async_trait(?Send)]
pub trait Step: Sized + 'static {
    async fn execute(&mut self, world: &State, ctx: &StepContext) -> Result<StepStatus, Error>;
}

/// Register the steps that have been executed so far.
///
/// This acts as a very generic history used to query what steps have been
/// executed before, filtered by kind (using `steps_for`) or exact match (using
/// `status_of`).
#[derive(Debug, Default)]
pub struct State(Vec<(Box<dyn Any>, StepStatus)>);

impl State {
    /// Execute a step and update state accordingly.
    pub async fn execute<S: Step>(
        &mut self,
        mut step: S,
        ctx: &StepContext,
    ) -> Result<StepStatus, Error> {
        let status = step.execute(self, ctx).await?;
        self.0.push((Box::new(step), status));
        Ok(status)
    }

    /// Execute a step and update state accordingly if and only if it has not
    /// been executed before.
    pub async fn execute_once<S: Step + PartialEq>(
        &mut self,
        step: S,
        ctx: &StepContext,
    ) -> Result<StepStatus, Error> {
        match self.status_of(&step) {
            Some(status) => Ok(status),
            None => self.execute(step, ctx).await,
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

#[async_trait(?Send)]
impl World for State {
    type Error = Infallible;

    async fn new() -> Result<Self, Self::Error> {
        Ok(Self::default())
    }
}
