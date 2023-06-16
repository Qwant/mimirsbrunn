#[derive(Debug, Clone)]
pub enum IndexStatus {
    Available,
    NotAvailable,
}

#[derive(Debug, Clone)]
pub struct Index {
    pub name: String,
    pub root: String,
    pub dataset: String,
    pub doc_type: String,
    pub docs_count: u32,
    pub status: IndexStatus,
}
