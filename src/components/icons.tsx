// Small stroke icons (16px grid), drawn to match Atkinson's open shapes.
import type { SVGProps } from "react";

type P = SVGProps<SVGSVGElement>;
const base = (props: P) => ({
  width: 16,
  height: 16,
  viewBox: "0 0 16 16",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.5,
  strokeLinecap: "round" as const,
  strokeLinejoin: "round" as const,
  "aria-hidden": true,
  ...props,
});

export const EyeIcon = (p: P) => (
  <svg {...base(p)}>
    <path d="M1.5 8s2.4-4.5 6.5-4.5S14.5 8 14.5 8 12.1 12.5 8 12.5 1.5 8 1.5 8Z" />
    <circle cx="8" cy="8" r="2" />
  </svg>
);
export const EyeOffIcon = (p: P) => (
  <svg {...base(p)}>
    <path d="M6.2 3.8A6.7 6.7 0 0 1 8 3.5c4.1 0 6.5 4.5 6.5 4.5a11 11 0 0 1-1.7 2.2M4.3 4.9A10.6 10.6 0 0 0 1.5 8s2.4 4.5 6.5 4.5a6.3 6.3 0 0 0 3-.8" />
    <path d="M2 2l12 12" />
  </svg>
);
export const PencilIcon = (p: P) => (
  <svg {...base(p)}>
    <path d="M10.5 2.5l3 3L5.5 13.5H2.5v-3z" />
  </svg>
);
export const TrashIcon = (p: P) => (
  <svg {...base(p)}>
    <path d="M2.5 4.5h11M6 4.5V2.5h4v2M4 4.5l.7 9h6.6l.7-9" />
  </svg>
);
export const CopyIcon = (p: P) => (
  <svg {...base(p)}>
    <rect x="5.5" y="5.5" width="8" height="8" rx="1.5" />
    <path d="M10.5 5.5v-2a1 1 0 0 0-1-1h-6a1 1 0 0 0-1 1v6a1 1 0 0 0 1 1h2" />
  </svg>
);
export const CheckIcon = (p: P) => (
  <svg {...base(p)}>
    <path d="M3 8.5l3 3 7-7" />
  </svg>
);
export const PlusIcon = (p: P) => (
  <svg {...base(p)}>
    <path d="M8 3v10M3 8h10" />
  </svg>
);
export const SearchIcon = (p: P) => (
  <svg {...base(p)}>
    <circle cx="7" cy="7" r="4.5" />
    <path d="M10.5 10.5L14 14" />
  </svg>
);
export const RefreshIcon = (p: P) => (
  <svg {...base(p)}>
    <path d="M13.5 8A5.5 5.5 0 1 1 11.9 4.1M13.5 2.5v3h-3" />
  </svg>
);
export const XIcon = (p: P) => (
  <svg {...base(p)}>
    <path d="M4 4l8 8M12 4l-8 8" />
  </svg>
);
export const SortIcon = ({ dir, ...p }: P & { dir: "asc" | "desc" | null }) => (
  <svg {...base(p)} width={12} height={12}>
    <path d="M5 6.5L8 3.5l3 3" opacity={dir === "asc" ? 1 : 0.3} />
    <path d="M5 9.5l3 3 3-3" opacity={dir === "desc" ? 1 : 0.3} />
  </svg>
);
export const SyncIcon = (p: P) => (
  <svg {...base(p)}>
    <path d="M2.5 6.5A5.5 5.5 0 0 1 12.6 4.2M13.5 2v2.5H11M13.5 9.5A5.5 5.5 0 0 1 3.4 11.8M2.5 14v-2.5H5" />
  </svg>
);
