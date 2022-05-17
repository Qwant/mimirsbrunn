use crate::{
    adapters::primary::common::settings::{BuildWeight, ImportanceQueryBoosts, StringQuery, Types},
    domain::model::configuration::INDEX_ROOT,
};
use common::document::ContainerDocument;
use geojson::Geometry;
use places::addr::Addr;
use serde_json::json;

use super::{coord::Coord, filters, settings};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueryType {
    PREFIX,
    FUZZY,
    SEARCH,
}

impl QueryType {
    pub fn as_str(self) -> &'static str {
        match self {
            QueryType::PREFIX => "prefix",
            QueryType::FUZZY => "fuzzy",
            QueryType::SEARCH => "search",
        }
    }
}

pub fn build_query(
    q: &str,
    filters: &filters::Filters,
    lang: &str,
    settings: &settings::QuerySettings,
    query_type: QueryType,
    excludes: Option<&[String]>,
) -> serde_json::Value {
    let type_query = build_place_type_boost(&settings.type_query);
    let boosts = build_boosts(settings, filters, query_type);

    let string_query =
        build_string_query(q, lang, &settings.string_query, query_type, &filters.coord);

    let filters = [
        build_filters(
            filters.shape.as_ref(),
            filters.poi_types.as_deref(),
            filters.zone_types.as_deref(),
        ),
        vec![
            build_matching_condition(q, lang, query_type),
            build_house_number_condition(q),
        ],
    ]
    .concat();

    let mut query = json!({
        "query": {
            "bool": {
                "must": [ type_query, string_query ],
                "should": boosts,
                "filter": filters
            }
        },
    });

    if let Some(values) = excludes {
        query["_source"] = json!({ "excludes": values });
    }

    query
}

fn build_string_query(
    q: &str,
    lang: &str,
    settings: &StringQuery,
    query_type: QueryType,
    coord: &Option<Coord>,
) -> serde_json::Value {
    let mut string_should = vec![
        build_multi_match_query(
            q,
            &["name", &format!("names.{}", lang)],
            settings.boosts.name,
        ),
        build_multi_match_query(
            q,
            &["alt_name", &format!("alt_names.{}", lang)],
            settings.boosts.alt_name,
        ),
        build_multi_match_query(
            q,
            &["loc_name", &format!("loc_names.{}", lang)],
            settings.boosts.loc_name,
        ),
        build_multi_match_query(
            q,
            &["label", &format!("labels.{}", lang)],
            settings.boosts.label,
        ),
        build_match_query(q, "zip_codes", settings.boosts.zip_codes),
        build_match_query(q, "house_number", settings.boosts.house_number),
    ];

    if query_type != QueryType::SEARCH {
        string_should.push(build_multi_match_query(
            q,
            &["label.prefix", &format!("labels.{}.prefix", lang)],
            settings.boosts.label_prefix,
        ));
    }

    if query_type == QueryType::FUZZY {
        if coord.is_some() {
            string_should.push(build_multi_match_query(
                q,
                &["label.ngram", &format!("labels.{}.ngram", lang)],
                settings.boosts.label_ngram_with_coord,
            ));
        } else {
            string_should.push(build_multi_match_query(
                q,
                &["label.ngram", &format!("labels.{}.ngram", lang)],
                settings.boosts.label_ngram,
            ));
        }
    }

    json!({
        "bool": {
            "boost": settings.global,
            "should": string_should
        }
    })
}

fn build_boosts(
    settings: &settings::QuerySettings,
    filters: &filters::Filters,
    query_type: QueryType,
) -> Vec<serde_json::Value> {
    let weights = build_weight_depending_on_radius(&settings.importance_query, &filters.coord);

    let mut boosts = vec![build_with_weight(
        &weights,
        &settings.importance_query.weights.types,
    )];

    if query_type != QueryType::FUZZY {
        boosts.push(build_admin_weight_query(weights));
    }

    if let Some(coord) = filters.coord {
        let mut decay = settings.importance_query.proximity.decay.clone();

        if let Some(proximity) = &filters.proximity {
            decay.scale = proximity.scale;
            decay.offset = proximity.offset;
            decay.decay = proximity.decay;
        }

        let weight_boost = match query_type {
            QueryType::PREFIX | QueryType::SEARCH => settings.importance_query.proximity.weight,
            _ => settings.importance_query.proximity.weight_fuzzy,
        };

        boosts.push(build_proximity_boost(coord, &decay, weight_boost));
    }

    boosts
}

fn build_filters(
    shape: Option<&(Geometry, Vec<String>)>,
    poi_types: Option<&[String]>,
    zone_types: Option<&[String]>,
) -> Vec<serde_json::Value> {
    [
        shape.map(|(geometry, scope)| build_shape_query(geometry, scope)),
        poi_types.map(build_poi_types_filter),
        zone_types.map(build_zone_types_filter),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn build_weight_depending_on_radius(
    importance_query_settings: &ImportanceQueryBoosts,
    coord: &Option<Coord>,
) -> BuildWeight {
    let settings_weight = &importance_query_settings.weights;

    // Weights for minimal radius
    let min_weights = settings_weight.min_radius_prefix;

    // Weights for maximal radius
    let max_weights = settings_weight.max_radius;

    // Compute a linear combination of `min_weights` and `max_weights` depending of
    // the level of zoom.
    let zoom_ratio = match coord {
        None => 1.,
        Some(_) => {
            let (min_radius, max_radius) = settings_weight.radius_range;
            let curve = &importance_query_settings.proximity.decay;
            let radius = (curve.offset + curve.scale).min(max_radius).max(min_radius);
            (radius.ln_1p() - min_radius.ln_1p()) / (max_radius.ln_1p() - min_radius.ln_1p())
        }
    };

    BuildWeight {
        admin: (1. - zoom_ratio) * min_weights.admin + zoom_ratio * max_weights.admin,
        factor: (1. - zoom_ratio) * min_weights.factor + zoom_ratio * max_weights.factor,
        missing: (1. - zoom_ratio) * min_weights.missing + zoom_ratio * max_weights.missing,
    }
}

fn build_house_number_condition(q: &str) -> serde_json::Value {
    if q.split_whitespace().count() > 1 {
        // Filter to handle house number. We either want:
        // * to exactly match the document house_number
        // * or that the document is not an address
        //
        // Note that in previous versions of Bragi we were checking for the existence of the
        // house_number field instead of checking for the index name, but there is a performance
        // issue with such queries in recent elasticsearch versions:
        // https://github.com/elastic/elasticsearch/issues/64837
        json!({
            "bool": {
                "should": [
                    {
                        "bool": {
                            "must_not": {
                                "term": {
                                    "_index": format!("{}_{}", INDEX_ROOT, Addr::static_doc_type())
                                }
                            },
                        }
                    },
                    {
                        "match": {
                            "house_number": {
                                "query": q
                            }
                        }
                    }
                ]
            }
        })
    } else {
        // If the query contains a single word, we don't search for any address.
        json!({
            "bool": {
                "must_not": {
                    "term": {
                        "_index": format!("{}_{}", INDEX_ROOT, Addr::static_doc_type())
                    }
                }
            }
        })
    }
}

fn build_matching_condition(q: &str, lang: &str, query_type: QueryType) -> serde_json::Value {
    // Filter to handle house number.
    // We either want:
    // * to exactly match the document house_number
    // * or that the document has no house_number
    match query_type {
        // When the match type is Prefix, we want to use every possible information even though
        // these are not present in label, for instance, the zip_code.
        // The field full_label contains all of them and will do the trick.
        // The query must at least match with elision activated, matching without elision will
        // provide extra score bellow.
        QueryType::PREFIX => json!({
            "match": {
                "full_label.prefix": {
                  "query": q,
                  "operator": "and"
                }
            }
        }),
        // for fuzzy search we lower our expectation & we accept a certain percentage of token match
        // on full_label.ngram
        // The values defined here are empirical,
        // it's supposed to be able to manage cases BOTH missspelt one-word
        // www.elastic.co/guide/en/elasticsearch/guide/current/match-multi-word.html#match-precision
        // requests AND very long requests.
        // Missspelt one-word request:
        //     Vaureaaal (instead of Vaureal)
        // Very long requests:
        //     Caisse Primaire d'Assurance Maladie de Haute Garonne, 33 Rue du Lot, 31100 Toulouse
        QueryType::FUZZY => json!({
            "match": {
                "full_label.ngram": {
                    "query": q,
                    "minimum_should_match": "1<-1 3<-2 9<-4 20<25%"
                }
            }
        }),
        QueryType::SEARCH => json!({
            "multi_match": {
                "query": q,
                "fields": &["label", &format!("labels.{}", lang)],
                "fuzziness": "auto:4,8",
                "minimum_should_match": "4<-1 7<-25%"
            }
        }),
    }
}

fn build_admin_weight_query(weights: BuildWeight) -> serde_json::Value {
    json!({
        "function_score": {
            "boost_mode": "replace",
            "score_mode": "max",
            "functions": [
                { "weight": 0 }, // default value when not matching will be 0
                {
                    "filter": { "term": { "type": "admin" } },
                    "field_value_factor": {
                        "field": "weight",
                        "factor": 1e6,
                        "modifier": "log1p",
                        "missing": 0
                    },
                    "weight": weights.admin
                },
            ]
        }
    })
}

fn build_place_type_boost(settings: &settings::TypeQueryBoosts) -> serde_json::Value {
    let boosts = &settings.boosts;
    json!({
        "bool": {
            "should": [
                { "term": { "type": { "value": "admin", "boost": boosts.admin } } },
                { "term": { "type": { "value": "addr", "boost": boosts.address } } },
                { "term": { "type": { "value": "stop", "boost": boosts.stop } } },
                { "term": { "type": { "value": "poi", "boost": boosts.poi } } },
                { "term": { "type": { "value": "street", "boost": boosts.street } } },
            ],
            "boost": settings.global,
        }
    })
}

/// Create a `Query` that boosts results according to the
/// distance to `coord`.
fn build_proximity_boost(
    coord: Coord,
    settings_decay: &settings::Decay,
    weight_boost: f64,
) -> serde_json::Value {
    let settings::Decay {
        func,
        scale,
        offset,
        decay,
    } = settings_decay;

    json!({
        "function_score": {
            "boost_mode": "replace",
            "functions": [
                {
                    func: {
                        "coord": {
                            "origin": {
                                "lat": coord.lat,
                                "lon": coord.lon
                            },
                            "scale": format!("{}km", scale),
                            "offset": format!("{}km", offset),
                            "decay": decay
                        }
                    }
                },
                {
                    "weight": weight_boost
                }
            ]
        }
    })
}

pub fn build_reverse_query(distance: &str, lat: f64, lon: f64) -> serde_json::Value {
    json!({
        "query": {
            "bool": {
                "filter": {
                    "geo_distance": {
                        "distance": distance,
                        "coord": {
                            "lat": lat,
                            "lon": lon
                        }
                    }
                }
            }
        },
        "_source": {
            "excludes": [ "boundary" ]
        },
        "sort": [{
            "_geo_distance": {
                "coord": {
                    "lat": lat,
                    "lon": lon
                },
                "order": "asc",
                "unit": "m",
                "mode": "min",
                "distance_type": "arc",
                "ignore_unmapped": true
            }
        }]
    })
}

/*If there is a shape, all the places listed in shape_scope are restricted to the shape.
and the places that are not listed are not restricted.
So if shape_scope = {A, B}, we should end up with something like
should [
  must {               => filwer_w_shape
    term: _type in {A, B}
    filter: geoshape
  },
  must_not {            => filter_wo_shape
     term: _type in {A, B}
  }
]
*/
pub fn build_shape_query(shape: &Geometry, scope: &[String]) -> serde_json::Value {
    json!({
        "bool": {
            "should": [
                {
                    "bool": {
                        "must": {
                            "terms": {
                                "type": scope
                            }
                        },
                        "filter": {
                            "geo_shape": {
                                "approx_coord": {
                                    "shape": shape,
                                    "relation": "intersects"
                                }
                            }
                        }
                    }
                },
                {
                    "bool": {
                        "must_not": {
                            "terms": {
                                "type": scope
                            }
                        }
                    }
                }
            ]
        }
    })
}

/*If we search for POIs and we specify poi_types, then we add a filter that should say something
like:
If the place is a POI, then its poi_type must be part of the given list
So if poi_types = {A, B}, we should end up with something like
should [
  must {               => for pois, filter their poi types
    type: poi
    filter: poi_types = {A, B}
  },
  must_not {            => or don't filter for poi types on other places
     type: poi
  }
]*/
pub fn build_poi_types_filter(poi_types: &[String]) -> serde_json::Value {
    json!({
        "bool": {
            "should": [
                {
                    "bool": {
                        "must": [
                            {
                                "term": {
                                    "type": "poi"
                                }
                            },
                            {
                                "terms": {
                                    "poi_type.id": poi_types
                                }
                            }
                        ]
                    }
                },
                {
                    "bool": {
                        "must_not": {
                            "term": {
                                "type": "poi"
                            }
                        }
                    }
                }
            ]
        }
    })
}

/*If we search for administrative regions and we specify zone_types, then we add a filter that should say something
like:
If the place is an administrative region, then its zone_type must be part of the given list
So if zone_type = {A, B}, we should end up with something like
should [
  must {               => for admins, make sure they have the right zone type
    type: admin
    filter: zone_types
  },
  must_not {            => no filter on zone types for other places
     type: admin
  }
]
*/
pub fn build_zone_types_filter(zone_types: &[String]) -> serde_json::Value {
    json!({
        "bool": {
            "should": [
                {
                    "bool": {
                        "must": [
                            {
                                "term": {
                                    "type": "admin"
                                }
                            },
                            {
                                "terms": {
                                    "zone_type": zone_types
                                }
                            }
                        ]
                    }
                },
                {
                    "bool": {
                        "must_not": {
                            "term": {
                                "type": "admin"
                            }
                        }
                    }
                }
            ]
        }
    })
}

pub fn build_features_query(indices: &[String], doc_id: &str) -> serde_json::Value {
    let vec: Vec<serde_json::Value> = indices
        .iter()
        .map(|index| {
            json!({
                "_index": index,
                "_id" : doc_id,
                "_source" : {
                    "exclude" : "boundary"
                }
            })
        })
        .collect();
    json!({ "docs": vec })
}

fn build_multi_match_query(query: &str, fields: &[&str], boost: f64) -> serde_json::Value {
    json!({
        "multi_match": {
            "query": query,
            "fields": fields,
            "boost": boost
        }
    })
}

fn build_match_query(query: &str, field: &str, boost: f64) -> serde_json::Value {
    json!({
        "match": {
            field: {
                "query": query,
                "boost": boost
            }
        }
    })
}

fn build_with_weight(build_weight: &BuildWeight, types: &Types) -> serde_json::Value {
    json!({
        "function_score": {
            "boost_mode": "replace",
            "functions": [
                {
                    "filter": { "term": { "type": "stop" } },
                    "field_value_factor": {
                        "field": "weight",
                        "factor": build_weight.factor,
                        "missing": build_weight.missing
                    },
                    "weight": types.stop
                },
                {
                    "filter": { "term": { "type": "address" } },
                    "filter": { "term": { "type": "addr" } },
                    "field_value_factor": {
                        "field": "weight",
                        "factor": build_weight.factor,
                        "missing": build_weight.missing
                    },
                    "weight": types.address
                },
                {
                    "filter": { "term": { "type": "admin" } },
                    "field_value_factor": {
                        "field": "weight",
                        "factor": build_weight.factor,
                        "missing": build_weight.missing
                    },
                    "weight": types.admin
                },
                {
                    "filter": { "term": { "type": "poi" } },
                    "field_value_factor": {
                        "field": "weight",
                        "factor": build_weight.factor,
                        "missing": build_weight.missing
                    },
                    "weight": types.poi
                },
                {
                    "filter": { "term": { "type": "street" } },
                    "field_value_factor": {
                        "field": "weight",
                        "factor": build_weight.factor,
                        "missing": build_weight.missing
                    },
                    "weight": types.street
                }
            ]
        }
    })
}
