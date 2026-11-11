import { memo, useCallback, useMemo, useState } from "react";
import { View, Text } from "react-native";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import {
  CheckCircle2,
  Circle,
  ChevronDown,
  ChevronRight,
  ListTodo,
  Plus,
  Loader2,
} from "lucide-react-native";
import { PressHighlight } from "@/components/ui/press-highlight";
import { EditingTextInput } from "@/components/ui/text-input";
import type { Theme } from "@/styles/theme";

export interface TaskItem {
  id: string;
  title: string;
  completed: boolean;
  inProgress?: boolean;
}

interface AgentPlanWidgetProps {
  text: string;
  onTasksChange?: (tasks: TaskItem[]) => void;
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

  // Mark the first non-completed item as inProgress
  let foundFirstPending = false;
  for (const item of tasks) {
    if (!item.completed && !foundFirstPending) {
      item.inProgress = true;
      foundFirstPending = true;
    }
  }

  return tasks;
}

function toggleTaskInList(source: TaskItem[], targetId: string): TaskItem[] {
  const result: TaskItem[] = [];
  for (const t of source) {
    if (t.id === targetId) {
      result.push(Object.assign({}, t, { completed: !t.completed }));
    } else {
      result.push(t);
    }
  }
  return result;
}

const ThemedListTodo = withUnistyles(ListTodo);
const ThemedChevronDown = withUnistyles(ChevronDown);
const ThemedChevronRight = withUnistyles(ChevronRight);
const ThemedCheckCircle = withUnistyles(CheckCircle2);
const ThemedCircle = withUnistyles(Circle);
const ThemedLoader = withUnistyles(Loader2);
const ThemedPlus = withUnistyles(Plus);

const primaryIconStyle = (theme: Theme) => ({ color: theme.colors.foreground });
const mutedIconStyle = (theme: Theme) => ({ color: theme.colors.foregroundMuted });
const successIconStyle = (theme: Theme) => ({ color: theme.colors.success || "#22c55e" });
const inProgressIconStyle = () => ({ color: "#3b82f6" });

interface TaskRowProps {
  task: TaskItem;
  onToggle: (id: string) => void;
}

const TaskRow = memo(function TaskRow({ task, onToggle }: TaskRowProps) {
  const handleToggle = useCallback(() => {
    onToggle(task.id);
  }, [onToggle, task.id]);

  const accessibilityState = useMemo(() => ({ checked: task.completed }), [task.completed]);

  const taskIcon = useMemo(() => {
    if (task.completed) {
      return <ThemedCheckCircle size={15} uniProps={successIconStyle} />;
    }
    if (task.inProgress) {
      return <ThemedLoader size={15} uniProps={inProgressIconStyle} />;
    }
    return <ThemedCircle size={15} uniProps={mutedIconStyle} />;
  }, [task.completed, task.inProgress]);

  const textStyle = useMemo(() => {
    if (task.completed) {
      return styles.taskCompleted;
    }
    if (task.inProgress) {
      return styles.taskInProgress;
    }
    return styles.taskPending;
  }, [task.completed, task.inProgress]);

  return (
    <PressHighlight
      style={styles.taskRow}
      onPress={handleToggle}
      accessibilityRole="checkbox"
      accessibilityState={accessibilityState}
      accessibilityLabel={task.title}
    >
      <View style={styles.taskIcon}>{taskIcon}</View>
      <Text style={[styles.taskTitle, textStyle]} numberOfLines={2}>
        {task.title}
      </Text>
      {task.inProgress && (
        <View style={styles.inProgressBadge}>
          <Text style={styles.inProgressText}>Running</Text>
        </View>
      )}
    </PressHighlight>
  );
});

export const AgentPlanWidget = memo(function AgentPlanWidget({
  text,
  onTasksChange,
}: AgentPlanWidgetProps) {
  const [expanded, setExpanded] = useState(true);
  const [localTasks, setLocalTasks] = useState<TaskItem[] | null>(null);
  const [newTaskInput, setNewTaskInput] = useState("");
  const [isAdding, setIsAdding] = useState(false);

  const initialTasks = useMemo(() => parseChecklistTasks(text), [text]);
  const tasks = localTasks ?? initialTasks;

  const toggleExpanded = useCallback(() => {
    setExpanded((prev) => !prev);
  }, []);

  const handleToggleTask = useCallback(
    (id: string) => {
      setLocalTasks((prev) => {
        const source = prev ?? initialTasks;
        const updated = toggleTaskInList(source, id);
        onTasksChange?.(updated);
        return updated;
      });
    },
    [initialTasks, onTasksChange],
  );

  const handleAddTask = useCallback(() => {
    const trimmed = newTaskInput.trim();
    if (!trimmed) {
      setIsAdding(false);
      return;
    }
    setLocalTasks((prev) => {
      const source = prev ?? initialTasks;
      const updated = [
        ...source,
        {
          id: `task_${source.length}_${trimmed.slice(0, 16)}`,
          title: trimmed,
          completed: false,
        },
      ];
      onTasksChange?.(updated);
      return updated;
    });
    setNewTaskInput("");
    setIsAdding(false);
  }, [newTaskInput, initialTasks, onTasksChange]);

  const handleStartAdding = useCallback(() => {
    setIsAdding(true);
  }, []);

  if (tasks.length < 2 && !isAdding) {
    return null;
  }

  const completedCount = tasks.filter((t) => t.completed).length;
  const totalCount = tasks.length;
  const percent = totalCount > 0 ? Math.round((completedCount / totalCount) * 100) : 0;

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
          <Text style={styles.headerTitle}>Task Execution Plan</Text>
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
            <TaskRow key={task.id} task={task} onToggle={handleToggleTask} />
          ))}

          {isAdding ? (
            <View style={styles.addTaskRow}>
              <EditingTextInput
                initialValue={newTaskInput}
                onChangeText={setNewTaskInput}
                placeholder="Add next action item..."
                onSubmitEditing={handleAddTask}
                autoFocus
                style={styles.addTaskInput}
              />
            </View>
          ) : (
            <PressHighlight style={styles.addTaskButton} onPress={handleStartAdding}>
              <ThemedPlus size={13} uniProps={mutedIconStyle} />
              <Text style={styles.addTaskButtonText}>Add Step</Text>
            </PressHighlight>
          )}
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
    fontWeight: "600",
    color: theme.colors.foregroundMuted,
  },
  headerRight: {
    flexDirection: "row",
    alignItems: "center",
  },
  progressTrack: {
    height: 3,
    width: "100%",
    backgroundColor: theme.colors.surface3 || "#333",
  },
  progressBar: {
    height: "100%",
    backgroundColor: theme.colors.success || "#22c55e",
  },
  tasksContainer: {
    paddingVertical: theme.spacing[1],
    paddingHorizontal: theme.spacing[2],
    gap: 2,
  },
  taskRow: {
    flexDirection: "row",
    alignItems: "center",
    paddingVertical: 4,
    paddingHorizontal: theme.spacing[2],
    borderRadius: theme.borderRadius.sm,
  },
  taskIcon: {
    marginRight: theme.spacing[2],
    width: 18,
    alignItems: "center",
    justifyContent: "center",
  },
  taskTitle: {
    flex: 1,
    fontSize: 12,
  },
  taskPending: {
    color: theme.colors.foreground,
  },
  taskInProgress: {
    color: "#3b82f6",
    fontWeight: "500",
  },
  taskCompleted: {
    color: theme.colors.foregroundMuted,
    textDecorationLine: "line-through",
  },
  inProgressBadge: {
    paddingHorizontal: 6,
    paddingVertical: 1,
    borderRadius: 4,
    backgroundColor: "rgba(59, 130, 246, 0.15)",
  },
  inProgressText: {
    fontSize: 10,
    color: "#3b82f6",
    fontWeight: "600",
  },
  addTaskRow: {
    paddingHorizontal: theme.spacing[2],
    paddingVertical: 4,
  },
  addTaskInput: {
    fontSize: 12,
    paddingVertical: 4,
    paddingHorizontal: 8,
    borderRadius: 4,
    backgroundColor: theme.colors.surface2,
    color: theme.colors.foreground,
  },
  addTaskButton: {
    flexDirection: "row",
    alignItems: "center",
    gap: 4,
    paddingVertical: 4,
    paddingHorizontal: theme.spacing[2],
    marginTop: 2,
  },
  addTaskButtonText: {
    fontSize: 11,
    color: theme.colors.foregroundMuted,
  },
}));
