use serde::de::Deserializer;
use serde::ser::Serializer;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::iter::FromIterator;

use super::Property;

#[derive(Default, Debug, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct I18nProperties(pub Vec<Property>);

impl serde::Serialize for I18nProperties {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_map(self.0.iter().map(|p| (&p.key, &p.value)))
    }
}

impl<'de> Deserialize<'de> for I18nProperties {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let properties = BTreeMap::<String, String>::deserialize(deserializer)?
            .into_iter()
            .collect();
        Ok(properties)
    }
}

impl FromIterator<(String, String)> for I18nProperties {
    fn from_iter<I: IntoIterator<Item = (String, String)>>(iter: I) -> Self {
        let properties = iter
            .into_iter()
            .map(|(k, v)| Property { key: k, value: v })
            .collect::<Vec<_>>();
        I18nProperties(properties)
    }
}

impl I18nProperties {
    pub fn get(&self, lang: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|p| p.key == lang)
            .map(|p| p.value.as_ref())
    }
}
