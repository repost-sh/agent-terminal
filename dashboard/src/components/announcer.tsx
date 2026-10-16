import { useSyncExternalStore } from "react";

import { lastAnnouncement, subscribeToAnnouncements } from "../lib/announcements";

interface AnnouncersProps { title: string }

export const Announcers = ({ title }: AnnouncersProps) => {
  const { text, times } = useSyncExternalStore(subscribeToAnnouncements, lastAnnouncement);

  return (
    <>
      <div className="sr-only" aria-live="assertive" aria-atomic="true">
        {title}
      </div>
      <div className="sr-only" aria-live="polite" aria-atomic="true">
        <span key={times}>{text}</span>
      </div>
    </>
  );
};
