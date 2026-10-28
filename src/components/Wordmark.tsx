/**
 * The VARden wordmark: {VAR}den, with the three status dots under "VAR".
 * The braces say "variable"; the dots are the app's statuses.
 */
export function Wordmark({ className = "" }: { className?: string }) {
  return (
    <span className={`inline-flex items-baseline font-semibold tracking-tight ${className}`} aria-label="VARden">
      <span aria-hidden className="font-normal text-faint">
        {"{"}
      </span>
      <span aria-hidden className="relative">
        VAR
        <span className="absolute inset-x-0 -bottom-[0.32em] flex justify-center gap-[0.14em]">
          <span className="h-[0.2em] w-[0.2em] rounded-full bg-synced" />
          <span className="h-[0.2em] w-[0.2em] rounded-full bg-persistent" />
          <span className="h-[0.2em] w-[0.2em] rounded-full bg-process" />
        </span>
      </span>
      <span aria-hidden className="font-normal text-faint">
        {"}"}
      </span>
      <span aria-hidden>den</span>
    </span>
  );
}
