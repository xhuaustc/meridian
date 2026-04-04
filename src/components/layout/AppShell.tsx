import { Outlet } from 'react-router-dom';
import { Sidebar } from './Sidebar';

export function AppShell() {
  return (
    <div className="grid grid-cols-[220px_1fr] h-screen">
      <Sidebar />
      <main className="overflow-y-auto flex flex-col">
        <Outlet />
      </main>
    </div>
  );
}
