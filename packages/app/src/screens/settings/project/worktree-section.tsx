import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { Alert } from "@/components/ui/alert";
import { ExternalLink } from "@/components/ui/external-link";
import { SettingsTextAreaCard } from "@/components/settings-textarea";
import { SettingsGroup } from "@/components/settings/headings/settings-group";
import { SettingsSection } from "@/components/settings/headings/settings-section";

const WORKTREE_DOCS_URL = "https://paseo.sh/docs/worktrees";

interface ProjectWorktreeSectionProps {
  setupText: string;
  teardownText: string;
  hasUncommittedWorktreeSetupChanges: boolean;
  onSetupChange: (text: string) => void;
  onTeardownChange: (text: string) => void;
}

export function ProjectWorktreeSection({
  setupText,
  teardownText,
  hasUncommittedWorktreeSetupChanges,
  onSetupChange,
  onTeardownChange,
}: ProjectWorktreeSectionProps) {
  const { t } = useTranslation();
  const setupDocsLink = useMemo(
    () => (
      <ExternalLink
        href={WORKTREE_DOCS_URL}
        label={t("settings.project.worktree.docs")}
        tooltip={t("settings.project.worktree.docsTooltip")}
        testID="worktree-setup-docs-link"
      />
    ),
    [t],
  );
  const teardownDocsLink = useMemo(
    () => (
      <ExternalLink
        href={WORKTREE_DOCS_URL}
        label={t("settings.project.worktree.docs")}
        tooltip={t("settings.project.worktree.docsTooltip")}
        testID="worktree-teardown-docs-link"
      />
    ),
    [t],
  );

  return (
    <SettingsGroup
      title={t("settings.project.worktree.title")}
      info={t("settings.project.worktree.info")}
      testID="worktree-group"
    >
      <SettingsSection
        title={t("settings.project.worktree.setup")}
        testID="worktree-setup-section"
        trailing={setupDocsLink}
      >
        {hasUncommittedWorktreeSetupChanges ? (
          <Alert
            size="sm"
            variant="warning"
            title={t("settings.project.worktree.uncommittedTitle")}
            description={t("settings.project.worktree.uncommittedDescription")}
          />
        ) : null}
        <SettingsTextAreaCard
          testID="worktree-setup-input"
          accessibilityLabel={t("settings.project.worktree.setupAccessibility")}
          value={setupText}
          onChangeText={onSetupChange}
          placeholder="npm install"
        />
      </SettingsSection>

      <SettingsSection
        title={t("settings.project.worktree.teardown")}
        testID="worktree-teardown-section"
        trailing={teardownDocsLink}
        flush
      >
        <SettingsTextAreaCard
          testID="worktree-teardown-input"
          accessibilityLabel={t("settings.project.worktree.teardownAccessibility")}
          value={teardownText}
          onChangeText={onTeardownChange}
          placeholder="docker compose down"
        />
      </SettingsSection>
    </SettingsGroup>
  );
}
