// Build + launch a `vscode://` deep link to open a file at a line in the
// user's installed editor. Both VS Code and Cursor (a VS Code fork) register
// for this scheme, so a single call covers both without asking the user to
// pick. If no handler is registered, the browser silently drops the request.
//
// Path encoding: `encodeURI` preserves `/`, `:` and the other URL-reserved
// characters VS Code's scheme parser expects, while escaping spaces (`%20`)
// and other whitespace/unicode that would otherwise break the URI.

export function buildIdeLink(absPath: string, line: number, col?: number): string {
  const encoded = encodeURI(absPath);
  const loc = col && col > 0 ? `${line}:${col}` : `${line}`;
  return `vscode://file${encoded.startsWith('/') ? '' : '/'}${encoded}:${loc}`;
}

export function openInIde(absPath: string, line: number, col?: number): void {
  window.open(buildIdeLink(absPath, line, col));
}
