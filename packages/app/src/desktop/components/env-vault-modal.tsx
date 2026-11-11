import { memo, useState, useCallback } from "react";
import { View, Text, Modal, Pressable, TextInput, ScrollView } from "react-native";
import { StyleSheet, useUnistyles } from "react-native-unistyles";
import {
  Lock,
  Eye,
  EyeOff,
  Copy,
  Check,
  Plus,
  Trash2,
  X,
  ShieldAlert,
  KeyRound,
} from "lucide-react-native";
import * as Clipboard from "expo-clipboard";
import { Button } from "@/components/ui/button";

export interface EnvVariable {
  key: string;
  value: string;
  isSecret: boolean;
}

export interface EnvVaultModalProps {
  visible: boolean;
  onClose: () => void;
  envFileName?: string;
  initialVariables?: EnvVariable[];
  onSave?: (variables: EnvVariable[]) => void;
}

const DEFAULT_ENV_VARS: EnvVariable[] = [
  { key: "PORT", value: "3000", isSecret: false },
  { key: "NODE_ENV", value: "development", isSecret: false },
  {
    key: "DATABASE_URL",
    value: "postgresql://postgres:secretpassword@localhost:5432/kronos",
    isSecret: true,
  },
  { key: "OPENAI_API_KEY", value: "sk-proj-98a7sd987fa98sdf79as8d7f", isSecret: true },
  { key: "ANTHROPIC_API_KEY", value: "sk-ant-api03-as98df7as8d7fa89sd", isSecret: true },
];

export const EnvVaultModal = memo(function EnvVaultModal({
  visible,
  onClose,
  envFileName = ".env.local",
  initialVariables = DEFAULT_ENV_VARS,
  onSave,
}: EnvVaultModalProps) {
  const { theme } = useUnistyles();
  const [variables, setVariables] = useState<EnvVariable[]>(initialVariables);
  const [revealedKeys, setRevealedKeys] = useState<Record<string, boolean>>({});
  const [copiedKey, setCopiedKey] = useState<string | null>(null);
  const [newKey, setNewKey] = useState("");
  const [newValue, setNewValue] = useState("");

  const toggleReveal = useCallback((key: string) => {
    setRevealedKeys((prev) => ({
      ...prev,
      [key]: !prev[key],
    }));
  }, []);

  const handleCopy = useCallback(async (key: string, val: string) => {
    await Clipboard.setStringAsync(val);
    setCopiedKey(key);
    setTimeout(() => {
      setCopiedKey(null);
    }, 2000);
  }, []);

  const handleAdd = useCallback(() => {
    if (!newKey.trim()) return;
    const isSecret = /key|token|secret|password|auth|cert/i.test(newKey);
    const updated: EnvVariable[] = [
      ...variables,
      { key: newKey.trim().toUpperCase(), value: newValue.trim(), isSecret },
    ];
    setVariables(updated);
    setNewKey("");
    setNewValue("");
    onSave?.(updated);
  }, [newKey, newValue, variables, onSave]);

  const handleDelete = useCallback(
    (key: string) => {
      const updated = variables.filter((v) => v.key !== key);
      setVariables(updated);
      onSave?.(updated);
    },
    [variables, onSave],
  );

  if (!visible) return null;

  return (
    <Modal visible={visible} transparent animationType="fade" onRequestClose={onClose}>
      <View style={styles.backdrop}>
        <View style={styles.card}>
          {/* Header */}
          <View style={styles.header}>
            <View style={styles.titleRow}>
              <Lock size={18} color={theme.colors.accent} />
              <View>
                <Text style={styles.titleText}>Environment Secrets Vault</Text>
                <Text style={styles.subtitleText}>{envFileName}</Text>
              </View>
            </View>
            <Pressable onPress={onClose} style={styles.closeBtn}>
              <X size={18} color={theme.colors.foregroundMuted} />
            </Pressable>
          </View>

          {/* Security Banner */}
          <View style={styles.securityBanner}>
            <ShieldAlert size={14} color="#f59e0b" />
            <Text style={styles.securityBannerText}>
              Values are masked by default. Sensitive keys are filtered before being sent to public
              agent contexts.
            </Text>
          </View>

          {/* Add Variable Row */}
          <View style={styles.addRow}>
            <TextInput
              value={newKey}
              onChangeText={setNewKey}
              placeholder="VARIABLE_NAME"
              placeholderTextColor={theme.colors.foregroundMuted}
              style={styles.keyInput}
              autoCapitalize="characters"
            />
            <TextInput
              value={newValue}
              onChangeText={setNewValue}
              placeholder="value"
              placeholderTextColor={theme.colors.foregroundMuted}
              style={styles.valInput}
              secureTextEntry={false}
            />
            <Button size="sm" variant="default" onPress={handleAdd}>
              <Plus size={14} color="#ffffff" />
              <Text style={styles.addBtnText}>Add</Text>
            </Button>
          </View>

          {/* Variables List */}
          <ScrollView style={styles.list}>
            {variables.map((v) => {
              const isRevealed = revealedKeys[v.key] ?? !v.isSecret;
              const isCopied = copiedKey === v.key;
              return (
                <View key={v.key} style={styles.row}>
                  <View style={styles.keyContainer}>
                    <KeyRound size={12} color={theme.colors.foregroundMuted} />
                    <Text style={styles.keyText}>{v.key}</Text>
                  </View>

                  <Text style={styles.valueText} numberOfLines={1}>
                    {isRevealed ? v.value : "••••••••••••••••••••••••"}
                  </Text>

                  <View style={styles.actions}>
                    {v.isSecret && (
                      <Pressable
                        onPress={() => toggleReveal(v.key)}
                        style={styles.iconBtn}
                        accessibilityLabel="Toggle reveal"
                      >
                        {isRevealed ? (
                          <EyeOff size={14} color={theme.colors.foregroundMuted} />
                        ) : (
                          <Eye size={14} color={theme.colors.foregroundMuted} />
                        )}
                      </Pressable>
                    )}

                    <Pressable
                      onPress={() => handleCopy(v.key, v.value)}
                      style={styles.iconBtn}
                      accessibilityLabel="Copy value"
                    >
                      {isCopied ? (
                        <Check size={14} color={theme.colors.success ?? "#16a34a"} />
                      ) : (
                        <Copy size={14} color={theme.colors.foregroundMuted} />
                      )}
                    </Pressable>

                    <Pressable
                      onPress={() => handleDelete(v.key)}
                      style={styles.iconBtn}
                      accessibilityLabel="Delete variable"
                    >
                      <Trash2 size={14} color="#dc2626" />
                    </Pressable>
                  </View>
                </View>
              );
            })}
          </ScrollView>

          {/* Footer */}
          <View style={styles.footer}>
            <Text style={styles.footerCount}>{variables.length} environment variables</Text>
            <Button size="sm" variant="default" onPress={onClose}>
              Done
            </Button>
          </View>
        </View>
      </View>
    </Modal>
  );
});

const styles = StyleSheet.create((theme) => ({
  backdrop: {
    flex: 1,
    backgroundColor: "rgba(0, 0, 0, 0.65)",
    justifyContent: "center",
    alignItems: "center",
    padding: theme.spacing[4],
  },
  card: {
    width: "92%",
    maxWidth: 720,
    maxHeight: 620,
    backgroundColor: theme.colors.surface1,
    borderRadius: theme.borderRadius.lg,
    borderWidth: 1,
    borderColor: theme.colors.border,
    overflow: "hidden",
  },
  header: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    paddingHorizontal: theme.spacing[4],
    paddingVertical: theme.spacing[3],
    backgroundColor: theme.colors.surface0,
    borderBottomWidth: 1,
    borderBottomColor: theme.colors.border,
  },
  titleRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
  },
  titleText: {
    fontSize: theme.fontSize.base,
    fontWeight: "600",
    color: theme.colors.foreground,
  },
  subtitleText: {
    fontSize: theme.fontSize.code,
    color: theme.colors.foregroundMuted,
  },
  closeBtn: {
    padding: theme.spacing[1],
  },
  securityBanner: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
    backgroundColor: "rgba(245, 158, 11, 0.12)",
    paddingHorizontal: theme.spacing[3],
    paddingVertical: theme.spacing[2],
    borderBottomWidth: 1,
    borderBottomColor: theme.colors.border,
  },
  securityBannerText: {
    fontSize: theme.fontSize.code,
    color: "#b45309",
    flex: 1,
    lineHeight: 16,
  },
  addRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
    paddingHorizontal: theme.spacing[3],
    paddingVertical: theme.spacing[2],
    backgroundColor: theme.colors.surface0,
    borderBottomWidth: 1,
    borderBottomColor: theme.colors.border,
  },
  keyInput: {
    width: 180,
    fontSize: theme.fontSize.code,
    fontFamily: theme.fontFamily.mono,
    color: theme.colors.foreground,
    backgroundColor: theme.colors.surface2,
    paddingHorizontal: theme.spacing[2],
    paddingVertical: 4,
    borderRadius: theme.borderRadius.sm,
  },
  valInput: {
    flex: 1,
    fontSize: theme.fontSize.code,
    color: theme.colors.foreground,
    backgroundColor: theme.colors.surface2,
    paddingHorizontal: theme.spacing[2],
    paddingVertical: 4,
    borderRadius: theme.borderRadius.sm,
  },
  addBtnText: {
    color: "#ffffff",
    fontWeight: "600",
  },
  list: {
    maxHeight: 380,
    padding: theme.spacing[3],
  },
  row: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    paddingHorizontal: theme.spacing[3],
    paddingVertical: theme.spacing[2],
    backgroundColor: theme.colors.surface0,
    borderRadius: theme.borderRadius.sm,
    borderWidth: 1,
    borderColor: theme.colors.border,
    marginBottom: theme.spacing[2],
    gap: theme.spacing[2],
  },
  keyContainer: {
    flexDirection: "row",
    alignItems: "center",
    gap: 6,
    width: 200,
  },
  keyText: {
    fontSize: theme.fontSize.code,
    fontFamily: theme.fontFamily.mono,
    fontWeight: "600",
    color: theme.colors.foreground,
  },
  valueText: {
    flex: 1,
    fontFamily: theme.fontFamily.mono,
    fontSize: theme.fontSize.code,
    color: theme.colors.foregroundMuted,
  },
  actions: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[1],
  },
  iconBtn: {
    padding: 6,
  },
  footer: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    paddingHorizontal: theme.spacing[4],
    paddingVertical: theme.spacing[2],
    backgroundColor: theme.colors.surface0,
    borderTopWidth: 1,
    borderTopColor: theme.colors.border,
  },
  footerCount: {
    fontSize: theme.fontSize.code,
    color: theme.colors.foregroundMuted,
  },
}));
