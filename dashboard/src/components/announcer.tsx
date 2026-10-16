import { announcements } from "../lib/announcements";
import { useStore } from "../lib/store";

interface AnnouncersProps { title: string }

export const Announcers = ({ title }: AnnouncersProps) => {
  const { text, times } = useStore(announcements);

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
