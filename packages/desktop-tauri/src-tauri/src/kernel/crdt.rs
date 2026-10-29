use yrs::updates::decoder::Decode;
use yrs::updates::encoder::Encode;
use yrs::{Doc, GetString, ReadTxn, Text, TextRef, Transact, Update};

#[derive(Clone)]
pub struct CrdtDoc {
    doc: Doc,
    text: TextRef,
}

impl CrdtDoc {
    pub fn new() -> Self {
        let doc = Doc::new();
        let text = doc.get_or_insert_text("content");
        Self { doc, text }
    }


    pub fn apply_edit(&self, index: u32, text: &str) {
        let mut txn = self.doc.transact_mut();
        self.text.insert(&mut txn, index, text);
    }

    pub fn delete_range(&self, index: u32, length: u32) {
        let mut txn = self.doc.transact_mut();
        self.text.remove_range(&mut txn, index, length);
    }

    pub fn export_update(&self) -> Vec<u8> {
        let txn = self.doc.transact();
        txn.store().encode_v1()
    }

    pub fn apply_update(&self, update: &[u8]) -> Result<(), String> {
        let mut txn = self.doc.transact_mut();
        let update = Update::decode_v1(update).map_err(|e| e.to_string())?;
        txn.apply_update(update).map_err(|e| e.to_string())
    }

    pub fn to_string(&self) -> String {
        let txn = self.doc.transact();
        self.text.get_string(&txn)
    }

    pub fn len(&self) -> u32 {
        let txn = self.doc.transact();
        self.text.len(&txn)
    }
}
impl Default for CrdtDoc {
    fn default() -> Self {
        Self::new()
    }
}
