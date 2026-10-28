import { useEffect, useId, useRef, useState, type ReactNode } from "react";

export interface MenuItem {
  label: string;
  icon?: ReactNode;
  hint?: string;
  disabled?: boolean;
  onSelect: () => void;
}

/** A button that opens a small list of actions. Arrow keys move, Esc closes. */
export function Menu({ label, icon, items, className = "" }: { label: string; icon?: ReactNode; items: MenuItem[]; className?: string }) {
  const [open, setOpen] = useState(false);
  const id = useId();
  const wrap = useRef<HTMLDivElement>(null);
  const button = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!open) return;
    const first = wrap.current?.querySelector<HTMLElement>('[role="menuitem"]:not(:disabled)');
    first?.focus();
    const onDown = (e: MouseEvent) => !wrap.current?.contains(e.target as Node) && setOpen(false);
    window.addEventListener("mousedown", onDown);
    return () => window.removeEventListener("mousedown", onDown);
  }, [open]);

  const onKey = (e: React.KeyboardEvent) => {
    const all = [...(wrap.current?.querySelectorAll<HTMLElement>('[role="menuitem"]:not(:disabled)') ?? [])];
    const i = all.indexOf(document.activeElement as HTMLElement);
    if (e.key === "ArrowDown") all[(i + 1) % all.length]?.focus();
    else if (e.key === "ArrowUp") all[(i - 1 + all.length) % all.length]?.focus();
    else if (e.key === "Escape" || e.key === "Tab") {
      setOpen(false);
      if (e.key === "Escape") button.current?.focus();
    } else return;
    e.preventDefault();
  };

  return (
    <div ref={wrap} className="relative" onKeyDown={open ? onKey : undefined}>
      <button
        ref={button}
        type="button"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? id : undefined}
        onClick={() => setOpen(!open)}
        onKeyDown={(e) => e.key === "ArrowDown" && (e.preventDefault(), setOpen(true))}
        className={`inline-flex h-8 items-center gap-1.5 whitespace-nowrap rounded-md border border-rule px-3 text-sm font-semibold text-fg transition-colors hover:bg-raised aria-expanded:bg-raised ${className}`}
      >
        {icon}
        {label}
        <svg width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.6" aria-hidden className="text-muted">
          <path d="m4 6 4 4 4-4" />
        </svg>
      </button>
      {open && (
        <div
          id={id}
          role="menu"
          aria-label={label}
          className="absolute right-0 z-30 mt-1 min-w-48 rounded-lg border border-rule bg-panel p-1 shadow-lg shadow-[var(--shadow)]"
        >
          {items.map((it) => (
            <button
              key={it.label}
              type="button"
              role="menuitem"
              disabled={it.disabled}
              onClick={() => {
                setOpen(false);
                it.onSelect();
              }}
              className="flex w-full items-center gap-2.5 rounded-md px-2.5 py-1.5 text-left text-sm text-fg hover:bg-raised focus:bg-raised focus:outline-none disabled:opacity-40"
            >
              <span className="text-muted">{it.icon}</span>
              <span className="flex-1">{it.label}</span>
              {it.hint && <span className="text-xs text-faint">{it.hint}</span>}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
