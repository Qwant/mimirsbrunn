// FIXME Probably should not be there
//
#[derive(Clone, Copy, Debug)]
pub struct Coord {
    pub lat: f32,
    pub lon: f32,
}

impl Coord {
    pub fn new(lat: f32, lon: f32) -> Self {
        Coord { lat, lon }
    }
}
