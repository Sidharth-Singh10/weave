"use client";

import { create } from "zustand";
import { getMe, postLogout, setUnauthorizedHandler, type AuthUser } from "./api";

export type AuthStatus = "loading" | "authenticated" | "unauthenticated";

interface AuthState {
  status: AuthStatus;
  user: AuthUser | null;
  bootstrap: () => Promise<void>;
  logout: () => Promise<void>;
  setStatus: (s: AuthStatus) => void;
}

export const useAuthStore = create<AuthState>((set) => ({
  status: "loading",
  user: null,

  async bootstrap() {
    try {
      const me = await getMe();
      if (me.authenticated && me.user) {
        set({ status: "authenticated", user: me.user });
      } else {
        set({ status: "unauthenticated", user: null });
      }
    } catch {
      set({ status: "unauthenticated", user: null });
    }
  },

  async logout() {
    try {
      await postLogout();
    } finally {
      set({ status: "unauthenticated", user: null });
    }
  },

  setStatus(status) {
    set({ status, user: status === "authenticated" ? this.user : null });
  },
}));

export function isAdmin(user: AuthUser | null): boolean {
  return user?.role === "admin";
}

/** Preserve the intended destination and go to the login page. */
export function redirectToLogin() {
  if (typeof window === "undefined") return;
  const next = encodeURIComponent(window.location.pathname + window.location.search);
  // Full navigation intentionally keeps a clean session boundary; the next
  // param is restored after login.
  // eslint-disable-next-line @next/next/no-location-assign-relative-destination
  window.location.href = `/login?next=${next}`;
}

// Centralized 401 handling: any protected API returning 401 marks the session
// expired and sends the user to /login.
setUnauthorizedHandler(() => {
  const { status } = useAuthStore.getState();
  if (status === "authenticated") {
    useAuthStore.getState().setStatus("unauthenticated");
    redirectToLogin();
  }
});
