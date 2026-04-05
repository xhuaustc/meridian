import { getCurrentWindow } from '@tauri-apps/api/window';

interface ContentToolbarProps {
  title: string;
  children?: React.ReactNode;
}

export function ContentToolbar({ title, children }: ContentToolbarProps) {
  const isInteractive = (e: React.MouseEvent) =>
    !!(e.target as HTMLElement).closest('button, input, select, a');

  const handleDrag = (e: React.MouseEvent) => {
    if (isInteractive(e)) return;
    e.preventDefault();
    getCurrentWindow().startDragging();
  };

  const handleDoubleClick = (e: React.MouseEvent) => {
    if (isInteractive(e)) return;
    getCurrentWindow().toggleMaximize();
  };

  return (
    <div
      className="h-12 flex items-center justify-between px-5 border-b border-border shrink-0"
      onMouseDown={handleDrag}
      onDoubleClick={handleDoubleClick}
    >
      <h1 className="text-[16px] font-semibold tracking-[-0.01em] text-text-primary">
        {title}
      </h1>
      {children && (
        <div className="flex items-center gap-3">
          {children}
        </div>
      )}
    </div>
  );
}
