import { HashRouter, Route, Routes } from "react-router-dom";

import { AppLayout } from "@/components/layout/app-layout";
import { ThemeProvider } from "@/components/theme-provider";
import { Toaster } from "@/components/ui/sonner";
import { TooltipProvider } from "@/components/ui/tooltip";
import { HomePage } from "@/pages/home-page";
import { ServersPage } from "@/pages/servers-page";
import { SettingsPage } from "@/pages/settings-page";
import { HelpPage } from "@/pages/help-page";

export default function App() {
  return (
    <ThemeProvider>
      <TooltipProvider delayDuration={250}>
        <HashRouter>
          <Routes>
            <Route element={<AppLayout />}>
              <Route index element={<HomePage />} />
              <Route path="/servers" element={<ServersPage />} />
              <Route path="/help" element={<HelpPage />} />
        <Route path="/settings" element={<SettingsPage />} />
            </Route>
          </Routes>
          <Toaster position="bottom-right" closeButton />
        </HashRouter>
      </TooltipProvider>
    </ThemeProvider>
  );
}
