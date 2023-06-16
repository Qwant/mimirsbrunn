use std::ops::AddAssign;

#[derive(Debug, Default, Clone, Copy)]
pub struct InsertStats {
    pub created: usize,
    pub updated: usize,
    pub skipped: usize,
    pub deleted: usize,
}

impl AddAssign<InsertStats> for InsertStats {
    fn add_assign(&mut self, rhs: InsertStats) {
        self.created += rhs.created;
        self.updated += rhs.updated;
        self.skipped += rhs.skipped;
        self.deleted += rhs.deleted;
    }
}
