use serde::Deserialize;

use crate::forge_clients::types::{ForgeUser, MergeRequestThread, ThreadNote};

#[derive(Debug, Deserialize)]
pub struct GlDiscussion {
    pub id: String,
    #[allow(dead_code)]
    pub individual_note: bool,
    pub notes: Vec<GlDiscussionNote>,
}

#[derive(Debug, Deserialize)]
pub struct GlDiscussionNote {
    pub id: u64,
    pub body: String,
    pub author: GlUser,
    pub created_at: String,
    pub system: bool,
    #[serde(default)]
    pub resolvable: bool,
    #[serde(default)]
    pub resolved: bool,
    #[serde(default)]
    pub position: Option<GlDiffPosition>,
}

#[derive(Debug, Deserialize)]
pub struct GlUser {
    pub id: u64,
    pub name: String,
    pub username: String,
}

#[derive(Debug, Deserialize)]
pub struct GlDiffPosition {
    pub new_path: String,
    #[serde(default)]
    pub new_line: Option<u32>,
}

impl GlDiscussion {
    pub fn to_thread(&self) -> MergeRequestThread {
        let first_note = self.notes.first();
        let resolvable = first_note.is_some_and(|n| n.resolvable);
        let resolved = resolvable && self.notes.iter().all(|n| !n.resolvable || n.resolved);

        let (file_path, line) = first_note
            .and_then(|n| n.position.as_ref())
            .map(|p| (Some(p.new_path.clone()), p.new_line))
            .unwrap_or((None, None));

        MergeRequestThread {
            id: self.id.clone(),
            notes: self
                .notes
                .iter()
                .filter(|n| !n.system)
                .map(|n| ThreadNote {
                    id: n.id,
                    author: ForgeUser {
                        id: n.author.id,
                        username: n.author.username.clone(),
                        name: n.author.name.clone(),
                    },
                    body: n.body.clone(),
                    created_at: n.created_at.clone(),
                    system: n.system,
                })
                .collect(),
            resolvable,
            resolved,
            file_path,
            line,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gl_discussion_to_thread_resolved() {
        let disc: GlDiscussion = serde_json::from_str(
            r#"{
            "id": "abc123",
            "individual_note": false,
            "notes": [
                {
                    "id": 1,
                    "body": "Fix this",
                    "author": {"id": 1, "name": "Alice", "username": "alice"},
                    "created_at": "2026-01-01T00:00:00Z",
                    "system": false,
                    "resolvable": true,
                    "resolved": true,
                    "position": {"new_path": "src/main.rs", "new_line": 42}
                },
                {
                    "id": 2,
                    "body": "Fixed",
                    "author": {"id": 2, "name": "Bob", "username": "bob"},
                    "created_at": "2026-01-01T01:00:00Z",
                    "system": false,
                    "resolvable": true,
                    "resolved": true
                }
            ]
        }"#,
        )
        .unwrap();

        let thread = disc.to_thread();
        assert!(thread.resolvable);
        assert!(thread.resolved);
        assert_eq!(thread.file_path, Some("src/main.rs".to_string()));
        assert_eq!(thread.line, Some(42));
        assert_eq!(thread.notes.len(), 2);
    }

    #[test]
    fn test_gl_discussion_to_thread_unresolved() {
        let disc: GlDiscussion = serde_json::from_str(
            r#"{
            "id": "def456",
            "individual_note": false,
            "notes": [
                {
                    "id": 3,
                    "body": "Needs work",
                    "author": {"id": 1, "name": "Alice", "username": "alice"},
                    "created_at": "2026-01-01T00:00:00Z",
                    "system": false,
                    "resolvable": true,
                    "resolved": false
                }
            ]
        }"#,
        )
        .unwrap();

        let thread = disc.to_thread();
        assert!(thread.resolvable);
        assert!(!thread.resolved);
        assert!(thread.file_path.is_none());
    }

    #[test]
    fn test_gl_discussion_system_notes_filtered() {
        let disc: GlDiscussion = serde_json::from_str(
            r#"{
            "id": "sys789",
            "individual_note": true,
            "notes": [
                {
                    "id": 4,
                    "body": "added label ~review",
                    "author": {"id": 0, "name": "System", "username": "system"},
                    "created_at": "2026-01-01T00:00:00Z",
                    "system": true,
                    "resolvable": false,
                    "resolved": false
                }
            ]
        }"#,
        )
        .unwrap();

        let thread = disc.to_thread();
        assert!(!thread.resolvable);
        assert!(thread.notes.is_empty());
    }

    #[test]
    fn test_gl_discussion_non_resolvable() {
        let disc: GlDiscussion = serde_json::from_str(
            r#"{
            "id": "gen000",
            "individual_note": true,
            "notes": [
                {
                    "id": 5,
                    "body": "LGTM!",
                    "author": {"id": 3, "name": "Charlie", "username": "charlie"},
                    "created_at": "2026-01-02T00:00:00Z",
                    "system": false,
                    "resolvable": false,
                    "resolved": false
                }
            ]
        }"#,
        )
        .unwrap();

        let thread = disc.to_thread();
        assert!(!thread.resolvable);
        assert!(!thread.resolved);
        assert_eq!(thread.notes.len(), 1);
        assert_eq!(thread.notes[0].body, "LGTM!");
    }
}
