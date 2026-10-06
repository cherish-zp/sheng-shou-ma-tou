import { useEffect } from "react";
import { Outlet } from "react-router-dom";

import { Sidebar } from "@/components/layout/sidebar";
import { ensureStoreInitialized } from "@/store/tunnel-store";

export function AppLayout() {
  useEffect(() => {
    void ensureStoreInitialized();
  }, []);

  return (
    <div className="flex h-screen overflow-hidden bg-background text-foreground">
      <Sidebar />
      <main className="pier-scroll flex-1 overflow-y-auto">
        <Outlet />
      </main>
    </div>
  );
}
