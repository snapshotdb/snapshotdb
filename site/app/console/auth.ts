import { cookies } from "next/headers";
import { verifySession, type SessionUser } from "./session";
export { encodeSession, verifySession, type SessionUser } from "./session";

export const SESSION_COOKIE = "ab_session";
export const OAUTH_STATE_COOKIE = "ab_oauth_state";

export async function getSession(): Promise<SessionUser | null> {
  const jar = await cookies();
  return verifySession(jar.get(SESSION_COOKIE)?.value);
}
