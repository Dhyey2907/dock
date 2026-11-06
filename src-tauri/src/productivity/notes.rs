//! Notes and to-dos. Their files are a contract: the command bar searches them, so the shapes
//! stay exactly `notes.json` = `[{id,text,updated}]` and `todos.json` = `[{id,text,done,created}]`.

use std::sync::atomic::{AtomicU32, Ordering};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Longest note kept, in characters; anything longer is cut.
pub const MAX_NOTE: usize = 100_000;
/// Longest to-do, in characters.
pub const MAX_TODO: usize = 500;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Note {
    pub id: String,
    pub text: String,
    #[ts(type = "string")]
    pub updated: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Todo {
    pub id: String,
    pub text: String,
    pub done: bool,
    #[ts(type = "string")]
    pub created: DateTime<Utc>,
}

/// A new id: the time in milliseconds plus a counter, so ids made in the same millisecond differ.
pub fn new_id(now: DateTime<Utc>) -> String {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed) & 0xffff;
    format!("{:x}{n:04x}", now.timestamp_millis())
}

fn clip(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

/// Save `text` as note `id` (a new note when `id` is `None` or unknown). Returns its id.
pub fn upsert_note(notes: &mut Vec<Note>, id: Option<&str>, text: &str, now: DateTime<Utc>) -> String {
    let text = clip(text, MAX_NOTE);
    if let Some(note) = id.and_then(|id| notes.iter_mut().find(|n| n.id == id)) {
        if note.text != text {
            note.text = text;
            note.updated = now;
        }
        return note.id.clone();
    }
    let id = id.map(str::to_string).unwrap_or_else(|| new_id(now));
    notes.push(Note {
        id: id.clone(),
        text,
        updated: now,
    });
    id
}

pub fn delete_note(notes: &mut Vec<Note>, id: &str) {
    notes.retain(|n| n.id != id);
}

/// Add a to-do; blank text adds nothing. Returns the new id.
pub fn add_todo(todos: &mut Vec<Todo>, text: &str, now: DateTime<Utc>) -> Option<String> {
    let text = clip(text.trim(), MAX_TODO);
    if text.is_empty() {
        return None;
    }
    let id = new_id(now);
    todos.push(Todo {
        id: id.clone(),
        text,
        done: false,
        created: now,
    });
    Some(id)
}

/// A change to one to-do.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum TodoChange {
    Add { text: String },
    SetDone { id: String, done: bool },
    Rename { id: String, text: String },
    Delete { id: String },
    ClearDone,
}

pub fn apply_todo(todos: &mut Vec<Todo>, change: TodoChange, now: DateTime<Utc>) {
    match change {
        TodoChange::Add { text } => {
            add_todo(todos, &text, now);
        }
        TodoChange::SetDone { id, done } => {
            if let Some(t) = todos.iter_mut().find(|t| t.id == id) {
                t.done = done;
            }
        }
        TodoChange::Rename { id, text } => {
            let text = clip(text.trim(), MAX_TODO);
            if text.is_empty() {
                todos.retain(|t| t.id != id);
            } else if let Some(t) = todos.iter_mut().find(|t| t.id == id) {
                t.text = text;
            }
        }
        TodoChange::Delete { id } => todos.retain(|t| t.id != id),
        TodoChange::ClearDone => todos.retain(|t| !t.done),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(secs: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(1_800_000_000 + secs, 0).unwrap()
    }

    #[test]
    fn notes_are_added_then_edited_in_place() {
        let mut notes = Vec::new();
        let id = upsert_note(&mut notes, None, "milk", at(0));
        assert_eq!(notes.len(), 1);
        let same = upsert_note(&mut notes, Some(&id), "milk and eggs", at(5));
        assert_eq!(same, id);
        assert_eq!(notes[0].text, "milk and eggs");
        assert_eq!(notes[0].updated, at(5));
        // Saving the same text again doesn't move the time.
        upsert_note(&mut notes, Some(&id), "milk and eggs", at(9));
        assert_eq!(notes[0].updated, at(5));
        delete_note(&mut notes, &id);
        assert!(notes.is_empty());
    }

    #[test]
    fn a_note_can_be_made_with_a_chosen_id() {
        let mut notes = Vec::new();
        let id = upsert_note(&mut notes, Some("meeting-abc"), "agenda", at(0));
        assert_eq!(id, "meeting-abc");
    }

    #[test]
    fn ids_differ_within_one_millisecond() {
        assert_ne!(new_id(at(0)), new_id(at(0)));
    }

    #[test]
    fn notes_file_shape_is_the_contract() {
        let note = Note {
            id: "a".into(),
            text: "t".into(),
            updated: at(0),
        };
        let json = serde_json::to_value(&note).unwrap();
        let mut keys: Vec<_> = json.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        assert_eq!(keys, ["id", "text", "updated"]);
        let todo = Todo {
            id: "a".into(),
            text: "t".into(),
            done: false,
            created: at(0),
        };
        let json = serde_json::to_value(&todo).unwrap();
        let mut keys: Vec<_> = json.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        assert_eq!(keys, ["created", "done", "id", "text"]);
    }

    #[test]
    fn todo_changes() {
        let mut todos = Vec::new();
        apply_todo(&mut todos, TodoChange::Add { text: "  ".into() }, at(0));
        assert!(todos.is_empty());
        apply_todo(&mut todos, TodoChange::Add { text: " a ".into() }, at(0));
        apply_todo(&mut todos, TodoChange::Add { text: "b".into() }, at(1));
        assert_eq!(todos[0].text, "a");
        let (a, b) = (todos[0].id.clone(), todos[1].id.clone());
        apply_todo(
            &mut todos,
            TodoChange::SetDone {
                id: a.clone(),
                done: true,
            },
            at(2),
        );
        assert!(todos[0].done);
        apply_todo(
            &mut todos,
            TodoChange::Rename {
                id: b.clone(),
                text: "c".into(),
            },
            at(3),
        );
        assert_eq!(todos[1].text, "c");
        apply_todo(&mut todos, TodoChange::ClearDone, at(4));
        assert_eq!(todos.len(), 1);
        // Renaming to nothing deletes.
        apply_todo(
            &mut todos,
            TodoChange::Rename {
                id: b,
                text: "".into(),
            },
            at(5),
        );
        assert!(todos.is_empty());
    }
}
