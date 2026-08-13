import type { Metadata } from "next";
import { RequireAdmin, AdminNav } from "@/components/auth/RequireAdmin";

export const metadata: Metadata = {
  title: "Weave - admin",
};

export default function AdminLayout({ children }: { children: React.ReactNode }) {
  return (
    <RequireAdmin>
      <div className="flex min-h-dvh bg-background text-foreground">
        <AdminNav />
        <main className="min-w-0 flex-1 overflow-y-auto p-6">{children}</main>
      </div>
    </RequireAdmin>
  );
}
