import { cn } from '../../lib/utils';

interface SkeletonProps {
  className?: string;
}

export function Skeleton({ className }: SkeletonProps) {
  return (
    <div
      className={cn(
        'animate-pulse bg-bg-sidebar rounded-[var(--radius-sm)]',
        className,
      )}
    />
  );
}

export function SkeletonTable({ rows = 5 }: { rows?: number }) {
  return (
    <div className="bg-bg-secondary border border-border rounded-[var(--radius-md)] overflow-hidden">
      {/* Header */}
      <div className="flex gap-4 px-4 py-3 bg-bg-sidebar border-b border-border">
        <Skeleton className="h-3 w-24" />
        <Skeleton className="h-3 w-16" />
        <Skeleton className="h-3 w-32" />
        <Skeleton className="h-3 w-16" />
        <Skeleton className="h-3 w-12" />
      </div>
      {/* Rows */}
      {Array.from({ length: rows }).map((_, i) => (
        <div key={i} className="flex gap-4 px-4 py-3.5 border-b border-border last:border-b-0">
          <Skeleton className="h-4 w-28" />
          <Skeleton className="h-4 w-14" />
          <Skeleton className="h-4 w-40" />
          <Skeleton className="h-4 w-14" />
          <Skeleton className="h-4 w-10" />
        </div>
      ))}
    </div>
  );
}

export function SkeletonCards({ count = 3 }: { count?: number }) {
  return (
    <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
      {Array.from({ length: count }).map((_, i) => (
        <div key={i} className="bg-bg-secondary border border-border rounded-[var(--radius-md)] p-4">
          <Skeleton className="h-4 w-32 mb-3" />
          <Skeleton className="h-3 w-48 mb-2" />
          <Skeleton className="h-3 w-24 mb-4" />
          <div className="flex gap-2">
            <Skeleton className="h-6 w-16 rounded-full" />
            <Skeleton className="h-6 w-16 rounded-full" />
          </div>
        </div>
      ))}
    </div>
  );
}

export function SkeletonStats() {
  return (
    <div className="grid grid-cols-4 gap-3 mb-5">
      {Array.from({ length: 4 }).map((_, i) => (
        <div key={i} className="bg-bg-secondary border border-border rounded-[var(--radius-md)] px-4 py-3.5">
          <Skeleton className="h-2.5 w-20 mb-2" />
          <Skeleton className="h-6 w-10 mb-1.5" />
          <Skeleton className="h-2.5 w-16" />
        </div>
      ))}
    </div>
  );
}
