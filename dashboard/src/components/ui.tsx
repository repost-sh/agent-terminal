import { Copy, type LucideIcon } from "lucide-react";
import type { AnchorHTMLAttributes, ButtonHTMLAttributes, ReactNode, Ref } from "react";

import { reason } from "../lib/api";
import { settle } from "../lib/settle";
import { toast } from "../lib/toasts";

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

interface ButtonLook {
  variant?: keyof typeof BUTTON_VARIANT;
  size?: keyof typeof BUTTON_SIZE;
  icon?: LucideIcon;
}

type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & { ref?: Ref<HTMLButtonElement> | undefined; look?: ButtonLook };

export const Button = ({ look = {}, ...button }: ButtonProps) => {
  const Icon = look.icon;
  const looks = cn(BUTTON_BASE, BUTTON_VARIANT[look.variant ?? "default"], BUTTON_SIZE[look.size ?? "sm"], button.className);

  return (
    <button type="button" {...button} className={looks}>
      {Icon && <Icon aria-hidden="true" />}
      {button.children}
    </button>
  );
};

const ICON_BUTTON = {
  head: "flex size-6 items-center justify-center rounded text-muted-foreground outline-none hover:bg-muted hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/50 disabled:pointer-events-none disabled:opacity-40 [&_svg]:size-[13px]",
  tool: "inline-flex min-h-6 min-w-6 shrink-0 items-center justify-center rounded p-1 text-muted-foreground transition-colors outline-none hover:bg-muted hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/50 disabled:pointer-events-none disabled:opacity-40 aria-pressed:text-foreground [&_svg]:size-4",
};

interface IconLook {
  label: string;
  icon: LucideIcon;
  tooltip?: string;
  kind?: keyof typeof ICON_BUTTON;
}

type IconButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & { ref?: Ref<HTMLButtonElement>; look: IconLook };

export const IconButton = ({ look, ...button }: IconButtonProps) => (
  <button type="button" data-tooltip={look.tooltip ?? look.label} {...button} className={cn(ICON_BUTTON[look.kind ?? "tool"], button.className)}>
    <look.icon aria-hidden="true" />
    <span className="sr-only">{look.label}</span>
  </button>
);

type IconLinkProps = AnchorHTMLAttributes<HTMLAnchorElement> & { href: string; look: IconLook };

export const IconLink = ({ look, ...link }: IconLinkProps) => (
  <a data-tooltip={look.tooltip ?? look.label} {...link} className={cn(ICON_BUTTON.tool, link.className)}>
    <look.icon aria-hidden="true" />
    <span className="sr-only">{look.label}</span>
  </a>
);

const BADGE = {
  secondary: "border-transparent bg-secondary text-secondary-foreground",
  outline: "border-border text-foreground",
};

interface BadgeProps { variant?: keyof typeof BADGE; className?: string; children: ReactNode }

export const Badge = (props: BadgeProps) => (
  <span
    data-slot="badge"
    className={cn(
      "inline-flex h-4 w-fit shrink-0 items-center justify-center gap-1 overflow-hidden rounded-4xl border px-1.5 py-0.5 text-[10px] font-medium whitespace-nowrap tabular-nums",
      BADGE[props.variant ?? "secondary"],
      props.className,
    )}
  >
    {props.children}
  </span>
);

export type Tone = "muted" | "destructive" | "warning" | "success";

const CHIP: Record<Exclude<Tone, "success">, string> = {
  muted: "border-border text-muted-foreground",
  destructive: "border-destructive/40 text-destructive",
  warning: "border-warning/50 text-yellow-700 dark:text-warning",
};

interface ChipProps { tone: Exclude<Tone, "success">; children: ReactNode }

export const Chip = ({ tone, children }: ChipProps) => (
  <span className={cn("shrink-0 rounded border px-1 py-px text-[9px] leading-none whitespace-nowrap", CHIP[tone])}>{children}</span>
);

const DOT: Record<Tone, string> = {
  success: "bg-success",
  warning: "bg-warning",
  muted: "bg-muted-foreground/60",
  destructive: "bg-destructive",
};

interface DotProps { tone: Tone; className?: string }

export const Dot = ({ tone, className }: DotProps) => (
  <span aria-hidden="true" className={cn("inline-block size-2 shrink-0 rounded-full", DOT[tone], className)} />
);

export const copy = async (value: string) => {
  const copied = await settle(async () => {
    await navigator.clipboard.writeText(value);
    toast("success", { title: "Copied", description: value });
  });

  if (!copied.ok) {
    toast("error", { title: "Could not copy", description: reason(copied.error) });
  }
};

interface CliBlockProps { line: string }

export const CliBlock = ({ line }: CliBlockProps) => (
  <div className="mx-auto mt-2 flex max-w-full items-center gap-2 rounded-md bg-muted px-2.5 py-1 text-left">
    <code className="min-w-0 flex-1 font-mono text-[11px] break-all text-foreground">{line}</code>
    <IconButton look={{ label: `Copy ${line}`, tooltip: "Copy", icon: Copy }} className="size-6 p-0 [&_svg]:size-[13px]" onClick={() => {
      void copy(line);
    }} />
  </div>
);
