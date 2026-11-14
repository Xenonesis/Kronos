import React, { memo, useState, useCallback } from "react";
import { View, Text, type StyleProp, type ViewStyle } from "react-native";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import { StickyNote } from "lucide-react-native";
import { PressHighlight } from "@/components/ui/press-highlight";
import { useNotesStore } from "./use-notes";
import { NotePickerModal } from "./NotePickerModal";
import type { NoteRecord } from "./types";

const ThemedStickyNote = withUnistyles(StickyNote, (theme) => ({
  color: theme.colors.foregroundMuted,
}));

export interface NoteActionChipProps {
  onInsertText?: (text: string) => void;
  style?: StyleProp<ViewStyle> | ((state: { hovered?: boolean; pressed?: boolean }) => StyleProp<ViewStyle>);
  testID?: string;
  size?: number;
}

export const NoteActionChip = memo(function NoteActionChip({
  onInsertText,
  style,
  testID = "composer-note-chip",
  size = 18,
}: NoteActionChipProps) {
  const [pickerOpen, setPickerOpen] = useState(false);
  const { formatNoteAsContext, openDrawer } = useNotesStore();

  const handleOpenPicker = useCallback(() => {
    setPickerOpen(true);
  }, []);

  const handleClosePicker = useCallback(() => {
    setPickerOpen(false);
  }, []);

  const handleSelectNote = useCallback(
    (note: NoteRecord) => {
      const contextText = formatNoteAsContext(note);
      if (onInsertText) {
        onInsertText(contextText);
      } else if (typeof window !== "undefined") {
        window.dispatchEvent(
          new CustomEvent("kronos:insert-composer", {
            detail: { text: contextText },
          }),
        );
      }
    },
    [formatNoteAsContext, onInsertText],
  );

  const handleOpenFullDrawer = useCallback(() => {
    setPickerOpen(false);
    openDrawer();
  }, [openDrawer]);

  const resolveStyle = useCallback(
    (state: { hovered?: boolean; pressed?: boolean }) => {
      const userStyle = typeof style === "function" ? style(state) : style;
      return [styles.chip, userStyle];
    },
    [style],
  );

  return (
    <>
      <PressHighlight
        style={resolveStyle}
        onPress={handleOpenPicker}
        accessibilityRole="button"
        accessibilityLabel="Attach Note (@note)"
        testID={testID}
      >
        <ThemedStickyNote size={size} />
      </PressHighlight>
      <NotePickerModal
        visible={pickerOpen}
        onClose={handleClosePicker}
        onSelectNote={handleSelectNote}
        onOpenFullDrawer={handleOpenFullDrawer}
      />
    </>
  );
});

const styles = StyleSheet.create((theme) => ({
  chip: {
    padding: theme.spacing[2],
    borderRadius: theme.borderRadius.sm,
    justifyContent: "center",
    alignItems: "center",
  },
}));
