import { Outlet } from 'react-router-dom';
import { Sidebar } from './Sidebar';

export function AppShell() {
  return (
    <div className="grid grid-cols-[176px_1fr] h-screen">
      <Sidebar />
      <main className="flex flex-col overflow-hidden bg-bg-primary">
        <Outlet />
      </main>
    </div>
  );
}
