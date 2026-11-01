use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tokio::sync::broadcast;
use yrs::updates::decoder::Decode;
use yrs::updates::encoder::Encode;
use yrs::{Doc, GetString, ReadTxn, StateVector, Text, TextRef, Transact, Update};

/// Represents an individual CRDT document backed by Yrs.
#[derive(Clone)]
pub struct CrdtDoc {
    doc: Doc,
    text: TextRef,
}

impl CrdtDoc {
    /// Creates a new `CrdtDoc` with an empty root text type `"content"`.
    pub fn new() -> Self {
        let doc = Doc::new();
        let text = doc.get_or_insert_text("content");
        Self { doc, text }
    }

    /// Inserts text at the specified UTF-16/character index.
    pub fn apply_edit(&self, index: u32, text: &str) {
        let mut txn = self.doc.transact_mut();
        self.text.insert(&mut txn, index, text);
    }

    /// Deletes a range of text starting at `index` with length `length`.
    pub fn delete_range(&self, index: u32, length: u32) {
        let mut txn = self.doc.transact_mut();
        self.text.remove_range(&mut txn, index, length);
    }

    /// Exports the entire document update history encoded in Yjs v1 binary format.
    pub fn export_update(&self) -> Vec<u8> {
        let txn = self.doc.transact();
        txn.store().encode_v1()
    }

    /// Exports the current state vector encoded in Yjs v1 format.
    pub fn export_state_vector(&self) -> Vec<u8> {
        let txn = self.doc.transact();
        txn.state_vector().encode_v1()
    }

    /// Encodes only the missing updates since the provided remote state vector (differential sync).
    pub fn encode_diff(&self, remote_sv_bytes: &[u8]) -> Result<Vec<u8>, String> {
        let txn = self.doc.transact();
        let remote_sv = StateVector::decode_v1(remote_sv_bytes)
            .map_err(|e| format!("Invalid remote state vector: {e}"))?;
        Ok(txn.encode_diff_v1(&remote_sv))
    }

    /// Applies a remote binary update encoded in Yjs v1 format to this document.
    pub fn apply_update(&self, update: &[u8]) -> Result<(), String> {
        let mut txn = self.doc.transact_mut();
        let update = Update::decode_v1(update).map_err(|e| e.to_string())?;
        txn.apply_update(update).map_err(|e| e.to_string())
    }

    /// Returns the current string representation of the text document.
    pub fn to_string(&self) -> String {
        let txn = self.doc.transact();
        self.text.get_string(&txn)
    }

    /// Returns the character length of the text document.
    pub fn len(&self) -> u32 {
        let txn = self.doc.transact();
        self.text.len(&txn)
    }

    /// Returns true if the text document is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for CrdtDoc {
    fn default() -> Self {
        Self::new()
    }
}

/// Broadcast event for real-time CRDT updates to peers.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CrdtUpdateEvent {
    pub room: String,
    pub update_base64: String,
    pub source_id: Option<String>,
}

/// Multi-Room CRDT Document Hub managing concurrent collaborative rooms across agents and humans.
#[derive(Clone)]
pub struct CrdtEngine {
    rooms: Arc<RwLock<HashMap<String, Arc<CrdtDoc>>>>,
    event_tx: broadcast::Sender<CrdtUpdateEvent>,
}

impl CrdtEngine {
    /// Creates a new `CrdtEngine` with an event broadcast channel.
    pub fn new() -> Self {
        let (event_tx, _) = broadcast::channel(1024);
        Self {
            rooms: Arc::new(RwLock::new(HashMap::new())),
            event_tx,
        }
    }

    /// Retrieves or instantiates a collaborative CRDT document room by name.
    pub fn get_or_create_room(&self, room: &str) -> Arc<CrdtDoc> {
        let mut rooms = self.rooms.write().unwrap();
        rooms
            .entry(room.to_string())
            .or_insert_with(|| Arc::new(CrdtDoc::new()))
            .clone()
    }

    /// Checks if a room currently exists in memory.
    pub fn has_room(&self, room: &str) -> bool {
        let rooms = self.rooms.read().unwrap();
        rooms.contains_key(room)
    }

    /// Returns a list of all active room identifiers.
    pub fn list_rooms(&self) -> Vec<String> {
        let rooms = self.rooms.read().unwrap();
        let mut list: Vec<String> = rooms.keys().cloned().collect();
        list.sort();
        list
    }

    /// Applies a binary Yjs update to a room and broadcasts the update to all active subscribers.
    pub fn apply_update(
        &self,
        room: &str,
        update: &[u8],
        source_id: Option<String>,
    ) -> Result<String, String> {
        let doc = self.get_or_create_room(room);
        doc.apply_update(update)?;

        let update_base64 = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            update,
        );

        let _ = self.event_tx.send(CrdtUpdateEvent {
            room: room.to_string(),
            update_base64: update_base64.clone(),
            source_id,
        });

        Ok(update_base64)
    }

    /// Applies a text-level edit (insert or delete) directly to a room's document.
    /// Exports the resulting Yjs update and broadcasts it to peers.
    pub fn apply_text_edit(
        &self,
        room: &str,
        action: &str,
        index: u32,
        text: Option<&str>,
        length: Option<u32>,
        source_id: Option<String>,
    ) -> Result<String, String> {
        let doc = self.get_or_create_room(room);
        let before_sv = doc.export_state_vector();

        match action {
            "insert" => {
                let t = text.unwrap_or("");
                doc.apply_edit(index, t);
            }
            "delete" => {
                let len = length.unwrap_or(0);
                if len > 0 {
                    doc.delete_range(index, len);
                }
            }
            other => return Err(format!("Unsupported text edit action: '{other}'")),
        }

        // Compute incremental diff generated by this edit
        let diff = doc.encode_diff(&before_sv)?;
        let update_base64 = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            &diff,
        );

        let _ = self.event_tx.send(CrdtUpdateEvent {
            room: room.to_string(),
            update_base64: update_base64.clone(),
            source_id,
        });

        Ok(update_base64)
    }

    /// Subscribes to the broadcast channel for real-time CRDT updates across all rooms.
    pub fn subscribe(&self) -> broadcast::Receiver<CrdtUpdateEvent> {
        self.event_tx.subscribe()
    }

    /// Returns the current text content of a room.
    pub fn get_text(&self, room: &str) -> String {
        let doc = self.get_or_create_room(room);
        doc.to_string()
    }

    /// Returns the state vector of a room for differential synchronization.
    pub fn get_state_vector(&self, room: &str) -> Vec<u8> {
        let doc = self.get_or_create_room(room);
        doc.export_state_vector()
    }

    /// Computes differential sync updates for a room given a client's state vector.
    pub fn sync_room(&self, room: &str, remote_sv_bytes: &[u8]) -> Result<Vec<u8>, String> {
        let doc = self.get_or_create_room(room);
        doc.encode_diff(remote_sv_bytes)
    }

    /// Removes a room from memory.
    pub fn remove_room(&self, room: &str) -> bool {
        let mut rooms = self.rooms.write().unwrap();
        rooms.remove(room).is_some()
    }
}

impl Default for CrdtEngine {
    fn default() -> Self {
        Self::new()
    }
}
