import { HashRouter, Route, Routes } from "react-router-dom";

import { AppLayout } from "@/components/layout/app-layout";
import { ThemeProvider } from "@/components/theme-provider";
import { Toaster } from "@/components/ui/sonner";
import { TooltipProvider } from "@/components/ui/tooltip";
import { HomePage } from "@/pages/home-page";
import { SettingsPage } from "@/pages/settings-page";

export default function App() {
  return (
    <ThemeProvider>
      <TooltipProvider delayDuration={250}>
        <HashRouter>
          <Routes>
            <Route element={<AppLayout />}>
              <Route index element={<HomePage />} />
              <Route path="/settings" element={<SettingsPage />} />
            </Route>
          </Routes>
          <Toaster position="bottom-right" closeButton />
        </HashRouter>
      </TooltipProvider>
    </ThemeProvider>
  );
}
