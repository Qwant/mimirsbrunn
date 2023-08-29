use schemars::JsonSchema;
use serde::Serialize;
use tagger::{Span, Tag, TaggedPart};

// FIXME: once the legacy tagger is not in use anymore we should
//  remove legacy payloads

#[derive(Debug, Eq, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "UPPERCASE")]
pub struct TaggerResponseLegacy {
    pub nlu: Vec<TaggedPartLegacy>,
}

#[derive(Debug, Eq, PartialEq, Serialize, JsonSchema)]
pub struct TaggedPartLegacy {
    pub tag: String,
    pub phrase: String,
    pub span: Vec<usize>,
    pub extra: ExtraTagPartLegacy,
}

#[derive(Debug, Eq, PartialEq, Serialize, JsonSchema)]
pub struct ExtraTagPartLegacy {
    pub category: Vec<String>,
}

impl From<TaggedPart> for TaggedPartLegacy {
    fn from(value: TaggedPart) -> Self {
        TaggedPartLegacy {
            tag: match value.tag {
                Tag::Category(_) => "category",
                Tag::Brand => "brand",
                Tag::City => "location",
                Tag::Address => "location",
                Tag::Street => "address",
                Tag::Location => "location",
                Tag::Poi => "POI",
                Tag::None => "none",
            }
            .to_string(),
            phrase: value.phrase,
            span: vec![value.span.start, value.span.end],
            extra: ExtraTagPartLegacy {
                category: match value.tag {
                    Tag::Category(category) => vec![category],
                    _ => vec![],
                },
            },
        }
    }
}

#[derive(Debug, Eq, PartialEq, Serialize, JsonSchema)]
pub struct TaggedPartDto {
    pub span: SpanDto,
    pub tag: TagDto,
    pub phrase: String,
}

#[derive(Debug, Eq, PartialEq, Serialize, JsonSchema)]
#[serde(rename = "lowercase", tag = "type", content = "content")]
pub enum TagDto {
    Category(String),
    Brand,
    City,
    Location,
    Address,
    Street,
    Poi,
    None,
}

#[derive(PartialEq, Eq, Debug, Copy, Clone, Serialize, JsonSchema)]
pub struct SpanDto {
    pub start: usize,
    pub end: usize,
}

impl From<TaggedPart> for TaggedPartDto {
    fn from(value: TaggedPart) -> Self {
        TaggedPartDto {
            span: SpanDto::from(value.span),
            tag: TagDto::from(value.tag),
            phrase: value.phrase,
        }
    }
}

impl From<Span> for SpanDto {
    fn from(value: Span) -> Self {
        SpanDto {
            start: value.start,
            end: value.end,
        }
    }
}

impl From<Tag> for TagDto {
    fn from(value: Tag) -> Self {
        match value {
            Tag::Category(cat) => TagDto::Category(cat),
            Tag::Brand => TagDto::Brand,
            Tag::City => TagDto::City,
            Tag::Location => TagDto::Location,
            Tag::None => TagDto::None,
            Tag::Address => TagDto::Address,
            Tag::Street => TagDto::Street,
            Tag::Poi => TagDto::Poi,
        }
    }
}
