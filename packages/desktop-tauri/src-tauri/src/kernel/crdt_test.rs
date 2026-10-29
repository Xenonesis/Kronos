use crate::kernel::crdt::CrdtDoc;

#[test]
fn test_crdt_multi_agent_concurrent_convergence() {
    let doc_a = CrdtDoc::new();
    let doc_b = CrdtDoc::new();

    // Initial base state synchronization
    doc_a.apply_edit(0, "Kronos Kernel");
    let initial_update = doc_a.export_update();
    doc_b
        .apply_update(&initial_update)
        .expect("doc_b should accept initial update");

    assert_eq!(doc_a.to_string(), "Kronos Kernel");
    assert_eq!(doc_b.to_string(), "Kronos Kernel");

    // Agent A makes edits concurrently
    doc_a.apply_edit(6, " Hyper-Native");

    // Agent B makes edits concurrently
    doc_b.apply_edit(13, " v1.0");

    // Exchange updates between agents
    let update_a = doc_a.export_update();
    let update_b = doc_b.export_update();

    doc_a
        .apply_update(&update_b)
        .expect("doc_a should apply update from doc_b");
    doc_b
        .apply_update(&update_a)
        .expect("doc_b should apply update from doc_a");

    // Verify deterministic convergence
    let text_a = doc_a.to_string();
    let text_b = doc_b.to_string();

    assert_eq!(text_a, text_b);
    assert_eq!(doc_a.len(), doc_b.len());
    assert!(text_a.contains("Kronos"));
    assert!(text_a.contains("Hyper-Native"));
    assert!(text_a.contains("Kernel"));
    assert!(text_a.contains("v1.0"));
}

#[test]
fn test_crdt_insert_delete_operations() {
    let doc = CrdtDoc::new();
    assert_eq!(doc.len(), 0);
    assert_eq!(doc.to_string(), "");

    // Insert text
    doc.apply_edit(0, "Hello World");
    assert_eq!(doc.len(), 11);
    assert_eq!(doc.to_string(), "Hello World");

    // Delete range: remove " World" (length 6 starting at index 5)
    doc.delete_range(5, 6);
    assert_eq!(doc.len(), 5);
    assert_eq!(doc.to_string(), "Hello");

    // Insert text at end
    doc.apply_edit(5, ", Paseo!");
    assert_eq!(doc.len(), 13);
    assert_eq!(doc.to_string(), "Hello, Paseo!");

    // Delete prefix: remove "Hello, " (length 7 starting at index 0)
    doc.delete_range(0, 7);
    assert_eq!(doc.len(), 6);
    assert_eq!(doc.to_string(), "Paseo!");

    // Delete all
    doc.delete_range(0, 6);
    assert_eq!(doc.len(), 0);
    assert_eq!(doc.to_string(), "");
}
