import React, { memo, useCallback, useState } from "react";
import { View, Text, ScrollView } from "react-native";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import { CheckSquare, Square, Plus, ChevronUp, ChevronDown, Clock, X } from "lucide-react-native";
import { AdaptiveTextInput } from "@/components/adaptive-text-input";
import { PressHighlight } from "@/components/ui/press-highlight";
import { ToolbarButton } from "@/components/ui/pane-content-toolbar";
import { Button } from "@/components/ui/button";

const ThemedCheckIcon = withUnistyles(CheckSquare, (theme) => ({ color: theme.colors.accent }));
const ThemedSquareIcon = withUnistyles(Square, (theme) => ({
  color: theme.colors.foregroundMuted,
}));
const ThemedPlusIcon = withUnistyles(Plus, (theme) => ({ color: theme.colors.foregroundMuted }));
const ThemedChevronUpIcon = withUnistyles(ChevronUp, (theme) => ({
  color: theme.colors.foregroundMuted,
}));
const ThemedChevronDownIcon = withUnistyles(ChevronDown, (theme) => ({
  color: theme.colors.foregroundMuted,
}));
const ThemedClockIcon = withUnistyles(Clock, (theme) => ({ color: theme.colors.foregroundMuted }));
const ThemedCloseIcon = withUnistyles(X, (theme) => ({ color: theme.colors.foregroundMuted }));

export interface TaskChecklistItem {
  id: string;
  title: string;
  status: "pending" | "in_progress" | "completed";
  durationSeconds?: number;
}

export interface DynamicTaskChecklistHudProps {
  title?: string;
  tasks?: TaskChecklistItem[];
  onToggleTask?: (taskId: string) => void;
  onAddTask?: (title: string) => void;
  onClose?: () => void;
}

const ChecklistRow = memo(function ChecklistRow({
  task,
  onToggle,
}: {
  task: TaskChecklistItem;
  onToggle: (id: string) => void;
}) {
  const isDone = task.status === "completed";
  const isInProgress = task.status === "in_progress";
  const handlePress = useCallback(() => {
    onToggle(task.id);
  }, [onToggle, task.id]);

  return (
    <PressHighlight onPress={handlePress} style={[styles.taskRow, isDone && styles.taskRowDone]}>
      {isDone ? <ThemedCheckIcon size={14} /> : <ThemedSquareIcon size={14} />}
      <Text
        style={[
          styles.taskText,
          isDone && styles.taskTextDone,
          isInProgress && styles.taskTextInProgress,
        ]}
        numberOfLines={2}
      >
        {task.title}
      </Text>
      {task.durationSeconds ? (
        <View style={styles.timerBadge}>
          <ThemedClockIcon size={10} />
          <Text style={styles.timerText}>{task.durationSeconds}s</Text>
        </View>
      ) : null}
    </PressHighlight>
  );
});

function toggleTaskItemStatus(prev: TaskChecklistItem[], id: string): TaskChecklistItem[] {
  return prev.map((t) =>
    t.id === id ? { ...t, status: t.status === "completed" ? "pending" : "completed" } : t,
  );
}

export const DynamicTaskChecklistHud = memo(function DynamicTaskChecklistHud({
  title = "Agent Execution Checklist",
  tasks: externalTasks,
  onToggleTask,
  onAddTask,
  onClose,
}: DynamicTaskChecklistHudProps) {
  const [internalTasks, setInternalTasks] = useState<TaskChecklistItem[]>([]);
  const [collapsed, setCollapsed] = useState(false);
  const [isAdding, setIsAdding] = useState(false);
  const [newTaskTitle, setNewTaskTitle] = useState("");
  const [inputResetKey, setInputResetKey] = useState(0);

  const tasks = externalTasks ?? internalTasks;
  const completedCount = tasks.filter((t) => t.status === "completed").length;
  const progressPercent =
    tasks.length > 0 ? Math.round((completedCount / tasks.length) * 100) : 0;

  const toggleCollapsed = useCallback(() => setCollapsed((prev) => !prev), []);
  const startAdding = useCallback(() => setIsAdding(true), []);

  const toggleTaskStatus = useCallback(
    (id: string) => {
      if (onToggleTask) {
        onToggleTask(id);
        return;
      }
      setInternalTasks((prev) => toggleTaskItemStatus(prev, id));
    },
    [onToggleTask],
  );

  const handleAddTask = useCallback(() => {
    if (!newTaskTitle.trim()) return;
    if (onAddTask) {
      onAddTask(newTaskTitle.trim());
    } else {
      setInternalTasks((prev) => [
        ...prev,
        {
          id: `t-${Date.now()}`,
          title: newTaskTitle.trim(),
          status: "pending",
        },
      ]);
    }
    setNewTaskTitle("");
    setInputResetKey((k) => k + 1);
    setIsAdding(false);
  }, [newTaskTitle, onAddTask]);

  return (
    <View style={styles.hudCard} data-testid="dynamic-task-checklist-hud">
      <View style={styles.header}>
        <View style={styles.headerTitleRow}>
          <ThemedCheckIcon size={16} />
          <Text style={styles.headerTitle}>{title}</Text>
          <Text style={styles.progressBadge}>
            {completedCount}/{tasks.length} ({progressPercent}%)
          </Text>
        </View>
        <View style={styles.headerControls}>
          <ToolbarButton label="Toggle checklist" onPress={toggleCollapsed} style={styles.iconButton}>
            {collapsed ? <ThemedChevronDownIcon size={14} /> : <ThemedChevronUpIcon size={14} />}
          </ToolbarButton>
          {onClose ? (
            <ToolbarButton label="Close checklist" onPress={onClose} style={styles.iconButton}>
              <ThemedCloseIcon size={14} />
            </ToolbarButton>
          ) : null}
        </View>
      </View>

      {!collapsed ? (
        <>
          <View style={styles.progressBarTrack}>
            <View style={[styles.progressBarFill, { width: `${progressPercent}%` }]} />
          </View>

          <ScrollView style={styles.tasksList}>
            {tasks.length === 0 ? (
              <Text style={styles.emptyText}>No active tasks recorded.</Text>
            ) : (
              tasks.map((task) => (
                <ChecklistRow key={task.id} task={task} onToggle={toggleTaskStatus} />
              ))
            )}
          </ScrollView>

          {isAdding ? (
            <View style={styles.addTaskInputRow}>
              <AdaptiveTextInput
                style={styles.addInput}
                initialValue={newTaskTitle}
                resetKey={inputResetKey}
                onChangeText={setNewTaskTitle}
                placeholder="New subtask title..."
                onSubmitEditing={handleAddTask}
              />
              <Button size="sm" variant="default" onPress={handleAddTask} style={styles.saveAddButton}>
                <Text style={styles.saveAddButtonText}>Add</Text>
              </Button>
            </View>
          ) : (
            <PressHighlight onPress={startAdding} style={styles.addStepButton}>
              <ThemedPlusIcon size={12} />
              <Text style={styles.addStepText}>Add Subtask</Text>
            </PressHighlight>
          )}
        </>
      ) : null}
    </View>
  );
});

const styles = StyleSheet.create((theme) => ({
  hudCard: {
    backgroundColor: theme.colors.surface1,
    borderRadius: 8,
    borderWidth: 1,
    borderColor: theme.colors.border,
    padding: 10,
    marginVertical: 6,
    boxShadow: "0 2px 8px rgba(0,0,0,0.2)",
  },
  header: {
    flexDirection: "row",
    justifyContent: "space-between",
    alignItems: "center",
  },
  headerTitleRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: 6,
  },
  headerTitle: {
    fontSize: 12,
    fontWeight: "600",
    color: theme.colors.foreground,
  },
  progressBadge: {
    fontSize: 10,
    fontWeight: "600",
    color: theme.colors.accent,
    backgroundColor: "rgba(99, 102, 241, 0.12)",
    paddingHorizontal: 5,
    paddingVertical: 1,
    borderRadius: 4,
  },
  headerControls: {
    flexDirection: "row",
    alignItems: "center",
    gap: 4,
  },
  iconButton: {
    padding: 2,
  },
  progressBarTrack: {
    height: 3,
    backgroundColor: theme.colors.border,
    borderRadius: 2,
    marginVertical: 8,
    overflow: "hidden",
  },
  progressBarFill: {
    height: "100%",
    backgroundColor: theme.colors.accent,
  },
  tasksList: {
    maxHeight: 180,
  },
  emptyText: {
    fontSize: 11,
    color: theme.colors.foregroundMuted,
    paddingVertical: 4,
  },
  taskRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: 8,
    paddingVertical: 4,
  },
  taskRowDone: {
    opacity: 0.65,
  },
  taskText: {
    flex: 1,
    fontSize: 12,
    color: theme.colors.foreground,
  },
  taskTextDone: {
    textDecorationLine: "line-through",
    color: theme.colors.foregroundMuted,
  },
  taskTextInProgress: {
    fontWeight: "600",
    color: theme.colors.accent,
  },
  timerBadge: {
    flexDirection: "row",
    alignItems: "center",
    gap: 3,
  },
  timerText: {
    fontSize: 10,
    color: theme.colors.foregroundMuted,
    fontFamily: "monospace",
  },
  addStepButton: {
    flexDirection: "row",
    alignItems: "center",
    gap: 4,
    paddingTop: 6,
  },
  addStepText: {
    fontSize: 11,
    color: theme.colors.foregroundMuted,
  },
  addTaskInputRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: 6,
    marginTop: 6,
  },
  addInput: {
    flex: 1,
    fontSize: 12,
    color: theme.colors.foreground,
    backgroundColor: theme.colors.surface0,
    borderWidth: 1,
    borderColor: theme.colors.border,
    borderRadius: 4,
    paddingHorizontal: 8,
    paddingVertical: 4,
  },
  saveAddButton: {
    backgroundColor: theme.colors.accent,
    paddingHorizontal: 10,
    paddingVertical: 5,
    borderRadius: 4,
  },
  saveAddButtonText: {
    color: "#ffffff",
    fontSize: 11,
    fontWeight: "600",
  },
}));
