import { announcements } from "../lib/announcements";
import { useStore } from "../lib/store";

interface AnnouncersProps { title: string }

export const Announcers = ({ title }: AnnouncersProps) => {
  const { text, times } = useStore(announcements);

  return (
    <>
      <div className="sr-only" role="alert" aria-label="Screen" aria-live="assertive" aria-atomic="true">
        {title}
      </div>
      <div className="sr-only" role="status" aria-label="Announcements" aria-live="polite" aria-atomic="true">
        <span key={times}>{text}</span>
      </div>
    </>
  );
};
