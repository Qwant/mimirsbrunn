use crate::doc_type::{root_doctype, root_doctype_dataset};
use crate::filters::Type;
use places::addr::Addr;
use places::admin::Admin;
use places::poi::Poi;
use places::stop::Stop;
use places::street::Street;
use places::ContainerDocument;

pub fn build_es_indices_to_search(
    index_root: &str,
    types: &Option<Vec<Type>>,
    pt_dataset: &Option<Vec<String>>,
    poi_dataset: &Option<Vec<String>>,
) -> Vec<String> {
    // some specific types are requested,
    // let's search only for these types of objects
    if let Some(types) = types {
        let mut indices = Vec::new();
        for doc_type in types.iter() {
            match doc_type {
                Type::House => indices.push(root_doctype(index_root, Addr::static_doc_type())),
                Type::Street => indices.push(root_doctype(index_root, Street::static_doc_type())),
                Type::Zone | Type::City => {
                    indices.push(root_doctype(index_root, Admin::static_doc_type()))
                }
                Type::Poi => {
                    let doc_type_str = Poi::static_doc_type();
                    // if some poi_dataset are specified
                    // we search for poi only in the corresponding es indices
                    if let Some(poi_datasets) = poi_dataset {
                        for poi_dataset in poi_datasets.iter() {
                            indices.push(root_doctype_dataset(
                                index_root,
                                doc_type_str,
                                poi_dataset,
                            ));
                        }
                    } else {
                        // no poi_dataset specified
                        // we search in the global alias for all poi
                        indices.push(root_doctype(index_root, doc_type_str));
                    }
                }
                Type::StopArea => {
                    // if some pt_dataset are specified
                    // we search for stops only in the corresponding es indices
                    let doc_type_str = Stop::static_doc_type();
                    if let Some(pt_datasets) = pt_dataset {
                        for pt_dataset in pt_datasets.iter() {
                            indices.push(root_doctype_dataset(
                                index_root,
                                doc_type_str,
                                pt_dataset,
                            ));
                        }
                    } else {
                        // no pt_dataset specified
                        // we search in the global alias for all stops
                        indices.push(root_doctype(index_root, doc_type_str));
                    }
                }
            }
        }
        indices
    } else {
        let mut indices = vec![
            root_doctype(index_root, Addr::static_doc_type()),
            root_doctype(index_root, Street::static_doc_type()),
            root_doctype(index_root, Admin::static_doc_type()),
        ];
        if let Some(pt_datasets) = pt_dataset {
            let doc_type_str = Stop::static_doc_type();
            for pt_dataset in pt_datasets.iter() {
                indices.push(root_doctype_dataset(index_root, doc_type_str, pt_dataset));
            }
        }
        if let Some(poi_datasets) = poi_dataset {
            let doc_type_str = Poi::static_doc_type();
            for poi_dataset in poi_datasets.iter() {
                indices.push(root_doctype_dataset(index_root, doc_type_str, poi_dataset));
            }
        } else {
            indices.push(root_doctype(index_root, Poi::static_doc_type()))
        }
        indices
    }
}
