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
    #[serde(default)]
    pub attachments: Vec<Attachment>,
    #[serde(default)]
    pub processed: bool,
    #[serde(default)]
    pub archived: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Attachment {
    pub id: u64,
    #[serde(default, deserialize_with = "null_to_default")]
    pub filename: String,
    #[serde(default, deserialize_with = "null_to_default")]
    pub content_type: String,
    #[serde(default)]
    pub byte_size: u64,
    #[serde(default, deserialize_with = "null_to_default")]
    pub url: String,
}

impl Attachment {
    pub fn is_image(&self) -> bool {
        self.content_type.starts_with("image/")
    }

    pub fn is_audio(&self) -> bool {
        self.content_type.starts_with("audio/")
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_attachments() {
        let json = r#"{
            "id": 42,
            "name": "Case",
            "source": "radiology",
            "attachments": [
                {
                    "id": 66,
                    "filename": "photo-6255_singular_display_fullPicture.jpg",
                    "content_type": "image/jpeg",
                    "byte_size": 5129556,
                    "url": "https://example.dev/rails/active_storage/blobs/redirect/abc/photo.jpg"
                },
                {
                    "id": 67,
                    "filename": "voice-note.mp3",
                    "content_type": "audio/mpeg",
                    "byte_size": 1024,
                    "url": "https://example.dev/audio.mp3"
                }
            ]
        }"#;

        let inbox: Inbox = serde_json::from_str(json).unwrap();
        assert_eq!(inbox.attachments.len(), 2);
        assert!(inbox.attachments[0].is_image());
        assert!(!inbox.attachments[0].is_audio());
        assert!(inbox.attachments[1].is_audio());
        assert_eq!(inbox.attachments[1].byte_size, 1024);
    }

    #[test]
    fn missing_attachments_defaults_to_empty() {
        let json = r#"{"id": 1, "name": "n", "source": "s"}"#;
        let inbox: Inbox = serde_json::from_str(json).unwrap();
        assert!(inbox.attachments.is_empty());
        assert!(!inbox.processed);
        assert!(!inbox.archived);
    }

    #[test]
    fn parses_processed_and_archived() {
        let json = r#"{
            "id": 2,
            "name": "n",
            "source": "s",
            "processed": true,
            "archived": false
        }"#;
        let inbox: Inbox = serde_json::from_str(json).unwrap();
        assert!(inbox.processed);
        assert!(!inbox.archived);
    }
}
