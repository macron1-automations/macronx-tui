use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Inbox {
    pub id: u64,
    #[serde(default, deserialize_with = "null_to_default")]
    pub name: String,
    #[serde(default, deserialize_with = "null_to_default")]
    pub source: String,
    #[serde(default)]
    pub tag: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default, deserialize_with = "null_to_default")]
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Tag {
    #[serde(default, deserialize_with = "null_to_default")]
    pub name: String,
    #[serde(default)]
    pub color: Option<String>,
}

fn null_to_default<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Option::<String>::deserialize(deserializer)?.unwrap_or_default())
}
