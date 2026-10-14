import { Circle } from "lucide-react";

import { reason, startRecording, stopRecording } from "../lib/api";
import type { ToolbarActionProps } from "../toolbar-actions";
import { toast } from "./toaster";
import { IconButton } from "./ui";

const RecordingDot = () => (
  <span className="flex size-4 items-center justify-center" aria-hidden="true">
    <span className="size-2.5 rounded-full bg-red-500" />
  </span>
);

export const RecordButton = ({ session, tab }: ToolbarActionProps) => {
  const recording = tab.recording;
  const toggle = async () => {
    try {
      if (recording === null) toast("success", "Recording", await startRecording(session, tab.tab));
      else toast("success", "Recording saved", await stopRecording(session, tab.tab));
    } catch (error) {
      toast("error", recording === null ? "Could not start recording" : "Could not stop recording", reason(error));
    }
  };
  return (
    <IconButton
      label={recording === null ? "Start recording" : "Stop recording"}
      tooltip={recording === null ? "Record" : `Recording to ${recording}`}
      icon={Circle}
      glyph={recording === null ? undefined : <RecordingDot />}
      aria-pressed={recording !== null}
      disabled={tab.exited}
      onClick={toggle}
    />
  );
};
