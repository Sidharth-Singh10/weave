import { NextResponse, type NextRequest } from "next/server";

const SESSION_COOKIE = process.env.SESSION_COOKIE_NAME ?? "weave_session";

/**
 * Fast-path route protection for /app and /admin: when the session cookie is
 * absent entirely, redirect to /login (preserving the destination). The
 * authoritative check is the backend /auth/me, handled client-side; this
 * avoids rendering protected pages for clearly-signed-out visitors.
 */
export function proxy(request: NextRequest) {
  const { pathname } = request.nextUrl;
  if (!request.cookies.get(SESSION_COOKIE)) {
    const login = new URL("/login", request.url);
    login.searchParams.set("next", pathname);
    return NextResponse.redirect(login);
  }
  return NextResponse.next();
}

export const config = {
  matcher: ["/app/:path*", "/admin/:path*"],
};
