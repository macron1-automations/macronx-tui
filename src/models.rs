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
    #[serde(default)]
    pub payload: serde_json::Value,
    #[serde(default)]
    pub metadata: serde_json::Value,
    #[serde(default)]
    pub attachments: Vec<Attachment>,
    #[serde(default, deserialize_with = "null_to_default")]
    pub created_at: String,
    #[serde(default, deserialize_with = "null_to_default")]
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Attachment {
    #[serde(default)]
    pub id: u64,
    #[serde(default, deserialize_with = "null_to_default")]
    pub filename: String,
    #[serde(default)]
    pub content_type: Option<String>,
    #[serde(default)]
    pub byte_size: u64,
    #[serde(default, deserialize_with = "null_to_default")]
    pub url: String,
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
