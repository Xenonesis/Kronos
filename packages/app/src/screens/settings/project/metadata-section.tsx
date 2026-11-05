import { useCallback } from "react";
import { useTranslation } from "react-i18next";
import { SettingsTextAreaCard } from "@/components/settings-textarea";
import { SettingsGroup } from "@/components/settings/headings/settings-group";
import { SettingsSection } from "@/components/settings/headings/settings-section";
import {
  METADATA_PROMPT_KEYS,
  type MetadataPromptKey,
  type ProjectConfigDraft,
} from "@/utils/project-config-form";

interface MetadataPromptField {
  titleKey: string;
  placeholderKey: string;
  sectionTestID: string;
  inputTestID: string;
}

const METADATA_PROMPT_FIELDS: Record<MetadataPromptKey, MetadataPromptField> = {
  branchName: {
    titleKey: "settings.project.metadata.branchName",
    placeholderKey: "settings.project.metadata.branchNamePlaceholder",
    sectionTestID: "metadata-prompt-branch-name-section",
    inputTestID: "metadata-prompt-branch-name-input",
  },
  commitMessage: {
    titleKey: "settings.project.metadata.commitMessage",
    placeholderKey: "settings.project.metadata.commitMessagePlaceholder",
    sectionTestID: "metadata-prompt-commit-message-section",
    inputTestID: "metadata-prompt-commit-message-input",
  },
  pullRequest: {
    titleKey: "settings.project.metadata.pullRequest",
    placeholderKey: "settings.project.metadata.pullRequestPlaceholder",
    sectionTestID: "metadata-prompt-pull-request-section",
    inputTestID: "metadata-prompt-pull-request-input",
  },
};

interface ProjectMetadataSectionProps {
  metadataPrompts: ProjectConfigDraft["metadataPrompts"];
  onChange: (key: MetadataPromptKey, text: string) => void;
}

export function ProjectMetadataSection({ metadataPrompts, onChange }: ProjectMetadataSectionProps) {
  const { t } = useTranslation();
  return (
    <SettingsGroup
      title={t("settings.project.metadata.title")}
      info={t("settings.project.metadata.info")}
      testID="metadata-group"
    >
      {METADATA_PROMPT_KEYS.map((key, index) => (
        <MetadataPromptSection
          key={key}
          promptKey={key}
          value={metadataPrompts[key]}
          onChange={onChange}
          flush={index === METADATA_PROMPT_KEYS.length - 1}
        />
      ))}
    </SettingsGroup>
  );
}

interface MetadataPromptSectionProps {
  promptKey: MetadataPromptKey;
  value: string;
  onChange: (key: MetadataPromptKey, text: string) => void;
  flush?: boolean;
}

function MetadataPromptSection({ promptKey, value, onChange, flush }: MetadataPromptSectionProps) {
  const { t } = useTranslation();
  const meta = METADATA_PROMPT_FIELDS[promptKey];
  const title = t(meta.titleKey);
  const handleChange = useCallback(
    (text: string) => onChange(promptKey, text),
    [onChange, promptKey],
  );
  return (
    <SettingsSection title={title} testID={meta.sectionTestID} flush={flush}>
      <SettingsTextAreaCard
        testID={meta.inputTestID}
        accessibilityLabel={title}
        value={value}
        onChangeText={handleChange}
        placeholder={t(meta.placeholderKey)}
      />
    </SettingsSection>
  );
}
