import { useCallback, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { View } from "react-native";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import type {
  PaseoConfigRaw,
  PaseoConfigRevision,
  ProjectConfigRpcError,
} from "@getpaseo/protocol/messages";
import type { DaemonClient } from "@getpaseo/client/internal/daemon-client";
import { useToast } from "@/contexts/toast-context";
import { confirmDialog } from "@/utils/confirm-dialog";
import {
  applyDraftToConfig,
  configToDraft,
  type LifecycleOriginalKind,
  type MetadataPromptKey,
  type ProjectConfigDraft,
  type ProjectScriptDraft,
} from "@/utils/project-config-form";
import { ProjectWorktreeSection } from "./worktree-section";
import { ProjectMetadataSection } from "./metadata-section";
import { ProjectScriptsSection } from "./scripts-section";
import { ScriptEditModal } from "./script-edit-modal";
import { validateScript } from "./script-validation";
import { ProjectConfigSaveActions } from "./save-actions";

type ReadProjectConfigData = Awaited<ReturnType<DaemonClient["readProjectConfig"]>>;

export interface ProjectConfigFormProps {
  baseConfig: PaseoConfigRaw;
  revision: PaseoConfigRevision | null;
  hasUncommittedWorktreeSetupChanges: boolean;
  repoRoot: string;
  queryKey: readonly [string, string, string];
  client: DaemonClient;
  onReload: () => void;
}

export function ProjectConfigForm({
  baseConfig,
  revision,
  hasUncommittedWorktreeSetupChanges,
  repoRoot,
  queryKey,
  client,
  onReload,
}: ProjectConfigFormProps) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const toast = useToast();

  const [draft, setDraft] = useState<ProjectConfigDraft>(() => configToDraft(baseConfig));
  const [writeError, setWriteError] = useState<ProjectConfigRpcError | null>(null);
  const [editingScriptId, setEditingScriptId] = useState<string | null>(null);

  const saveMutation = useMutation({
    mutationFn: async (input: {
      config: PaseoConfigRaw;
      expectedRevision: PaseoConfigRevision | null;
    }) => {
      return client.writeProjectConfig({
        repoRoot,
        config: input.config,
        expectedRevision: input.expectedRevision,
      });
    },
    onSuccess: (result) => {
      if (result.ok) {
        queryClient.setQueryData<ReadProjectConfigData>(queryKey, {
          ok: true,
          config: result.config,
          revision: result.revision,
          requestId: "local-cache",
          repoRoot,
          ...(result.hasUncommittedWorktreeSetupChanges === undefined
            ? {}
            : {
                hasUncommittedWorktreeSetupChanges: result.hasUncommittedWorktreeSetupChanges,
              }),
        });
        setWriteError(null);
        queryClient.invalidateQueries({ queryKey: ["projects"] });
        toast.show(t("settings.project.actions.saved"), { variant: "success" });
      } else {
        setWriteError(result.error);
      }
    },
  });

  const handleSave = useCallback(() => {
    if (writeError?.code === "stale_project_config") return;
    const config = applyDraftToConfig({ draft, base: baseConfig });
    saveMutation.mutate({ config, expectedRevision: revision });
  }, [draft, baseConfig, revision, writeError, saveMutation]);

  const handleReload = useCallback(() => {
    setWriteError(null);
    onReload();
  }, [onReload]);

  const updateDraft = useCallback((updater: (draft: ProjectConfigDraft) => ProjectConfigDraft) => {
    setDraft((prev) => updater(prev));
  }, []);

  const handleSetupChange = useCallback(
    (text: string) => updateDraft((d) => ({ ...d, setupText: text })),
    [updateDraft],
  );
  const handleTeardownChange = useCallback(
    (text: string) => updateDraft((d) => ({ ...d, teardownText: text })),
    [updateDraft],
  );

  const handleMetadataPromptChange = useCallback(
    (key: MetadataPromptKey, text: string) =>
      updateDraft((d) => ({
        ...d,
        metadataPrompts: { ...d.metadataPrompts, [key]: text },
      })),
    [updateDraft],
  );

  const handleRemoveScript = useCallback(
    async (script: ProjectScriptDraft) => {
      const ok = await confirmDialog({
        title: t("settings.project.scripts.removeTitle"),
        message: t("settings.project.scripts.removeMessage", {
          name: script.name || t("settings.project.scripts.removeFallbackName"),
        }),
        confirmLabel: t("settings.project.scripts.actions.remove"),
        cancelLabel: t("settings.project.actions.cancel"),
        destructive: true,
      });
      if (!ok) return;
      updateDraft((d) => ({
        ...d,
        scripts: d.scripts.filter((entry) => entry.id !== script.id),
      }));
    },
    [t, updateDraft],
  );

  const handleEditScript = useCallback((script: ProjectScriptDraft) => {
    setEditingScriptId(script.id);
  }, []);

  const handleAddScript = useCallback(() => {
    const id = `script-draft-new-${Date.now()}`;
    updateDraft((d) => ({
      ...d,
      scripts: [
        ...d.scripts,
        {
          id,
          name: "",
          commandText: "",
          commandOriginalKind: "missing" satisfies LifecycleOriginalKind,
          type: "",
          portText: "",
          rawEntry: {},
        },
      ],
    }));
    setEditingScriptId(id);
  }, [updateDraft]);

  const handleEditingDraftChange = useCallback(
    (next: ProjectScriptDraft) => {
      updateDraft((d) => ({
        ...d,
        scripts: d.scripts.map((entry) => (entry.id === next.id ? next : entry)),
      }));
    },
    [updateDraft],
  );

  const handleCancelEditing = useCallback(() => {
    if (!editingScriptId) {
      return;
    }
    updateDraft((d) => {
      const entry = d.scripts.find((row) => row.id === editingScriptId);
      if (!entry) return d;
      const isEmpty =
        entry.name.trim().length === 0 &&
        entry.commandText.trim().length === 0 &&
        entry.type.trim().length === 0 &&
        entry.portText.trim().length === 0;
      if (!isEmpty) return d;
      return { ...d, scripts: d.scripts.filter((row) => row.id !== editingScriptId) };
    });
    setEditingScriptId(null);
  }, [editingScriptId, updateDraft]);

  const handleSaveEditing = useCallback(() => {
    setEditingScriptId(null);
  }, []);

  const editingScript = draft.scripts.find((entry) => entry.id === editingScriptId);

  const hasInvalidScripts = useMemo(
    () => draft.scripts.some((script) => validateScript(script, t).hasErrors),
    [draft.scripts, t],
  );

  return (
    <View>
      <ProjectWorktreeSection
        setupText={draft.setupText}
        teardownText={draft.teardownText}
        hasUncommittedWorktreeSetupChanges={hasUncommittedWorktreeSetupChanges}
        onSetupChange={handleSetupChange}
        onTeardownChange={handleTeardownChange}
      />

      <ProjectScriptsSection
        scripts={draft.scripts}
        onAdd={handleAddScript}
        onEdit={handleEditScript}
        onRemove={handleRemoveScript}
      />

      <ProjectMetadataSection
        metadataPrompts={draft.metadataPrompts}
        onChange={handleMetadataPromptChange}
      />

      <ProjectConfigSaveActions
        writeError={writeError}
        isPending={saveMutation.isPending}
        hasInvalidScripts={hasInvalidScripts}
        onSave={handleSave}
        onReload={handleReload}
      />

      {editingScript ? (
        <ScriptEditModal
          script={editingScript}
          onChange={handleEditingDraftChange}
          onCancel={handleCancelEditing}
          onSave={handleSaveEditing}
        />
      ) : null}
    </View>
  );
}
