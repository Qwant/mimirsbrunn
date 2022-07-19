#[derive(Debug, Default)]
pub struct InsertStats {
    pub created: usize,
    pub updated: usize,
    pub skipped: usize,
    pub deleted: usize,
}
