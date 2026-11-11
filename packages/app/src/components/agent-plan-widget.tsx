import { memo, useCallback, useMemo, useState } from "react";
import { View, Text } from "react-native";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import { CheckCircle2, Circle, ChevronDown, ChevronRight, ListTodo } from "lucide-react-native";
import { PressHighlight } from "@/components/ui/press-highlight";
import type { Theme } from "@/styles/theme";

interface TaskItem {
  id: string;
  title: string;
  completed: boolean;
}

interface AgentPlanWidgetProps {
  text: string;
}

const CHECKLIST_REGEX = /^\s*(?:[-*]|\d+\.)\s+\[([ xX])\]\s+(.+)$/gm;

export function parseChecklistTasks(text: string): TaskItem[] {
  if (!text || (!text.includes("[ ]") && !text.includes("[x]") && !text.includes("[X]"))) {
    return [];
  }

  const tasks: TaskItem[] = [];
  let match: RegExpExecArray | null;
  CHECKLIST_REGEX.lastIndex = 0;

  while ((match = CHECKLIST_REGEX.exec(text)) !== null) {
    const isCompleted = match[1].toLowerCase() === "x";
    const rawTitle = match[2].trim();
    if (rawTitle.length > 0) {
      tasks.push({
        id: `task_${tasks.length}_${rawTitle.slice(0, 16)}`,
        title: rawTitle,
        completed: isCompleted,
      });
    }
  }

  return tasks;
}

const ThemedListTodo = withUnistyles(ListTodo);
const ThemedChevronDown = withUnistyles(ChevronDown);
const ThemedChevronRight = withUnistyles(ChevronRight);
const ThemedCheckCircle = withUnistyles(CheckCircle2);
const ThemedCircle = withUnistyles(Circle);

const primaryIconStyle = (theme: Theme) => ({ color: theme.colors.foreground });
const mutedIconStyle = (theme: Theme) => ({ color: theme.colors.foregroundMuted });
const successIconStyle = (theme: Theme) => ({ color: theme.colors.success || "#22c55e" });

export const AgentPlanWidget = memo(function AgentPlanWidget({ text }: AgentPlanWidgetProps) {
  const [expanded, setExpanded] = useState(true);
  const toggleExpanded = useCallback(() => {
    setExpanded((prev) => !prev);
  }, []);
  const tasks = useMemo(() => parseChecklistTasks(text), [text]);

  if (tasks.length < 2) {
    return null;
  }

  const completedCount = tasks.filter((t) => t.completed).length;
  const totalCount = tasks.length;
  const percent = Math.round((completedCount / totalCount) * 100);

  return (
    <View style={styles.card} testID="agent-plan-widget">
      <PressHighlight
        style={styles.header}
        onPress={toggleExpanded}
        accessibilityRole="button"
        accessibilityLabel={`Execution Plan: ${completedCount} of ${totalCount} tasks completed`}
      >
        <View style={styles.headerLeft}>
          <ThemedListTodo size={16} uniProps={primaryIconStyle} />
          <Text style={styles.headerTitle}>Execution Plan</Text>
          <View style={styles.badge}>
            <Text style={styles.badgeText}>
              {completedCount}/{totalCount} ({percent}%)
            </Text>
          </View>
        </View>
        <View style={styles.headerRight}>
          {expanded ? (
            <ThemedChevronDown size={16} uniProps={mutedIconStyle} />
          ) : (
            <ThemedChevronRight size={16} uniProps={mutedIconStyle} />
          )}
        </View>
      </PressHighlight>

      <View style={styles.progressTrack}>
        <View style={[styles.progressBar, { width: `${percent}%` }]} />
      </View>

      {expanded ? (
        <View style={styles.tasksContainer}>
          {tasks.map((task) => (
            <View key={task.id} style={styles.taskRow}>
              <View style={styles.taskIcon}>
                {task.completed ? (
                  <ThemedCheckCircle size={15} uniProps={successIconStyle} />
                ) : (
                  <ThemedCircle size={15} uniProps={mutedIconStyle} />
                )}
              </View>
              <Text
                style={[
                  styles.taskTitle,
                  task.completed ? styles.taskCompleted : styles.taskPending,
                ]}
                numberOfLines={2}
              >
                {task.title}
              </Text>
            </View>
          ))}
        </View>
      ) : null}
    </View>
  );
});

const styles = StyleSheet.create((theme) => ({
  card: {
    marginVertical: theme.spacing[2],
    backgroundColor: theme.colors.surface1,
    borderRadius: theme.spacing[2],
    borderWidth: 1,
    borderColor: theme.colors.border,
    overflow: "hidden",
  },
  header: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    paddingHorizontal: theme.spacing[3],
    paddingVertical: theme.spacing[2],
    backgroundColor: theme.colors.surface2,
  },
  headerLeft: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
  },
  headerTitle: {
    fontSize: 13,
    fontWeight: "600",
    color: theme.colors.foreground,
  },
  badge: {
    paddingHorizontal: theme.spacing[2],
    paddingVertical: 2,
    borderRadius: 9999,
    backgroundColor: theme.colors.surface3 || theme.colors.surface2,
    borderWidth: 1,
    borderColor: theme.colors.border,
  },
  badgeText: {
    fontSize: 11,
    fontWeight: "500",
    color: theme.colors.foregroundMuted,
  },
  headerRight: {
    padding: 2,
  },
  progressTrack: {
    height: 3,
    width: "100%",
    backgroundColor: theme.colors.surface3 || "#262626",
  },
  progressBar: {
    height: "100%",
    backgroundColor: theme.colors.primary || "#3b82f6",
  },
  tasksContainer: {
    paddingHorizontal: theme.spacing[3],
    paddingVertical: theme.spacing[2],
    gap: theme.spacing[2],
  },
  taskRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
  },
  taskIcon: {
    width: 18,
    alignItems: "center",
    justifyContent: "center",
  },
  taskTitle: {
    flex: 1,
    fontSize: 13,
    lineHeight: 18,
  },
  taskPending: {
    color: theme.colors.foreground,
  },
  taskCompleted: {
    color: theme.colors.foregroundMuted,
    textDecorationLine: "line-through",
  },
}));
