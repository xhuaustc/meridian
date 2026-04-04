import { useEffect } from "react";
import { Routes, Route, useNavigate } from "react-router-dom";
import { AppShell } from "./components/layout/AppShell";
import { ToastContainer } from "./components/ui/Toast";
import { DashboardPage } from "./pages/DashboardPage";
import { ProxyFormPage } from "./pages/ProxyFormPage";
import { CertsPage } from "./pages/CertsPage";
import { AccessPage } from "./pages/AccessPage";
import { HostsPage } from "./pages/HostsPage";
import { LogsPage } from "./pages/LogsPage";
import { MonitorPage } from "./pages/MonitorPage";
import { SettingsPage } from "./pages/SettingsPage";
import { useSettingsStore } from "./stores/settings-store";
import { useTranslation } from "react-i18next";
import { usePlatform } from "./hooks/usePlatform";

function App() {
  const initialize = useSettingsStore((s) => s.initialize);
  const language = useSettingsStore((s) => s.language);
  const { i18n } = useTranslation();

  useEffect(() => {
    initialize();
  }, [initialize]);

  // Expose navigate to Rust tray menu handler
  const navigate = useNavigate();
  useEffect(() => {
    (window as any).__navigate = (path: string) => navigate(path);
    return () => { delete (window as any).__navigate; };
  }, [navigate]);

  useEffect(() => {
    i18n.changeLanguage(language);
  }, [language, i18n]);

  const platform = usePlatform();
  useEffect(() => {
    if (platform) {
      document.documentElement.setAttribute('data-platform', platform);
    }
  }, [platform]);

  return (
    <>
      <Routes>
        <Route element={<AppShell />}>
          <Route path="/" element={<DashboardPage />} />
          <Route path="/monitor" element={<MonitorPage />} />
          <Route path="/proxy/new" element={<ProxyFormPage />} />
          <Route path="/proxy/:id" element={<ProxyFormPage />} />
          <Route path="/certs" element={<CertsPage />} />
          <Route path="/access" element={<AccessPage />} />
          <Route path="/hosts" element={<HostsPage />} />
          <Route path="/logs" element={<LogsPage />} />
          <Route path="/settings" element={<SettingsPage />} />
        </Route>
      </Routes>
      <ToastContainer />
    </>
  );
}

export default App;
