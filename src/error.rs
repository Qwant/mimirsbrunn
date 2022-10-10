use snafu::Snafu;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum Error {
    #[snafu(display("invalid insee id: `{}`", id))]
    InvalidInseeId { id: String },

    #[snafu(display("invalid fantoir id: `{}`", id))]
    InvalidFantoirId { id: String },

    #[snafu(display("Invalid coordinates: {:?}", detail))]
    InvalidCoordinates { detail: places::coord::CoordError },
}
