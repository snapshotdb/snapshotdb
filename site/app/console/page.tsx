import { getSession, githubConfigured } from "./auth";
import SignIn from "./SignIn";
import Console from "./Console";

export const metadata = { title: "Console" };

// The console talks to a live server and reads per-viewer cookies — never prerender it.
export const dynamic = "force-dynamic";

export default async function Page({
  searchParams,
}: {
  searchParams: Promise<{ error?: string }>;
}) {
  const user = await getSession();
  if (user) return <Console user={user} />;

  const { error } = await searchParams;
  return <SignIn githubReady={githubConfigured()} error={error} />;
}
