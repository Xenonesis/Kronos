import React, { useState } from "react";
import { Pressable, Text, TextInput, View } from "react-native";
import { StyleSheet, useUnistyles } from "react-native-unistyles";
import { Check, Globe, Compass, FileText, KeyRound, X } from "lucide-react-native";
import { Button } from "@/components/ui/button";

export interface BrowserImportDialogProps {
  visible: boolean;
  onClose: () => void;
  onImport: (cookiesText: string, source: "chrome" | "edge" | "manual") => void;
  testID?: string;
}

export function BrowserImportDialog({
  visible,
  onClose,
  onImport,
  testID = "browser-import-dialog",
}: BrowserImportDialogProps) {
  const { theme } = useUnistyles();
  const [manualCookies, setManualCookies] = useState("");
  const [selectedSource, setSelectedSource] = useState<"chrome" | "edge" | "manual">("chrome");
  const [importedStatus, setImportedStatus] = useState<string | null>(null);

  if (!visible) return null;

  const handleExecuteImport = () => {
    onImport(manualCookies, selectedSource);
    setImportedStatus(`Imported session data from ${selectedSource}`);
    setTimeout(() => {
      onClose();
      setImportedStatus(null);
    }, 800);
  };

  return (
    <View style={styles.overlay} testID={testID}>
      <View style={styles.dialog}>
        <View style={styles.header}>
          <View style={styles.headerTitleRow}>
            <KeyRound size={16} color={theme.colors.foreground} />
            <Text style={styles.title}>Import Browser Session & Cookies</Text>
          </View>
          <Pressable onPress={onClose} style={styles.closeBtn} testID="browser-import-close-btn">
            <X size={16} color={theme.colors.foregroundMuted} />
          </Pressable>
        </View>

        <Text style={styles.subtitle}>
          Import authentication cookies from your default browser so you are already logged in on
          local and web apps.
        </Text>

        {/* Source Selector */}
        <View style={styles.sourceList}>
          <Pressable
            style={[styles.sourceItem, selectedSource === "chrome" && styles.sourceItemActive]}
            onPress={() => setSelectedSource("chrome")}
            testID="import-source-chrome"
          >
            <Globe size={18} color={theme.colors.foreground} />
            <View style={styles.sourceTextCol}>
              <Text style={styles.sourceTitle}>Google Chrome</Text>
              <Text style={styles.sourceDesc}>
                Auto-detect session cookies from default Chrome profile
              </Text>
            </View>
            {selectedSource === "chrome" ? (
              <Check size={16} color={theme.colors.foreground} />
            ) : null}
          </Pressable>

          <Pressable
            style={[styles.sourceItem, selectedSource === "edge" && styles.sourceItemActive]}
            onPress={() => setSelectedSource("edge")}
            testID="import-source-edge"
          >
            <Compass size={18} color={theme.colors.foreground} />
            <View style={styles.sourceTextCol}>
              <Text style={styles.sourceTitle}>Microsoft Edge</Text>
              <Text style={styles.sourceDesc}>Auto-detect cookies from Edge profile</Text>
            </View>
            {selectedSource === "edge" ? <Check size={16} color={theme.colors.foreground} /> : null}
          </Pressable>

          <Pressable
            style={[styles.sourceItem, selectedSource === "manual" && styles.sourceItemActive]}
            onPress={() => setSelectedSource("manual")}
            testID="import-source-manual"
          >
            <FileText size={18} color={theme.colors.foreground} />
            <View style={styles.sourceTextCol}>
              <Text style={styles.sourceTitle}>Paste Cookies / Storage JSON</Text>
              <Text style={styles.sourceDesc}>Enter custom Cookie header or token JSON</Text>
            </View>
            {selectedSource === "manual" ? (
              <Check size={16} color={theme.colors.foreground} />
            ) : null}
          </Pressable>
        </View>

        {/* Manual Input if selected */}
        {selectedSource === "manual" ? (
          <TextInput
            style={styles.textInput}
            multiline
            placeholder="session_id=xyz; auth_token=abc..."
            placeholderTextColor={theme.colors.foregroundMuted}
            value={manualCookies}
            onChangeText={setManualCookies}
            testID="browser-import-manual-input"
          />
        ) : null}

        {importedStatus ? <Text style={styles.statusSuccess}>{importedStatus}</Text> : null}

        {/* Footer Actions */}
        <View style={styles.footer}>
          <Button variant="ghost" size="sm" onPress={onClose}>
            Cancel
          </Button>
          <Button
            variant="default"
            size="sm"
            onPress={handleExecuteImport}
            testID="browser-import-confirm-btn"
          >
            Import Data
          </Button>
        </View>
      </View>
    </View>
  );
}

const styles = StyleSheet.create((theme) => ({
  overlay: {
    ...StyleSheet.absoluteFillObject,
    backgroundColor: "rgba(0,0,0,0.65)",
    alignItems: "center",
    justifyContent: "center",
    zIndex: 50,
    padding: theme.spacing[4],
  },
  dialog: {
    width: 480,
    maxWidth: "100%",
    backgroundColor: theme.colors.surface1,
    borderRadius: theme.borderRadius.lg,
    borderWidth: 1,
    borderColor: theme.colors.border,
    padding: theme.spacing[4],
    gap: theme.spacing[3],
  },
  header: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
  },
  headerTitleRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
  },
  title: {
    fontSize: theme.fontSize.base,
    fontWeight: theme.fontWeight.semibold,
    color: theme.colors.foreground,
  },
  closeBtn: {
    width: 24,
    height: 24,
    alignItems: "center",
    justifyContent: "center",
  },
  subtitle: {
    fontSize: theme.fontSize.sm,
    color: theme.colors.foregroundMuted,
    lineHeight: 18,
  },
  sourceList: {
    gap: theme.spacing[2],
  },
  sourceItem: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[3],
    padding: theme.spacing[3],
    borderRadius: theme.borderRadius.md,
    backgroundColor: theme.colors.surface0,
    borderWidth: 1,
    borderColor: theme.colors.border,
  },
  sourceItemActive: {
    borderColor: theme.colors.foreground,
    backgroundColor: theme.colors.surface2,
  },
  sourceTextCol: {
    flex: 1,
    gap: 2,
  },
  sourceTitle: {
    fontSize: theme.fontSize.sm,
    fontWeight: theme.fontWeight.medium,
    color: theme.colors.foreground,
  },
  sourceDesc: {
    fontSize: 11,
    color: theme.colors.foregroundMuted,
  },
  textInput: {
    minHeight: 80,
    backgroundColor: theme.colors.surface0,
    borderRadius: theme.borderRadius.md,
    borderWidth: 1,
    borderColor: theme.colors.border,
    padding: theme.spacing[2],
    color: theme.colors.foreground,
    fontSize: theme.fontSize.sm,
    textAlignVertical: "top",
  },
  statusSuccess: {
    fontSize: theme.fontSize.sm,
    color: "#10b981",
    textAlign: "center",
  },
  footer: {
    flexDirection: "row",
    justifyContent: "flex-end",
    gap: theme.spacing[2],
    marginTop: theme.spacing[2],
  },
}));
