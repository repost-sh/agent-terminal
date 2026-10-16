import { cn } from "./ui";

export const LABEL = "text-xs font-medium text-muted-foreground";

export const INPUT =
  "w-full rounded-md border border-border bg-transparent px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-ring placeholder:text-muted-foreground/60 aria-invalid:border-destructive aria-invalid:focus:ring-destructive/40";

interface FieldErrorProps { id: string; text: string | undefined }

const FieldError = ({ id, text }: FieldErrorProps) =>
  text ? (
    <p id={id} role="alert" className="rounded-md bg-destructive/10 px-3 py-2 text-xs text-destructive">
      {text}
    </p>
  ) : null;

interface TextFieldProps {
  id: string;
  label: string;
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  error?: string | undefined;
  autoFocus?: boolean;
}

export const TextField = (props: TextFieldProps) => (
  <div className="grid gap-1.5">
    <label htmlFor={props.id} className={LABEL}>
      {props.label}
    </label>
    <input
      id={props.id}
      type="text"
      className={cn(INPUT, "font-mono text-[13px]")}
      value={props.value}
      placeholder={props.placeholder}
      spellCheck={false}
      autoComplete="off"
      aria-invalid={props.error ? true : undefined}
      aria-describedby={props.error ? `${props.id}-error` : undefined}
      data-autofocus={(props.autoFocus ?? false) || undefined}
      onChange={(event) => {
        props.onChange(event.target.value);
      }}
    />
    <FieldError id={`${props.id}-error`} text={props.error} />
  </div>
);

interface SegmentedProps {
  id: string;
  legend: string;
  value: string;
  options: [string, string][];
  onChange: (value: string) => void;
  error?: string | undefined;
}

export const Segmented = (props: SegmentedProps) => (
  <fieldset className="grid gap-1.5" aria-describedby={props.error ? `${props.id}-error` : undefined}>
    <legend className={cn(LABEL, "mb-1.5")}>{props.legend}</legend>
    <div className="grid gap-2" style={{ gridTemplateColumns: `repeat(${String(props.options.length)}, minmax(0, 1fr))` }}>
      {props.options.map(([option, label]) => (
        <label
          key={option}
          className="flex min-h-8 cursor-pointer items-center justify-center rounded-md border border-border px-2 py-1.5 text-center text-sm text-muted-foreground transition-colors hover:text-foreground has-checked:border-ring has-checked:bg-muted has-checked:text-foreground has-focus-visible:ring-2 has-focus-visible:ring-ring/50"
        >
          <input
            type="radio"
            className="sr-only"
            id={`${props.id}-${option}`}
            name={props.id}
            value={option}
            checked={props.value === option}
            onChange={() => {
              props.onChange(option);
            }}
          />
          {label}
        </label>
      ))}
    </div>
    <FieldError id={`${props.id}-error`} text={props.error} />
  </fieldset>
);
