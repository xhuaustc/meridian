import { Outlet } from 'react-router-dom';
import { Titlebar } from './Titlebar';
import { Sidebar } from './Sidebar';

export function AppShell() {
  return (
    <div className="grid grid-cols-[220px_1fr] grid-rows-[48px_1fr] h-screen">
      <Titlebar />
      <Sidebar />
      <main className="overflow-y-auto p-6">
        <Outlet />
      </main>
    </div>
  );
}
