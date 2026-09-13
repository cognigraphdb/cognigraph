export function apiOrigin(pageOrigin: string, developmentPort?: string | null): string {
  const url = new URL(pageOrigin);
  if (developmentPort) url.port = developmentPort;
  return url.origin;
}

export function defaultApiOrigin(): string {
  let developmentPort: string | null | undefined;
  try {
    // Bun's HTML dev server inlines this public variable via bunfig.toml.
    // The production build explicitly replaces it with null.
    developmentPort = process.env.COGNIGRAPH_UI_DEV_API_PORT;
  } catch {
    // A bundle made without environment inlining still defaults to its origin.
  }
  return apiOrigin(window.location.origin, developmentPort);
}
