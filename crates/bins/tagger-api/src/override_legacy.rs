use crate::dto::{TaggedPartLegacy, TaggerResponseLegacy};
use crate::errors::AppError;
use crate::extractors::Json;
use crate::AppState;
use autometrics::autometrics;
use axum::extract::State;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use tagger::{normalize_diacritics, Span, Tag, TaggedPart, TaggerQueryBuilder, Tokenizer};
use tracing::info;

#[derive(Deserialize, Debug, JsonSchema)]
pub struct TaggerLegacyQuery {
    text: String,
    #[allow(unused)]
    tagger: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "UPPERCASE")]
pub struct LegacyResponse {
    nlu: Vec<LegacyResponsePart>,
}

#[derive(Deserialize, Debug)]
pub struct LegacyResponsePart {
    phrase: String,
    tag: String,
}

// FIXME:  This function should be removed when we drop the old tagger
#[autometrics]
pub async fn tag_legacy(
    State(state): State<AppState>,
    Json(body): Json<TaggerLegacyQuery>,
) -> Result<Json<TaggerResponseLegacy>, AppError> {
    info!("{:?}", body);
    let mut tagged_part = vec![];
    let text_normalized = normalize_diacritics(&body.text);
    let legacy_response: LegacyResponse = state
        .client
        .post(state.legacy_tagger_url)
        .json(&json!( {
            "text": text_normalized,
            "domain": "MAPS",
            "lang": "fr"
        }))
        .send()
        .await?
        .json()
        .await?;

    let mut start = 0;
    let mut end = 0;
    for part in legacy_response.nlu {
        end += part.phrase.split(' ').count();
        match part.tag.as_str() {
            "city" | "country" | "state" | "street" => {
                tagged_part.push(TaggedPart {
                    tag: Tag::Location,
                    phrase: part.phrase,
                    span: Span { start, end },
                });
            }
            "POI" => {
                tagged_part.push(TaggedPart {
                    tag: Tag::Poi,
                    phrase: part.phrase,
                    span: Span { start, end },
                });
            }
            _ => {
                // ignore non location tags
            }
        }
        start = end;
    }

    let mut tagged_indices = vec![];
    for part in tagged_part.iter() {
        let indices: Vec<usize> = (part.span.start..=part.span.end).collect();
        tagged_indices.extend(indices)
    }

    let new_tags = TaggerQueryBuilder::build()
        .with_brand()
        .with_categories()
        .with_countries()
        .with_capital_cities()
        .with_cities()
        .apply_taggers(&body.text);

    for tag in new_tags {
        let indices: Vec<usize> = (tag.span.start..tag.span.end).collect();
        if !indices.iter().any(|idx| tagged_indices.contains(idx)) {
            tagged_indices.extend(indices);
            tagged_part.push(tag)
        }
    }

    let mut unttaged = vec![];

    for (idx, token) in Tokenizer::parse(&body.text).tokens.into_iter().enumerate() {
        if !tagged_indices.contains(&idx) {
            unttaged.push(TaggedPart {
                span: Span {
                    start: idx,
                    end: idx + 1,
                },
                tag: Tag::None,
                phrase: token,
            })
        }
    }
    tagged_part.extend(unttaged);

    Ok(Json(TaggerResponseLegacy {
        nlu: tagged_part
            .into_iter()
            .map(TaggedPartLegacy::from)
            .collect(),
    }))
}
