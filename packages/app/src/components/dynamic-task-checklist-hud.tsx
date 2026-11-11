import React, { memo, useState, useEffect } from "react";
import { View, Text, Pressable, ScrollView, TextInput } from "react-native";
import { StyleSheet, useUnistyles } from "react-native-unistyles";
import {
  CheckSquare,
  Square,
  Clock,
  Plus,
  ChevronDown,
  ChevronUp,
  Sparkles,
  X,
} from "lucide-react-native";

export interface TaskChecklistItem {
  id: string;
  title: string;
  status: "pending" | "in_progress" | "completed";
  durationSeconds?: number;
}

export interface DynamicTaskChecklistHudProps {
  visible: boolean;
  tasks?: TaskChecklistItem[];
  onTasksChange?: (tasks: TaskChecklistItem[]) => void;
  onClose?: () => void;
}

const DEFAULT_TASKS: TaskChecklistItem[] = [
  {
    id: "task-1",
    title: "Analyze repository dependencies & architecture",
    status: "completed",
    durationSeconds: 12,
  },
  {
    id: "task-2",
    title: "Implement core business logic & edge case guards",
    status: "in_progress",
    durationSeconds: 45,
  },
  { id: "task-3", title: "Generate unit tests and verify full suite passes", status: "pending" },
  {
    id: "task-4",
    title: "Format code and verify zero TypeScript diagnostic errors",
    status: "pending",
  },
];

export const DynamicTaskChecklistHud = memo(function DynamicTaskChecklistHud({
  visible,
  tasks: initialTasks = DEFAULT_TASKS,
  onTasksChange,
  onClose,
}: DynamicTaskChecklistHudProps) {
  const { theme } = useUnistyles();
  const [tasks, setTasks] = useState<TaskChecklistItem[]>(initialTasks);
  const [collapsed, setCollapsed] = useState(false);
  const [newTaskTitle, setNewTaskTitle] = useState("");
  const [isAdding, setIsAdding] = useState(false);

  // Live timer tick for active in_progress task
  useEffect(() => {
    const timer = setInterval(() => {
      setTasks((prevTasks) =>
        prevTasks.map((t) =>
          t.status === "in_progress" ? { ...t, durationSeconds: (t.durationSeconds ?? 0) + 1 } : t,
        ),
      );
    }, 1000);
    return () => clearInterval(timer);
  }, []);

  if (!visible) return null;

  const toggleTaskStatus = (id: string) => {
    const updated: TaskChecklistItem[] = tasks.map((t) => {
      if (t.id === id) {
        const nextStatus: "completed" | "in_progress" | "pending" =
          t.status === "completed" ? "pending" : "completed";
        return { ...t, status: nextStatus };
      }
      return t;
    });
    setTasks(updated);
    onTasksChange?.(updated);
  };

  const handleAddTask = () => {
    if (!newTaskTitle.trim()) return;
    const newTask: TaskChecklistItem = {
      id: `task-${Date.now()}`,
      title: newTaskTitle.trim(),
      status: "pending",
    };
    const updated = [...tasks, newTask];
    setTasks(updated);
    onTasksChange?.(updated);
    setNewTaskTitle("");
    setIsAdding(false);
  };

  const completedCount = tasks.filter((t) => t.status === "completed").length;
  const progressPercent = tasks.length > 0 ? Math.round((completedCount / tasks.length) * 100) : 0;

  return (
    <View style={styles.hudCard} data-testid="dynamic-task-checklist-hud">
      {/* Header */}
      <View style={styles.header}>
        <View style={styles.headerTitleRow}>
          <Sparkles size={14} color={theme.colors.accent} />
          <Text style={styles.headerTitle}>Task Plan & Decomposition</Text>
          <Text style={styles.progressBadge}>{progressPercent}% Done</Text>
        </View>
        <View style={styles.headerControls}>
          <Pressable onPress={() => setCollapsed(!collapsed)} style={styles.iconButton}>
            {collapsed ? (
              <ChevronDown size={14} color={theme.colors.foregroundMuted} />
            ) : (
              <ChevronUp size={14} color={theme.colors.foregroundMuted} />
            )}
          </Pressable>
          {onClose ? (
            <Pressable onPress={onClose} style={styles.iconButton}>
              <X size={14} color={theme.colors.foregroundMuted} />
            </Pressable>
          ) : null}
        </View>
      </View>

      {!collapsed ? (
        <>
          {/* Progress bar */}
          <View style={styles.progressBarTrack}>
            <View style={[styles.progressBarFill, { width: `${progressPercent}%` }]} />
          </View>

          {/* Task Items */}
          <ScrollView style={styles.tasksList}>
            {tasks.map((task) => {
              const isDone = task.status === "completed";
              const isInProgress = task.status === "in_progress";

              return (
                <Pressable
                  key={task.id}
                  onPress={() => toggleTaskStatus(task.id)}
                  style={[styles.taskRow, isDone && styles.taskRowDone]}
                >
                  {isDone ? (
                    <CheckSquare size={14} color={theme.colors.accent} />
                  ) : (
                    <Square size={14} color={theme.colors.foregroundMuted} />
                  )}
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
                      <Clock size={10} color={theme.colors.foregroundMuted} />
                      <Text style={styles.timerText}>{task.durationSeconds}s</Text>
                    </View>
                  ) : null}
                </Pressable>
              );
            })}
          </ScrollView>

          {/* Add Task Block */}
          {isAdding ? (
            <View style={styles.addTaskInputRow}>
              <TextInput
                style={styles.addInput}
                value={newTaskTitle}
                onChangeText={setNewTaskTitle}
                placeholder="New subtask title..."
                placeholderTextColor={theme.colors.foregroundMuted}
                autoFocus
                onSubmitEditing={handleAddTask}
              />
              <Pressable onPress={handleAddTask} style={styles.saveAddButton}>
                <Text style={styles.saveAddButtonText}>Add</Text>
              </Pressable>
            </View>
          ) : (
            <Pressable onPress={() => setIsAdding(true)} style={styles.addStepButton}>
              <Plus size={12} color={theme.colors.foregroundMuted} />
              <Text style={styles.addStepText}>Add Subtask</Text>
            </Pressable>
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
  taskRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: 8,
    paddingVertical: 5,
    paddingHorizontal: 4,
    borderRadius: 4,
  },
  taskRowDone: {
    opacity: 0.6,
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
    color: theme.colors.accent,
    fontWeight: "500",
  },
  timerBadge: {
    flexDirection: "row",
    alignItems: "center",
    gap: 3,
    backgroundColor: "rgba(0,0,0,0.2)",
    paddingHorizontal: 4,
    paddingVertical: 1,
    borderRadius: 3,
  },
  timerText: {
    fontSize: 10,
    fontFamily: "monospace",
    color: theme.colors.foregroundMuted,
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
    gap: 6,
    paddingTop: 6,
  },
  addInput: {
    flex: 1,
    backgroundColor: theme.colors.surface0,
    borderRadius: 4,
    borderWidth: 1,
    borderColor: theme.colors.border,
    paddingHorizontal: 8,
    paddingVertical: 4,
    fontSize: 11,
    color: theme.colors.foreground,
  },
  saveAddButton: {
    backgroundColor: theme.colors.accent,
    paddingHorizontal: 10,
    justifyContent: "center",
    borderRadius: 4,
  },
  saveAddButtonText: {
    color: "#ffffff",
    fontSize: 11,
    fontWeight: "600",
  },
}));
