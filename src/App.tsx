import { useEffect } from "react";
import { Routes, Route, useNavigate } from "react-router-dom";
import { listen } from "@tauri-apps/api/event";
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
import { NotFoundPage } from "./pages/NotFoundPage";
import { useSettingsStore } from "./stores/settings-store";
import { useEngineStore } from "./stores/engine-store";
import { useToastStore } from "./stores/toast-store";
import { useTranslation } from "react-i18next";
import { usePlatform } from "./hooks/usePlatform";

function App() {
  const initialize = useSettingsStore((s) => s.initialize);
  const language = useSettingsStore((s) => s.language);
  const fetchStatus = useEngineStore((s) => s.fetchStatus);
  const addToast = useToastStore((s) => s.addToast);
  const { i18n, t } = useTranslation();

  useEffect(() => {
    initialize();
  }, [initialize]);

  // Listen for nginx crash events from the backend health check
  useEffect(() => {
    const unlisten = listen('nginx-status-changed', () => {
      fetchStatus();
      addToast('error', t('engine.stoppedUnexpectedly'));
    });
    return () => { unlisten.then(fn => fn()); };
  }, [fetchStatus, addToast, t]);

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
          <Route path="*" element={<NotFoundPage />} />
        </Route>
      </Routes>
      <ToastContainer />
    </>
  );
}

export default App;
