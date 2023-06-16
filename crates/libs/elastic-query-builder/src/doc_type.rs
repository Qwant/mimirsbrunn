pub fn root_doctype_dataset_ts(index_root: &str, doc_type: &str, dataset: &str) -> String {
    format!(
        "{index_root}_{doc_type}_{dataset}_{}",
        chrono::Utc::now().format("%Y%m%d_%H%M%S_%f")
    )
}

pub fn root_doctype_dataset(index_root: &str, doc_type: &str, dataset: &str) -> String {
    format!("{index_root}_{doc_type}_{dataset}")
}

pub fn aliases(index_root: &str, doc_type: &str, dataset: &str) -> Vec<String> {
    vec![
        index_root.to_string(),
        root_doctype(index_root, doc_type),
        root_doctype_dataset(index_root, doc_type, dataset),
    ]
}

pub fn root_doctype(index_root: &str, doc_type: &str) -> String {
    format!("{index_root}_{doc_type}")
}
