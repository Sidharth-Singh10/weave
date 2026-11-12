import type { Metadata } from "next";
import { RequireAdmin, AdminNav } from "@/components/auth/RequireAdmin";

export const metadata: Metadata = {
  title: "Weave - admin",
};

export default function AdminLayout({ children }: { children: React.ReactNode }) {
  return (
    <RequireAdmin>
      <div className="flex min-h-dvh flex-col bg-background text-foreground md:flex-row">
        <AdminNav />
        <main className="min-w-0 flex-1 overflow-y-auto p-4 sm:p-6 md:p-8">
          {children}
        </main>
      </div>
    </RequireAdmin>
  );
}
