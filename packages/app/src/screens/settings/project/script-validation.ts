import type { TFunction } from "i18next";
import type { ProjectScriptDraft } from "@/utils/project-config-form";

export interface ScriptValidation {
  hasErrors: boolean;
  nameError: string | null;
  commandError: string | null;
}

export function validateScript(script: ProjectScriptDraft, t: TFunction): ScriptValidation {
  const nameError =
    script.name.trim().length === 0 ? t("settings.project.scripts.nameRequired") : null;
  const commandError =
    script.commandText.trim().length === 0 ? t("settings.project.scripts.commandRequired") : null;
  return {
    hasErrors: Boolean(nameError || commandError),
    nameError,
    commandError,
  };
}
