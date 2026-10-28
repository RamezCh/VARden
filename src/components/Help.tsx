import { useEffect, useId, useRef, useState, type ReactNode } from "react";

interface Props {
  /** What the help explains, read by screen readers on the button. */
  label: string;
  children: ReactNode;
}

const WIDTH = 288;

/**
 * A small "?" that explains a term. Opens on hover or keyboard focus; a
 * click pins it open. Positioned with `fixed` so overflow-hidden parents
 * (the table, dialogs) never clip it.
 */
export function Help({ label, children }: Props) {
  const id = useId();
  const button = useRef<HTMLButtonElement>(null);
  const [pinned, setPinned] = useState(false);
  const [hover, setHover] = useState(false);
  const [pos, setPos] = useState<{ top: number; left: number } | null>(null);
  const open = pinned || hover;

  useEffect(() => {
    if (!open || !button.current) return;
    const r = button.current.getBoundingClientRect();
    const left = Math.min(Math.max(8, r.left - 12), window.innerWidth - WIDTH - 8);
    const below = r.bottom + 8;
    // Flip above when there isn't room underneath.
    setPos({ top: below + 160 > window.innerHeight ? Math.max(8, r.top - 8 - 160) : below, left });
  }, [open]);

  useEffect(() => {
    if (!pinned) return;
    const close = (e: MouseEvent | KeyboardEvent) => {
      if (e instanceof KeyboardEvent ? e.key === "Escape" : !button.current?.contains(e.target as Node)) {
        setPinned(false);
        setHover(false);
      }
    };
    window.addEventListener("mousedown", close);
    window.addEventListener("keydown", close);
    return () => {
      window.removeEventListener("mousedown", close);
      window.removeEventListener("keydown", close);
    };
  }, [pinned]);

  return (
    <>
      <button
        ref={button}
        type="button"
        aria-label={label}
        aria-expanded={open}
        aria-describedby={open ? id : undefined}
        onClick={(e) => {
          e.stopPropagation();
          setPinned((p) => !p);
        }}
        onMouseEnter={() => setHover(true)}
        onMouseLeave={() => setHover(false)}
        // Only keyboard focus opens it (a dialog focusing it on open shouldn't).
        onFocus={(e) => e.currentTarget.matches(":focus-visible") && setHover(true)}
        onBlur={() => setHover(false)}
        className="inline-grid h-4 w-4 shrink-0 cursor-help place-items-center rounded-full border border-faint align-middle font-sans text-[0.65rem] font-semibold normal-case leading-none tracking-normal text-muted transition-colors hover:border-process hover:text-process aria-expanded:border-process aria-expanded:text-process"
      >
        ?
      </button>
      {open && pos && (
        <div
          id={id}
          role="tooltip"
          style={{ top: pos.top, left: pos.left, width: WIDTH }}
          className="fixed z-[60] rounded-lg border border-rule bg-raised px-3.5 py-3 text-left text-sm font-normal normal-case leading-relaxed tracking-normal text-fg shadow-xl shadow-black/40 [&_p+p]:mt-2"
        >
          {children}
        </div>
      )}
    </>
  );
}
