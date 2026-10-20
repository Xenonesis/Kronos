import React, { useCallback, useMemo, useState } from "react";
import { Text, View } from "react-native";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import { Check, Globe, Compass, FileText, KeyRound, X } from "lucide-react-native";
import { Button } from "@/components/ui/button";
import { PressHighlight } from "@/components/ui/press-highlight";
import { EditingTextInput as TextInput } from "@/components/ui/text-input";
import type { Theme } from "@/styles/theme";

type ImportSource = "chrome" | "edge" | "manual";

const ThemedCheck = withUnistyles(Check);
const ThemedGlobe = withUnistyles(Globe);
const ThemedCompass = withUnistyles(Compass);
const ThemedFileText = withUnistyles(FileText);
const ThemedKeyRound = withUnistyles(KeyRound);
const ThemedX = withUnistyles(X);

const foregroundIconMapping = (theme: Theme) => ({ color: theme.colors.foreground });

export interface BrowserImportDialogProps {
  visible: boolean;
  onClose: () => void;
  onImport: (cookiesText: string, source: ImportSource) => void;
  testID?: string;
}

const SOURCE_ROWS: {
  source: ImportSource;
  testID: string;
  title: string;
  description: string;
  Icon: typeof ThemedGlobe;
}[] = [
  {
    source: "chrome",
    testID: "import-source-chrome",
    title: "Google Chrome",
    description: "Auto-detect session cookies from default Chrome profile",
    Icon: ThemedGlobe,
  },
  {
    source: "edge",
    testID: "import-source-edge",
    title: "Microsoft Edge",
    description: "Auto-detect cookies from Edge profile",
    Icon: ThemedCompass,
  },
  {
    source: "manual",
    testID: "import-source-manual",
    title: "Paste Cookies / Storage JSON",
    description: "Enter custom Cookie header or token JSON",
    Icon: ThemedFileText,
  },
];

function ImportSourceRow({
  source,
  testID,
  title,
  description,
  Icon,
  selected,
  onSelect,
}: {
  source: ImportSource;
  testID: string;
  title: string;
  description: string;
  Icon: typeof ThemedGlobe;
  selected: boolean;
  onSelect: (source: ImportSource) => void;
}) {
  const handlePress = useCallback(() => {
    onSelect(source);
  }, [onSelect, source]);

  const accessibilityState = useMemo(() => ({ selected }), [selected]);

  return (
    <PressHighlight
      accessibilityRole="button"
      accessibilityState={accessibilityState}
      style={[styles.sourceItem, selected && styles.sourceItemActive]}
      onPress={handlePress}
      testID={testID}
    >
      <Icon size={18} uniProps={foregroundIconMapping} />
      <View style={styles.sourceTextCol}>
        <Text style={styles.sourceTitle}>{title}</Text>
        <Text style={styles.sourceDesc}>{description}</Text>
      </View>
      {selected ? <ThemedCheck size={16} uniProps={foregroundIconMapping} /> : null}
    </PressHighlight>
  );
}

export function BrowserImportDialog({
  visible,
  onClose,
  onImport,
  testID = "browser-import-dialog",
}: BrowserImportDialogProps) {
  const [manualCookies, setManualCookies] = useState("");
  const [selectedSource, setSelectedSource] = useState<ImportSource>("chrome");
  const [importedStatus, setImportedStatus] = useState<string | null>(null);

  const handleExecuteImport = useCallback(() => {
    onImport(manualCookies, selectedSource);
    setImportedStatus(`Imported session data from ${selectedSource}`);
    setTimeout(() => {
      onClose();
      setImportedStatus(null);
    }, 800);
  }, [manualCookies, onClose, onImport, selectedSource]);

  if (!visible) return null;

  return (
    <View style={styles.overlay} testID={testID}>
      <View style={styles.dialog}>
        <View style={styles.header}>
          <View style={styles.headerTitleRow}>
            <ThemedKeyRound size={16} uniProps={foregroundIconMapping} />
            <Text style={styles.title}>Import Browser Session & Cookies</Text>
          </View>
          <Button
            size="xs"
            variant="ghost"
            onPress={onClose}
            style={styles.closeBtn}
            testID="browser-import-close-btn"
          >
            <ThemedX size={16} uniProps={foregroundIconMapping} />
          </Button>
        </View>

        <Text style={styles.subtitle}>
          Import authentication cookies from your default browser so you are already logged in on
          local and web apps.
        </Text>

        {/* Source Selector */}
        <View style={styles.sourceList}>
          {SOURCE_ROWS.map((row) => (
            <ImportSourceRow
              key={row.source}
              source={row.source}
              testID={row.testID}
              title={row.title}
              description={row.description}
              Icon={row.Icon}
              selected={selectedSource === row.source}
              onSelect={setSelectedSource}
            />
          ))}
        </View>

        {/* Manual Input if selected */}
        {selectedSource === "manual" ? (
          <TextInput
            style={styles.textInput}
            multiline
            placeholder="session_id=xyz; auth_token=abc..."
            placeholderTextColor={styles.placeholder.color}
            initialValue={manualCookies}
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
  placeholder: {
    color: theme.colors.foregroundMuted,
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
