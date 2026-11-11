import React, { memo, useCallback } from "react";
import { Modal, ScrollView, Text, View } from "react-native";
import { StyleSheet } from "react-native-unistyles";
import { ExternalLink, X } from "lucide-react-native";
import { PressHighlight } from "@/components/ui/press-highlight";
import type { UniversalIndexedSessionItem } from "./session-types";
import { getProviderTheme, ProviderGlyph } from "./provider-theme";

interface SessionDetailsModalProps {
  session: UniversalIndexedSessionItem | null;
  onClose: () => void;
  onResume?: (session: UniversalIndexedSessionItem) => void;
}

const ModalProviderBadge = memo(function ModalProviderBadge({
  session,
}: {
  session: UniversalIndexedSessionItem;
}) {
  const providerMeta = getProviderTheme(session.provider);
  return (
    <View style={styles.cardBadgeRow}>
      <View style={[styles.providerPill, { backgroundColor: providerMeta.bg }]}>
        <ProviderGlyph provider={session.provider} color={providerMeta.color} />
        <Text style={[styles.providerText, { color: providerMeta.color }]}>
          {providerMeta.name}
        </Text>
      </View>
      {session.model ? <Text style={styles.modelTag}>{session.model}</Text> : null}
    </View>
  );
});

export const SessionDetailsModal = memo(function SessionDetailsModal({
  session,
  onClose,
  onResume,
}: SessionDetailsModalProps) {
  const handleResume = useCallback(() => {
    if (!session || !onResume) return;
    onClose();
    onResume(session);
  }, [session, onClose, onResume]);

  if (!session) return null;

  return (
    <Modal visible transparent animationType="fade" onRequestClose={onClose}>
      <View style={styles.modalOverlay}>
        <View style={styles.modalContent}>
          <View style={styles.modalHeader}>
            <View style={styles.modalTitleCol}>
              <ModalProviderBadge session={session} />
              <Text style={styles.modalTitle} numberOfLines={2}>
                {session.title}
              </Text>
            </View>
            <PressHighlight onPress={onClose} style={styles.closeModalButton}>
              <X size={16} color="#94a3b8" />
            </PressHighlight>
          </View>

          <ScrollView style={styles.modalBody}>
            {session.preview ? (
              <View style={styles.modalSection}>
                <Text style={styles.modalSectionLabel}>Last Exchange Preview</Text>
                <Text style={styles.modalPreviewText}>{session.preview}</Text>
              </View>
            ) : null}

            <View style={styles.modalSection}>
              <Text style={styles.modalSectionLabel}>Session Information</Text>
              <View style={styles.modalInfoRow}>
                <Text style={styles.modalInfoKey}>Messages:</Text>
                <Text style={styles.modalInfoVal}>{session.message_count}</Text>
              </View>
              <View style={styles.modalInfoRow}>
                <Text style={styles.modalInfoKey}>Subagents:</Text>
                <Text style={styles.modalInfoVal}>{session.subagent_count}</Text>
              </View>
              {session.directory ? (
                <View style={styles.modalInfoRow}>
                  <Text style={styles.modalInfoKey}>Project Path:</Text>
                  <Text style={styles.modalInfoVal} numberOfLines={1}>
                    {session.directory}
                  </Text>
                </View>
              ) : null}
              {session.raw_path ? (
                <View style={styles.modalInfoRow}>
                  <Text style={styles.modalInfoKey}>Transcript File:</Text>
                  <Text style={styles.modalInfoVal} numberOfLines={1}>
                    {session.raw_path}
                  </Text>
                </View>
              ) : null}
            </View>
          </ScrollView>

          <View style={styles.modalFooter}>
            {onResume ? (
              <PressHighlight style={styles.resumeButton} onPress={handleResume}>
                <ExternalLink size={14} color="#ffffff" />
                <Text style={styles.resumeButtonText}>Import / Resume in Kronos</Text>
              </PressHighlight>
            ) : null}
            <PressHighlight style={styles.dismissButton} onPress={onClose}>
              <Text style={styles.dismissButtonText}>Close</Text>
            </PressHighlight>
          </View>
        </View>
      </View>
    </Modal>
  );
});

const styles = StyleSheet.create({
  modalOverlay: {
    flex: 1,
    backgroundColor: "rgba(0, 0, 0, 0.7)",
    justifyContent: "center",
    alignItems: "center",
    padding: 16,
  },
  modalContent: {
    width: "100%",
    maxWidth: 580,
    maxHeight: "85%",
    backgroundColor: "#111217",
    borderRadius: 12,
    borderWidth: 1,
    borderColor: "rgba(255, 255, 255, 0.1)",
    padding: 16,
  },
  modalHeader: {
    flexDirection: "row",
    alignItems: "flex-start",
    justifyContent: "space-between",
    marginBottom: 14,
    paddingBottom: 12,
    borderBottomWidth: 1,
    borderBottomColor: "rgba(255, 255, 255, 0.06)",
  },
  modalTitleCol: {
    flex: 1,
    gap: 6,
  },
  modalTitle: {
    fontSize: 15,
    fontWeight: "700",
    color: "#ffffff",
  },
  closeModalButton: {
    padding: 4,
  },
  cardBadgeRow: {
    flexDirection: "row",
    alignItems: "center",
    flexWrap: "wrap",
    gap: 6,
  },
  providerPill: {
    flexDirection: "row",
    alignItems: "center",
    gap: 4,
    paddingHorizontal: 6,
    paddingVertical: 2,
    borderRadius: 4,
  },
  providerText: {
    fontSize: 11,
    fontWeight: "600",
  },
  modelTag: {
    fontSize: 11,
    color: "#64748b",
  },
  modalBody: {
    flex: 1,
  },
  modalSection: {
    marginBottom: 14,
    backgroundColor: "rgba(255, 255, 255, 0.03)",
    borderRadius: 8,
    padding: 12,
  },
  modalSectionLabel: {
    fontSize: 11,
    fontWeight: "600",
    color: "#94a3b8",
    textTransform: "uppercase",
    letterSpacing: 0.5,
    marginBottom: 6,
  },
  modalPreviewText: {
    fontSize: 12,
    color: "#e2e8f0",
    lineHeight: 18,
  },
  modalInfoRow: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    paddingVertical: 4,
  },
  modalInfoKey: {
    fontSize: 12,
    color: "#94a3b8",
  },
  modalInfoVal: {
    fontSize: 12,
    fontWeight: "500",
    color: "#f1f5f9",
    maxWidth: "70%",
  },
  modalFooter: {
    flexDirection: "row",
    justifyContent: "flex-end",
    gap: 8,
    paddingTop: 12,
    borderTopWidth: 1,
    borderTopColor: "rgba(255, 255, 255, 0.06)",
  },
  resumeButton: {
    flexDirection: "row",
    alignItems: "center",
    gap: 6,
    backgroundColor: "#3b82f6",
    paddingHorizontal: 14,
    paddingVertical: 8,
    borderRadius: 6,
  },
  resumeButtonText: {
    fontSize: 12,
    fontWeight: "600",
    color: "#ffffff",
  },
  dismissButton: {
    paddingHorizontal: 14,
    paddingVertical: 8,
    borderRadius: 6,
    backgroundColor: "rgba(255, 255, 255, 0.06)",
  },
  dismissButtonText: {
    fontSize: 12,
    color: "#cbd5e1",
  },
});
