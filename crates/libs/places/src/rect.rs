use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Copy, Clone, Debug, JsonSchema)]
pub struct Rect([f64; 4]);

impl From<geo_types::Rect<f64>> for Rect {
    fn from(value: geo_types::Rect<f64>) -> Self {
        Self([value.min().x, value.min().y, value.max().x, value.max().y])
    }
}
