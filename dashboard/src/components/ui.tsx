import { Copy, type LucideIcon } from "lucide-react";
import type { AnchorHTMLAttributes, ButtonHTMLAttributes, ReactNode, Ref } from "react";

import { toast } from "./toaster";

export const cn = (...parts: (string | false | null | undefined)[]) => parts.filter(Boolean).join(" ");

const BUTTON_BASE =
  "group/button inline-flex shrink-0 items-center justify-center border border-transparent bg-clip-padding font-medium whitespace-nowrap transition-all outline-none select-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 active:not-aria-[haspopup]:translate-y-px disabled:pointer-events-none disabled:opacity-50 [&_svg]:pointer-events-none [&_svg]:shrink-0";

const BUTTON_VARIANT = {
  default: "bg-primary text-primary-foreground hover:bg-primary/80",
  ghost: "hover:bg-muted hover:text-foreground aria-expanded:bg-muted aria-expanded:text-foreground dark:hover:bg-muted/50",
  outline: "border-border bg-background hover:bg-muted hover:text-foreground dark:bg-input/30 dark:border-input dark:hover:bg-input/50",
  destructive:
    "bg-destructive/10 text-destructive hover:bg-destructive/20 focus-visible:border-destructive/40 focus-visible:ring-destructive/20 dark:bg-destructive/20 dark:hover:bg-destructive/30 dark:focus-visible:ring-destructive/40",
};

const BUTTON_SIZE = {
  sm: "h-7 gap-1 rounded-[min(var(--radius-md),12px)] px-2.5 text-[0.8rem] [&_svg]:size-3.5",
  "icon-sm": "size-7 rounded-[min(var(--radius-md),12px)] text-sm [&_svg]:size-4",
};

type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  ref?: Ref<HTMLButtonElement>;
  variant?: keyof typeof BUTTON_VARIANT;
  size?: keyof typeof BUTTON_SIZE;
  icon?: LucideIcon;
};

export const Button = ({ variant = "default", size = "sm", icon: Icon, className, children, ...rest }: ButtonProps) => (
  <button type="button" className={cn(BUTTON_BASE, BUTTON_VARIANT[variant], BUTTON_SIZE[size], className)} {...rest}>
    {Icon && <Icon aria-hidden="true" />}
    {children}
  </button>
);

const ICON_BUTTON = {
  head: "flex size-6 items-center justify-center rounded text-muted-foreground outline-none hover:bg-muted hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/50 disabled:pointer-events-none disabled:opacity-40 [&_svg]:size-[13px]",
  tool: "inline-flex min-h-6 min-w-6 shrink-0 items-center justify-center rounded p-1 text-muted-foreground transition-colors outline-none hover:bg-muted hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/50 disabled:pointer-events-none disabled:opacity-40 aria-pressed:text-foreground [&_svg]:size-4",
};

type IconButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  ref?: Ref<HTMLButtonElement>;
  label: string;
  icon: LucideIcon;
  glyph?: ReactNode;
  tooltip?: string;
  kind?: keyof typeof ICON_BUTTON;
};

export const IconButton = ({ label, icon: Icon, glyph, tooltip, kind = "tool", className, ...rest }: IconButtonProps) => (
  <button type="button" data-tooltip={tooltip ?? label} className={cn(ICON_BUTTON[kind], className)} {...rest}>
    {glyph ?? <Icon aria-hidden="true" />}
    <span className="sr-only">{label}</span>
  </button>
);

type IconLinkProps = AnchorHTMLAttributes<HTMLAnchorElement> & {
  href: string;
  label: string;
  icon: LucideIcon;
  tooltip?: string;
};

export const IconLink = ({ label, icon: Icon, tooltip, className, ...rest }: IconLinkProps) => (
  <a data-tooltip={tooltip ?? label} className={cn(ICON_BUTTON.tool, className)} {...rest}>
    <Icon aria-hidden="true" />
    <span className="sr-only">{label}</span>
  </a>
);

const BADGE = {
  secondary: "border-transparent bg-secondary text-secondary-foreground",
  outline: "border-border text-foreground",
};

export const Badge = ({ variant = "secondary", className, children }: { variant?: keyof typeof BADGE; className?: string; children: ReactNode }) => (
  <span
    data-slot="badge"
    className={cn(
      "inline-flex h-4 w-fit shrink-0 items-center justify-center gap-1 overflow-hidden rounded-4xl border px-1.5 py-0.5 text-[10px] font-medium whitespace-nowrap tabular-nums",
      BADGE[variant],
      className,
    )}
  >
    {children}
  </span>
);

export type Tone = "muted" | "destructive" | "warning" | "success";

const CHIP: Record<Exclude<Tone, "success">, string> = {
  muted: "border-border text-muted-foreground",
  destructive: "border-destructive/40 text-destructive",
  warning: "border-warning/50 text-yellow-700 dark:text-warning",
};

export const Chip = ({ tone, children }: { tone: Exclude<Tone, "success">; children: ReactNode }) => (
  <span className={cn("shrink-0 rounded border px-1 py-px text-[9px] leading-none whitespace-nowrap", CHIP[tone])}>{children}</span>
);

const DOT: Record<Tone, string> = {
  success: "bg-success",
  warning: "bg-warning",
  muted: "bg-muted-foreground/60",
  destructive: "bg-destructive",
};

export const Dot = ({ tone, className }: { tone: Tone; className?: string }) => (
  <span aria-hidden="true" className={cn("inline-block size-2 shrink-0 rounded-full", DOT[tone], className)} />
);

export const copy = async (value: string) => {
  try {
    await navigator.clipboard.writeText(value);
    toast("success", "Copied", value);
  } catch (error) {
    toast("error", "Could not copy", error instanceof Error ? error.message : String(error));
  }
};

export const CliBlock = ({ line }: { line: string }) => (
  <div className="mx-auto mt-2 flex max-w-full items-center gap-2 rounded-md bg-muted px-2.5 py-1 text-left">
    <code className="min-w-0 flex-1 font-mono text-[11px] break-all text-foreground">{line}</code>
    <IconButton label={`Copy ${line}`} tooltip="Copy" icon={Copy} className="size-6 p-0 [&_svg]:size-[13px]" onClick={() => copy(line)} />
  </div>
);
