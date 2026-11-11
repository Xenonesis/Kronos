import { memo, useState, useCallback, useEffect, useRef } from "react";
import { Pressable, View } from "react-native";
import { StyleSheet, useUnistyles } from "react-native-unistyles";
import { Mic, MicOff } from "lucide-react-native";
import { playTaskSuccessSound } from "@/audio/audio-suite";
import { isWeb } from "@/constants/platform";

export interface ComposerDictationButtonProps {
  onTranscript: (text: string) => void;
  disabled?: boolean;
}

export const ComposerDictationButton = memo(function ComposerDictationButton({
  onTranscript,
  disabled = false,
}: ComposerDictationButtonProps) {
  const { theme } = useUnistyles();
  const [isListening, setIsListening] = useState(false);
  const [isSupported, setIsSupported] = useState(false);
  const recognitionRef = useRef<any>(null);

  useEffect(() => {
    if (!isWeb || typeof window === "undefined") {
      setIsSupported(false);
      return;
    }
    const SpeechRecognition =
      (window as any).SpeechRecognition || (window as any).webkitSpeechRecognition;
    setIsSupported(Boolean(SpeechRecognition));
  }, []);

  const stopListening = useCallback(() => {
    if (recognitionRef.current) {
      try {
        recognitionRef.current.stop();
      } catch {
        // Ignore stop error
      }
      recognitionRef.current = null;
    }
    setIsListening(false);
  }, []);

  const startListening = useCallback(() => {
    if (!isWeb || typeof window === "undefined") return;
    const SpeechRecognition =
      (window as any).SpeechRecognition || (window as any).webkitSpeechRecognition;
    if (!SpeechRecognition) return;

    try {
      const recognition = new SpeechRecognition();
      recognition.continuous = true;
      recognition.interimResults = true;
      recognition.lang = "en-US";

      recognition.onstart = () => {
        setIsListening(true);
        playTaskSuccessSound();
      };

      recognition.onresult = (event: any) => {
        let finalTranscript = "";
        for (let i = event.resultIndex; i < event.results.length; ++i) {
          if (event.results[i].isFinal) {
            finalTranscript += event.results[i][0].transcript;
          }
        }
        if (finalTranscript.trim()) {
          onTranscript(finalTranscript.trim());
        }
      };

      recognition.onerror = () => {
        stopListening();
      };

      recognition.onend = () => {
        setIsListening(false);
      };

      recognition.start();
      recognitionRef.current = recognition;
    } catch {
      setIsListening(false);
    }
  }, [onTranscript, stopListening]);

  const toggleListening = useCallback(() => {
    if (isListening) {
      stopListening();
    } else {
      startListening();
    }
  }, [isListening, startListening, stopListening]);

  if (!isSupported) {
    return null;
  }

  return (
    <Pressable
      onPress={toggleListening}
      disabled={disabled}
      style={[
        styles.button,
        isListening && styles.buttonListening,
        disabled && styles.buttonDisabled,
      ]}
      accessibilityLabel={isListening ? "Stop voice dictation" : "Start voice dictation"}
    >
      {isListening ? (
        <View style={styles.listeningContent}>
          <MicOff size={14} color="#dc2626" />
          <View style={styles.pulseDot} />
        </View>
      ) : (
        <Mic size={14} color={theme.colors.foregroundMuted} />
      )}
    </Pressable>
  );
});

const styles = StyleSheet.create((theme) => ({
  button: {
    width: 28,
    height: 28,
    borderRadius: 6,
    alignItems: "center",
    justifyContent: "center",
    backgroundColor: theme.colors.surface2,
    borderWidth: 1,
    borderColor: theme.colors.border,
  },
  buttonListening: {
    backgroundColor: "rgba(220, 38, 38, 0.12)",
    borderColor: "#dc2626",
  },
  buttonDisabled: {
    opacity: 0.5,
  },
  listeningContent: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "center",
    position: "relative",
  },
  pulseDot: {
    position: "absolute",
    top: -2,
    right: -2,
    width: 6,
    height: 6,
    borderRadius: 3,
    backgroundColor: "#dc2626",
  },
}));
