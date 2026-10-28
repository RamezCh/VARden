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
export const ArrowUpIcon = (p: P) => (
  <svg {...base(p)}>
    <path d="M8 13V3M3.5 7.5 8 3l4.5 4.5" />
  </svg>
);
export const ArrowDownIcon = (p: P) => (
  <svg {...base(p)}>
    <path d="M8 3v10M3.5 8.5 8 13l4.5-4.5" />
  </svg>
);
export const LockIcon = (p: P) => (
  <svg {...base(p)}>
    <rect x="3" y="7" width="10" height="7" rx="1.5" />
    <path d="M5.5 7V5a2.5 2.5 0 0 1 5 0v2" />
  </svg>
);
export const FolderIcon = (p: P) => (
  <svg {...base(p)}>
    <path d="M1.5 4.5a1 1 0 0 1 1-1h3.6l1.5 1.5h5.9a1 1 0 0 1 1 1v6.5a1 1 0 0 1-1 1h-11a1 1 0 0 1-1-1Z" />
  </svg>
);
export const VariablesIcon = (p: P) => (
  <svg {...base(p)}>
    <path d="M5 2.5c-1.4 0-2 .6-2 2v1.8c0 .8-.4 1.2-1.2 1.2.8 0 1.2.4 1.2 1.2v1.8c0 1.4.6 2 2 2M11 2.5c1.4 0 2 .6 2 2v1.8c0 .8.4 1.2 1.2 1.2-.8 0-1.2.4-1.2 1.2v1.8c0 1.4-.6 2-2 2" />
    <path d="M6.3 6l3.4 4M9.7 6l-3.4 4" />
  </svg>
);
export const ProfilesIcon = (p: P) => (
  <svg {...base(p)}>
    <path d="M8 2 14 5 8 8 2 5Z" />
    <path d="M2 8l6 3 6-3M2 11l6 3 6-3" />
  </svg>
);
export const TerminalIcon = (p: P) => (
  <svg {...base(p)}>
    <rect x="1.5" y="2.5" width="13" height="11" rx="1.5" />
    <path d="M4.5 6.5 6.5 8.5l-2 2M8.5 10.5h3" />
  </svg>
);
export const BackupIcon = (p: P) => (
  <svg {...base(p)}>
    <path d="M2.5 8a5.5 5.5 0 1 0 1.6-3.9M2.5 2.5v2.5H5" />
    <path d="M8 5v3l2 1.5" />
  </svg>
);
export const SunIcon = (p: P) => (
  <svg {...base(p)}>
    <circle cx="8" cy="8" r="2.75" />
    <path d="M8 1.5v1.5M8 13v1.5M1.5 8H3M13 8h1.5M3.4 3.4l1 1M11.6 11.6l1 1M3.4 12.6l1-1M11.6 4.4l1-1" />
  </svg>
);
export const MoonIcon = (p: P) => (
  <svg {...base(p)}>
    <path d="M13.5 9.5A5.5 5.5 0 0 1 6.5 2.5a5.5 5.5 0 1 0 7 7Z" />
  </svg>
);
export const MonitorIcon = (p: P) => (
  <svg {...base(p)}>
    <rect x="1.5" y="2.5" width="13" height="8.5" rx="1.5" />
    <path d="M5.5 14h5M8 11v3" />
  </svg>
);
export const ChevronDownIcon = (p: P) => (
  <svg {...base(p)}>
    <path d="m4 6 4 4 4-4" />
  </svg>
);
export const ChevronRightIcon = (p: P) => (
  <svg {...base(p)}>
    <path d="m6 4 4 4-4 4" />
  </svg>
);
export const ImportIcon = (p: P) => (
  <svg {...base(p)}>
    <path d="M8 2v8M4.5 6.5 8 10l3.5-3.5M2.5 11.5v1a1 1 0 0 0 1 1h9a1 1 0 0 0 1-1v-1" />
  </svg>
);
export const ExportIcon = (p: P) => (
  <svg {...base(p)}>
    <path d="M8 10V2M4.5 5.5 8 2l3.5 3.5M2.5 11.5v1a1 1 0 0 0 1 1h9a1 1 0 0 0 1-1v-1" />
  </svg>
);
export const PathIcon = (p: P) => (
  <svg {...base(p)}>
    <path d="M2.5 4h11M2.5 8h11M2.5 12h7" />
    <circle cx="12.5" cy="12" r="1.2" />
  </svg>
);
