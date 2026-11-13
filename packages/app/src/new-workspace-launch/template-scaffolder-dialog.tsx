import React, { memo, useCallback, useState } from "react";
import { View, Text, ScrollView } from "react-native";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import { Rocket, Sparkles, X } from "lucide-react-native";
import { useToast } from "@/contexts/toast-context";
import { AdaptiveTextInput } from "@/components/adaptive-text-input";
import { Button } from "@/components/ui/button";
import { PressHighlight } from "@/components/ui/press-highlight";

const ThemedRocketIcon = withUnistyles(Rocket, (theme) => ({ color: theme.colors.accent }));
const ThemedCloseIcon = withUnistyles(X, (theme) => ({ color: theme.colors.foregroundMuted }));

export interface ProjectTemplate {
  id: string;
  name: string;
  tagline: string;
  category: "Fullstack" | "Frontend" | "Desktop" | "Backend";
  stack: string[];
  defaultCommand: string;
}

export const POPULAR_TEMPLATES: ProjectTemplate[] = [
  {
    id: "nextjs-15",
    name: "Next.js 15 App Router",
    tagline: "Modern fullstack React with Server Components & Tailwind CSS",
    category: "Fullstack",
    stack: ["Next.js 15", "React 19", "Tailwind CSS", "TypeScript"],
    defaultCommand: "npx create-next-app@latest my-app --typescript --tailwind --app --eslint",
  },
  {
    id: "vite-react",
    name: "Vite + React SPA",
    tagline: "Ultra-fast frontend Single Page Application with Vitest",
    category: "Frontend",
    stack: ["Vite", "React 19", "Tailwind CSS", "Vitest"],
    defaultCommand: "npm create vite@latest my-app -- --template react-ts",
  },
  {
    id: "tauri-desktop",
    name: "Tauri 2.0 Native Desktop",
    tagline: "High-performance native desktop app powered by Rust & React",
    category: "Desktop",
    stack: ["Tauri v2", "Rust", "React", "TypeScript"],
    defaultCommand: "npm create tauri-app@latest my-app",
  },
  {
    id: "sveltekit",
    name: "SvelteKit 2 App",
    tagline: "Lightweight reactive web application with Svelte 5 runes",
    category: "Frontend",
    stack: ["SvelteKit 2", "Svelte 5", "Vite", "Tailwind CSS"],
    defaultCommand: "npx sv create my-app",
  },
  {
    id: "fastapi-python",
    name: "FastAPI Microservice",
    tagline: "High-performance Python async backend with OpenAPI & Pydantic",
    category: "Backend",
    stack: ["Python 3.12", "FastAPI", "Pydantic v2", "Uvicorn"],
    defaultCommand: "pip install fastapi uvicorn pydantic",
  },
  {
    id: "express-ts",
    name: "Express.js REST API",
    tagline: "Type-safe Express backend with Zod validation and Vitest",
    category: "Backend",
    stack: ["Express", "TypeScript", "Zod", "Vitest"],
    defaultCommand: "npm init -y && npm i express zod && npm i -D typescript @types/node",
  },
];

export interface TemplateScaffolderDialogProps {
  visible: boolean;
  onClose: () => void;
  onScaffold: (template: ProjectTemplate, projectName: string) => void;
}

const TemplateCardItem = memo(function TemplateCardItem({
  template,
  isSelected,
  onSelect,
}: {
  template: ProjectTemplate;
  isSelected: boolean;
  onSelect: (template: ProjectTemplate) => void;
}) {
  const handlePress = useCallback(() => {
    onSelect(template);
  }, [onSelect, template]);

  return (
    <PressHighlight
      onPress={handlePress}
      style={[styles.templateCard, isSelected && styles.templateCardSelected]}
    >
      <View style={styles.cardHeader}>
        <Text style={styles.templateName}>{template.name}</Text>
        <Text style={styles.categoryBadge}>{template.category}</Text>
      </View>
      <Text style={styles.templateTagline}>{template.tagline}</Text>
      <View style={styles.stackPills}>
        {template.stack.map((s) => (
          <View key={s} style={styles.stackPill}>
            <Text style={styles.stackText}>{s}</Text>
          </View>
        ))}
      </View>
    </PressHighlight>
  );
});

export const TemplateScaffolderDialog = memo(function TemplateScaffolderDialog({
  visible,
  onClose,
  onScaffold,
}: TemplateScaffolderDialogProps) {
  const toast = useToast();
  const [selectedTemplate, setSelectedTemplate] = useState<ProjectTemplate>(POPULAR_TEMPLATES[0]);
  const [projectName, setProjectName] = useState("my-kronos-app");

  const handleCreate = useCallback(() => {
    if (!projectName.trim()) {
      toast.show("Please enter a valid project name", { variant: "error" });
      return;
    }
    onScaffold(selectedTemplate, projectName.trim());
    toast.show(`Scaffolding ${selectedTemplate.name} template...`, { variant: "success" });
    onClose();
  }, [onClose, onScaffold, projectName, selectedTemplate, toast]);

  if (!visible) return null;

  return (
    <View style={styles.overlay}>
      <View style={styles.modal}>
        {/* Header */}
        <View style={styles.header}>
          <View style={styles.headerTitleRow}>
            <ThemedRocketIcon size={20} />
            <Text style={styles.headerTitle}>1-Click Project Template Scaffolder</Text>
          </View>
          <PressHighlight onPress={onClose} style={styles.closeButton}>
            <ThemedCloseIcon size={18} />
          </PressHighlight>
        </View>

        {/* Project Name Input */}
        <View style={styles.projectNameBlock}>
          <Text style={styles.inputLabel}>Project Directory Name</Text>
          <AdaptiveTextInput
            style={styles.projectNameInput}
            initialValue={projectName}
            onChangeText={setProjectName}
            placeholder="my-kronos-app"
          />
        </View>

        {/* Templates Grid */}
        <ScrollView style={styles.templateScroll}>
          {POPULAR_TEMPLATES.map((tmpl) => (
            <TemplateCardItem
              key={tmpl.id}
              template={tmpl}
              isSelected={selectedTemplate.id === tmpl.id}
              onSelect={setSelectedTemplate}
            />
          ))}
        </ScrollView>

        {/* Footer */}
        <View style={styles.footer}>
          <Button variant="outline" size="sm" onPress={onClose}>
            Cancel
          </Button>
          <Button variant="default" size="sm" onPress={handleCreate} leftIcon={Sparkles}>
            Scaffold &amp; Launch
          </Button>
        </View>
      </View>
    </View>
  );
});

const styles = StyleSheet.create((theme) => ({
  overlay: {
    position: "absolute",
    top: 0,
    left: 0,
    right: 0,
    bottom: 0,
    backgroundColor: "rgba(0,0,0,0.7)",
    justifyContent: "center",
    alignItems: "center",
    zIndex: 1200,
  },
  modal: {
    width: "90%",
    maxWidth: 640,
    maxHeight: "85%",
    backgroundColor: theme.colors.surface0,
    borderRadius: 12,
    borderWidth: 1,
    borderColor: theme.colors.border,
    padding: 20,
    boxShadow: "0 10px 30px rgba(0,0,0,0.5)",
  },
  header: {
    flexDirection: "row",
    justifyContent: "space-between",
    alignItems: "center",
    marginBottom: 16,
  },
  headerTitleRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: 8,
  },
  headerTitle: {
    fontSize: 16,
    fontWeight: "700",
    color: theme.colors.foreground,
  },
  closeButton: {
    padding: 6,
  },
  projectNameBlock: {
    marginBottom: 16,
  },
  inputLabel: {
    fontSize: 11,
    fontWeight: "600",
    color: theme.colors.foregroundMuted,
    marginBottom: 6,
  },
  projectNameInput: {
    backgroundColor: theme.colors.surface1,
    borderRadius: 8,
    borderWidth: 1,
    borderColor: theme.colors.border,
    paddingHorizontal: 12,
    paddingVertical: 8,
    color: theme.colors.foreground,
    fontSize: 13,
  },
  templateScroll: {
    maxHeight: 380,
    marginBottom: 16,
  },
  templateCard: {
    backgroundColor: theme.colors.surface1,
    borderRadius: 8,
    borderWidth: 1,
    borderColor: theme.colors.border,
    padding: 12,
    marginBottom: 8,
  },
  templateCardSelected: {
    borderColor: theme.colors.accent,
    backgroundColor: "rgba(99, 102, 241, 0.08)",
  },
  cardHeader: {
    flexDirection: "row",
    justifyContent: "space-between",
    alignItems: "center",
    marginBottom: 4,
  },
  templateName: {
    fontSize: 13,
    fontWeight: "700",
    color: theme.colors.foreground,
  },
  categoryBadge: {
    fontSize: 10,
    fontWeight: "600",
    color: theme.colors.accent,
    backgroundColor: "rgba(99, 102, 241, 0.15)",
    paddingHorizontal: 6,
    paddingVertical: 2,
    borderRadius: 4,
  },
  templateTagline: {
    fontSize: 11,
    color: theme.colors.foregroundMuted,
    marginBottom: 8,
  },
  stackPills: {
    flexDirection: "row",
    flexWrap: "wrap",
    gap: 6,
  },
  stackPill: {
    backgroundColor: "rgba(0,0,0,0.2)",
    paddingHorizontal: 6,
    paddingVertical: 2,
    borderRadius: 4,
  },
  stackText: {
    fontSize: 10,
    color: theme.colors.foregroundMuted,
  },
  footer: {
    flexDirection: "row",
    justifyContent: "flex-end",
    gap: 8,
  },
  cancelButton: {
    paddingHorizontal: 12,
    paddingVertical: 8,
    borderRadius: 6,
    borderWidth: 1,
    borderColor: theme.colors.border,
  },
  cancelButtonText: {
    fontSize: 12,
    color: theme.colors.foregroundMuted,
  },
  createButton: {
    flexDirection: "row",
    alignItems: "center",
    gap: 6,
    backgroundColor: theme.colors.accent,
    paddingHorizontal: 16,
    paddingVertical: 8,
    borderRadius: 6,
  },
  createButtonText: {
    fontSize: 12,
    fontWeight: "600",
    color: "#ffffff",
  },
}));
